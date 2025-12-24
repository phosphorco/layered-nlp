# RFC-001 Appendix: Prior Art and Design Inspiration

This appendix synthesizes research across six domains that inform the obligation graph diffing design. Each section identifies patterns from external systems and maps them to our design.

---

## 1. Entity Resolution and Coreference

### 1.1 NIL Clustering (Entity Linking)

**Source**: TAC-KBP shared tasks (2009-2017), AIDA system

Entity linking systems face the same problem we do: some mentions cannot be resolved to known entities. The standard solution is **NIL clustering** - grouping unresolved mentions that likely refer to the same unknown entity.

**Key insight**: NIL mentions aren't failures; they're first-class citizens that can be clustered and later linked when the entity becomes known.

```
Mention "Acme Corp" → Not in KB → NIL_001
Mention "Acme"      → Not in KB → NIL_001 (same cluster)
Mention "the Company" → Resolved → "Acme Corp" → NIL_001 promoted to KB entry
```

**Application to our design**: Our `HoleId` system directly implements NIL clustering. When a party reference can't be resolved to a `PronounChain`, it gets a stable hole ID that:
- Groups identical/similar mentions together
- Persists across operations
- Can be "filled" when resolution arrives

```rust
// Our design mirrors NIL clustering
pub enum EntityBinding {
    Resolved { chain_id: u32, ... },   // Linked to KB
    Hole { hole_id: HoleId, ... },     // NIL cluster
}
```

### 1.2 Fellegi-Sunter Model (Record Linkage)

**Source**: Fellegi & Sunter, "A Theory for Record Linkage" (JASA 1969)

The classical record linkage model uses three decision zones instead of binary match/no-match:

| Zone | Action | Our Equivalent |
|------|--------|----------------|
| **Match** | Confidence > upper threshold | `MatchResult::Definite` |
| **Uncertain** | Between thresholds | `MatchResult::Indeterminate` |
| **Non-match** | Confidence < lower threshold | `MatchResult::NoMatch` |

**Key insight**: The "uncertain" zone is explicit, not an afterthought. Items in this zone are routed to human review.

**Application**: Our `MatchResult` enum with `Indeterminate` variant implements this three-zone model. The `ResolutionHint` guides reviewers on what information would resolve uncertainty.

### 1.3 Cluster Repair (COMA++)

**Source**: Cluster-based entity resolution systems

When new evidence arrives, existing clusters may need to be split or merged. This is called **cluster repair**:

- **Split**: "Actually, NIL_001 contains two different entities"
- **Merge**: "NIL_001 and NIL_003 are the same entity"

**Application**: Our `HoleRegistry::unify_holes()` implements merge. We should add:

```rust
impl HoleRegistry {
    /// Split a hole into two (when we discover it contains multiple entities)
    pub fn split_hole(&mut self, hole_id: HoleId) -> (HoleId, HoleId) {
        // Create new hole, reassign some occurrences
    }
}
```

### 1.4 Confidence Calibration

**Source**: Entity resolution systems with human feedback loops

Entity resolution systems track prediction accuracy over time and adjust confidence scores:

```
Initial: P("Seller" = "Vendor") = 0.85
After 100 human reviews: Actual accuracy = 0.92
Calibration: Adjust model to output 0.92 for similar cases
```

**Application**: Our `VerificationAction` feedback should drive calibration:

```rust
pub struct CalibrationMetrics {
    /// Predictions grouped by confidence bucket
    predictions_by_bucket: HashMap<ConfidenceBucket, PredictionStats>,
}

impl CalibrationMetrics {
    /// After human verification, update calibration
    pub fn record_outcome(&mut self, predicted: f64, actual: bool) {
        // Track accuracy per confidence bucket
        // Use for future confidence adjustments
    }
}
```

---

## 2. Type Theory and Holes

### 2.1 Typed Holes (GHC Haskell)

**Source**: GHC Typed Holes extension (2014+)

Haskell's type system allows "holes" marked with `_` that the compiler reasons about without requiring implementation:

```haskell
foo :: Int -> Bool
foo x = _  -- Compiler reports: Found hole with type Bool
           -- Relevant bindings: x :: Int
```

**Key insight**: Holes preserve type constraints and propagate through the system. The compiler can still type-check surrounding code.

**Application**: Our holes should similarly preserve constraints:

```rust
#[derive(Debug, Clone)]
pub struct HoleConstraints {
    /// Must be a party (not a section reference)
    pub must_be_party: bool,
    /// Must match this pattern if resolved
    pub name_pattern: Option<String>,
    /// Cross-document constraint: if hole A in doc1 maps to X,
    /// then hole B in doc2 should also map to X
    pub linked_holes: Vec<(DocumentId, HoleId)>,
}
```

### 2.2 Union-Find (Type Unification)

**Source**: Hindley-Milner type inference, Tarjan's Union-Find

Type inference unifies type variables using Union-Find data structure with near-constant time operations:

| Operation | Time Complexity |
|-----------|-----------------|
| Find (lookup root) | O(α(n)) ≈ O(1) |
| Union (merge sets) | O(α(n)) ≈ O(1) |

**Key insight**: When two holes are unified, we don't copy data - we point one to the other.

**Application**: Replace naive hole merging with Union-Find:

```rust
/// Efficient hole unification using Union-Find
pub struct UnionFindHoleRegistry {
    /// parent[i] = parent of hole i (or i if root)
    parent: Vec<u32>,
    /// rank[i] = tree depth for balancing
    rank: Vec<u8>,
    /// Info stored only at roots
    info: HashMap<u32, HoleInfo>,
}

impl UnionFindHoleRegistry {
    /// Find root of hole's equivalence class
    pub fn find(&mut self, hole_id: HoleId) -> HoleId {
        let i = hole_id.0 as usize;
        if self.parent[i] != hole_id.0 {
            // Path compression
            self.parent[i] = self.find(HoleId(self.parent[i])).0;
        }
        HoleId(self.parent[i])
    }

    /// Unify two holes (union by rank)
    pub fn unify(&mut self, a: HoleId, b: HoleId) {
        let root_a = self.find(a);
        let root_b = self.find(b);
        if root_a == root_b { return; }

        // Union by rank
        let (ra, rb) = (self.rank[root_a.0 as usize], self.rank[root_b.0 as usize]);
        if ra < rb {
            self.parent[root_a.0 as usize] = root_b.0;
        } else if ra > rb {
            self.parent[root_b.0 as usize] = root_a.0;
        } else {
            self.parent[root_b.0 as usize] = root_a.0;
            self.rank[root_a.0 as usize] += 1;
        }
    }
}
```

### 2.3 Skolemization (Logic)

**Source**: First-order logic, type theory

When we need a "fresh" entity that we know exists but can't name, we introduce a Skolem constant:

```
∃x. P(x) → P(sk₁)  // sk₁ is a fresh constant
```

**Application**: Our holes are essentially Skolem constants for contract entities:

```
"the Buyer shall pay" → Buyer is Hole(sk₁)
Later: "Buyer means ABC Corp" → sk₁ := "ABC Corp"
```

---

## 3. Active Learning and Verification

### 3.1 Priority Scoring Formula

**Source**: Active learning literature (uncertainty sampling, query-by-committee)

Standard active learning prioritizes items by:

```
priority = w₁×Uncertainty + w₂×Representativeness + w₃×Diversity + w₄×Impact
```

Where:
- **Uncertainty**: 1 - confidence (items we're least sure about)
- **Representativeness**: How many similar items exist (high = batch resolve)
- **Diversity**: Coverage of different patterns (avoid tunnel vision)
- **Impact**: Business criticality (monetary terms, prohibitions)

**Application**: Our priority formula from the RFC:

```rust
pub fn calculate_priority(item: &VerificationQueueItem, context: &VerificationContext) -> f64 {
    let uncertainty = 1.0 - item.confidence;

    let representativeness = context
        .similarity_clusters
        .get(&item.cluster_id)
        .map(|c| (c.size as f64).log2() / 10.0)  // Log scale
        .unwrap_or(0.0);

    let impact = calculate_impact_score(item);

    let disagreement_penalty = if item.has_rule_llm_disagreement {
        0.2
    } else {
        0.0
    };

    // Weighted combination
    0.4 * uncertainty
        + 0.2 * representativeness
        + 0.3 * impact
        + disagreement_penalty
}
```

### 3.2 Label Propagation (Semi-Supervised Learning)

**Source**: Zhu & Ghahramani (2002), graph-based semi-supervised learning

When a human labels one item, propagate to similar items with confidence decay:

```
label(neighbor) = label(source) × similarity × decay_factor
```

**Application**: Our `SimilarityCluster` system:

```rust
/// Propagate verification decision through cluster
pub fn propagate_verification(
    nodes: &mut [Scored<ObligationNode>],
    cluster: &SimilarityCluster,
    action: VerificationAction,
    decay: f64,  // e.g., 0.9 per hop
) -> Vec<PropagationResult> {
    let mut results = Vec::new();
    let mut visited = HashSet::new();

    // BFS from verified item
    let mut queue = VecDeque::from([(cluster.primary_item.clone(), 1.0)]);

    while let Some((item, confidence)) = queue.pop_front() {
        if !visited.insert(item.id()) { continue; }
        if confidence < 0.5 { continue; }  // Stop propagating below threshold

        // Apply action with reduced confidence
        let propagated = action.with_confidence(confidence);
        results.push(apply_action(nodes, &item, propagated));

        // Queue neighbors
        for neighbor in cluster.neighbors_of(&item) {
            let sim = cluster.similarity(&item, neighbor);
            queue.push_back((neighbor.clone(), confidence * sim * decay));
        }
    }

    results
}
```

### 3.3 Snorkel (Weak Supervision)

**Source**: Ratner et al., "Snorkel: Rapid Training Data Creation with Weak Supervision" (2017)

Combine multiple noisy labeling functions using a generative model:

```
LF₁: If action contains "pay", probably Duty
LF₂: If subject is "may", probably Permission
LF₃: If contains "shall not", probably Prohibition
→ Combine with learned weights → Final label
```

**Application**: Our multi-pass extraction already does this implicitly. Make it explicit:

```rust
/// Labeling function for obligation type
pub trait LabelingFunction {
    fn label(&self, text: &str) -> Option<(ObligationType, f64)>;
    fn name(&self) -> &str;
}

/// Combine multiple labeling functions
pub fn combine_labels(
    lfs: &[Box<dyn LabelingFunction>],
    text: &str,
    weights: &LFWeights,
) -> Scored<ObligationType> {
    let mut votes: HashMap<ObligationType, f64> = HashMap::new();

    for lf in lfs {
        if let Some((label, conf)) = lf.label(text) {
            let weight = weights.get(lf.name());
            *votes.entry(label).or_default() += conf * weight;
        }
    }

    // Return highest weighted vote
    votes.into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(label, score)| Scored::derived(label, score.min(1.0)))
        .unwrap_or_else(|| Scored::derived(ObligationType::Duty, 0.5))
}
```

---

## 4. Legal Tech Systems

### 4.1 LKIF (Legal Knowledge Interchange Format)

**Source**: ESTRELLA project, EU legal informatics

LKIF defines deontic concepts as an OWL ontology:

```xml
<owl:Class rdf:about="&lkif;Obligation">
    <rdfs:subClassOf rdf:resource="&lkif;Normative_Statement"/>
    <owl:equivalentClass>
        <owl:Restriction>
            <owl:onProperty rdf:resource="&lkif;bearer"/>
            <owl:someValuesFrom rdf:resource="&lkif;Agent"/>
        </owl:Restriction>
    </owl:equivalentClass>
</owl:Class>
```

Key concepts:
- **Bearer**: Who holds the obligation (our `obligor`)
- **Counterparty**: Who benefits (our `beneficiary`)
- **Action**: What must be done (our `action_text`)
- **Norm**: The deontic modality (our `ObligationType`)

**Application**: Consider LKIF-compatible export:

```rust
/// Export obligation graph to LKIF-compatible format
pub fn to_lkif_rdf(graph: &ObligationGraph) -> String {
    let mut writer = RdfWriter::new();

    for node in graph.nodes() {
        writer.add_triple(
            &format!("obligation:{}", node.node_id),
            "rdf:type",
            "lkif:Obligation",
        );
        writer.add_triple(
            &format!("obligation:{}", node.node_id),
            "lkif:bearer",
            &format!("party:{}", node.obligor.chain_id_or_hole()),
        );
        // ... more triples
    }

    writer.to_string()
}
```

### 4.2 LegalRuleML

**Source**: OASIS LegalRuleML Technical Committee

LegalRuleML extends RuleML for legal rules with:
- **Deontic operators**: Obligation, Permission, Prohibition
- **Temporal qualifications**: Valid from/until
- **Jurisdiction**: Applicable law
- **Strength**: Mandatory vs. directory

```xml
<lrml:Obligation>
    <lrml:Bearer><lrml:Variable>Seller</lrml:Variable></lrml:Bearer>
    <lrml:Action>deliver goods</lrml:Action>
    <lrml:Beneficiary><lrml:Variable>Buyer</lrml:Variable></lrml:Beneficiary>
    <lrml:Condition>
        <lrml:If>payment received</lrml:If>
    </lrml:Condition>
</lrml:Obligation>
```

**Application**: Our `ContractClause` maps directly to LegalRuleML structure:

```rust
impl ContractClause {
    pub fn to_legal_rule_ml(&self) -> String {
        format!(r#"
<lrml:{modality}>
    <lrml:Bearer>{obligor}</lrml:Bearer>
    <lrml:Action>{action}</lrml:Action>
    {conditions}
</lrml:{modality}>
"#,
            modality = self.duty.obligation_type.to_lrml_tag(),
            obligor = self.obligor.display_text,
            action = self.duty.action,
            conditions = self.conditions.iter()
                .map(|c| format!("<lrml:Condition><lrml:{}>{}</lrml:{}></lrml:Condition>",
                    c.condition_type.to_lrml_tag(),
                    c.text,
                    c.condition_type.to_lrml_tag()))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}
```

### 4.3 Accord Project Concerto

**Source**: Linux Foundation, Accord Project

Concerto is a schema language for smart legal contracts:

```javascript
namespace org.example.contract

asset Obligation identified by obligationId {
    o String obligationId
    --> Party obligor
    --> Party beneficiary optional
    o String action
    o ObligationType obligationType
    o DateTime effectiveDate optional
}

enum ObligationType {
    o DUTY
    o PERMISSION
    o PROHIBITION
}
```

**Application**: For interoperability, support Concerto export:

```rust
/// Generate Concerto schema for obligation graph
pub fn generate_concerto_schema(namespace: &str) -> String {
    format!(r#"
namespace {namespace}

concept ClauseParty {{
    o String displayText
    o Integer chainId optional
    o Boolean verified default = false
}}

asset ObligationNode identified by nodeId {{
    o String nodeId
    o ClauseParty obligor
    o ClauseParty[] beneficiaries
    o String actionText
    o String obligationType
    o Double confidence
}}
"#, namespace = namespace)
}
```

### 4.4 CUAD Dataset

**Source**: The Atticus Project, "CUAD: An Expert-Annotated NLP Dataset for Legal Contract Review"

CUAD provides 13,000+ annotations across 41 clause types. Relevant categories:

| CUAD Category | Our Equivalent |
|---------------|----------------|
| Parties | `PronounChain` + party identification |
| Obligations | `ObligationNode` |
| Conditions | `ConditionLink` |
| Termination | `ContractKeyword::Unless/SubjectTo` |
| Confidentiality | Specific clause type (future) |

**Application**: Use CUAD for validation:

```rust
/// Validate extraction against CUAD-style annotations
pub struct CUADValidator {
    pub fn validate(
        &self,
        extracted: &[ObligationNode],
        gold_annotations: &[CUADAnnotation],
    ) -> ValidationReport {
        // Compare extracted obligations against gold standard
        // Report precision, recall, F1 per category
    }
}
```

### 4.5 ContractNLI Dataset

**Source**: Koreeda & Manning, "ContractNLI: A Dataset for Document-level Natural Language Inference for Contracts"

ContractNLI tests whether a hypothesis is entailed by a contract:

```
Contract: "Seller shall deliver within 30 days"
Hypothesis: "Seller must deliver within 60 days"
Label: Entailment (30 days satisfies 60 day requirement)
```

**Application**: For semantic equivalence testing in diff:

```rust
/// Check if obligation A entails obligation B
pub fn check_entailment(a: &ObligationNode, b: &ObligationNode) -> EntailmentResult {
    // Use embeddings + rules to determine if A satisfies B
    // "30 days" entails "60 days" but not vice versa
}
```

---

## 5. Semantic Diff Systems

### 5.1 GumTree (AST Diff)

**Source**: Falleri et al., "Fine-grained and Accurate Source Code Differencing" (2014)

GumTree computes tree edit distance on ASTs:

1. **Bottom-up matching**: Match identical subtrees first
2. **Top-down propagation**: Extend matches to parents
3. **Edit script**: Insert, delete, move, update operations

**Application**: Our obligation graphs have tree-like structure:

```
Aggregate
├── Node
│   ├── Clause 1
│   ├── Clause 2
│   └── Conditions
└── Beneficiaries
```

We can adapt GumTree's matching:

```rust
pub fn tree_diff(source: &AggregateTree, target: &AggregateTree) -> Vec<TreeEdit> {
    // Phase 1: Match identical clauses by content hash
    let exact_matches = match_by_hash(source, target);

    // Phase 2: Match similar clauses by structure
    let structural_matches = match_by_structure(source, target, &exact_matches);

    // Phase 3: Generate edit script
    generate_edits(source, target, &exact_matches, &structural_matches)
}
```

### 5.2 Semantic Versioning for Schemas

**Source**: JSON Schema, Protocol Buffers, Avro

Schema evolution systems classify changes:

| Change Type | Example | Compatibility |
|-------------|---------|---------------|
| **Addition** | New optional field | Forward compatible |
| **Removal** | Delete field | Backward compatible |
| **Modification** | Change type | Breaking |

**Application**: Classify obligation changes similarly:

```rust
pub enum ChangeCompatibility {
    /// Can be processed by old consumers (field added)
    ForwardCompatible,
    /// Can process old data (field removed)
    BackwardCompatible,
    /// Both directions (cosmetic change)
    FullyCompatible,
    /// Requires review (semantic change)
    Breaking,
}

impl ObligationDelta {
    pub fn compatibility(&self) -> ChangeCompatibility {
        match &self.changes[..] {
            // Adding a beneficiary = forward compatible
            [ChangeDetail::BeneficiaryAdded { .. }] => ChangeCompatibility::ForwardCompatible,
            // Removing condition = more permissive = forward compatible
            [ChangeDetail::ConditionRemoved { .. }] => ChangeCompatibility::ForwardCompatible,
            // Type change = breaking
            [ChangeDetail::ObligationTypeChanged { .. }] => ChangeCompatibility::Breaking,
            // Multiple changes = analyze each
            _ => self.analyze_combined_compatibility(),
        }
    }
}
```

---

## 6. Deontic Logic Systems

### 6.1 Standard Deontic Logic (SDL)

**Source**: Von Wright (1951), McNamara (SEP)

Core operators and relationships:

```
O(A) = Obligatory to do A (Duty)
P(A) = Permitted to do A (Permission)
F(A) = Forbidden to do A (Prohibition)

Relationships:
P(A) = ¬O(¬A)   -- Permitted iff not obligated to not do
F(A) = O(¬A)    -- Forbidden iff obligated to not do
O(A) → P(A)     -- If obligatory, then permitted
```

**Application**: Already reflected in our `ObligationType`:

```rust
impl ObligationType {
    /// Check if two types are deontic opposites
    pub fn is_opposite(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (ObligationType::Duty, ObligationType::Prohibition) |
            (ObligationType::Prohibition, ObligationType::Duty)
        )
    }

    /// Check for deontic conflict
    pub fn conflicts_with(&self, other: &Self, same_action: bool) -> bool {
        same_action && self.is_opposite(other)
    }
}
```

### 6.2 Conflict Detection Algorithms

**Source**: Deontic logic consistency checking

Conflicts occur when same party has both O(A) and F(A) for the same action:

```
Check for each party P:
  For each duty D where obligor = P:
    For each prohibition R where obligor = P:
      If similarity(D.action, R.action) > threshold:
        Report conflict unless guarded by exclusive conditions
```

**Application**: Implement conflict detection:

```rust
pub fn detect_conflicts(nodes: &[ObligationNode]) -> Vec<ConflictReport> {
    let mut conflicts = Vec::new();

    // Group by party
    let by_party = group_by_party(nodes);

    for (party, party_nodes) in by_party {
        let duties: Vec<_> = party_nodes.iter()
            .filter(|n| n.obligation_type() == ObligationType::Duty)
            .collect();
        let prohibitions: Vec<_> = party_nodes.iter()
            .filter(|n| n.obligation_type() == ObligationType::Prohibition)
            .collect();

        for duty in &duties {
            for prohibition in &prohibitions {
                let action_sim = text_similarity(&duty.action_text(), &prohibition.action_text());
                if action_sim > 0.8 {
                    // Check if conditions are mutually exclusive
                    if !conditions_mutually_exclusive(duty, prohibition) {
                        conflicts.push(ConflictReport {
                            party: party.clone(),
                            duty: duty.clone(),
                            prohibition: prohibition.clone(),
                            action_similarity: action_sim,
                            severity: ConflictSeverity::High,
                        });
                    }
                }
            }
        }
    }

    conflicts
}
```

### 6.3 Temporal Deontic Logic

**Source**: Temporal extensions to deontic logic

Obligations can have temporal qualifications:

- **Maintenance**: O(A) until T - must keep doing A until time T
- **Achievement**: O(◊A before T) - must do A at least once before T
- **Deadline**: O(A by T) - must complete A by time T

**Application**: Extend our model:

```rust
#[derive(Debug, Clone)]
pub enum TemporalQualification {
    /// Must be maintained throughout period
    Maintenance { until: Option<NaiveDate> },
    /// Must be achieved at least once
    Achievement { deadline: Option<NaiveDate> },
    /// Specific deadline
    Deadline { by: NaiveDate },
    /// Triggered by event
    EventTriggered { event: String, within: Option<Duration> },
}
```

---

## 7. Synthesis: Design Recommendations

Based on the prior art survey, here are concrete recommendations:

### 7.1 Immediate Adoption (Phase 0-1)

| Pattern | Source | Implementation |
|---------|--------|----------------|
| **Union-Find for holes** | Type theory | `UnionFindHoleRegistry` for O(1) unification |
| **Three-zone matching** | Fellegi-Sunter | `MatchResult::{Definite, Indeterminate, NoMatch}` |
| **Priority formula** | Active learning | `w₁×uncertainty + w₂×impact + w₃×representativeness` |
| **Label propagation** | Semi-supervised | `propagate_verification()` through similarity clusters |

### 7.2 Medium-Term (Phase 2-3)

| Pattern | Source | Implementation |
|---------|--------|----------------|
| **NIL cluster repair** | Entity resolution | `HoleRegistry::split_hole()` |
| **Calibration tracking** | Entity resolution | `CalibrationMetrics` for confidence adjustment |
| **Tree diff algorithm** | GumTree | Hierarchical matching for aggregate/node/clause |
| **Change compatibility** | Schema versioning | `ChangeCompatibility` classification |

### 7.3 Long-Term (Phase 4+)

| Pattern | Source | Implementation |
|---------|--------|----------------|
| **LKIF/LegalRuleML export** | Legal informatics | Interoperability with legal knowledge systems |
| **Concerto schema** | Accord Project | Smart contract compatibility |
| **CUAD validation** | NLP research | Extraction quality benchmarking |
| **Temporal deontic logic** | Formal methods | Time-qualified obligations |

---

## 8. References

### Academic Papers

1. Fellegi, I. P., & Sunter, A. B. (1969). "A Theory for Record Linkage." *Journal of the American Statistical Association*, 64(328), 1183-1210.

2. Falleri, J. R., et al. (2014). "Fine-grained and Accurate Source Code Differencing." *ASE 2014*.

3. Ratner, A., et al. (2017). "Snorkel: Rapid Training Data Creation with Weak Supervision." *VLDB 2017*.

4. Zhu, X., & Ghahramani, Z. (2002). "Learning from Labeled and Unlabeled Data with Label Propagation." *CMU Technical Report*.

5. Von Wright, G. H. (1951). "Deontic Logic." *Mind*, 60(237), 1-15.

6. Koreeda, Y., & Manning, C. D. (2021). "ContractNLI: A Dataset for Document-level Natural Language Inference for Contracts." *Findings of EMNLP 2021*.

7. Hendrix, S., et al. (2021). "CUAD: An Expert-Annotated NLP Dataset for Legal Contract Review." *arXiv:2103.06268*.

### Systems and Standards

8. OASIS LegalRuleML Technical Committee. "LegalRuleML Core Specification Version 1.0."

9. ESTRELLA Project. "LKIF Core Ontology." https://github.com/RinkeHoekstra/lkif-core

10. Accord Project. "Concerto Modeling Language." https://accordproject.org/

11. TAC Knowledge Base Population. https://tac.nist.gov/

### Tools and Libraries

12. GHC Typed Holes. https://downloads.haskell.org/ghc/latest/docs/users_guide/exts/typed_holes.html

13. GumTree. https://github.com/GumTreeDiff/gumtree

14. Union-Find (Tarjan). "Efficiency of a Good But Not Linear Set Union Algorithm." *JACM 1975*.

---

## Appendix: Quick Reference Matrix

| Our Concept | Prior Art Term | Source Domain |
|-------------|----------------|---------------|
| `HoleId` | NIL cluster ID | Entity linking |
| `EntityBinding::Hole` | Skolem constant | Logic / Type theory |
| `HoleRegistry::unify_holes` | Union-Find union | Type inference |
| `MatchResult::Indeterminate` | Uncertain zone | Fellegi-Sunter |
| `ResolutionHint` | Query strategy | Active learning |
| `SimilarityCluster` | Batch verification | Weak supervision |
| `propagate_verification` | Label propagation | Semi-supervised |
| `ObligationType` | Deontic modality | SDL |
| `detect_conflicts` | Consistency check | Deontic logic |
| `ContractClause` | Normative statement | LKIF |
| `HierarchicalDiff` | Tree edit script | GumTree |

---

*End of Prior Art Appendix*

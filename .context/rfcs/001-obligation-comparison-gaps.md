# RFC-001 Addendum: Critical Gaps and Design Clarifications

This addendum addresses gaps identified in the initial RFC and proposes solutions informed by deep codebase analysis.

---

## 1. The Single-Mention Entity Problem

### 1.1 Current Behavior

From `pronoun_chain.rs:300-303`:
```rust
// Skip chains with only one mention (no coreference)
if builder.mentions.len() < 2 {
    continue;
}
```

And documented in `contract_clause.rs:34-37`:
```rust
/// Chain IDs only appear when [`PronounChainResolver`] produced a chain
/// (there must be at least two mentions), so single-mention parties will
/// leave this as `None`.
pub chain_id: Option<u32>,
```

**Impact**: Many obligors and beneficiaries will have `chain_id: None`. The RFC's `PartyMapping` design assumed chain IDs exist for cross-document alignment, but this isn't guaranteed.

### 1.2 Proposed Solution: Holes as First-Class Citizens

Introduce **Holes** - stable identifiers for unresolved entity references that can be unified later.

```rust
/// A hole represents unresolved information with a stable identity.
///
/// Like type variables in type inference, holes can appear in multiple
/// places and be filled by external input (human verification, LLM, etc.)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HoleId(pub u32);

/// An entity reference that may or may not be resolved.
#[derive(Debug, Clone, PartialEq)]
pub enum EntityBinding {
    /// Resolved to a concrete pronoun chain
    Resolved {
        chain_id: u32,
        canonical_name: String,
        verified: bool,
    },
    /// Unresolved placeholder (hole)
    ///
    /// Multiple references with the same display_text (normalized) share
    /// the same hole_id, enabling batch resolution.
    Hole {
        hole_id: HoleId,
        display_text: String,
        /// Candidates for resolution (from context, embeddings, etc.)
        candidates: Vec<ResolutionCandidate>,
    },
}

#[derive(Debug, Clone)]
pub struct ResolutionCandidate {
    pub resolved_to: String,
    pub confidence: f64,
    pub source: CandidateSource,
}

#[derive(Debug, Clone)]
pub enum CandidateSource {
    /// Inferred from nearby defined terms
    ContextualInference,
    /// Matched via embedding similarity
    SemanticSimilarity,
    /// Suggested by LLM analysis
    LLMSuggestion,
    /// Cross-document alignment
    CrossDocumentMapping,
}
```

### 1.3 Hole Assignment Algorithm

When building `ClauseParty`, instead of leaving `chain_id: None`:

```rust
/// Registry for tracking holes within a document or comparison set.
pub struct HoleRegistry {
    /// Map from normalized display_text to hole_id
    text_to_hole: HashMap<String, HoleId>,
    /// Reverse map for lookup
    hole_info: HashMap<HoleId, HoleInfo>,
    next_hole_id: u32,
}

#[derive(Debug, Clone)]
pub struct HoleInfo {
    pub display_text: String,
    /// All locations where this hole appears
    pub occurrences: Vec<OccurrenceLocation>,
    /// Resolution candidates with confidence
    pub candidates: Vec<ResolutionCandidate>,
    /// If filled, what it resolved to
    pub resolution: Option<EntityBinding>,
}

impl HoleRegistry {
    /// Get or create a hole for an entity that lacks a chain_id.
    pub fn get_or_create_hole(&mut self, display_text: &str) -> HoleId {
        let normalized = normalize_party_name(display_text);

        if let Some(&hole_id) = self.text_to_hole.get(&normalized) {
            return hole_id;
        }

        let hole_id = HoleId(self.next_hole_id);
        self.next_hole_id += 1;

        self.text_to_hole.insert(normalized.clone(), hole_id);
        self.hole_info.insert(hole_id, HoleInfo {
            display_text: display_text.to_string(),
            occurrences: Vec::new(),
            candidates: Vec::new(),
            resolution: None,
        });

        hole_id
    }

    /// Fill a hole with a resolution (cascades to all references).
    pub fn fill_hole(&mut self, hole_id: HoleId, resolution: EntityBinding) -> bool {
        if let Some(info) = self.hole_info.get_mut(&hole_id) {
            info.resolution = Some(resolution);
            return true;
        }
        false
    }

    /// Unify two holes (they represent the same entity).
    pub fn unify_holes(&mut self, a: HoleId, b: HoleId) {
        // Merge b into a
        if let Some(info_b) = self.hole_info.remove(&b) {
            if let Some(info_a) = self.hole_info.get_mut(&a) {
                info_a.occurrences.extend(info_b.occurrences);
                info_a.candidates.extend(info_b.candidates);
            }
            // Update text_to_hole mappings
            let text = normalize_party_name(&info_b.display_text);
            self.text_to_hole.insert(text, a);
        }
    }
}
```

### 1.4 Impact on Party Mapping

Cross-document party mapping now handles three cases:

| Source | Target | Strategy |
|--------|--------|----------|
| Resolved (chain) | Resolved (chain) | Match by canonical_name |
| Resolved | Hole | Hole becomes candidate for that chain |
| Hole | Resolved | Hole becomes candidate for that chain |
| Hole | Hole | Create hypothesis link; verify later |

```rust
impl PartyMapping {
    pub fn align_with_holes(
        source_bindings: &[EntityBinding],
        target_bindings: &[EntityBinding],
        source_holes: &HoleRegistry,
        target_holes: &HoleRegistry,
    ) -> Self {
        let mut mappings = Vec::new();
        let mut hypotheses = Vec::new();

        for src in source_bindings {
            for tgt in target_bindings {
                match (src, tgt) {
                    (EntityBinding::Resolved { canonical_name: a, .. },
                     EntityBinding::Resolved { canonical_name: b, .. }) => {
                        if normalize_party_name(a) == normalize_party_name(b) {
                            mappings.push(/* confirmed mapping */);
                        }
                    }
                    (EntityBinding::Hole { display_text: a, .. },
                     EntityBinding::Hole { display_text: b, .. }) => {
                        if normalize_party_name(a) == normalize_party_name(b) {
                            hypotheses.push(AlignmentHypothesis {
                                source_hole: src.clone(),
                                target_hole: tgt.clone(),
                                confidence: 0.7,  // Same text = likely same entity
                                needs_verification: true,
                            });
                        }
                    }
                    // ... other combinations
                }
            }
        }

        PartyMapping {
            confirmed_mappings: mappings,
            hypothetical_mappings: hypotheses,
            // ...
        }
    }
}
```

### 1.5 External Linking Support

The user mentioned wanting to support "multiple external linked IDs." Holes enable this:

```rust
/// External identity links for a resolved entity.
#[derive(Debug, Clone)]
pub struct ExternalLinks {
    /// Link to company registry (e.g., SEC CIK, LEI)
    pub legal_entity_id: Option<String>,
    /// Link to internal party database
    pub internal_party_id: Option<String>,
    /// Links to same entity in other contracts
    pub cross_contract_links: Vec<CrossContractLink>,
}

#[derive(Debug, Clone)]
pub struct CrossContractLink {
    pub document_id: String,
    pub chain_id: Option<u32>,
    pub hole_id: Option<HoleId>,
    pub confidence: f64,
}

impl EntityBinding {
    pub fn with_external_links(self, links: ExternalLinks) -> ResolvedEntity {
        ResolvedEntity {
            binding: self,
            external_links: links,
        }
    }
}
```

### 1.6 Registry Scope and Persistence Semantics

Holes only add value if every consumer can unambiguously refer to the same unresolved entity across runs without leaking context between unrelated documents. We therefore scope each `HoleRegistry` and its monotonic `HoleId` counter by an **owner identifier** and a **registry kind**:

```text
RegistryKind::Document  + owner_id = document_id/version pair
RegistryKind::Comparison + owner_id = comparison_id
```

- **Per-document registries** capture holes discovered during extraction. Their owner_id is the canonical document identifier (optionally suffixed with the version). Rehydrating `DocumentSnapshot` must include the tuple `(RegistryKind::Document, owner_id, hole_registry)` so later verification events can target the right namespace.
- **Per-comparison registries** track hole hypotheses introduced while aligning two documents. Their owner_id equals `ComparisonSet.comparison_id`, ensuring speculative pairings do not bleed into unrelated comparisons.

Persistence is deliberately two-tiered:

1. **Session storage**: keeps a registry alive for the duration of an analysis session (e.g., repeated runs on the same draft) without publishing it beyond that context.
2. **Application storage**: elevates a registry to long-term storage when a document enters a managed corpus or a comparison becomes canonical (e.g., Amendment 2 vs. Master). Promotion requires explicit intent so that `HoleId` 17 in `contract_a` never collides with `HoleId` 17 in an unrelated matter.

Unknowns:
- The serialization format for the `(RegistryKind, owner_id, HoleId)` tuple still needs to be specified (JSON vs. binary).
- We have not yet defined cache eviction/migration when a document is renamed; until then, owner_id renames must be avoided or accompanied by a registry remap tool.
- WASM compatibility for persisting registries is unverified; the current plan assumes server-side storage.

---

## 2. ObligationGraph vs ObligationPropertyGraph

### 2.1 Current State

From `accountability_analytics.rs`:
```rust
pub struct ObligationGraph<'a> {
    nodes: &'a [Scored<ObligationNode>],
}
```

This is a thin wrapper with query methods (`for_party`, `with_condition`, etc.).

### 2.2 Recommendation: Unified Structure with Lazy Indexing

Rather than two separate structures, extend `ObligationGraph` with optional edge indexes:

```rust
/// Analytics and graph operations over obligation nodes.
///
/// The graph representation is computed lazily when graph algorithms are needed.
pub struct ObligationGraph<'a> {
    /// Core node storage (existing)
    nodes: &'a [Scored<ObligationNode>],

    /// Lazily-computed edge indexes (new)
    edge_index: OnceCell<EdgeIndex>,

    /// Hole registry for unresolved entities (new)
    holes: Option<&'a HoleRegistry>,
}

/// Edge indexes computed on-demand for graph algorithms.
#[derive(Debug)]
pub struct EdgeIndex {
    /// party_chain_id → Vec<node_id> for O(1) party lookup
    obligor_index: HashMap<u32, Vec<u32>>,
    beneficiary_index: HashMap<u32, Vec<u32>>,

    /// hole_id → Vec<node_id> for hole-based queries
    hole_obligor_index: HashMap<HoleId, Vec<u32>>,
    hole_beneficiary_index: HashMap<HoleId, Vec<u32>>,

    /// condition pattern → Vec<node_id>
    condition_index: HashMap<String, Vec<u32>>,

    /// For WL-hashing: adjacency representation
    adjacency: HashMap<VertexId, Vec<(EdgeType, VertexId)>>,
}

impl<'a> ObligationGraph<'a> {
    /// Get or build edge indexes for graph operations.
    fn edge_index(&self) -> &EdgeIndex {
        self.edge_index.get_or_init(|| self.build_edge_index())
    }

    /// Compute structural fingerprint (uses edge index).
    pub fn structural_fingerprint(&self) -> u64 {
        let index = self.edge_index();
        // WL-hash over adjacency
        weisfeiler_leman_hash(&index.adjacency, 3)
    }

    /// Convert to snapshot for comparison.
    pub fn to_snapshot(&self, document_id: &str, version: Option<&str>) -> DocumentSnapshot {
        DocumentSnapshot {
            document_id: document_id.to_string(),
            version: version.map(String::from),
            snapshot_at: Utc::now(),
            obligations: self.nodes.iter().map(|n| normalize_obligation(n)).collect(),
            party_chains: self.extract_party_chains(),
            hole_registry: self.holes.cloned(),
            structural_fingerprint: self.structural_fingerprint(),
        }
    }
}
```

### 2.3 Testability Split

If a clean split aids testing, the division should be:

| Component | Responsibility | Location |
|-----------|---------------|----------|
| `ObligationGraph` | Node storage + simple queries | `accountability_analytics.rs` (existing) |
| `EdgeIndex` | Graph structure + algorithms | `accountability_analytics.rs` (new section) |
| `ComparisonEngine` | Cross-document operations | `comparison/` (new module) |

The key insight is that edge indexing is an **optimization**, not a separate data model.

---

## 3. Diff Granularity: Clauses, Aggregates, and Nodes

### 3.1 Understanding the Hierarchy

From the code:

```
ObligationPhrase (Layer 5)
    ↓
ContractClause (Layer 7) - one phrase → one clause
    ↓
ClauseAggregate (Layer 8) - groups contiguous clauses by obligor
    ↓
ObligationNode (Layer 9) - adds beneficiaries and condition edges
```

**ClauseAggregate** (`clause_aggregate.rs:15-28`):
```rust
pub struct ClauseAggregate {
    pub aggregate_id: u32,
    pub obligor: ClauseParty,
    pub clause_ids: Vec<u32>,          // Multiple clauses
    pub clauses: Vec<ClauseAggregateEntry>,  // Details for each
    pub source_start: usize,
    pub source_end: usize,
}
```

**ObligationNode** (`accountability_graph.rs:44-62`):
```rust
pub struct ObligationNode {
    pub node_id: u32,          // = aggregate_id
    pub aggregate_id: u32,
    pub obligor: ClauseParty,
    pub beneficiaries: Vec<BeneficiaryLink>,   // Added in Layer 9
    pub condition_links: Vec<ConditionLink>,    // Added in Layer 9
    pub clauses: Vec<ClauseAggregateEntry>,     // From aggregate
    pub verification_notes: Vec<VerificationNote>,
}
```

### 3.2 Three-Level Diff Model

```rust
/// Hierarchical diff result at multiple granularities.
#[derive(Debug, Clone)]
pub struct HierarchicalDiff {
    /// High-level: which aggregates matched, changed, appeared, disappeared
    pub aggregate_diff: AggregateDiff,
    /// Mid-level: for matched aggregates, how did beneficiaries/conditions change
    pub node_diff: Vec<NodeDiff>,
    /// Low-level: for matched nodes, how did individual clauses change
    pub clause_diff: Vec<ClauseDiff>,
}

#[derive(Debug, Clone)]
pub struct AggregateDiff {
    pub matched: Vec<AggregateMatch>,
    pub added: Vec<AggregateRef>,    // New aggregates in target
    pub removed: Vec<AggregateRef>,  // Missing from target
}

#[derive(Debug, Clone)]
pub struct AggregateMatch {
    pub source_aggregate_id: u32,
    pub target_aggregate_id: u32,
    pub obligor_match: PartyMatchType,
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub enum PartyMatchType {
    /// Same chain_id
    ChainMatch { chain_id: u32 },
    /// Same normalized name, one or both holes
    NameMatch { normalized: String, has_holes: bool },
    /// Different names but mapped by external input
    MappedMatch { source_name: String, target_name: String },
}

#[derive(Debug, Clone)]
pub struct NodeDiff {
    pub aggregate_match: AggregateMatch,
    /// Changes to beneficiary set
    pub beneficiary_changes: Vec<BeneficiaryChange>,
    /// Changes to condition links
    pub condition_changes: Vec<ConditionChange>,
}

#[derive(Debug, Clone)]
pub struct ClauseDiff {
    pub node_diff_index: usize,
    pub source_clause_id: Option<u32>,
    pub target_clause_id: Option<u32>,
    pub action_text_change: Option<TextChange>,
    pub obligation_type_change: Option<(ObligationType, ObligationType)>,
    pub condition_changes: Vec<ConditionChange>,
}
```

### 3.3 Diff Strategy by Level

| Level | Matching Strategy | What We Compare |
|-------|-------------------|-----------------|
| **Aggregate** | Party identity (chain/hole/name) | Did the grouping change? |
| **Node** | Already matched by aggregate | Beneficiary/condition edge changes |
| **Clause** | Action text similarity within matched node | Individual clause wording |

**Default behavior**: Report at node level (most useful for "what changed for Seller?")

**Drill-down**: When node reports "modified", show clause-level changes

**Roll-up**: Summary counts by aggregate

### 3.4 Surfacing the Hierarchy in ComparisonSet

To let downstream systems opt into the richer hierarchy without breaking existing code, `ComparisonSet` should expose **both** representations:

```rust
pub struct ComparisonSet {
    pub deltas: Vec<ObligationDelta>,              // Back-compat flat list
    pub hierarchical: Option<HierarchicalDiff>,    // New structured view
    pub match_results: Vec<MatchResultMetadata>,   // Indexed by delta_id
    // ...
}
```

- `deltas` remains the primary list for legacy consumers and summary counts.
- `hierarchical` contains aggregate/node/clause trees. When present, it is the source-of-truth for building UI drill-downs or analytics.
- `match_results` aligns to each `delta_id` (or node diff) and stores `Definite` vs. `Indeterminate` metadata. This keeps ambiguity metadata adjacent to the data it qualifies.

Open items:
- The binary and JSON payload schemas need versioning so older clients can ignore `hierarchical`.
- We still need to define whether `hierarchical` is populated in Phase 1 or guarded by a feature flag.
- `match_results` may instead be embedded directly in `ObligationDelta`; final placement should be validated once serialization prototypes exist.

---

## 4. WL-Hash Justification and Alternatives

### 4.1 When WL-Hash Adds Value

Weisfeiler-Leman hashing captures **structural patterns** in the obligation graph:

| Pattern | WL-Hash Captures |
|---------|------------------|
| Obligation with 2 conditions | Different fingerprint from 1 condition |
| Obligation to 3 beneficiaries | Different from 1 beneficiary |
| Cascading conditions (A → B → C) | Path structure in hash |

**Good for**:
- **Template matching**: "This contract matches our standard NDA template (95% structural similarity)"
- **Anomaly detection**: "This clause has unusual structure compared to typical indemnification clauses"
- **Corpus search**: "Find contracts with similar obligation patterns"

**Overkill for**:
- Simple version diff (same parties, looking for text changes)
- Contracts with <50 obligations

### 4.2 Simpler Alternatives for MVP

```rust
/// Simple structural fingerprint without graph algorithms.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimpleFingerprint {
    pub obligor_normalized: String,
    pub obligation_type: ObligationType,
    pub action_hash: u64,
    pub beneficiary_count: usize,
    pub condition_count: usize,
    pub condition_types: Vec<ContractKeyword>,
}

impl SimpleFingerprint {
    pub fn from_node(node: &ObligationNode) -> Self {
        Self {
            obligor_normalized: normalize_party_name(&node.obligor.display_text),
            obligation_type: node.clauses.first()
                .map(|c| c.duty.obligation_type)
                .unwrap_or(ObligationType::Duty),
            action_hash: hash_action_text(node),
            beneficiary_count: node.beneficiaries.len(),
            condition_count: node.condition_links.len(),
            condition_types: node.condition_links.iter()
                .map(|c| c.condition.condition_type.clone())
                .collect(),
        }
    }

    /// Quick structural similarity (0.0 to 1.0)
    pub fn similarity(&self, other: &Self) -> f64 {
        let mut score = 0.0;
        let mut weight = 0.0;

        // Same obligor (normalized): 0.3
        if self.obligor_normalized == other.obligor_normalized {
            score += 0.3;
        }
        weight += 0.3;

        // Same obligation type: 0.2
        if self.obligation_type == other.obligation_type {
            score += 0.2;
        }
        weight += 0.2;

        // Similar beneficiary count: 0.2
        let ben_diff = (self.beneficiary_count as i32 - other.beneficiary_count as i32).abs();
        score += 0.2 * (1.0 - (ben_diff as f64 / 5.0).min(1.0));
        weight += 0.2;

        // Similar condition structure: 0.3
        let cond_overlap = jaccard_similarity(&self.condition_types, &other.condition_types);
        score += 0.3 * cond_overlap;
        weight += 0.3;

        score / weight
    }
}
```

### 4.3 Recommendation

**Phase 1 (MVP)**: Use `SimpleFingerprint` for structural comparison
**Phase 2**: Add WL-hash for corpus-level operations (template matching, anomaly detection)
**Phase 3**: Integrate with vector embeddings for semantic+structural similarity

---

## 5. Relaxed Matching and Indeterminate Results

### 5.1 Current Behavior

From `clause_aggregate.rs:131-154`:
```rust
/// Relaxed matching for party keys.
///
/// Two keys match if:
/// 1. They are exactly equal (same chain_id and name), OR
/// 2. They have the same normalized name AND at least one side has no chain_id
fn matches_relaxed(&self, other: &ClausePartyKey) -> bool {
    // ...
    match (self.chain_id, other.chain_id) {
        (Some(a), Some(b)) => a == b,
        _ => true,  // At least one is None, allow merge
    }
}
```

This means: **we're not always sure if two references are the same party**.

### 5.2 Comparison with Indeterminate Results

Instead of forcing a binary match/no-match, surface uncertainty:

```rust
/// Match result with explicit indeterminacy.
#[derive(Debug, Clone)]
pub enum MatchResult {
    /// High confidence match
    Definite {
        confidence: f64,
        match_type: MatchType,
    },
    /// Likely match but needs verification
    Indeterminate {
        confidence: f64,
        ambiguity: AmbiguityReason,
        /// What information would resolve this?
        resolution_needed: ResolutionHint,
    },
    /// Definitely not a match
    NoMatch,
}

#[derive(Debug, Clone)]
pub enum AmbiguityReason {
    /// Both sides have holes (unresolved entities)
    BothUnresolved { source_hole: HoleId, target_hole: HoleId },
    /// Same text but different chain IDs (possible different entities)
    ConflictingChains { name: String, source_chain: u32, target_chain: u32 },
    /// Semantically similar but not identical text
    SemanticAmbiguity { similarity: f64, source_text: String, target_text: String },
    /// Multiple plausible interpretations exist
    MultipleInterpretations { candidates: Vec<InterpretationCandidate> },
}

#[derive(Debug, Clone)]
pub enum ResolutionHint {
    /// Need human to confirm party identity
    VerifyPartyIdentity { display_text: String },
    /// Need LLM to analyze semantic equivalence
    SemanticAnalysisNeeded { source_text: String, target_text: String },
    /// Need domain expert for legal interpretation
    LegalInterpretationNeeded { clause_context: String },
}
```

### 5.3 Surfacing Indeterminacy in Diff Results

```rust
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    /// Definite matches with changes
    pub definite_changes: Vec<ObligationDelta>,
    /// Matches we're unsure about
    pub indeterminate_matches: Vec<IndeterminateMatch>,
    /// Items we couldn't match at all
    pub unmatched_source: Vec<ObligationRef>,
    pub unmatched_target: Vec<ObligationRef>,
    /// What verification would improve confidence
    pub verification_suggestions: Vec<VerificationSuggestion>,
}

#[derive(Debug, Clone)]
pub struct IndeterminateMatch {
    pub source_ref: ObligationRef,
    pub target_ref: ObligationRef,
    pub match_result: MatchResult,
    /// If we assume this is a match, what's the delta?
    pub hypothetical_delta: Option<ObligationDelta>,
}
```

### 5.3.1 Verification and Storage Contract

- Every `ObligationDelta` (and/or `NodeDiff`) should embed a `MatchResultMetadata` payload:

```rust
pub struct MatchResultMetadata {
    pub delta_id: u32,
    pub result: MatchResult,
}
```

- Verification queues read `result` directly, so ambiguous matches can be prioritized without querying the hole registry.
- When a resolution arrives (human confirmation, LLM verdict, etc.), `ComparisonResult::apply_resolution` updates the metadata and promotes the affected entries from `indeterminate_matches` into `definite_changes`.
- The metadata must also travel with serialized comparison payloads; otherwise, downstream systems cannot reproduce the same queue ordering.

Unknowns:
- Whether `MatchResultMetadata` is stored inline (per delta) or in a side-table keyed by `delta_id`.
- How to deduplicate metadata when multiple clause-level entries roll up into a single node-level delta.
- The verification UI flow still needs design for referencing either hole ids or concrete clause refs when presenting `resolution_hint`.

### 5.4 Injecting External Attributes

The user mentioned: *"We would need somebody to inject attributes that are more concrete into the system."*

```rust
/// External input that resolves indeterminacy.
#[derive(Debug, Clone)]
pub enum ExternalResolution {
    /// Human confirms party identity
    PartyConfirmation {
        hole_id: HoleId,
        resolved_to: EntityBinding,
        verifier: String,
    },
    /// LLM provides semantic analysis
    SemanticAnalysis {
        source_clause: ClauseRef,
        target_clause: ClauseRef,
        verdict: SemanticVerdict,
        reasoning: String,
        model: String,
    },
    /// Domain expert provides legal interpretation
    LegalInterpretation {
        clause_ref: ClauseRef,
        interpretation: String,
        confidence: f64,
        expert_id: String,
    },
}

#[derive(Debug, Clone)]
pub enum SemanticVerdict {
    Equivalent,
    SubstantiallyEquivalent { differences: Vec<String> },
    MateriallyDifferent { key_differences: Vec<String> },
    Opposite,
    CannotDetermine { reason: String },
}

impl ComparisonResult {
    /// Apply external resolution and recompute affected matches.
    pub fn apply_resolution(&mut self, resolution: ExternalResolution) {
        // Update indeterminate matches based on new information
        // Move resolved items from indeterminate to definite
    }
}
```

---

## 6. Beneficiary Detection Limitations

### 6.1 Current Detection Pattern

From `accountability_graph.rs:107`:
```rust
while let Some(rel_idx) = lower[cursor..].find(" to ") {
```

This simple pattern has known issues:

| Input | Result | Problem |
|-------|--------|---------|
| "deliver goods to the Buyer" | ✓ Buyer detected | Works |
| "provide services for the Buyer" | ✗ No beneficiary | Pattern miss |
| "pursuant to Section 5" | ✗ "Section 5" as beneficiary | False positive |
| "pay to the order of Bank" | ? Depends on filter | Ambiguous |

### 6.2 Impact on Comparison

Noisy beneficiary data means:
- Some beneficiaries won't be detected in one version but will be in another
- False positive beneficiaries add noise to diffs
- Comparison confidence should account for detection quality

```rust
/// Beneficiary match with quality assessment.
#[derive(Debug, Clone)]
pub struct BeneficiaryMatchAssessment {
    pub source_beneficiary: Option<BeneficiaryLink>,
    pub target_beneficiary: Option<BeneficiaryLink>,
    pub match_type: BeneficiaryMatchType,
    /// Did the detection pattern likely capture this correctly?
    pub detection_quality: DetectionQuality,
}

#[derive(Debug, Clone)]
pub enum DetectionQuality {
    /// Clear "to X" pattern with known entity
    HighConfidence,
    /// Pattern matched but entity unknown
    MediumConfidence,
    /// Possible false positive (e.g., "pursuant to")
    LowConfidence { reason: String },
    /// Detection pattern didn't fire
    NotDetected,
}
```

### 6.3 Recommendation for RFC

Acknowledge beneficiary detection limitations in the diff confidence calculation:

```rust
fn calculate_delta_confidence(
    source: &ObligationNode,
    target: &ObligationNode,
    match_result: &MatchResult,
) -> f64 {
    let mut confidence = match_result.confidence();

    // Penalize when beneficiary detection is uncertain
    if source.beneficiaries.iter().any(|b| b.needs_verification) {
        confidence -= 0.05;
    }
    if target.beneficiaries.iter().any(|b| b.needs_verification) {
        confidence -= 0.05;
    }

    // Extra penalty if beneficiary counts differ (could be detection issue)
    if source.beneficiaries.len() != target.beneficiaries.len() {
        confidence -= 0.10;
    }

    confidence.clamp(0.0, 1.0)
}
```

---

## 7. Line Number Corrections

The RFC cited incorrect line numbers. Corrections:

| Reference | RFC Citation | Actual Location |
|-----------|--------------|-----------------|
| `ObligationNode` | `accountability_graph.rs:42-59` | `accountability_graph.rs:44-62` |
| `BeneficiaryLink` | `accountability_graph.rs:18-31` | `accountability_graph.rs:18-32` |
| `ConditionLink` | `accountability_graph.rs:33-40` | `accountability_graph.rs:34-42` |
| Chain skip logic | Not cited | `pronoun_chain.rs:300-303` |
| Relaxed matching | Not cited | `clause_aggregate.rs:131-154` |
| Beneficiary pattern | Not cited | `accountability_graph.rs:107` |

---

## 8. Revised Open Questions

Based on the user's input, here's the updated status:

### Resolved

| Question | Resolution |
|----------|------------|
| How to handle `chain_id: None`? | Introduce Holes as first-class citizens with stable IDs |
| ObligationGraph vs PropertyGraph? | Same structure with lazy edge indexing |
| WL-hash value? | MVP uses SimpleFingerprint; WL for advanced use cases |

### Partially Resolved

| Question | Direction | Needs More Work |
|----------|-----------|-----------------|
| Diff granularity | Three-level (aggregate/node/clause) | Needs UX for level selection |
| Relaxed matching | Surface indeterminacy explicitly | Needs verification workflow design |
| External linking | Holes support multiple external IDs | Needs schema for external ID types |

### Still Open

| Question | Notes |
|----------|-------|
| Embedding model choice | Depends on latency/accuracy tradeoffs |
| Amendment temporal model | Event stream clear; bi-temporal if regulated |
| WASM compatibility for comparison | Needs investigation |

---

## 9. Updated Implementation Plan

### Phase 0: Foundation (Before Phase 1)

Add hole infrastructure:
1. `HoleId` and `EntityBinding` types
2. `HoleRegistry` with get_or_create, unify, fill operations
3. Update `ClauseParty` to use `EntityBinding` instead of `Option<u32>`
4. Migration path for existing code
5. Define `(RegistryKind, owner_id)` namespacing and persistence contract (document vs. comparison scope, session vs. application storage) and ensure serialization prototypes exist even if persistence backend is TBD

### Revised Phase 1: Basic Diffing with Holes

1. `DocumentSnapshot` with `HoleRegistry`
2. `SimpleFingerprint` for structural matching
3. `MatchResult` with `Indeterminate` variant
4. Three-level `HierarchicalDiff` output
5. `ComparisonSet` carries both `deltas` and optional `hierarchical` plus `MatchResultMetadata` (location can remain TBD but must be captured in the plan)
6. Comparison payload serialization includes match metadata even if consumers ignore it initially

### Revised Phase 2: Edge Indexing

1. Add `EdgeIndex` to `ObligationGraph` (lazy)
2. Implement party/beneficiary indexes with hole support
3. Add `structural_fingerprint()` using simple approach
4. Optional: WL-hash behind feature flag

---

## 10. Summary of Key Design Changes

| Original RFC | Revised Design |
|--------------|----------------|
| Assumed chain_id exists for mapping | Holes provide stable IDs for unresolved entities |
| Separate ObligationPropertyGraph | Unified with lazy edge indexing |
| Binary match/no-match | Explicit indeterminacy with resolution hints |
| Single diff granularity | Three-level hierarchy |
| WL-hash for all cases | SimpleFingerprint for MVP, WL optional |
| Hidden detection limitations | Surfaced in confidence and quality flags |

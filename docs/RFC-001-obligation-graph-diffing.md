# RFC-001: Obligation Graph Diffing and Cross-Contract Comparison

| Field | Value |
|-------|-------|
| **RFC ID** | RFC-001 |
| **Title** | Obligation Graph Diffing and Cross-Contract Comparison |
| **Status** | Draft |
| **Created** | 2025-12-18 |
| **Authors** | Architecture Design Session |

## Addendum

**[RFC-001 Addendum: Critical Gaps and Design Clarifications](./RFC-001-addendum-critical-gaps.md)** addresses:
- The single-mention entity problem (holes as first-class citizens)
- ObligationGraph vs ObligationPropertyGraph unification
- Three-level diff granularity (aggregate/node/clause)
- WL-hash justification and simpler alternatives
- Relaxed matching and indeterminate results
- Beneficiary detection limitations

**[RFC-001 Appendix: Prior Art and Design Inspiration](./RFC-001-appendix-prior-art.md)** synthesizes research from:
- Entity resolution (NIL clustering, Fellegi-Sunter three-zone model, cluster repair)
- Type theory (typed holes, Union-Find unification, Skolemization)
- Active learning (priority scoring, label propagation, weak supervision)
- Legal informatics (LKIF, LegalRuleML, Accord Project Concerto)
- Semantic diff systems (GumTree tree diff, schema versioning)
- Deontic logic (SDL operators, conflict detection, temporal extensions)

---

## Table of Contents

1. [Summary](#1-summary)
2. [Motivation](#2-motivation)
3. [Background](#3-background)
4. [Design Overview](#4-design-overview)
5. [Detailed Design](#5-detailed-design)
   - [5.1 Bounded Contexts](#51-bounded-contexts)
   - [5.2 Graph Data Model](#52-graph-data-model)
   - [5.3 Comparison Pipeline](#53-comparison-pipeline)
   - [5.4 Temporal Modeling](#54-temporal-modeling)
   - [5.5 Verification Integration](#55-verification-integration)
   - [5.6 Formal Semantics](#56-formal-semantics)
6. [API Design](#6-api-design)
7. [Algorithms](#7-algorithms)
8. [Integration Points](#8-integration-points)
9. [Trade-offs and Alternatives](#9-trade-offs-and-alternatives)
10. [Implementation Plan](#10-implementation-plan)
11. [Open Questions](#11-open-questions)
12. [References](#12-references)

---

## 1. Summary

This RFC proposes an architecture for diffing and comparing contract obligations across document versions and related contracts. The design introduces a **Comparison Context** as a new bounded context that consumes the existing extraction pipeline output and produces structured diffs with semantic awareness.

Key capabilities:
- **Version diffing**: Compare obligations between contract versions (v1.0 → v2.0)
- **Cross-contract comparison**: Find materially similar clauses across different contracts
- **Party mapping**: Align party identities across documents ("Seller" in A = "Vendor" in B)
- **Semantic equivalence**: Detect same legal effect despite different wording
- **Temporal queries**: Answer "What were obligations as of date X?"

The architecture synthesizes six complementary paradigms:
1. Graph-theoretic (property graphs, graph edit distance)
2. Event sourcing (amendments as deltas, bi-temporal modeling)
3. Formal methods (deontic logic, refinement types)
4. LLM/embeddings (semantic similarity, LLM-as-judge)
5. Human-in-the-loop (active learning, annotation propagation)
6. Domain-driven design (bounded contexts, aggregates)

---

## 2. Motivation

### 2.1 Current Limitations

The existing `layered-contracts` implementation ([`layered-contracts/src/`](../layered-contracts/src/)) provides excellent single-document analysis through its 9-layer resolver stack. However, it lacks:

1. **No cross-document operations**: Each `LLLine` is processed independently with no mechanism to compare obligations across documents.

2. **No version tracking**: Amendments and contract revisions cannot be modeled as incremental changes.

3. **No semantic matching**: Two clauses with identical legal effect but different wording cannot be identified as equivalent.

4. **Limited party resolution**: The `PronounChain` system ([`pronoun_chain.rs:15-45`](../layered-contracts/src/pronoun_chain.rs)) resolves entities within a single document but cannot map parties across documents.

### 2.2 Use Cases Enabled

This design enables:

| Use Case | Description |
|----------|-------------|
| **Amendment analysis** | "What changed between the original contract and Amendment 2?" |
| **Template deviation** | "How does this negotiated contract differ from our standard template?" |
| **Portfolio search** | "Find all contracts with indemnification clauses similar to this one" |
| **Compliance tracking** | "What were Seller's obligations as of March 2024?" |
| **Risk assessment** | "Which changes increase our liability exposure?" |

### 2.3 Design Goals

1. **Preserve existing architecture**: Build on top of Layer 9 output, not replace it
2. **Separation of concerns**: Comparison logic in dedicated bounded context
3. **Incremental adoption**: Each capability independently valuable
4. **Deterministic + AI-augmented**: Rule-based core with optional LLM enhancement
5. **Auditable**: Full provenance for all comparisons and matches

---

## 3. Background

### 3.1 Current Data Model

The existing extraction pipeline produces:

```
Layer 9: AccountabilityGraphResolver
         └── Scored<ObligationNode>
```

**`ObligationNode`** ([`accountability_graph.rs:42-59`](../layered-contracts/src/accountability_graph.rs)):
```rust
pub struct ObligationNode {
    pub node_id: u32,
    pub aggregate_id: u32,
    pub obligor: ClauseParty,
    pub beneficiaries: Vec<BeneficiaryLink>,
    pub condition_links: Vec<ConditionLink>,
    pub clauses: Vec<ClauseAggregateEntry>,
    pub verification_notes: Vec<VerificationNote>,
}
```

**`Scored<T>`** ([`scored.rs:8-25`](../layered-contracts/src/scored.rs)):
```rust
pub struct Scored<T> {
    pub value: T,
    pub confidence: f64,
    pub source: ScoreSource,
}
```

**`ScoreSource`** ([`scored.rs:27-45`](../layered-contracts/src/scored.rs)):
```rust
pub enum ScoreSource {
    RuleBased { rule_name: String },
    LLMPass { model: String, pass_id: String },
    HumanVerified { verifier_id: String },
    Derived,
}
```

### 3.2 Existing Analytics Layer

The `ObligationGraph` wrapper ([`accountability_analytics.rs:24-120`](../layered-contracts/src/accountability_analytics.rs)) provides party-centric queries:

```rust
impl ObligationGraph {
    pub fn for_party(&self, chain_id: u32) -> PartyAnalytics;
    pub fn with_condition(&self, predicate: impl Fn(&ConditionLink) -> bool) -> Vec<...>;
    pub fn verification_queue(&self) -> Vec<VerificationQueueItem>;
    pub fn payload(&self) -> AccountabilityPayload;
}
```

### 3.3 Verification System

The verification module ([`verification.rs:10-120`](../layered-contracts/src/verification.rs)) provides:

```rust
pub enum VerificationAction {
    VerifyNode { node_id: u32, verifier_id: String, note: Option<String> },
    ResolveBeneficiary { node_id: u32, source_clause_id: u32, ... },
}

pub fn apply_verification_action(
    nodes: &mut [Scored<ObligationNode>],
    action: VerificationAction,
) -> bool;
```

---

## 4. Design Overview

### 4.1 Architectural Layers

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         EXTRACTION CONTEXT                               │
│                     (existing layered-contracts)                         │
│                                                                          │
│  Input Text → Layers 1-9 → Scored<ObligationNode> + Scored<PronounChain>│
└────────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     │ ObligationExtracted events
                                     ↓
┌─────────────────────────────────────────────────────────────────────────┐
│                      GRAPH CONSTRUCTION LAYER                            │
│                                                                          │
│  ObligationPropertyGraph::from_extraction(nodes, chains, doc_id)        │
│  ├── PartyVertex (from PronounChain)                                    │
│  ├── ObligationVertex (from ObligationNode)                             │
│  ├── ConditionVertex (from ConditionLink)                               │
│  └── Typed edges: ObligorOf, BeneficiaryOf, TriggeredBy, SubjectTo     │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │
                 ┌───────────────────┴───────────────────┐
                 ↓                                       ↓
┌────────────────────────────────────┐   ┌────────────────────────────────┐
│      COMPARISON CONTEXT            │   │     TEMPORAL CONTEXT           │
│                                    │   │                                │
│  ComparisonSet (aggregate root)    │   │  ContractEventStore            │
│  ├── source: DocumentSnapshot     │   │  ├── append(events)            │
│  ├── target: DocumentSnapshot     │   │  ├── replay_to(marker)         │
│  ├── party_mapping: PartyMapping  │   │  └── obligations_as_of(date)   │
│  └── deltas: Vec<ObligationDelta> │   │                                │
│                                    │   │  TemporalObligationStore       │
│  DiffPipeline                      │   │  ├── bi-temporal queries       │
│  ├── ContentHashMatcher            │   │  └── version changelog         │
│  ├── StructuralMatcher (WL-hash)   │   │                                │
│  ├── SemanticMatcher (embeddings)  │   └────────────────────────────────┘
│  └── LLMJudge (optional)           │
└────────────────────────────────────┘
                 │
                 ↓
┌─────────────────────────────────────────────────────────────────────────┐
│                      VERIFICATION CONTEXT                                │
│                   (extends existing verification.rs)                     │
│                                                                          │
│  PrioritizedQueue                                                        │
│  ├── priority = (1 - confidence) × impact + disagreement_penalty        │
│  ├── SimilarityCluster (one decision → many resolutions)                │
│  └── DisagreementRecord (rule vs LLM conflict flagging)                 │
│                                                                          │
│  FeedbackLoop                                                            │
│  ├── CorrectionRecord → allowlist expansion                             │
│  └── CalibrationMetrics → confidence adjustment                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Aggregate root for comparison** | `ComparisonSet` | Different invariants than extraction; on-demand lifecycle |
| **Party mapping approach** | Explicit `PartyMapping` with confidence | Allows manual override; auditable |
| **Matching strategy** | Hybrid pipeline (hash → structure → semantic → LLM) | Balance accuracy and performance |
| **Temporal model** | Event stream with optional bi-temporal | Start simple, add complexity as needed |
| **LLM integration** | Optional, trait-based | Deterministic core, AI-augmented when needed |

---

## 5. Detailed Design

### 5.1 Bounded Contexts

#### 5.1.1 Context Map

```
┌─────────────────┐         ┌─────────────────┐
│   EXTRACTION    │         │   COMPARISON    │
│    CONTEXT      │────────▶│    CONTEXT      │
│                 │  D/S    │                 │
│ (upstream)      │         │ (downstream)    │
└─────────────────┘         └─────────────────┘
         │                          │
         │                          │
         ▼                          ▼
┌─────────────────┐         ┌─────────────────┐
│   VERIFICATION  │◀────────│   TEMPORAL      │
│    CONTEXT      │  D/S    │    CONTEXT      │
│                 │         │                 │
│ (conformist)    │         │ (downstream)    │
└─────────────────┘         └─────────────────┘

Legend: D/S = Downstream/Supplier relationship
```

#### 5.1.2 Comparison Context Responsibilities

**Owns:**
- `ComparisonSet` aggregate
- `PartyMapping` value object
- `ObligationDelta` value object
- Diff algorithm implementations
- Semantic similarity computation

**Consumes (read-only):**
- `Scored<ObligationNode>` from Extraction
- `Scored<PronounChain>` from Extraction
- `VerificationStatus` from Verification

**Publishes:**
- `ComparisonCompleted` event
- `PartyMappingEstablished` event
- `MaterialDifferenceDetected` event

### 5.2 Graph Data Model

#### 5.2.1 Vertex Types

```rust
/// Vertex identifier with type discrimination
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VertexId {
    Party(u32),        // chain_id from PronounChain
    Obligation(u32),   // node_id from ObligationNode
    Condition(u32),    // synthetic ID for conditions
    Section(String),   // section reference (e.g., "5.2")
}

/// Party vertex derived from PronounChain
/// Source: pronoun_chain.rs:15-45
#[derive(Debug, Clone)]
pub struct PartyVertex {
    pub id: VertexId,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub is_defined_term: bool,
    pub has_verified_mention: bool,
    pub document_id: String,
}

/// Obligation vertex derived from ObligationNode
/// Source: accountability_graph.rs:42-59
#[derive(Debug, Clone)]
pub struct ObligationVertex {
    pub id: VertexId,
    pub obligation_type: ObligationType,
    pub action_text: String,
    pub action_text_hash: u64,      // For content-addressable matching
    pub structural_hash: u64,        // WL-hash for structural matching
    pub embedding: Option<Vec<f32>>, // For semantic matching
    pub confidence: f64,
    pub source_clause_ids: Vec<u32>,
    pub document_id: String,
}

/// Condition vertex derived from ConditionLink
/// Source: accountability_graph.rs:33-40
#[derive(Debug, Clone)]
pub struct ConditionVertex {
    pub id: VertexId,
    pub condition_type: ContractKeyword,  // If, Unless, SubjectTo, Provided
    pub text: String,
    pub text_hash: u64,
    pub references_section: Option<String>,
    pub has_unknown_entity: bool,
    pub document_id: String,
}
```

#### 5.2.2 Edge Types

```rust
/// Edge types representing relationships in the obligation graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeType {
    // Party → Obligation relationships
    ObligorOf,           // Party bears this obligation
    BeneficiaryOf,       // Party receives benefit of this obligation

    // Obligation → Condition relationships
    TriggeredBy,         // Obligation activated by condition (If, Unless)
    SubjectTo,           // Obligation qualified by condition (SubjectTo, Provided)

    // Cross-document relationships (added by Comparison Context)
    MapsTo,              // Party in doc A maps to party in doc B
    SimilarTo,           // Obligation is semantically similar (cross-doc)
    Supersedes,          // Obligation in new version supersedes old

    // Structural relationships
    ReferencesSection,   // Condition references a section
}

/// Edge with properties
#[derive(Debug, Clone)]
pub struct Edge {
    pub id: u64,
    pub edge_type: EdgeType,
    pub source: VertexId,
    pub target: VertexId,
    pub weight: f64,          // Confidence or similarity score
    pub document_id: String,  // Source document (or "cross" for cross-doc edges)
}
```

#### 5.2.3 Property Graph Container

```rust
/// In-memory property graph for obligation analysis
pub struct ObligationPropertyGraph {
    // Vertex storage
    parties: HashMap<u32, PartyVertex>,
    obligations: HashMap<u32, ObligationVertex>,
    conditions: HashMap<u32, ConditionVertex>,

    // Edge storage with adjacency indexing
    edges: HashMap<u64, Edge>,
    outgoing: HashMap<VertexId, HashSet<u64>>,
    incoming: HashMap<VertexId, HashSet<u64>>,
    edges_by_type: HashMap<EdgeType, HashSet<u64>>,

    // Document metadata
    document_id: String,
    document_version: Option<String>,
    extracted_at: DateTime<Utc>,

    // Counters for ID generation
    next_edge_id: u64,
    next_condition_id: u32,
}

impl ObligationPropertyGraph {
    /// Construct from Layer 9 extraction output
    ///
    /// # Arguments
    /// * `nodes` - ObligationNodes from AccountabilityGraphResolver
    /// * `chains` - PronounChains from PronounChainResolver
    /// * `document_id` - Unique identifier for source document
    pub fn from_extraction(
        nodes: &[Scored<ObligationNode>],
        chains: &[Scored<PronounChain>],
        document_id: &str,
    ) -> Self;

    /// Query builder for fluent graph queries
    pub fn query(&self) -> ObligationQuery<'_>;

    /// Compute Weisfeiler-Leman hash for structural fingerprinting
    pub fn structural_fingerprint(&self, iterations: usize) -> HashSet<u64>;
}
```

### 5.3 Comparison Pipeline

#### 5.3.1 Core Aggregates

```rust
/// Aggregate root for comparison operations
///
/// Invariants:
/// - source and target must have the same document_id OR explicit cross-doc flag
/// - party_mapping must cover all parties referenced in deltas
/// - deltas must be consistent with party_mapping
#[derive(Debug, Clone)]
pub struct ComparisonSet {
    pub comparison_id: ComparisonId,
    pub source: DocumentSnapshot,
    pub target: DocumentSnapshot,
    pub party_mapping: PartyMapping,
    pub deltas: Vec<ObligationDelta>,
    /// Optional hierarchical diff tree for aggregate/node/clause drill-downs
    pub hierarchical: Option<HierarchicalDiff>,
    /// Parallel list describing whether each delta is definite or indeterminate
    pub match_results: Vec<MatchResultMetadata>,
    pub status: ComparisonStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Immutable snapshot of a document's extracted obligations
#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    pub document_id: String,
    pub version: Option<String>,
    pub snapshot_at: DateTime<Utc>,
    pub obligations: Vec<NormalizedObligation>,
    pub party_chains: Vec<PartyChainSnapshot>,
    /// Optional hole registry snapshot so unresolved parties retain stable IDs
    pub hole_registry: Option<HoleRegistrySnapshot>,
    pub graph_fingerprint: u64,  // WL-hash for quick structural comparison
}

/// Captures hole identifiers plus the scope they belong to (document vs. comparison)
#[derive(Debug, Clone)]
pub struct HoleRegistrySnapshot {
    pub registry_kind: RegistryKind,
    pub owner_id: String,
    pub holes: Vec<HoleSnapshot>,
}

#[derive(Debug, Clone)]
pub struct HoleSnapshot {
    pub hole_id: u32,
    pub display_text: String,
    pub occurrence_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryKind {
    Document,
    Comparison,
}

> **Note:** Registry scope determines how long a hole ID remains valid. `RegistryKind::Document`
> registries persist with the document/version combo (session or corpus, depending on storage),
> while `RegistryKind::Comparison` registries exist only for a specific `comparison_id`. The
> concrete serialization format is still TBD but must carry `registry_kind` and `owner_id` so that
> verification events can resolve `hole_id` values without guessing.

/// Normalized obligation for comparison (stripped of document-specific IDs)
#[derive(Debug, Clone)]
pub struct NormalizedObligation {
    pub local_id: u32,                    // ID within this snapshot
    pub obligor_chain_id: u32,
    pub obligation_type: ObligationType,
    pub action_text: String,
    pub action_text_hash: u64,
    pub beneficiary_chain_ids: Vec<u32>,
    pub condition_signatures: Vec<ConditionSignature>,
    pub confidence: f64,
}

/// Signature for condition matching
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConditionSignature {
    pub condition_type: ContractKeyword,
    pub text_hash: u64,
    pub section_reference: Option<String>,
}
```

#### 5.3.2 Party Mapping

```rust
/// Maps parties between documents
#[derive(Debug, Clone)]
pub struct PartyMapping {
    pub mappings: Vec<PartyMappingEntry>,
    pub unmapped_source: Vec<PartyRef>,
    pub unmapped_target: Vec<PartyRef>,
    pub overall_confidence: f64,
}

#[derive(Debug, Clone)]
pub struct PartyMappingEntry {
    pub source_chain_id: u32,
    pub target_chain_id: u32,
    pub source_name: String,
    pub target_name: String,
    pub confidence: f64,
    pub mapping_method: MappingMethod,
    pub verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingMethod {
    /// Exact canonical name match (after normalization)
    ExactNameMatch,
    /// Names match after normalization (case, articles, quotes)
    NormalizedNameMatch,
    /// Matched via defined term reference
    DefinitionReference,
    /// High embedding similarity
    SemanticSimilarity,
    /// Manual mapping by human reviewer
    ManualMapping,
}
```

#### 5.3.3 Obligation Deltas

```rust
/// Describes a difference between obligations across documents
#[derive(Debug, Clone)]
pub struct ObligationDelta {
    pub delta_id: u32,
    pub delta_type: DeltaType,
    pub source_ref: Option<ObligationRef>,  // None if Added
    pub target_ref: Option<ObligationRef>,  // None if Removed
    pub match_confidence: f64,
    pub changes: Vec<ChangeDetail>,
    pub semantic_impact: SemanticImpact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaType {
    /// Obligation exists only in target (new)
    Added,
    /// Obligation exists only in source (deleted)
    Removed,
    /// Same obligation, different content
    Modified,
    /// Same content, different position
    Reordered,
    /// One obligation split into multiple
    Split,
    /// Multiple obligations merged into one
    Merged,
    /// Content identical (for completeness in reports)
    Unchanged,
}

#[derive(Debug, Clone)]
pub enum ChangeDetail {
    ObligorChanged { from: String, to: String },
    BeneficiaryAdded { name: String, chain_id: u32 },
    BeneficiaryRemoved { name: String, chain_id: u32 },
    ActionTextChanged { from: String, to: String, similarity: f64 },
    ObligationTypeChanged { from: ObligationType, to: ObligationType },
    ConditionAdded { condition: ConditionSignature },
    ConditionRemoved { condition: ConditionSignature },
    ConditionModified { from: ConditionSignature, to: ConditionSignature },
    ConfidenceChanged { from: f64, to: f64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticImpact {
    /// No material change to legal effect
    None,
    /// Clarification without substantive change
    Clarification,
    /// Party gains rights or reduced obligations
    FavorableToObligor,
    /// Party loses rights or increased obligations
    FavorableToBeneficiary,
    /// Material change requiring legal review
    MaterialChange,
    /// Potentially conflicting with other obligations
    PotentialConflict,
}

/// Optional aggregate/node/clause hierarchy backing the flat delta list
#[derive(Debug, Clone)]
pub struct HierarchicalDiff {
    pub aggregate_diff: AggregateDiff,
    pub node_diff: Vec<NodeDiff>,
    pub clause_diff: Vec<ClauseDiff>,
}

#[derive(Debug, Clone)]
pub struct AggregateDiff {
    pub matched: Vec<AggregateMatch>,
    pub added: Vec<AggregateRef>,
    pub removed: Vec<AggregateRef>,
}

#[derive(Debug, Clone)]
pub struct AggregateMatch {
    pub source_aggregate_id: u32,
    pub target_aggregate_id: u32,
    pub party_match: PartyMatchType,
    pub confidence: f64,
}

#[derive(Debug, Clone)]
pub struct AggregateRef {
    pub aggregate_id: u32,
    pub obligor_name: String,
}

#[derive(Debug, Clone)]
pub struct NodeDiff {
    pub aggregate_match: AggregateMatch,
    pub changes: Vec<ChangeDetail>,
}

#[derive(Debug, Clone)]
pub struct ClauseDiff {
    pub node_diff_index: usize,
    pub source_clause_id: Option<u32>,
    pub target_clause_id: Option<u32>,
    pub clause_changes: Vec<ChangeDetail>,
}

#[derive(Debug, Clone)]
pub enum PartyMatchType {
    ChainMatch { chain_id: u32 },
    NameMatch { normalized: String, has_holes: bool },
    MappedMatch { source_name: String, target_name: String },
}

/// Indicates whether a delta was a definite match or requires follow-up
#[derive(Debug, Clone)]
pub struct MatchResultMetadata {
    pub delta_id: u32,
    pub result: MatchResult,
}

#[derive(Debug, Clone)]
pub enum MatchResult {
    Definite { confidence: f64 },
    Indeterminate {
        confidence: f64,
        ambiguity: AmbiguityReason,
        resolution_needed: ResolutionHint,
    },
    NoMatch,
}

#[derive(Debug, Clone)]
pub enum AmbiguityReason {
    BothUnresolved { source_hole: u32, target_hole: u32 },
    ConflictingChains { name: String, source_chain: u32, target_chain: u32 },
    SemanticAmbiguity { similarity: f64 },
    MultipleInterpretations,
}

#[derive(Debug, Clone)]
pub enum ResolutionHint {
    VerifyPartyIdentity { display_text: String },
    SemanticAnalysisNeeded { source_clause_id: u32, target_clause_id: u32 },
    LegalInterpretationNeeded { clause_context: String },
}
```

To preserve backward compatibility, `ComparisonSet` continues to expose a flat `Vec<ObligationDelta>`,
but it can also attach a `HierarchicalDiff` describing aggregate/node/clause groupings plus a
`match_results` side-table. Consumers that need nuance (e.g., UI drill-downs, analytics) read the
hierarchy, while legacy code can keep reading `deltas`.

#### 5.3.4 Diff Pipeline Implementation

```rust
/// Multi-stage diff pipeline with escalating sophistication
pub struct DiffPipeline {
    content_matcher: ContentHashMatcher,
    structural_matcher: StructuralMatcher,
    semantic_matcher: Option<SemanticMatcher>,
    llm_judge: Option<Box<dyn LLMJudge>>,
    config: DiffConfig,
}

#[derive(Debug, Clone)]
pub struct DiffConfig {
    /// Minimum similarity for structural match
    pub structural_threshold: f64,      // default: 0.85
    /// Minimum similarity for semantic match
    pub semantic_threshold: f64,        // default: 0.80
    /// When to escalate to LLM
    pub llm_escalation_threshold: f64,  // default: 0.70
    /// Maximum obligations to send to LLM per comparison
    pub llm_budget: usize,              // default: 50
}

impl DiffPipeline {
    /// Execute full diff pipeline
    pub fn diff(
        &self,
        source: &DocumentSnapshot,
        target: &DocumentSnapshot,
        party_mapping: &PartyMapping,
    ) -> Vec<ObligationDelta> {
        let mut deltas = Vec::new();
        let mut matched_source: HashSet<u32> = HashSet::new();
        let mut matched_target: HashSet<u32> = HashSet::new();

        // Phase 1: Content hash exact matching (O(n))
        let exact_matches = self.content_matcher.find_matches(source, target);
        for (src_id, tgt_id) in exact_matches {
            matched_source.insert(src_id);
            matched_target.insert(tgt_id);
            // Still check for edge changes (beneficiaries, conditions)
            if let Some(delta) = self.compare_structure(source, target, src_id, tgt_id) {
                deltas.push(delta);
            }
        }

        // Phase 2: Structural matching via WL-hash (O(n * m) with pruning)
        let unmatched_source: Vec<_> = source.obligations.iter()
            .filter(|o| !matched_source.contains(&o.local_id))
            .collect();
        let unmatched_target: Vec<_> = target.obligations.iter()
            .filter(|o| !matched_target.contains(&o.local_id))
            .collect();

        let structural_matches = self.structural_matcher
            .find_matches(&unmatched_source, &unmatched_target, self.config.structural_threshold);
        // ... process structural matches

        // Phase 3: Semantic matching via embeddings (if available)
        if let Some(ref semantic) = self.semantic_matcher {
            // ... process semantic matches
        }

        // Phase 4: LLM judgment for ambiguous cases (if configured)
        if let Some(ref llm) = self.llm_judge {
            // ... LLM escalation for remaining ambiguous pairs
        }

        // Phase 5: Remaining unmatched = additions/deletions
        // ...

        deltas
    }
}
```

### 5.4 Temporal Modeling

#### 5.4.1 Event Schema

```rust
/// Events that build up contract state over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContractEvent {
    // Document lifecycle
    DocumentImported {
        document_id: String,
        effective_date: Option<NaiveDate>,
        imported_at: DateTime<Utc>,
    },
    DocumentAnalyzed {
        document_id: String,
        version: String,
        analysis_timestamp: DateTime<Utc>,
        obligation_count: usize,
    },

    // Obligation lifecycle
    ObligationExtracted {
        document_id: String,
        obligation_id: u32,
        obligor: String,
        obligation_type: ObligationType,
        action_text: String,
        confidence: f64,
        effective_from: Option<NaiveDate>,
    },
    ObligationVerified {
        document_id: String,
        obligation_id: u32,
        verifier_id: String,
        verified_at: DateTime<Utc>,
    },

    // Amendment events
    AmendmentApplied {
        base_document_id: String,
        amendment_document_id: String,
        effective_date: NaiveDate,
        superseded_obligations: Vec<u32>,
        new_obligations: Vec<u32>,
    },

    // Comparison events
    ComparisonCompleted {
        comparison_id: String,
        source_document: String,
        target_document: String,
        delta_count: usize,
        completed_at: DateTime<Utc>,
    },
}
```

#### 5.4.2 Bi-Temporal Store

```rust
/// Bi-temporal obligation tracking
///
/// Two time dimensions:
/// - Valid time: When was this obligation in effect in the real world?
/// - Transaction time: When did we learn about/record this fact?
#[derive(Debug, Clone)]
pub struct BiTemporalRange {
    pub valid_from: NaiveDate,
    pub valid_until: Option<NaiveDate>,  // None = still valid
    pub recorded_at: DateTime<Utc>,
    pub superseded_at: Option<DateTime<Utc>>,  // None = current version
}

pub struct TemporalObligationStore {
    events: Vec<(u64, ContractEvent)>,
    // Materialized views
    obligations_by_date: BTreeMap<NaiveDate, Vec<TemporalObligation>>,
}

impl TemporalObligationStore {
    /// "What were Seller's obligations as of March 2024?"
    pub fn obligations_as_of(
        &self,
        party_name: &str,
        as_of_date: NaiveDate,
    ) -> Vec<&TemporalObligation>;

    /// "What did we know about obligations on March 2024, based on June 2024 data?"
    pub fn obligations_bi_temporal(
        &self,
        valid_as_of: NaiveDate,
        known_as_of: DateTime<Utc>,
    ) -> Vec<&TemporalObligation>;

    /// Full history of an obligation across versions
    pub fn obligation_history(&self, obligation_id: u32) -> Vec<&TemporalObligation>;
}
```

### 5.5 Verification Integration

#### 5.5.1 Priority Queue Extension

Extends existing verification system ([`verification.rs`](../layered-contracts/src/verification.rs)):

```rust
/// Extended verification item with priority scoring
pub struct PrioritizedVerificationItem {
    pub item: VerificationQueueItem,  // From existing verification.rs
    pub priority_score: f64,
    pub priority_factors: Vec<PriorityFactor>,
    pub similar_item_count: usize,    // Items resolved by this decision
}

#[derive(Debug, Clone)]
pub struct PriorityFactor {
    pub factor_name: String,
    pub contribution: f64,
    pub explanation: String,
}

/// Priority calculation
///
/// Formula: priority = (1 - confidence) × impact_weight + uncertainty_penalty
///
/// Impact weights:
/// - Monetary terms: +0.30
/// - Prohibition clause: +0.20
/// - Multiple parties: +0.15
/// - Unknown entity in condition: +0.15
/// - Plain noun obligor: +0.10
pub fn calculate_priority(item: &VerificationQueueItem, context: &VerificationContext) -> f64;
```

`MatchResultMetadata` feeds directly into the queue: deltas marked as `Indeterminate` inherit an
uncertainty penalty and include the `resolution_needed` hint so the reviewer understands how to
resolve the ambiguity (confirm party identity, request LLM analysis, etc.).

#### 5.5.2 Similarity Clustering

```rust
/// Cluster of similar verification items
///
/// One human decision on the primary item resolves all similar items.
pub struct SimilarityCluster {
    pub cluster_id: u32,
    pub primary_item: VerificationTarget,
    pub similar_items: Vec<VerificationTarget>,
    pub similarity_basis: SimilarityBasis,
    pub aggregate_confidence_lift: f64,
}

#[derive(Debug, Clone)]
pub enum SimilarityBasis {
    /// Same party name (case-insensitive, normalized)
    PartyName { normalized_name: String },
    /// Same condition pattern (e.g., "subject to Section X")
    ConditionPattern { keyword: ContractKeyword, pattern: String },
    /// Same pronoun chain
    PronounChain { chain_id: u32, canonical_name: String },
    /// Same unknown entity text
    UnknownEntity { entity_text: String },
}

/// Propagate verification decision to similar items
pub fn propagate_verification(
    nodes: &mut [Scored<ObligationNode>],
    cluster: &SimilarityCluster,
    action: VerificationAction,
) -> Vec<PropagationResult>;
```

#### 5.5.3 Disagreement Detection

```rust
/// Tracks when rule-based and LLM extractions disagree
pub struct DisagreementRecord {
    pub target: VerificationTarget,
    pub rule_based_result: Option<ExtractionResult>,
    pub llm_result: Option<ExtractionResult>,
    pub confidence_delta: f64,
    pub semantic_disagreement: bool,
    pub requires_human_review: bool,
}

/// Triggers for mandatory human review:
/// - confidence_delta > 0.3
/// - Rule found X, LLM found Y (different values)
/// - One found something, other found nothing
pub fn detect_disagreements(
    rule_based: &[Scored<ObligationNode>],
    llm_extracted: &[LLMExtractionResult],
) -> Vec<DisagreementRecord>;
```

### 5.6 Formal Semantics

#### 5.6.1 Deontic Modality Types

```rust
/// Deontic modal operators as type-level markers
///
/// In standard deontic logic:
/// - O(A): Obligatory (MUST do A)
/// - P(A): Permitted (MAY do A)
/// - F(A): Forbidden (MUST NOT do A)
///
/// Relationships:
/// - P(A) = ¬O(¬A): Permitted iff not obligated to not do
/// - F(A) = O(¬A): Forbidden iff obligated to not do
/// - O(A) → P(A): If obligatory, then permitted
pub trait DeonticModality: sealed::Sealed {
    type Dual: DeonticModality;
    fn polarity() -> i8;  // +1 for O/P, -1 for F
}

pub struct Obligatory;  // Duty (shall)
pub struct Permitted;   // Permission (may)
pub struct Forbidden;   // Prohibition (shall not)

impl DeonticModality for Obligatory {
    type Dual = Forbidden;
    fn polarity() -> i8 { 1 }
}

impl DeonticModality for Forbidden {
    type Dual = Obligatory;
    fn polarity() -> i8 { -1 }
}
```

#### 5.6.2 Obligation Compatibility

```rust
/// Contract B is compatible with Contract A (B <: A) iff:
/// 1. Every duty in A is satisfied by B (B may have more duties)
/// 2. Every permission in B is permitted in A (B may have fewer permissions)
/// 3. Every prohibition in A is respected in B (B may have more prohibitions)
pub trait ContractCompatible<Other> {
    fn check_compatibility(&self, other: &Other) -> CompatibilityReport;
}

#[derive(Debug)]
pub struct CompatibilityReport {
    pub missing_duties: Vec<ObligationRef>,
    pub excess_permissions: Vec<ObligationRef>,
    pub violated_prohibitions: Vec<ObligationRef>,
    pub is_compatible: bool,
    pub compatibility_score: f64,
}
```

#### 5.6.3 Conflict Detection

```rust
/// Conflict occurs when same party has both O(A) and F(A)
pub struct ConflictAnalysis {
    pub direct_conflicts: Vec<DirectConflict>,
    pub potential_conflicts: Vec<PotentialConflict>,
    pub conflict_free_proof: Option<ConflictFreeProof>,
}

#[derive(Debug)]
pub struct DirectConflict {
    pub party: String,
    pub duty: ObligationRef,
    pub prohibition: ObligationRef,
    pub action_overlap: f64,
}

/// Proof that no conflicts exist
pub enum ConflictFreeProof {
    /// No (party, action) overlap between duties and prohibitions
    DisjointDomains,
    /// Overlaps guarded by mutually exclusive conditions
    MutuallyExclusiveConditions {
        condition_pairs: Vec<(ConditionSignature, ConditionSignature)>,
    },
}

pub fn detect_conflicts(obligations: &[NormalizedObligation]) -> ConflictAnalysis;
```

---

## 6. API Design

### 6.1 Comparison API

```rust
/// Create and execute a comparison between two documents
pub struct ComparisonBuilder {
    source_doc_id: String,
    target_doc_id: String,
    party_mapping_hints: Vec<PartyMappingHint>,
    config: DiffConfig,
}

impl ComparisonBuilder {
    pub fn new(source: &str, target: &str) -> Self;

    /// Provide hints for party mapping
    pub fn with_party_hint(self, source_name: &str, target_name: &str) -> Self;

    /// Configure diff thresholds
    pub fn with_config(self, config: DiffConfig) -> Self;

    /// Execute comparison
    pub fn execute(
        self,
        source_snapshot: &DocumentSnapshot,
        target_snapshot: &DocumentSnapshot,
    ) -> Result<ComparisonSet, ComparisonError>;
}

// Usage:
let comparison = ComparisonBuilder::new("contract_v1", "contract_v2")
    .with_party_hint("Seller", "Vendor")  // Manual mapping hint
    .with_config(DiffConfig::default())
    .execute(&snapshot_v1, &snapshot_v2)?;

for delta in &comparison.deltas {
    match delta.delta_type {
        DeltaType::Modified => println!("Changed: {:?}", delta.changes),
        DeltaType::Added => println!("New obligation"),
        DeltaType::Removed => println!("Removed obligation"),
        _ => {}
    }
}
```

### 6.2 Query API

```rust
/// Fluent query builder for obligation graphs
impl ObligationPropertyGraph {
    pub fn query(&self) -> ObligationQuery<'_> {
        ObligationQuery::new(self)
    }
}

impl<'g> ObligationQuery<'g> {
    /// Filter by party role
    pub fn for_party(self, name: &str) -> Self;
    pub fn where_obligor(self, chain_id: u32) -> Self;
    pub fn where_beneficiary(self, chain_id: u32) -> Self;

    /// Filter by obligation type
    pub fn duties(self) -> Self;
    pub fn permissions(self) -> Self;
    pub fn prohibitions(self) -> Self;

    /// Filter by conditions
    pub fn with_condition_type(self, keyword: ContractKeyword) -> Self;
    pub fn referencing_section(self, section: &str) -> Self;
    pub fn conditional(self) -> Self;
    pub fn unconditional(self) -> Self;

    /// Filter by confidence
    pub fn min_confidence(self, threshold: f64) -> Self;

    /// Execute and return results
    pub fn execute(self) -> Vec<&'g ObligationVertex>;
    pub fn count(self) -> usize;
}

// Usage:
let seller_duties = graph.query()
    .for_party("Seller")
    .duties()
    .referencing_section("Section 5")
    .min_confidence(0.7)
    .execute();
```

### 6.3 Temporal Query API

```rust
impl TemporalObligationStore {
    /// Point-in-time query
    pub fn at(&self, date: NaiveDate) -> TemporalView<'_>;

    /// Range query
    pub fn between(&self, from: NaiveDate, to: NaiveDate) -> TemporalRangeView<'_>;

    /// Version comparison
    pub fn compare_versions(&self, v1: &str, v2: &str) -> Option<ComparisonSet>;
}

impl TemporalView<'_> {
    pub fn obligations(&self) -> Vec<&TemporalObligation>;
    pub fn for_party(&self, name: &str) -> Vec<&TemporalObligation>;
    pub fn active_only(&self) -> Self;
}

// Usage:
let march_2024 = NaiveDate::from_ymd_opt(2024, 3, 15).unwrap();
let obligations = store.at(march_2024)
    .for_party("Seller")
    .active_only()
    .obligations();
```

### 6.4 Serialization Payloads

```rust
/// JSON-serializable comparison result
#[derive(Debug, Serialize, Deserialize)]
pub struct ComparisonPayload {
    pub comparison_id: String,
    pub source_document: DocumentSummary,
    pub target_document: DocumentSummary,
    pub party_mapping: Vec<PartyMappingPayload>,
    pub summary: ComparisonSummary,
    pub deltas: Vec<DeltaPayload>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComparisonSummary {
    pub total_source_obligations: usize,
    pub total_target_obligations: usize,
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    pub unchanged: usize,
    pub structural_similarity: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeltaPayload {
    pub delta_type: String,
    pub source_obligation: Option<ObligationSummary>,
    pub target_obligation: Option<ObligationSummary>,
    pub changes: Vec<ChangePayload>,
    pub semantic_impact: String,
    pub confidence: f64,
}

impl ComparisonSet {
    pub fn to_payload(&self) -> ComparisonPayload;
    pub fn to_json_string(&self) -> Result<String, serde_json::Error>;
}
```

---

## 7. Algorithms

### 7.1 Content Hash Matching

**Purpose**: O(1) identification of identical obligations

**Algorithm**:
1. Compute hash of normalized action text
2. Build hash → obligation_id index for both documents
3. Match by hash equality

**Hash function**: `xxHash64` on normalized text (lowercase, whitespace-collapsed)

**Complexity**: O(n + m) where n, m are obligation counts

```rust
impl ContentHashMatcher {
    pub fn find_matches(
        &self,
        source: &DocumentSnapshot,
        target: &DocumentSnapshot,
    ) -> Vec<(u32, u32)> {
        let source_index: HashMap<u64, u32> = source.obligations.iter()
            .map(|o| (o.action_text_hash, o.local_id))
            .collect();

        target.obligations.iter()
            .filter_map(|o| {
                source_index.get(&o.action_text_hash)
                    .map(|src_id| (*src_id, o.local_id))
            })
            .collect()
    }
}
```

### 7.2 Weisfeiler-Leman Structural Hashing

**Purpose**: Fingerprint graph structure for quick similarity estimation

**Algorithm**:
1. Initialize vertex labels from vertex type + key properties
2. Iterate k times:
   - For each vertex, collect neighbor labels
   - Hash (current_label, sorted_neighbor_labels)
   - Update label
3. Return multiset of final labels

**Complexity**: O(k × (V + E)) where k = iterations (typically 3-5)

```rust
impl ObligationPropertyGraph {
    pub fn weisfeiler_leman_hash(&self, iterations: usize) -> HashSet<u64> {
        let mut labels: HashMap<VertexId, u64> = self.initial_labels();

        for _ in 0..iterations {
            let mut new_labels = HashMap::new();
            for (vertex, label) in &labels {
                let neighbor_labels = self.neighbor_labels(vertex, &labels);
                new_labels.insert(
                    vertex.clone(),
                    hash_combine(*label, &neighbor_labels),
                );
            }
            labels = new_labels;
        }

        labels.values().cloned().collect()
    }

    pub fn structural_similarity(&self, other: &Self) -> f64 {
        let self_hash = self.weisfeiler_leman_hash(3);
        let other_hash = other.weisfeiler_leman_hash(3);
        jaccard_similarity(&self_hash, &other_hash)
    }
}
```

### 7.3 Embedding-Based Semantic Matching

**Purpose**: Find semantically similar obligations despite different wording

**Embedding strategy**:
```
ObligationVector = concat(
    embed(obligor_name),           // 64 dims
    one_hot(obligation_type),      // 3 dims
    embed(action_text),            // 384 dims (sentence-transformers)
    mean_pool(beneficiary_embeds), // 64 dims
    mean_pool(condition_embeds),   // 64 dims
    polarity_signal,               // 1 dim (+1 or -1)
)
```

**Similarity computation**:
```rust
pub fn obligation_similarity(a: &ObligationEmbedding, b: &ObligationEmbedding) -> f64 {
    // Polarity check: opposite polarities = dissimilar
    if a.polarity != b.polarity {
        return 0.0;
    }

    // Weighted component similarity
    let weights = SimilarityWeights {
        obligor: 0.15,
        action: 0.40,
        beneficiary: 0.20,
        condition: 0.25,
    };

    weights.obligor * cosine_sim(&a.obligor_vec, &b.obligor_vec)
        + weights.action * cosine_sim(&a.action_vec, &b.action_vec)
        + weights.beneficiary * cosine_sim(&a.beneficiary_vec, &b.beneficiary_vec)
        + weights.condition * cosine_sim(&a.condition_vec, &b.condition_vec)
}
```

### 7.4 LLM-as-Judge for Material Equivalence

**Purpose**: Determine if two clauses have the same legal effect

**When to escalate**:
- Embedding similarity between 0.70 and 0.85
- High-stakes clauses (indemnification, limitation of liability)
- Disagreement between rule-based and embedding matchers

**Prompt template**:
```
You are a legal analyst comparing two contract clauses for material equivalence.

CLAUSE A:
Obligor: {obligor_a}
Type: {type_a} (Duty/Permission/Prohibition)
Action: "{action_a}"
Beneficiary: {beneficiary_a}
Conditions: {conditions_a}

CLAUSE B:
Obligor: {obligor_b}
Type: {type_b}
Action: "{action_b}"
Beneficiary: {beneficiary_b}
Conditions: {conditions_b}

Analyze whether these clauses are materially equivalent:
1. Do they impose the same legal obligation on the same party?
2. Are the actions substantively the same (ignore stylistic differences)?
3. Are the conditions functionally equivalent?
4. Would changing from A to B alter any party's legal rights?

Respond in JSON:
{
  "verdict": "Equivalent" | "SubstantiallyEquivalent" | "MateriallyDifferent" | "Opposite",
  "confidence": 0.0-1.0,
  "reasoning": "step-by-step analysis",
  "key_differences": [...]
}
```

---

## 8. Integration Points

### 8.1 With Existing Extraction Pipeline

```rust
// After running full resolver stack
let ll_line = create_line_from_string(contract_text)
    .run(&POSTagResolver::default())
    // ... layers 1-8
    .run(&AccountabilityGraphResolver::default());

// Extract for graph construction
let nodes: Vec<Scored<ObligationNode>> = ll_line
    .query::<Scored<ObligationNode>>()
    .into_iter()
    .flat_map(|(_, _, attrs)| attrs)
    .cloned()
    .collect();

let chains: Vec<Scored<PronounChain>> = ll_line
    .query::<Scored<PronounChain>>()
    .into_iter()
    .flat_map(|(_, _, attrs)| attrs)
    .cloned()
    .collect();

// Build property graph
let graph = ObligationPropertyGraph::from_extraction(&nodes, &chains, "doc_001");

// Create snapshot for comparison
let snapshot = DocumentSnapshot::from_graph(&graph);
```

### 8.2 With Existing Verification System

```rust
// Extend VerificationAction enum (verification.rs)
pub enum VerificationAction {
    // Existing variants...
    VerifyNode { ... },
    ResolveBeneficiary { ... },

    // New variants for comparison context
    ConfirmPartyMapping {
        comparison_id: String,
        source_chain_id: u32,
        target_chain_id: u32,
        verifier_id: String,
    },
    ConfirmDelta {
        comparison_id: String,
        delta_id: u32,
        confirmed_type: DeltaType,
        verifier_id: String,
    },
    RejectDelta {
        comparison_id: String,
        delta_id: u32,
        reason: String,
        verifier_id: String,
    },
}
```

### 8.3 With Existing Analytics Layer

```rust
// Extend ObligationGraph (accountability_analytics.rs)
impl ObligationGraph<'_> {
    // Existing methods...

    /// Create snapshot for comparison
    pub fn to_snapshot(&self, document_id: &str) -> DocumentSnapshot;

    /// Compare against another graph
    pub fn diff(&self, other: &ObligationGraph<'_>) -> ComparisonSet;

    /// Find similar obligations in another graph
    pub fn find_similar_in(
        &self,
        other: &ObligationGraph<'_>,
        threshold: f64,
    ) -> Vec<(ObligationRef, ObligationRef, f64)>;
}
```

---

## 9. Trade-offs and Alternatives

### 9.1 Aggregate Root Choice

| Option | Pros | Cons | Recommendation |
|--------|------|------|----------------|
| **ObligationNode** (current) | Natural for extraction | Poor for cross-doc queries | Keep for extraction |
| **Party-centric** | Natural for "what does X owe?" | Two-phase extraction needed | Consider for analytics |
| **Contract aggregate** | Clear ownership | Large for long contracts | Not recommended |
| **ComparisonSet** (proposed) | Clean separation | Additional complexity | **Adopt for comparison** |

### 9.2 Matching Strategy

| Strategy | Accuracy | Performance | Cost | Use When |
|----------|----------|-------------|------|----------|
| Content hash only | 100% (exact) | O(n) | Free | Identical text expected |
| + Structural (WL) | ~85% | O(n log n) | Free | Similar structure |
| + Embeddings | ~92% | O(n²) | ~$0.001/obl | Paraphrasing expected |
| + LLM judge | ~97% | O(n) calls | ~$0.05/obl | High stakes, ambiguous |

**Recommendation**: Hybrid pipeline with escalation (hash → structure → embeddings → LLM)

### 9.3 Storage Backend

| Backend | Pros | Cons | Best For |
|---------|------|------|----------|
| In-memory HashMap | Fast, simple | No persistence | Single session |
| SQLite | Persistent, queryable | Join overhead | Document library |
| Neo4j | Full graph DB | Deployment complexity | Production analytics |
| PostgreSQL + JSONB | Flexible, reliable | Not graph-native | General purpose |

**Recommendation**: Start with in-memory, add SQLite persistence as needed

### 9.4 Temporal Model Complexity

| Level | Features | Complexity | Recommendation |
|-------|----------|------------|----------------|
| None | Point-in-time only | Low | MVP |
| Event stream | Audit trail | Medium | Phase 2 |
| Bi-temporal | "What did we know when?" | High | If regulated |

---

## 10. Implementation Plan

### Phase 0: Hole Infrastructure (Pre-req)

**Goal**: Introduce holes and persistence semantics before comparison logic.

**Deliverables**:
1. `HoleId`, `EntityBinding`, and `HoleRegistry` (document + comparison scoped)
2. `(RegistryKind, owner_id)` namespacing rules plus serialization format (even if stubbed)
3. `ClauseParty` migration to `EntityBinding`
4. Ability to promote a registry from session to corpus storage without leaking across documents
5. Telemetry/metrics for unresolved vs. resolved holes (baseline unknowns)

Unknowns captured here: the storage backend (file vs. DB) and eviction policy can remain TBD, but the API surface must exist before Phase 1 begins.

### Phase 1: Foundation (Weeks 1-2)

**Goal**: Basic diffing with content hash matching

**Deliverables**:
1. `DocumentSnapshot` value object (now includes optional `HoleRegistrySnapshot`)
2. `ContentHashMatcher` implementation
3. `ObligationDelta`, `DeltaType`, and `MatchResult`/metadata plumbing
4. Basic `ComparisonSet` aggregate with `hierarchical` optional payload
5. Unit tests with insta snapshots covering both flattened and hierarchical output

**Files to create**:
- `layered-contracts/src/comparison/mod.rs`
- `layered-contracts/src/comparison/snapshot.rs`
- `layered-contracts/src/comparison/delta.rs`
- `layered-contracts/src/comparison/matcher.rs`

### Phase 2: Structural Matching (Weeks 3-4)

**Goal**: WL-hash structural similarity

**Deliverables**:
1. `ObligationPropertyGraph` data structure
2. Weisfeiler-Leman hash implementation
3. `StructuralMatcher` with configurable threshold
4. Graph query builder API
5. Integration tests

**Files to create**:
- `layered-contracts/src/graph/mod.rs`
- `layered-contracts/src/graph/vertex.rs`
- `layered-contracts/src/graph/edge.rs`
- `layered-contracts/src/graph/query.rs`
- `layered-contracts/src/graph/algorithms.rs`

### Phase 3: Party Mapping (Weeks 5-6)

**Goal**: Cross-document party alignment

**Deliverables**:
1. `PartyMapping` and `PartyMappingEntry` structures
2. Name normalization utilities (extend `utils.rs`)
3. Mapping confidence scoring
4. Manual mapping override support
5. Verification integration for mapping confirmation

### Phase 4: Semantic Matching (Weeks 7-8)

**Goal**: Embedding-based similarity (optional LLM)

**Deliverables**:
1. `ObligationEmbedding` structure
2. Embedding service trait (pluggable backends)
3. `SemanticMatcher` implementation
4. LLM judge trait and prompt templates
5. Hybrid pipeline orchestration

### Phase 5: Temporal Features (Weeks 9-10)

**Goal**: Event sourcing and temporal queries

**Deliverables**:
1. `ContractEvent` enum
2. `ContractEventStore` trait and in-memory impl
3. `TemporalObligationStore` with point-in-time queries
4. Amendment tracking
5. Version changelog generation

### Phase 6: Verification Integration (Weeks 11-12)

**Goal**: Priority queue and similarity clustering

**Deliverables**:
1. `PrioritizedVerificationItem` with scoring
2. `SimilarityCluster` and propagation logic
3. `DisagreementRecord` for rule/LLM conflicts
4. Extended `VerificationAction` variants
5. Feedback loop infrastructure

---

## 11. Open Questions

### 11.1 Scope Clarification Needed

1. **Version vs cross-contract priority**: Should we optimize first for comparing versions of the same contract, or comparing different contracts?

2. **LLM integration depth**: Should LLM be:
   - Not included (pure rule-based)
   - Optional enhancement
   - Core feature with fallback

3. **Temporal requirements**: How critical is:
   - "What were obligations as of date X?"
   - Full amendment tracking
   - Bi-temporal audit compliance

### 11.2 Technical Decisions Pending

1. **Embedding model**: Which sentence transformer for action text?
   - `all-MiniLM-L6-v2` (fast, 384 dims)
   - `legal-bert` (domain-specific)
   - Custom fine-tuned model

2. **Graph storage**: For production deployment:
   - SQLite with adjacency lists
   - PostgreSQL with recursive CTEs
   - Neo4j for complex traversals

3. **Polarity handling**: For "shall not X" vs "shall Y where Y excludes X":
   - Treat as opposite polarities?
   - Semantic analysis required?

### 11.3 Integration Questions

1. **WASM compatibility**: Should comparison logic work in browser (via `layered-nlp-demo-wasm`)?

2. **Streaming**: Can we diff documents incrementally as they're parsed?

3. **Caching**: How to cache embeddings and comparison results?

---

## 12. References

### 12.1 Codebase References

| File | Relevant Sections |
|------|-------------------|
| [`accountability_graph.rs`](../layered-contracts/src/accountability_graph.rs) | `ObligationNode` (L42-59), `BeneficiaryLink` (L18-31), `ConditionLink` (L33-40) |
| [`accountability_analytics.rs`](../layered-contracts/src/accountability_analytics.rs) | `ObligationGraph` (L24-120), `PartyAnalytics` (L256-263) |
| [`verification.rs`](../layered-contracts/src/verification.rs) | `VerificationAction` (L42-120), `apply_verification_action` (L122-187) |
| [`scored.rs`](../layered-contracts/src/scored.rs) | `Scored<T>` (L8-25), `ScoreSource` (L27-45) |
| [`pronoun_chain.rs`](../layered-contracts/src/pronoun_chain.rs) | `PronounChain` (L15-45), `ChainMention` |
| [`contract_clause.rs`](../layered-contracts/src/contract_clause.rs) | `ContractClause`, `ClauseDuty`, `ClauseCondition` |
| [`utils.rs`](../layered-contracts/src/utils.rs) | `normalize_party_name` |

### 12.2 External References

| Topic | Reference |
|-------|-----------|
| Weisfeiler-Leman | Shervashidze et al., "Weisfeiler-Lehman Graph Kernels" (JMLR 2011) |
| Graph Edit Distance | Riesen & Bunke, "Approximate graph edit distance computation" (IJPRAI 2009) |
| Deontic Logic | McNamara, "Deontic Logic" (Stanford Encyclopedia of Philosophy) |
| Event Sourcing | Fowler, "Event Sourcing" (martinfowler.com) |
| Bi-temporal Data | Snodgrass, "Developing Time-Oriented Database Applications in SQL" |
| Sentence Embeddings | Reimers & Gurevych, "Sentence-BERT" (EMNLP 2019) |
| Domain-Driven Design | Evans, "Domain-Driven Design" (2003) |

---

## Appendix A: Example Diff Output

```json
{
  "comparison_id": "cmp_2024_001",
  "source_document": {
    "document_id": "contract_v1",
    "version": "1.0",
    "obligation_count": 24
  },
  "target_document": {
    "document_id": "contract_v2",
    "version": "2.0",
    "obligation_count": 27
  },
  "party_mapping": [
    {
      "source_name": "Seller",
      "target_name": "Vendor",
      "confidence": 0.95,
      "method": "NormalizedNameMatch"
    }
  ],
  "summary": {
    "added": 5,
    "removed": 2,
    "modified": 8,
    "unchanged": 14,
    "structural_similarity": 0.82
  },
  "deltas": [
    {
      "delta_type": "Modified",
      "source_obligation": {
        "obligor": "Seller",
        "action": "deliver goods within 30 days",
        "type": "Duty"
      },
      "target_obligation": {
        "obligor": "Vendor",
        "action": "deliver goods within 45 days",
        "type": "Duty"
      },
      "changes": [
        {
          "type": "ActionTextChanged",
          "from": "deliver goods within 30 days",
          "to": "deliver goods within 45 days",
          "similarity": 0.92
        }
      ],
      "semantic_impact": "FavorableToObligor",
      "confidence": 0.88
    }
  ]
}
```

---

## Appendix B: Verification UI Mockup

```
┌─────────────────────────────────────────────────────────────────────────┐
│ COMPARISON REVIEW: contract_v1 → contract_v2                            │
├─────────────────────────────────────────────────────────────────────────┤
│ PARTY MAPPING (2 pending verification)                                  │
│ ┌───────────────────────────────────────────────────────────────────┐  │
│ │ [✓] Seller → Vendor         (95% confidence, name match)          │  │
│ │ [?] "Regional Authority" → (unmapped, needs review)               │  │
│ └───────────────────────────────────────────────────────────────────┘  │
│                                                                         │
│ MODIFIED OBLIGATIONS (8 total, 3 high impact)                          │
│ ┌───────────────────────────────────────────────────────────────────┐  │
│ │ #1 [HIGH IMPACT] Delivery timeline changed                        │  │
│ │    v1: "Seller shall deliver within 30 days"                      │  │
│ │    v2: "Vendor shall deliver within 45 days"                      │  │
│ │    Impact: Favorable to obligor (+15 days)                        │  │
│ │    Confidence: 88%                                                │  │
│ │    [Confirm] [Reject] [Edit]                                      │  │
│ ├───────────────────────────────────────────────────────────────────┤  │
│ │ #2 [HIGH IMPACT] New condition added                              │  │
│ │    v1: "Seller shall deliver"                                     │  │
│ │    v2: "Vendor shall deliver, subject to Force Majeure"           │  │
│ │    Impact: Material change (new exception)                        │  │
│ │    Confidence: 91%                                                │  │
│ │    [Confirm] [Reject] [Edit]                                      │  │
│ └───────────────────────────────────────────────────────────────────┘  │
│                                                                         │
│ NAVIGATION: [← Previous] [Next →]  Page 1 of 3                         │
│ ACTIONS: [Export JSON] [Generate Report] [Complete Review]             │
└─────────────────────────────────────────────────────────────────────────┘
```

---

*End of RFC-001*

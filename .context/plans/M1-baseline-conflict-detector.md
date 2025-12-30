# M1: Baseline ConflictDetector

**FR:** FR-006 – Semantic Analysis and Conflict Detection  
**Status:** ✅ Complete  
**Effort:** M (2-4 hours)  
**Last Updated:** 2024-12-29

> **Summary:** M1 baseline ConflictDetector is implemented with:
> - Modal conflict detection (shall vs may for same party/action)
> - Temporal conflict detection (incompatible timing requirements)
> - Party conflict detection (same action assigned to different parties)
> - Document-level integration via `detect_in_document()` method
> - 54 unit/integration tests passing

> **Note:** This plan was reconstructed from session transcript `2024-12-29-conflict-detector.md` after the original plan and implementation were lost. The implementation reached Gate 3 complete, Gate 4 in-progress (tests failing) before being lost.

---

## Overview

The ConflictDetector identifies contradictory provisions within a single contract document. It operates as a `DocumentResolver`, running after obligation extraction to detect:

- **Modal conflicts**: Same party, same action, different obligation type (shall vs may)
- **Temporal conflicts**: Same obligation with incompatible timing requirements
- **Contradictory parties**: Same action assigned to different parties
- **Scope overlap**: Obligations that partially overlap with conflicting requirements

---

## Gates

### Gate 0: Core Types and Infrastructure
**Status:** ✅ Complete

**Deliverables:**
- [x] `Conflict` struct with `span_a: DocSpan`, `span_b: DocSpan`, `conflict_type: ConflictType`, `explanation: String`
- [x] `ConflictType` enum: `ModalConflict`, `TemporalConflict`, `ContradictoryParties`, `ScopeOverlap`
- [x] `NormalizedObligation` struct with:
  - `obligor: String` (normalized party name)
  - `obligation_type: ObligationType` (reuse from FR-001)
  - `action: String` (normalized/lemmatized)
  - `timing: Option<NormalizedTiming>`
  - `original_span: DocSpan`
  - `line_index: usize`
  - `topic: ObligationTopic` (added for convenience)
- [x] `ObligationTopic` enum: `Payment`, `Delivery`, `Confidentiality`, `Termination`, `Indemnification`, `Notice`, `Other`
- [ ] `SnapshotKind` impl for `Conflict` (deferred - snapshot module not in lib.rs yet)
- [x] All types derive `Debug, Clone, PartialEq`
- [x] `ObligationTopic` derives `Hash, Eq` (no String variant)

**Note:** `NormalizedTiming` and `TimeUnit` already exist in `temporal.rs`. Serde derives deferred since DocSpan doesn't have them.

**Verification:**
- [x] Types compile without errors
- [x] `cargo doc` generates documentation
- [x] Basic unit tests for `combined_span()` method (7 tests pass)

---

### Gate 1: Obligation Normalization
**Status:** ✅ Complete

**Deliverables:**
- [x] `ObligationNormalizer` struct with:
  - `extract_obligor_name(&ObligorReference) -> String`
  - `normalize_party(&str) -> String` (strips articles, lowercases)
  - `normalize_action(&str) -> String` (verb lemmatization)
  - `normalize_timing(&str) -> Option<NormalizedTiming>` (parses "30 days", "within 60 days", "thirty (30) days")
  - `normalize(&Scored<ObligationPhrase>, line_index, start_token, end_token) -> NormalizedObligation`
- [x] Verb lemma table: 22 contract verbs with ~4 inflected forms each
- [x] Written number parsing: "five", "twenty-five", compound numbers
- [x] Business days detection: "business days", "working days"

**Key Decisions (from prior session):**
- Prefer numeric digits over written numbers when both present (e.g., "thirty (30)" → 30)
- Timing extracted from `action` field only (limitation: timing in conditions not captured)

**Verification:**
- [x] 16 test scenarios covering all normalization methods (24 total tests now)
- [x] Written number edge cases (eighteen vs eight ordering)

---

### Gate 2: Topic Classification
**Status:** ✅ Complete

**Deliverables:**
- [x] `TopicClassifier` struct with:
  - `classify(&NormalizedObligation) -> ObligationTopic`
  - Keyword sets for each topic (payment, delivery, etc.)
  - Word boundary checking (avoid "repayment" matching "payment")
- [x] `group_by_topic(Vec<NormalizedObligation>) -> HashMap<ObligationTopic, Vec<NormalizedObligation>>`

**Verification:**
- [x] Test each topic with positive and negative examples (12 tests)
- [x] Test word boundary edge cases (2 tests: prevents false positives, allows valid matches)

---

### Gate 3: Conflict Detection Logic
**Status:** ✅ Complete

**Deliverables:**
- [x] `ConflictDetector` struct with:
  - `similarity_threshold: f64` (default 0.7)
  - `confidence_threshold: f64` (default 0.5)
  - `temporal_tolerance: f64` (default 0.5)
  - `classifier: TopicClassifier`
  - `normalizer: ObligationNormalizer`
- [x] Detection methods:
  - `detect_modal_conflict(&a, &b) -> Option<Scored<Conflict>>`
  - `detect_temporal_conflict(&a, &b) -> Option<Scored<Conflict>>`
  - `detect_party_conflict(&a, &b) -> Option<Scored<Conflict>>`
  - `detect_conflicts(&[NormalizedObligation]) -> Vec<Scored<Conflict>>`
- [x] `action_similarity(a: &str, b: &str) -> f64` using Jaccard word overlap
- [x] Timing tolerance for temporal conflicts (e.g., 10 vs 30 days = conflict; 29 vs 30 days = OK)

**Key Design (from session review):**
- Use `Scored<Conflict>` wrapper (consistent with `Scored<ObligationPhrase>` pattern)
- Deterministic ordering: compare by (line_index, token_start) to ensure reproducible results
- Party conflict: Check if same action assigned to different obligors

**Verification:**
- [x] Modal conflict: "shall deliver" vs "may deliver" for same party (14 tests)
- [x] Temporal conflict: "within 10 days" vs "within 30 days" for same obligation
- [x] Multiple conflicts in single document (test_detect_conflicts_multiple)

---

### Gate 4: Document Integration
**Status:** ✅ Complete

**Deliverables:**
- [x] `detect_in_document(&self, doc: &ContractDocument) -> Vec<Scored<Conflict>>` method
  - Queries all `Scored<ObligationPhrase>` from document lines
  - Converts line-level spans to `DocSpan` with line index
  - Normalizes obligations and runs conflict detection
  - Respects `confidence_threshold` when filtering obligations

**Note:** DocumentResolver trait and SemanticSpan don't exist in the codebase yet.
The `detect_in_document` method provides equivalent functionality.

**Critical Learnings:**
- Pipeline::standard() doesn't include POSTagResolver, which is required for obligor detection
- Test text must use defined term format: `ABC Corp (the "Company")` for obligations to be extracted
- Actions must be similar enough (Jaccard similarity > 0.7) to trigger conflict detection

**Verification:**
- [x] Integration test with full resolver chain (4 tests)
- [x] Modal conflict detected from sample contract text
- [x] Party conflict detected from sample contract text
- [x] No false positives when actions are different
- [x] Confidence threshold respected

---

### Gate 5: Comprehensive Testing and Documentation
**Status:** ✅ Complete

**Deliverables:**
- [x] End-to-end tests with realistic contract excerpts (4 integration tests in Gate 4)
- [x] Documentation in module header and all public types
- [x] Export `ConflictDetector` and all types from `lib.rs`
- [x] Update M1 status to reflect completion

**Note:** Snapshot tests deferred - SnapshotKind trait requires snapshot module which is not in public scope.

**Verification:**
- [x] All 54 conflict_detector tests pass
- [x] All 322 layered-contracts tests pass
- [x] `cargo doc` builds without warnings
- [x] Types accessible from crate root

---

## Appendix A: Type Definitions

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conflict {
    pub span_a: DocSpan,
    pub span_b: DocSpan,
    pub conflict_type: ConflictType,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConflictType {
    ModalConflict,      // shall vs may for same action
    TemporalConflict,   // incompatible timing
    ContradictoryParties, // same action, different obligors
    ScopeOverlap,       // partial overlap with conflicting requirements
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObligationTopic {
    Payment,
    Delivery,
    Confidentiality,
    Termination,
    Indemnification,
    Notice,
    Other,
}
```

---

## Appendix B: Action Similarity Algorithm

Use Jaccard word overlap with exact lemma bonus:

```
"deliver" vs "deliver" = 1.0
"deliver products" vs "deliver goods" = 0.67 (2/3 words overlap: "deliver")
"pay invoice" vs "submit payment" = 0.0 (no word overlap)
```

Threshold: 0.7 for "same action" classification.

---

## Appendix C: Session Learnings

From the 2024-12-29 implementation attempt:

1. **ObligationType exists**: Don't create `CanonicalModal`; reuse `ObligationType` from FR-001
2. **NormalizedTiming exists**: Already in `temporal.rs` with `TimeUnit`, `TemporalConverter`
3. **Scored<T> pattern**: Use `Scored<Conflict>` not embedded confidence
4. **Pipeline dependency**: Must run after `ObligationPhraseResolver`
5. **Defined term format**: Test text needs `"Party Name (the \"Term\")"` for term resolution
6. **Gate 4 failure**: Tests failed because obligations weren't being extracted from test text

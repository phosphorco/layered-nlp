# Phase 4 Master Orchestration Plan

**Last Updated:** 2025-12-31
**Status:** ✅ COMPLETE
**Purpose:** Coordinate completion of remaining Phase 4 milestones using gate-based execution

---

## Phase 4 Complete! 🎉

**All critical path milestones complete as of 2025-12-31:**
- ✅ M0: Foundation Types (SpanLink, ScopeOperator) — 36 tests
- ✅ M1: Baseline ConflictDetector — 54 tests
- ✅ M2: ClauseBoundary + Coordination — 61 tests
- ✅ M3: TermsOfArtResolver — 36 tests
- ✅ M4: Precedence Resolution — 49 tests
- ✅ M7: Negation + Quantifier Scope — 17 tests
- ✅ M8: Semantic Roles + Equivalence — 52 tests

**Total Test Coverage:** 305 tests across Phase 4 milestones

**Optional Milestones (Future Work):**
- M5: Metalinguistic + Deictic (not required for semantic diff)
- M6: PP/Relative Attachment (not required for semantic diff)

---

## Executive Summary

This plan orchestrated the completion of Phase 4 (FR-005/FR-006) using the `/plan-continue` and `/plan-complete` commands for each milestone. Phase 4 adds syntactic structure enhancement and semantic analysis to the layered-nlp contract intelligence system.

**Completed Work (Critical Path):**
```
M0 → M1 → M2 → M3 → M4 → M7 → M8 ✅

All critical path milestones complete!
```

---

## Critical Path Milestones

### 1. M2: ClauseBoundary + Coordination
**Plan:** `/Users/cole/phosphor/phosphor-copy/.context/layered-nlp/.context/plans/completed/M2-clause-boundary.md`
**Effort:** M (2-3 days)
**Dependencies:** M0 ✅
**Status:** ✅ Complete — 2025-12-31 — 61 tests

**Objective:** Transform clause detection into typed `SpanLink<ClauseRole>` relations for hierarchy and coordination.

**Deliverables:**
- Clause hierarchy (Parent/Child links)
- Coordination chains (Conjunct links)
- Exception clauses (Exception links)
- Query API: "what clause contains this span?"
- 5 gates with integration tests

**Execution:**
```bash
# Start M2 work
/plan-continue .context/layered-nlp/.context/plans/M2-clause-boundary.md

# Gates:
# - Gate 1: ClauseRole SpanLink integration
# - Gate 2: Coordination chains
# - Gate 3: Exception/carve-out detection
# - Gate 4: Query API
# - Gate 5: Integration & tests

# When complete:
/plan-complete .context/layered-nlp/.context/plans/M2-clause-boundary.md
```

**Verification:**
- [x] All 6 test scenarios pass (single/coordination/subordination/complex/exception/nested)
- [x] `cargo test -p layered-clauses` passes
- [x] ClauseQueryAPI answers containment queries
- [x] Snapshot tests show both Clause attributes and SpanLink edges

**Blocks:** M4, M6, M7 (now unblocked)

---

### 2. M4: Precedence Resolution
**Plan:** `.context/layered-nlp/.context/plans/completed/M4-precedence-resolution.md`
**Effort:** M (2-3 days)
**Dependencies:** M0 ✅, M1 ✅, M2 ✅
**Status:** ✅ Complete — 2025-12-31 — 49 tests

**Objective:** Precedence clauses become `ScopeOperator<PrecedenceOp>` to resolve conflicts.

**Core Design:**
```rust
pub struct PrecedenceOp {
    pub connective: String,       // "subject to", "notwithstanding"
    pub overrides_domain: bool,   // true if this clause overrides
    pub referenced_sections: Vec<String>,
}

// Emit: ScopeOperator<PrecedenceOp> for each precedence clause
```

**Deliverables:**
- `precedence_resolver.rs` emitting ScopeOperator<PrecedenceOp>
- Integration with M1 ConflictDetector
- ConflictResolution struct citing precedence source
- Cross-reference resolution ("notwithstanding Section 5")
- Tests: A vs B with "subject to", chains, cycles

**Plan Creation Required:**
```bash
# Create M4 plan (5 gates):
# - Gate 1: PrecedenceOp types
# - Gate 2: Precedence detection resolver
# - Gate 3: Cross-reference resolution
# - Gate 4: ConflictDetector integration
# - Gate 5: Integration & tests

# Then execute:
/plan-continue .context/layered-nlp/.context/plans/M4-precedence-resolution.md
/plan-complete .context/layered-nlp/.context/plans/M4-precedence-resolution.md
```

**Verification:**
- [x] Precedence clauses create ScopeOperator<PrecedenceOp>
- [x] Cross-section references resolve correctly
- [x] ConflictDetector uses precedence for resolution
- [x] Tests cover simple/chain/cycle cases
- [x] Integration tests with M1 conflicts

**Blocks:** M8

---

### 3. M7: Negation + Quantifier Scope
**Plan:** `.context/layered-nlp/.context/plans/completed/M7-negation-quantifier-scope.md`
**Effort:** M (2-3 days)
**Dependencies:** M0 ✅, M2 ✅
**Status:** ✅ Complete — 2025-12-31 — 17 tests

**Objective:** Scope operators for negation and quantifiers using clause boundaries.

**Core Design:**
```rust
pub struct NegationOp {
    pub marker: String,          // "not", "never", "no"
    pub kind: NegationKind,      // Sentential, Nominal, Other
}

pub struct QuantifierOp {
    pub marker: String,          // "each", "every", "any"
    pub kind: QuantifierKind,    // Universal, Existential, Negative
    pub cardinality: Option<QuantifierCardinality>,
}
```

**Deliverables:**
- `negation_scope_resolver.rs` emitting ScopeOperator<NegationOp>
- `quantifier_scope_resolver.rs` emitting ScopeOperator<QuantifierOp>
- Scope computation using M2 clause boundaries
- Interaction logic: negation vs quantifier ordering
- Tests: simple negation, double negation, scope ambiguities

**Plan Creation Required:**
```bash
# Create M7 plan (5 gates):
# - Gate 1: NegationOp types + detector
# - Gate 2: QuantifierOp types + detector
# - Gate 3: Scope computation (using M2 clauses)
# - Gate 4: Interaction logic
# - Gate 5: Integration & tests

# Then execute:
/plan-continue .context/layered-nlp/.context/plans/M7-scope-operators.md
/plan-complete .context/layered-nlp/.context/plans/M7-scope-operators.md
```

**Verification:**
- [x] Negation creates ScopeOperator with correct domain
- [x] Quantifiers create ScopeOperator with cardinality
- [x] Scope domains computed using clause boundaries
- [x] Multiple negations handled correctly
- [x] Tests cover scope ambiguities

**Blocks:** M8

---

### 4. M8: Semantic Roles + Equivalence
**Plan:** `.context/layered-nlp/.context/plans/completed/M8-semantic-roles-equivalence.md`
**Effort:** L (4-5 days)
**Dependencies:** M0 ✅, M2 ✅, M4 ✅, M7 ✅
**Status:** ✅ Complete — 2025-12-31 — 52 tests

**Objective:** Map obligations to semantic roles and detect obligation equivalence.

**Gate 1: Semantic Roles**
```rust
pub enum SemanticRole {
    Agent,
    Theme,
    Beneficiary,
    Condition,
    Exception,
    Location,
    Time,
}

// Emit: SpanLink<SemanticRole> from predicates to arguments
```

**Gate 2: Obligation Equivalence**
```rust
pub struct ObligationEquivalence {
    pub obligation_a: DocSpan,
    pub obligation_b: DocSpan,
    pub similarity: f64,
    pub differences: Vec<EquivalenceDifference>,
}
```

**Deliverables:**
- `semantic_roles.rs` with SemanticRoleLabeler and ObligationNormalizer
- Semantic role extraction (Agent, Patient, Theme, Recipient, etc.)
- Obligation equivalence detection across syntax variations
- Active/passive voice handling
- Verb lemmatization and synonym detection
- Modal difference detection
- 52 comprehensive tests

**Verification:**
- [x] Semantic roles assigned to obligations
- [x] Equivalence detection works across syntax variations
- [x] Active/passive voice handled correctly
- [x] Tests cover equivalent/near-equivalent/different cases
- [x] Integration with ObligationPhrase complete
- [x] 52 tests passing

---

## Off-Critical-Path Milestones (Parallelizable)

### 5. M5: Metalinguistic + Deictic
**Plan:** Create at `.context/layered-nlp/.context/plans/M5-metalinguistic-deictic.md`
**Effort:** M (2-3 days)
**Dependencies:** M0 ✅, FR-002/004
**Status:** 📋 Planned (no plan file yet)

**Objective:** SpanLinks for metalinguistic references + ScopeOperator for deictic frames.

**Core Design:**
```rust
pub struct DeicticFrame {
    pub speaker: Option<String>,
    pub time_anchor: Option<String>,
    pub quoted_span: Option<DocSpan>,
}
```

**Deliverables:**
- `metalinguistic_resolver.rs` — links for "foregoing", "above", "below"
- `deictic_resolver.rs` — ScopeOperator<DeicticFrame> for quoted speech
- Integration with existing layered-deixis crate
- Tests: intra-section, cross-section references

**Execution:**
```bash
# Can run in parallel with M2→M4→M7 critical path
/plan-continue .context/layered-nlp/.context/plans/M5-metalinguistic-deictic.md
/plan-complete .context/layered-nlp/.context/plans/M5-metalinguistic-deictic.md
```

**Verification:**
- [ ] Metalinguistic references create SpanLinks
- [ ] Deictic frames capture quoted speech context
- [ ] Tests cover intra/cross-section references

---

### 6. M6: PP/Relative Attachment
**Plan:** Create at `.context/layered-nlp/.context/plans/M6-pp-relative-attachment.md`
**Effort:** M/L (3-4 days)
**Dependencies:** M0 ✅, M2 ✅
**Status:** 📋 Planned (no plan file yet)

**Objective:** SpanLink<AttachmentRole> with N-best alternatives for ambiguous attachments.

**Core Design:**
```rust
pub enum AttachmentRole {
    Head,  // PP/RC attaches to this head
}

// Emit multiple Scored<SpanLink<AttachmentRole>> for ambiguous cases
```

**Deliverables:**
- `pp_attachment_resolver.rs` emitting multiple Scored<SpanLink<AttachmentRole>>
- `relative_clause_resolver.rs` for RC attachment
- N-best preservation (beam-like behavior)
- Ambiguity flagging using `Ambiguous<T>` wrapper
- Tests: classic attachment ambiguities

**Execution:**
```bash
# Can run in parallel with M4/M7, but requires M2 complete
/plan-continue .context/layered-nlp/.context/plans/M6-pp-relative-attachment.md
/plan-complete .context/layered-nlp/.context/plans/M6-pp-relative-attachment.md
```

**Verification:**
- [ ] PP attachments create N-best SpanLinks
- [ ] RC attachments handled correctly
- [ ] Ambiguity flags surface competing alternatives
- [ ] Tests cover classic ambiguity cases ("saw the man with the telescope")

---

## Implementation Phases

### Phase 1: Structure Foundation (Week 1)
**Goal:** Complete clause structure and coordination

- [x] M0: Foundation types (SpanLink, ScopeOperator) ✅
- [x] **M2: ClauseBoundary + Coordination** ✅ 2025-12-31

**Parallel Work:**
- [ ] Create M4, M7, M8 plan files

### Phase 2: Operators + Quality (Week 2)
**Goal:** Add precedence resolution and optional quality improvements

**Critical Path:**
- [x] **M4: Precedence Resolution** ✅ 2025-12-31

**Parallel (optional):**
- [ ] M5: Metalinguistic + Deictic

### Phase 3: Scope + Attachment (Week 3)
**Goal:** Complete scope operators and attachment resolution

**Critical Path:**
- [x] **M7: Negation + Quantifier Scope** ✅ 2025-12-31

**Parallel (optional):**
- [ ] M6: PP/Relative Attachment

### Phase 4: Semantic Integration (Week 4)
**Goal:** Semantic roles and equivalence detection

- [ ] **M8 Gate 1: Semantic Roles**
- [ ] **M8 Gate 2: Obligation Equivalence**

---

## Verification Gates

After each milestone completion, run full test suite:

```bash
# Per-crate tests
cargo test -p layered-nlp
cargo test -p layered-clauses
cargo test -p layered-contracts
cargo test -p layered-deixis

# Full workspace test
cargo test

# Documentation builds
cargo doc --no-deps

# Clippy lints
cargo clippy --all-targets
```

**Integration verification:**
```bash
# Run WASM demo build
cd layered-nlp-demo-wasm
wasm-pack build --target web --out-dir ../web/pkg

# Check demo functionality
cd ../web
python3 -m http.server 8080
# Open http://localhost:8080/contract-viewer.html
```

---

## Dependency Graph

```
┌─────────────────────────────────────────────────────┐
│ M0: Foundation (SpanLink, ScopeOperator)            │
│ Status: ✅ Complete (36 tests)                      │
└──────────────┬──────────────────────────────────────┘
               │
               ├──────────────────────────────────┐
               │                                  │
               ↓                                  ↓
┌──────────────────────────┐      ┌──────────────────────────┐
│ M2: ClauseBoundary       │      │ M5: Metalinguistic       │
│ Status: ✅ Complete      │      │ Status: 📋 Planned       │
│ (61 tests)               │      │ Blocks: M8               │
└──────────┬───────────────┘      └──────────┬───────────────┘
           │                                  │
           ├─────────┬────────────────────────┤
           │         │                        │
           ↓         ↓                        │
    ┌──────────┐  ┌──────────┐               │
    │ M6: PP   │  │ M7: Scope│               │
    │ Attach   │  │ Status:  │               │
    │ Status:  │  │ 📋 Plan  │               │
    │ 📋 Plan  │  │ Blocks:  │               │
    │          │  │ M8       │               │
    └──────────┘  └────┬─────┘               │
                       │                     │
                       ↓                     │
            ┌──────────────────────┐         │
            │ M1: ConflictDetector │         │
            │ Status: ✅ Complete  │         │
            │ (54 tests)           │         │
            └──────┬───────────────┘         │
                   │                         │
                   ↓                         │
        ┌──────────────────────┐             │
        │ M4: Precedence       │             │
        │ Status: 📋 Planned   │             │
        │ Blocks: M8           │             │
        └──────┬───────────────┘             │
               │                             │
               └─────────────┬───────────────┘
                             │
                             ↓
                ┌────────────────────────────┐
                │ M8: Semantic Roles + Equiv │
                │ Status: 📋 Planned         │
                │ (2 gates)                  │
                └────────────────────────────┘
```

**Legend:**
- ✅ Complete
- 📋 Planned
- ⏳ In Progress

---

## Success Criteria

Upon Phase 4 completion, the system will demonstrate:

1. **Conflict Resolution:** Conflicts show precedence-based resolution with citations
2. **Scope Awareness:** "Shall not" correctly scoped; quantifiers handled
3. **Semantic Equivalence:** Duplicate/overlapping obligations detected
4. **N-Best Preserved:** Ambiguous cases flagged for review with alternatives
5. **Graph Navigable:** All relations traversable via SpanLink/AssociatedSpan
6. **Clause Structure:** Query API answers "what clause contains this span?"
7. **Attachment Resolution:** PP/RC attachments with confidence scores

**Quantitative Goals:**
- [x] 200+ total tests across Phase 4 milestones (achieved: 305 tests)
- [x] All critical path milestones complete

---

## Tracking & Updates

### Progress Checklist

**Foundation (Complete):**
- [x] M0: SpanLink, ScopeOperator, ScopeIndex, Ambiguous
- [x] M1: ConflictDetector with temporal/modal/party conflicts
- [x] M3: TermsOfArtResolver

**Critical Path (Complete):**
- [x] M2: ClauseBoundary + Coordination
- [x] M4: Precedence Resolution
- [x] M7: Negation + Quantifier Scope
- [x] M8: Semantic Roles + Equivalence

**Off Critical Path (Optional):**
- [ ] M5: Metalinguistic + Deictic
- [ ] M6: PP/Relative Attachment

### Plan File Status

**Completed Plans:**
- ✅ `/Users/cole/phosphor/phosphor-copy/.context/layered-nlp/.context/plans/M0-foundation-types.md`
- ✅ `/Users/cole/phosphor/phosphor-copy/.context/layered-nlp/.context/plans/completed/M1-baseline-conflict-detector.md`
- ✅ `/Users/cole/phosphor/phosphor-copy/.context/layered-nlp/.context/plans/completed/M2-clause-boundary.md`
- ✅ `/Users/cole/phosphor/phosphor-copy/.context/layered-nlp/.context/plans/completed/M3-terms-of-art-resolver.md`
- ✅ `/Users/cole/phosphor/phosphor-copy/.context/layered-nlp/.context/plans/completed/M4-precedence-resolution.md`
- ✅ `/Users/cole/phosphor/phosphor-copy/.context/layered-nlp/.context/plans/completed/M7-negation-quantifier-scope.md`
- ✅ `/Users/cole/phosphor/phosphor-copy/.context/layered-nlp/.context/plans/completed/M8-semantic-roles-equivalence.md`

**Optional Future Work:**
- [ ] M5-metalinguistic-deictic.md (4 gates) - Off critical path
- [ ] M6-pp-relative-attachment.md (5 gates) - Off critical path

### Update Protocol

**After each milestone completion:**
1. Update this file's Progress Checklist
2. Move completed plan to `.context/plans/completed/`
3. Update `PHASE4-IMPLEMENTATION-AUDIT.md` with test counts and status
4. Commit plan updates separately from code

**Session handoffs:**
1. Update current milestone status in this file
2. Document any learnings in milestone plan's "Learnings & Deviations" section
3. Ensure plan changes are committed even if code is WIP

---

## Command Reference

### Starting Work on a Milestone
```bash
# View all available modules
/modules

# Search for specific module context
/module-search <pattern>

# View full module content
/module <module-path>

# Start working on a milestone plan
/plan-continue .context/layered-nlp/.context/plans/<plan-file>
```

### Completing a Milestone
```bash
# Mark milestone complete
/plan-complete .context/layered-nlp/.context/plans/<plan-file>

# Update audit status
# Edit PHASE4-IMPLEMENTATION-AUDIT.md manually

# Move plan to completed/
git mv .context/layered-nlp/.context/plans/<plan-file> \
       .context/layered-nlp/.context/plans/completed/<plan-file>

# Commit
git add -A
git commit -m "feat(layered-nlp): complete <milestone-name>

- <key achievements>
- <test count> tests passing
- Closes <related-issues>

🤖 Generated with Claude Code
Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

### Debugging & Exploration
```bash
# Debug with parallel agents
/debug <description>

# Parallel exploration
/explore <question>
```

---

## Risk Management

| Risk | Impact | Mitigation | Status |
|------|--------|------------|--------|
| Plan file creation overhead | Delays M4-M8 start | Create minimal plans (3-5 gates each) | ⚠️ Needs plans |
| M2 complexity underestimated | Blocks M4, M6, M7 | Keep coordination simple; defer advanced cases | 🟢 Plan reviewed |
| N-best explosion | Memory/performance issues | Enforce n_best=4, min_score=0.25 thresholds | 🟢 In M0 design |
| Cross-milestone integration bugs | Late-stage failures | Integration tests after each milestone | 🟢 In verification gates |
| Scope operator complexity | M7 delays M8 | Keep ScopeOperator structural; logic downstream | 🟢 In M0 design |

---

## Appendix: Architecture Alignment

Phase 4 aligns with layered-nlp's five principles:

1. **Everything is a span** ✅
   - All semantic content (clauses, operators, roles) has DocSpan position

2. **Semantics live in attributes** ✅
   - SpanLink<R> and ScopeOperator<O> are typed attributes attached to spans

3. **Relationships are first-class** ✅
   - SpanLink<R> creates typed edges; AssociatedSpan tracks provenance

4. **Confidence is explicit** ✅
   - Scored<T> wraps all semantic interpretations
   - Ambiguous<T> preserves N-best with confidence scores

5. **Queries are type-driven** ✅
   - TypeId-based: `query::<SpanLink<ClauseRole>>()`
   - Positional: ScopeIndex.covering_span()

---

## Phase 4 Completion Summary

**Completed 2025-12-31:**
1. ✅ Master orchestration plan created
2. ✅ M2 implementation complete (61 tests, all gates complete)
3. ✅ M4 implementation complete (49 tests, all gates complete)
4. ✅ M7 implementation complete (17 tests, all gates complete)
5. ✅ M8 implementation complete (52 tests, all gates complete)

**Total Achievement:**
- All 7 critical path milestones complete
- 305 tests passing across Phase 4
- Complete semantic analysis capability for contract intelligence
- Foundation for advanced contract diff and conflict detection

**Optional Future Enhancements:**
1. M5: Metalinguistic + Deictic references (off critical path)
2. M6: PP/Relative Attachment resolution (off critical path)

These optional milestones could enhance precision but are not required for the core semantic diff functionality.

---

*This orchestration plan coordinates Phase 4 completion using gate-based execution. Update this file after each milestone completion.*

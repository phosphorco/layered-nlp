# Phase 4 Implementation Audit (FR-005, FR-006)

Last updated: 2024-12-30

## Purpose

This document is the **single source of truth** for Phase 4 implementation status. It maps:
- Plan documents → Code modules → Test files → Actual status

Update this document whenever Phase 4 code is added, modified, or discovered missing.

## Status Legend

| Status | Meaning |
|--------|---------|
| 📋 Planned | Plan doc exists, no implementation committed |
| 🚧 In Progress | Implementation started but not complete/merged |
| ✅ Implemented | Code exists and is wired into pipeline |
| 🧪 Tested | Dedicated tests exist and pass |
| ✓ Verified | Implemented + Tested + manually validated |

---

## Phase 1-3 Summary (FR-001 through FR-004)

These foundational features are **complete and verified**. See FR-000-index.md for details.

| FR | Title | Key Code Modules | Status |
|----|-------|------------------|--------|
| FR-001 | Enhanced Obligation Structure | `modal_resolver.rs`, `condition_struct_resolver.rs`, `implicit_duty_resolver.rs`, `obligation_tree_resolver.rs` | ✓ Verified |
| FR-002 | Advanced Reference Resolution | `bridging_reference.rs`, `coordination.rs`, `pronoun.rs`, `section_reference_linker.rs`, `participant_chain.rs` | ✓ Verified |
| FR-003 | Cross-Line Semantic Spans | `semantic_span.rs`, `document.rs` (DocSpan, SemanticSpan, SpanIndex) | ✓ Verified |
| FR-004 | Enhanced Document Structure | `document_types.rs`, `recital_resolver.rs`, `appendix_boundary_resolver.rs`, `footnote_resolver.rs`, `precedence_clause_resolver.rs` | ✓ Verified |

---

## Phase 4 Milestone Matrix (Revised 2024-12-30)

> **Note:** Phase 4 has been restructured with a foundation-first approach.
> See [PHASE4-REVISED-ROADMAP.md](PHASE4-REVISED-ROADMAP.md) for full details.

| Milestone | FR | Plan Doc | Code Modules | Status | Notes |
|-----------|-----|----------|--------------|--------|-------|
| **M0** | Foundation | [M0-foundation-types.md](M0-foundation-types.md) | `span_link.rs`, `scope_operator.rs`, `scope_index.rs`, `ambiguity.rs` | ✓ Verified | 38 tests (2024-12-30) |
| **M1** | FR-006 | [M1-baseline-conflict-detector.md](completed/M1-baseline-conflict-detector.md) | `conflict_detector.rs` | ✓ Verified | 54 tests |
| **M2** | FR-005 | PHASE4-REVISED-ROADMAP §M2 | `clause_boundary_resolver.rs` | 📋 Planned | Uses SpanLink<ClauseRole> |
| **M3** | FR-005 | [M3-terms-of-art-resolver.md](completed/M3-terms-of-art-resolver.md) | `terms_of_art.rs` | ✓ Verified | 36 tests |
| **M4** | FR-006 | PHASE4-REVISED-ROADMAP §M4 | `precedence_resolver.rs` | 📋 Planned | Uses ScopeOperator<PrecedenceOp> |
| M5 | FR-006 | PHASE4-REVISED-ROADMAP §M5 | (metalinguistic + deictic) | 📋 Planned | Off critical path |
| M6 | FR-005 | PHASE4-REVISED-ROADMAP §M6 | (PP/relative attachment) | 📋 Planned | Off critical path |
| **M7** | FR-005 | PHASE4-REVISED-ROADMAP §M7 | `negation_scope.rs`, `quantifier_scope.rs` | 📋 Planned | Uses ScopeOperator |
| **M8** | FR-006 | PHASE4-REVISED-ROADMAP §M8 | (semantic roles + equivalence) | 📋 Planned | 2-gate milestone |

**Critical path:** M0 → M2 → M4 → M7 → M8

---

## Supporting Infrastructure Status

| Component | Plan | Code | Tests | Status | Notes |
|-----------|------|------|-------|--------|-------|
| Pipeline Orchestrator | FR-008 | `pipeline/mod.rs` | (integrated) | ✅ Implemented | Exists in working tree |
| Navigable Query Results | FR-009 | `navigable/*.rs` | (integrated) | ✅ Implemented | |
| Dual-Layer Snapshots | FR-010 | `snapshot/*.rs` | snapshot tests | ✅ Implemented | |
| Token Diff | (semantic-diff) | `token_diff.rs` | `token_diff::tests` | 🧪 Tested | 592 lines, comprehensive tests |
| Temporal Converter | FR-006/M1 | `temporal.rs` | `temporal::tests` | 🧪 Tested | TimeUnit, NormalizedTiming, TemporalConverter added |

---

## Recovery Notes (2024-12-29)

### What Was Lost

The M1 ConflictDetector implementation was developed in a Claude Code session (2024-12-29) through Gates 0-3, but:
- Code was **never committed to git**
- Working directory was later reset
- Files lost: `conflict_detector.rs`, `tests/conflict_detector_types.rs`, `tests/obligation_normalizer.rs`, `tests/topic_classifier.rs`

### What Was Preserved

- `temporal.rs` enhancements (TimeUnit, NormalizedTiming, TemporalConverter) - **committed**
- `token_diff.rs` - **committed**
- All 268 existing tests pass

### Recovery Strategy

1. Reconstruct M1 plan from session transcript (session contains gate definitions, examples, code snippets)
2. Treat M1 as fresh implementation using the reconstructed plan
3. Implement Gates 0-4 with proper git commits at each gate
4. Use learnings from session: need `Pipeline::standard()` for resolver chain, defined term format for obligations

---

## Workflow Rules

1. **Before writing code for a milestone:**
   - Ensure plan doc exists
   - Add/update row in this audit with Status = 📋 Planned

2. **During implementation:**
   - Update Status = 🚧 In Progress
   - Commit plan doc changes early (before code)

3. **On merge to main:**
   - Update Status = ✅ Implemented when code exists
   - Update Status = 🧪 Tested when tests pass
   - Update Status = ✓ Verified after manual validation

4. **End of every session:**
   - Ensure `.context/plans/` changes are staged
   - Commit any plan updates even if code is WIP

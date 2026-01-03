# Feature Request Index: Edge Case Coverage

This document indexes the feature requests derived from the edge case analysis conducted by three exploration agents examining contract language patterns, linguistic patterns, and document structure.

## Feature Requests Overview

| FR | Title | Priority | Complexity | Edge Cases | Status |
|----|-------|----------|------------|------------|--------|
| [FR-001](../completed-plans/FR-001-obligation-structure.md) | Enhanced Obligation Structure | High | Medium | 4 | ✅ Complete |
| [FR-002](../completed-plans/FR-002-reference-resolution.md) | Advanced Reference Resolution | High | Medium | 5 | ✅ Complete |
| [FR-003](../completed-plans/FR-003-cross-line-spans.md) | Cross-Line Semantic Spans | Critical | High | 3 | ✅ Complete |
| [FR-004](../completed-plans/FR-004-document-structure.md) | Enhanced Document Structure | High | High | 7 | ✅ Complete |
| [FR-005](FR-005-syntactic-enhancement.md) | Syntactic Structure Enhancement | Medium | High | 6 | Phase 4 (M2,M3,M6,M7) |
| [FR-006](FR-006-semantic-analysis.md) | Semantic Analysis & Conflicts | Medium | Medium | 6 | Phase 4 (M1,M4,M5,M8) |
| [FR-005/006 Roadmap](FR-005-006-implementation-roadmap.md) | Phase 4 Implementation Roadmap | - | - | - | 📋 Active |
| FR-008 | Pipeline Orchestrator | High | Medium | - | ✅ Complete |
| FR-009 | Navigable Query Results | High | Medium | - | ✅ Complete |
| FR-010 | Dual-Layer Test Snapshots | High | Medium | - | ✅ Complete |
| FR-011 | Test Migration to Dual-Layer | High | Medium | - | ✅ Complete |

**Current Focus:** M1 (Baseline ConflictDetector) plan is complete; previous implementation was lost. Ready to reimplement. See [M1-baseline-conflict-detector.md](M1-baseline-conflict-detector.md) for the gated plan.

**Implementation Tracking:** See [PHASE4-IMPLEMENTATION-AUDIT.md](PHASE4-IMPLEMENTATION-AUDIT.md) for plan→code→tests status.

## Completed Plans

Completed plans have been moved to [.context/completed-plans/](../completed-plans/) or [completed/](completed/):
- FR-001, FR-002, FR-003, FR-004 — Core infrastructure
- FR-008/Pipeline Orchestrator — Resolver dependency management
- FR-009 — Navigable query results
- FR-010 — Dual-layer test snapshot system
- FR-011 — Test migration to dual-layer snapshots
- Contract Language, Attribute Associations, Deixis WASM Integration

## Dependency Graph

```
                    ┌─────────────────┐
                    │   FR-003        │
                    │ Cross-Line      │
                    │ Spans           │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
     ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
     │   FR-001    │  │   FR-002    │  │   FR-004    │
     │ Obligation  │  │ Reference   │  │ Document    │
     │ Structure   │  │ Resolution  │  │ Structure   │
     └──────┬──────┘  └──────┬──────┘  └──────┬──────┘
            │                │                │
            │                │                │
            └────────────────┼────────────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
     ┌─────────────┐  ┌─────────────┐
     │   FR-005    │  │   FR-006    │
     │ Syntactic   │  │ Semantic    │
     │ Enhancement │  │ Analysis    │
     └─────────────┘  └─────────────┘
```

## Recommended Implementation Order

### Phase 1: Foundation (FR-003) ✅ COMPLETE

Cross-line spans are foundational infrastructure that enables multi-line definitions, VP ellipsis recovery, and proper document structure parsing.

**Deliverables (Completed December 2024):**
- ✅ `DocPosition` and `DocSpan` with ordering, containment, overlap detection
- ✅ `SpanRelation` enum for relationship classification  
- ✅ `SemanticSpan` with type-erased storage and association support
- ✅ `DocAssociatedSpan` - extends Associative Spans to cross-line references
- ✅ `SpanIndex` for O(log n) span lookup
- ✅ `DocumentResolver` trait for document-level pattern detection
- ✅ `ContractDocument.run_document_resolver()`, `query_doc<T>()`, `query_all<T>()`
- ✅ `UnifiedSpan<T>` and `SpanSource` for combined line/document queries

**Key Design Decision:** `DocAssociatedSpan` reuses `layered_nlp::Association` trait, ensuring semantic consistency between line-local and document-level associations.

**Implementation:** `layered-contracts/src/semantic_span.rs`, `layered-contracts/src/document.rs`

### Phase 2: Structure (FR-004, FR-001) ✅ COMPLETE

**FR-001 Deliverables (Completed December 2024):**
- ✅ Modal strength classification (`ExplicitModalResolver`)
- ✅ Condition tree structures (`ConditionStructResolver`)
- ✅ Exception hierarchy modeling
- ✅ Implicit obligation patterns (`ImplicitDutyResolver`)
- ✅ Document-level obligation trees (`ObligationTreeDocResolver`)

**FR-004 Deliverables (Completed December 2024):**
- ✅ Core types: `AppendixBoundary`, `RecitalSection`, `FootnoteBlock`, `PrecedenceRule`
- ✅ Recital classification (`RecitalResolver`)
- ✅ Exhibit/schedule document boundaries (`AppendixBoundaryResolver`)
- ✅ Footnote registry (`FootnoteResolver`)
- ✅ Precedence rules (`PrecedenceClauseResolver`)
- ✅ Enhanced structure builder (`EnhancedDocumentStructureBuilder`)

**Implementation:** 
- FR-001: `modal_resolver.rs`, `condition_struct_resolver.rs`, `implicit_duty_resolver.rs`, `obligation_tree_resolver.rs`, `obligation_types.rs`
- FR-004: `document_types.rs`, `recital_resolver.rs`, `appendix_boundary_resolver.rs`, `footnote_resolver.rs`, `precedence_clause_resolver.rs`, `enhanced_structure.rs`

---

### Resolver Dependency Order (Phase 2)

**Line-level resolvers (run in order per line):**
1. Tokenization & POS (`layered_nlp`, `layered_part_of_speech`)
2. `SectionHeaderResolver`
3. `ContractKeywordResolver` + `ProhibitionResolver`
4. `DefinedTermResolver`, `TermReferenceResolver`, `PronounResolver`
5. `ObligationPhraseResolver`
6. `ConditionStructResolver` (FR-001)
7. `ExplicitModalResolver` (FR-001)
8. `ImplicitDutyResolver` (FR-001)

**Document-level resolvers (run after all line resolvers):**
9. `DocumentStructureBuilder::build()` → `DocumentStructure`
10. `RecitalResolver` (FR-004)
11. `AppendixBoundaryResolver` (FR-004)
12. `FootnoteResolver` (FR-004)
13. `PrecedenceClauseResolver` (FR-004)
14. `ObligationTreeDocResolver` (FR-001)
15. `EnhancedDocumentStructureBuilder` (FR-004, final orchestration)

---

### Phase 3: Resolution (FR-002) ✅ COMPLETE
Reference resolution depends on document structure (for cross-reference linking) and benefits from obligation structure (for provenance tracking).

**Deliverables (Completed December 2024):**

| Step | Component | Status |
|------|-----------|--------|
| 1-2 | `BridgingReference` types & `BridgingReferenceResolver` | ✅ Complete |
| 3a | `CoordinationResolver` - detects "X and Y" patterns | ✅ Complete |
| 3b | Extend `PronounResolver` to use `Coordination` for "they" | ✅ Complete |
| 4 | Cross-document reference linking in `SectionReferenceLinker` | ✅ Complete |
| 5 | `ParticipantChainResolver` - transitive participant chains | ✅ Complete |

**Key Integration Points:**
- Uses `EnhancedDocumentStructure` and `DocumentLayer` from FR-004
- Uses `SpanIndex` and `DocAssociatedSpan` from FR-003
- Extends existing `PronounChainResolver` for backward compatibility

**Implementation:** `layered-contracts/src/bridging_reference.rs`, `coordination.rs`, `pronoun.rs`, `section_reference_linker.rs`, `participant_chain.rs`

### Phase 4: Enhancement (FR-005, FR-006) — In Progress

FR-005 and FR-006 are implemented in an **interleaved** approach via 8 milestones. See [FR-005/006 Implementation Roadmap](FR-005-006-implementation-roadmap.md) for the full roadmap.

| Milestone | Components | Effort | FR |
|-----------|------------|--------|-----|
| M1 | Baseline ConflictDetector | S/M | FR-006 |
| M2 | ClauseBoundaryResolver + CoordinationResolver upgrade | M | FR-005 |
| M3 | TermsOfArtResolver | S | FR-005 |
| M4 | Precedence resolution + richer conflicts | M | FR-006 |
| M5 | Metalinguistic references + deictic integration | M | FR-006 |
| M6 | PP & relative clause attachment | M/L | FR-005 |
| M7 | Negation & quantifier scope | M | FR-005 |
| M8 | Semantic roles + obligation equivalence | L | FR-006 |

**Key insight:** FR-006 conflict detection can start immediately using existing FR-001/FR-004 infrastructure, while FR-005 syntactic features are added incrementally to improve accuracy.

**Highest-value starting points:**
1. M1: Baseline ConflictDetector (immediate value)
2. M3: TermsOfArtResolver (cheap accuracy boost)
3. M2: ClauseBoundaryResolver (unblocks scope resolution)

## Edge Case Coverage Matrix

| Edge Case | Source | FR-001 | FR-002 | FR-003 | FR-004 | FR-005 | FR-006 |
|-----------|--------|--------|--------|--------|--------|--------|--------|
| Modal ambiguity | Contract | ✓ | | | | | |
| Nested conditions | Contract | ✓ | | | | | |
| Implicit obligations | Contract | ✓ | | | | | |
| Cross-references | Contract | | ✓ | | | | |
| Unusual definitions | Contract | | | ✓ | | | |
| Multi-party asymmetry | Contract | ✓ | | | | | |
| Event-based timing | Contract | ✓ | | | | | |
| Exception hierarchies | Contract | ✓ | | | | | |
| Bridging references | Linguistic | | ✓ | | | | |
| Split antecedents | Linguistic | | ✓ | | | ✓ | |
| Garden path | Linguistic | | | | | ✓ | |
| VP ellipsis | Linguistic | | | ✓ | | | |
| Metalinguistic | Linguistic | | | | | | ✓ |
| Quantifier scope | Linguistic | | | | | ✓ | |
| Transitive participants | Linguistic | | ✓ | | | | ✓ |
| Long-distance deps | Linguistic | | | ✓ | | ✓ | |
| Deictic shifts | Linguistic | | | | | | ✓ |
| Terms of art | Linguistic | | | | | ✓ | |
| Multi-level sections | Document | | | | ✓ | | |
| Exhibits as docs | Document | | | | ✓ | | |
| WHEREAS recitals | Document | | | | ✓ | | |
| Amendment patterns | Document | | | | ✓ | | |
| Multi-line defs | Document | | | ✓ | | | |
| Reordered sections | Document | | | | ✓ | | |
| Precedence clauses | Document | | | | ✓ | | ✓ |
| Footnotes | Document | | | | ✓ | | |

## Success Metrics

After full implementation:

- **Obligation coverage**: Detect 95%+ of obligations (currently ~70% due to ellipsis, implicit, multi-line gaps)
- **Reference accuracy**: Resolve 90%+ of references correctly (currently ~60% due to bridging, split antecedent gaps)
- **Diff precision**: Reduce false-positive changes by 50% through semantic equivalence detection
- **Conflict detection**: Flag 100% of directly contradictory provisions
- **Structure fidelity**: Correctly model exhibit boundaries, recitals, and section hierarchy

## Open Questions

1. **Syntactic complexity**: How much syntactic parsing is worth the complexity? FR-005 could be scaled back to just coordination + terms of art.

2. **External dependencies**: Should we integrate an external parser (e.g., spaCy via FFI) for syntactic structure, or build lightweight heuristics?

3. **Precedence rule format**: How should precedence rules be represented for maximum reusability across documents?

4. **Amendment reconstruction**: Should we actually reconstruct amended documents, or just parse amendments as deltas?

5. **Confidence thresholds**: What confidence thresholds should trigger human review for inferred obligations, bridging references, etc.?

---

## Future Direction: Multi-Perspective Architecture

Beyond Phase 4, the architecture is evolving to support **multiple coexisting interpretations** rather than a single "verified truth."

See [../ARCHITECTURE-ITERATIVE-ENRICHMENT.md](../ARCHITECTURE-ITERATIVE-ENRICHMENT.md) for the full vision.

### The Core Insight

**Spans already stack** — multiple spans can exist at the same position. This enables:
- AI produces initial interpretations with alternatives
- Lawyer A adds their perspective (agreeing or disagreeing)
- Lawyer B adds their perspective (independently)
- Synthesis layer compares and detects disagreements
- All perspectives are preserved, not overwritten

### What This Enables

| Capability | Description |
|------------|-------------|
| **Multi-reviewer workflows** | Multiple experts annotate same document |
| **AI + Human collaboration** | AI guesses, humans correct, both preserved |
| **Disagreement detection** | Surface where perspectives differ |
| **Audit trail** | Track interpretation evolution over time |
| **Per-perspective downstream** | Each view has its own obligation set |

### Implementation Path

1. **Phase 4**: Build single-perspective resolver stacks with confidence + alternatives
2. **Phase 5**: Add `Perspective` type to tag spans with their source
3. **Phase 5**: Add perspective-filtered queries and disagreement detection
4. **Phase 5**: Support per-perspective pipeline runs

---

*Generated from edge case analysis by three exploration agents, December 2024.*

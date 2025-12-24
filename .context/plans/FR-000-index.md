# Feature Request Index: Edge Case Coverage

This document indexes the feature requests derived from the edge case analysis conducted by three exploration agents examining contract language patterns, linguistic patterns, and document structure.

## Feature Requests Overview

| FR | Title | Priority | Complexity | Edge Cases Covered |
|----|-------|----------|------------|-------------------|
| [FR-001](FR-001-obligation-structure.md) | Enhanced Obligation Structure | High | Medium | 4 |
| [FR-002](FR-002-reference-resolution.md) | Advanced Reference Resolution | High | Medium | 5 |
| [FR-003](FR-003-cross-line-spans.md) | Cross-Line Semantic Spans | Critical | High | 3 |
| [FR-004](FR-004-document-structure.md) | Enhanced Document Structure | High | High | 7 |
| [FR-005](FR-005-syntactic-enhancement.md) | Syntactic Structure Enhancement | Medium | High | 6 |
| [FR-006](FR-006-semantic-analysis.md) | Semantic Analysis & Conflicts | Medium | Medium | 6 |

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

### Phase 1: Foundation (FR-003)
Cross-line spans are foundational infrastructure that enables multi-line definitions, VP ellipsis recovery, and proper document structure parsing.

**Deliverables:**
- `SemanticSpan` type with `DocPosition` ranges
- `DocumentResolver` trait for cross-line resolution
- Multi-line definition detection

### Phase 2: Structure (FR-004, FR-001)
Document structure and obligation modeling can proceed in parallel once cross-line infrastructure exists.

**FR-004 Deliverables:**
- Exhibit/schedule document boundaries
- Amendment document parsing
- Recital classification
- Footnote registry

**FR-001 Deliverables:**
- Modal strength classification
- Condition tree structures
- Exception hierarchy modeling
- Implicit obligation patterns

### Phase 3: Resolution (FR-002)
Reference resolution depends on document structure (for cross-reference linking) and benefits from obligation structure (for provenance tracking).

**Deliverables:**
- Bridging reference detection
- Split antecedent resolution
- Cross-reference content integration
- Transitive participant chains

### Phase 4: Enhancement (FR-005, FR-006)
Syntactic and semantic enhancements build on all previous work.

**FR-005 Deliverables:**
- Clause boundary detection
- Coordination structure parsing
- Terms of art dictionary
- Scope resolution

**FR-006 Deliverables:**
- Conflict detection
- Precedence rule application
- Deictic context tracking
- Semantic role labeling

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

*Generated from edge case analysis by three exploration agents, December 2024.*

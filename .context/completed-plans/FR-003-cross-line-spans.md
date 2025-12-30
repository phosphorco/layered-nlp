# FR-003: Cross-Line Semantic Spans

**Status:** ✅ Complete (December 2024)  
**Priority:** Critical  
**Complexity:** High

> _This plan file was reconstructed from FR-000-index.md after the original plan document was deleted._

---

## Summary

Foundational infrastructure enabling multi-line semantic spans, which is required for multi-line definitions, VP ellipsis recovery, and proper document structure parsing.

---

## Deliverables (Completed)

- ✅ `DocPosition` and `DocSpan` with ordering, containment, overlap detection
- ✅ `SpanRelation` enum for relationship classification
- ✅ `SemanticSpan` with type-erased storage and association support
- ✅ `DocAssociatedSpan` - extends Associative Spans to cross-line references
- ✅ `SpanIndex` for O(log n) span lookup
- ✅ `DocumentResolver` trait for document-level pattern detection
- ✅ `ContractDocument.run_document_resolver()`, `query_doc<T>()`, `query_all<T>()`
- ✅ `UnifiedSpan<T>` and `SpanSource` for combined line/document queries

---

## Implementation

**Primary files:**
- `layered-contracts/src/semantic_span.rs` - SemanticSpan, SpanIndex, DocumentResolver
- `layered-contracts/src/document.rs` - DocPosition, DocSpan, ContractDocument extensions

---

## Key Design Decision

`DocAssociatedSpan` reuses `layered_nlp::Association` trait, ensuring semantic consistency between line-local and document-level associations.

---

## Edge Cases Addressed

| Edge Case | Solution |
|-----------|----------|
| Multi-line definitions | DocSpan spans multiple lines |
| VP ellipsis | SemanticSpan links to antecedent across lines |
| Long-distance dependencies | SpanIndex enables O(log n) lookup |

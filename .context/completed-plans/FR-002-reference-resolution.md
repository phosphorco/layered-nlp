# FR-002: Advanced Reference Resolution

**Status:** ✅ Complete (December 2024)  
**Priority:** High  
**Complexity:** Medium

> _This plan file was reconstructed from FR-000-index.md after the original plan document was deleted._

---

## Summary

Enhanced reference resolution to handle bridging references, split antecedents, cross-document links, and transitive participant chains.

---

## Deliverables (Completed)

| Step | Component | Description |
|------|-----------|-------------|
| 1-2 | `BridgingReference` types & `BridgingReferenceResolver` | Handles implicit references ("the agreement" → contract) |
| 3a | `CoordinationResolver` | Detects "X and Y" patterns |
| 3b | Extended `PronounResolver` | Uses Coordination for "they" resolution |
| 4 | Cross-document reference linking | `SectionReferenceLinker` enhancements |
| 5 | `ParticipantChainResolver` | Transitive participant chains |

---

## Implementation

- `bridging_reference.rs` - BridgingReferenceResolver
- `coordination.rs` - CoordinationResolver
- `pronoun.rs` - Enhanced PronounResolver
- `section_reference_linker.rs` - Cross-reference linking
- `participant_chain.rs` - ParticipantChainResolver

---

## Key Integration Points

- Uses `EnhancedDocumentStructure` and `DocumentLayer` from FR-004
- Uses `SpanIndex` and `DocAssociatedSpan` from FR-003
- Extends existing `PronounChainResolver` for backward compatibility

---

## Edge Cases Addressed

| Edge Case | Solution |
|-----------|----------|
| Bridging references | BridgingReferenceResolver with entity type inference |
| Split antecedents | CoordinationResolver + PronounResolver |
| Cross-references | SectionReferenceLinker with document structure |
| Transitive participants | ParticipantChainResolver builds chains |
| Long-distance dependencies | SpanIndex enables efficient lookup |

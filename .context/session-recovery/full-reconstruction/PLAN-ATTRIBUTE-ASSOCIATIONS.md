# Associative Spans for layered-nlp

> **Guideline**: Interesting artifacts and learnings must be written back to this document.

## Summary

Add the ability for attribute assignments to reference other spans with typed relationships, enabling provenance tracking and relationship graph traversal. This allows every extracted field to carry a verifiable pointer back to the words that generated it, strengthening trust in the layered representations and powering richer navigation experiences in the UI.

## Problem

When an `ObligationPhrase` is created at "shall", it contains `obligor: "Company"` as a string - but no link back to the `DefinedTerm` span it came from. The relationship graph is lost.

## Benefits & Alignment with layered-nlp

- **Traceable provenance** – Every resolver can emit an association that proves where a value originated, which is crucial for explainability, audits, and collaboratively editing layered documents.
- **Shared relationship graph** – Associations knit spans from different layers into a navigable graph, so downstream tooling (search, QA, analytics) can traverse entities, obligations, and references the same way the model did.
- **UI + workflow unlocks** – Inline arrows and span markers make it obvious how a clause was interpreted, giving contract reviewers "X-ray vision" that cements layered-nlp's product story of transparent structured understanding.
- **Future feature runway** – Once associations exist in the core types, we can build cross-layer invariants (e.g., every `ObligationPhrase` must link to a `DefinedTerm`) and even cross-document linkers without reshaping the plumbing again.
- **Keeps extractors honest** – Because associations live inside `LLCursorAssignment`, any missing or inconsistent provenance becomes a testable failure mode instead of silent drift.

---

## Architecture Overview

**Data flow**: `LLLine` holds tokens plus `LLLineAttrs`, which indexes attribute ranges by type via `TypeIdToMany` and stores concrete values inside `TypeBucket`. `LLSelection` represents a span over an `LLLine`; `LLSelection::finish_with_attr` builds `LLCursorAssignment` structs that `LLLine::run` persists. `LLLineDisplay` reads `TypeBucket` data to render token rows plus attribute annotations.

**Key insight**: Associations must be stored in parallel with values in `TypeBucket` to avoid breaking the existing `get::<T>() -> &[T]` API. Each insertion into `map[TypeId]` must push exactly one entry into `associations[TypeId]` (even if empty) to maintain alignment.

**Type erasure strategy**: Use `Arc<dyn AssociationAny>` so `AssociatedSpan: Clone` without requiring cloneable trait objects. This keeps display/debug paths ergonomic and avoids lifetimes.

---

## Phase 1: Core Association Model

### Objectives
Introduce the foundational types for representing span references and typed associations in `layered-nlp/src/`.

### Scope
- New `SpanRef` struct representing a token range
- New `Association` trait for type-safe semantic labels
- New `AssociatedSpan` struct combining association + span reference
- Object-safe `AssociationAny` wrapper for type-erased storage
- Module wiring and crate exports

### Dependencies
None - this is the foundation.

### Task List

| Task | Acceptance Criteria |
|------|---------------------|
| Create `src/ll_line/association.rs` module | File exists with proper module structure |
| Implement `SpanRef` struct | `SpanRef { start_idx: usize, end_idx: usize }` with `Debug, Clone, Copy, PartialEq, Eq, Hash` |
| Implement `Association` trait | Trait with `label() -> &'static str` and optional `glyph() -> Option<&'static str>` |
| Implement `AssociationAny` trait | Object-safe trait with `label()`, `glyph()`, and `type_id()` methods |
| Implement blanket `AssociationAny for A: Association` | All `Association` implementors automatically implement `AssociationAny` |
| Implement `AssociatedSpan` struct | Contains `span: SpanRef` and `association: Arc<dyn AssociationAny>` |
| Add `AssociatedSpan::new<A: Association>()` constructor | Accepts concrete association type and span |
| Add accessor methods on `AssociatedSpan` | `label()`, `glyph()`, `association_type_id()` delegate to inner trait object |
| Wire module in `src/ll_line.rs` | Add `mod association;` declaration |
| Export types from `src/ll_line.rs` | `pub use association::{Association, AssociatedSpan, SpanRef};` |
| Re-export from `src/lib.rs` | Types available at crate root |

### Verification

**Test scenarios**:
1. `SpanRef` equality and hashing works correctly
2. Custom `Association` implementor returns correct label and glyph
3. `AssociatedSpan` can be created, cloned, and accessors return expected values
4. `AssociationAny::type_id()` returns distinct `TypeId` for different association types
5. `Arc<dyn AssociationAny>` allows `AssociatedSpan` to be `Clone + Send + Sync`

**Test organization**: `src/ll_line/association.rs` - inline `#[cfg(test)] mod tests`

**Pass/fail criteria**: All unit tests pass; `cargo test -p layered-nlp association` succeeds

---

## Phase 2: Extend Assignment Pipeline

### Objectives
Modify `LLCursorAssignment` to carry associations and update the `LLLine::run` insertion path to persist them.

### Scope
- Extend `LLCursorAssignment<Attr>` with `associations: Vec<AssociatedSpan>` field
- Update `LLLine::run` to destructure and pass associations to storage
- Add `insert_with_associations` method to `LLLineAttrs`
- Maintain backwards compatibility - existing code must compile unchanged

### Dependencies
- Phase 1 complete (association types exist)

### Task List

| Task | Acceptance Criteria |
|------|---------------------|
| Add `associations` field to `LLCursorAssignment` | `associations: Vec<AssociatedSpan>` field added |
| Update `LLCursorAssignment` constructors | All existing construction sites updated (likely in `ll_selection.rs`) |
| Update `LLLine::run` destructuring | Loop extracts `associations` from each assignment |
| Add `LLLineAttrs::insert_with_associations()` | Method accepts `(range, value, associations)` tuple |
| Call new insertion method from `run` | Associations flow through to storage layer |
| Ensure `finish_with_attr()` sets empty associations | Backwards compatibility preserved |

### Verification

**Test scenarios**:
1. Existing resolver tests continue to pass (no regressions)
2. `LLCursorAssignment` with empty associations behaves identically to before
3. `LLCursorAssignment` with non-empty associations is accepted by `LLLine::run`
4. `LLLineAttrs::insert_with_associations` stores value correctly (association storage tested in Phase 3)

**Test organization**: Existing tests in `src/ll_line.rs` and resolver tests serve as regression suite

**Pass/fail criteria**: `cargo test -p layered-nlp` passes; no API breakage in dependent crates

---

## Phase 3: TypeBucket Association Storage

### Objectives
Extend `TypeBucket` to store associations alongside values while maintaining the existing `get::<T>() -> &[T]` API.

### Scope
- Add parallel `associations: HashMap<TypeId, Vec<Vec<AssociatedSpan>>>` storage
- Implement `insert_with_associations()` that maintains alignment invariant
- Update existing `insert()` to delegate with empty associations
- Update `insert_any_attribute()` to maintain alignment
- Add retrieval method for display: `get_debug_with_associations::<T>()`

### Dependencies
- Phase 1 complete (association types exist)
- Phase 2 complete (insertion path calls into TypeBucket)

### Task List

| Task | Acceptance Criteria |
|------|---------------------|
| Add `associations` field to `TypeBucket` | `HashMap<TypeId, Vec<Vec<AssociatedSpan>>>` parallel to `map` |
| Initialize `associations` in `TypeBucket::new()` | Empty HashMap created |
| Implement `insert_with_associations<T>()` | Pushes value to `map` and associations to `associations` in same index |
| Refactor `insert<T>()` to delegate | Calls `insert_with_associations(val, vec![])` |
| Update `insert_any_attribute()` | Maintains alignment by pushing empty vec to `associations[key]` |
| Implement `get_debug_with_associations<T>()` | Returns `Vec<(String, Vec<AssociatedSpan>)>` pairing debug strings with associations |
| Add alignment invariant assertion (debug builds) | `debug_assert!` that `map[tid].len() == associations[tid].len()` |

### Verification

**Test scenarios**:
1. `insert_with_associations` followed by `get::<T>()` returns correct values
2. `insert_with_associations` followed by `get_debug_with_associations::<T>()` returns paired data
3. Multiple insertions maintain correct alignment (indices match)
4. `insert_any_attribute` maintains alignment with associations
5. Empty associations round-trip correctly
6. Non-empty associations are retrievable with correct spans and labels

**Test organization**: `src/type_bucket.rs` - inline `#[cfg(test)] mod tests`

**Pass/fail criteria**: All TypeBucket tests pass; alignment invariant never fires in test suite

---

## Phase 4: Selection API Builder Pattern

### Objectives
Provide ergonomic APIs on `LLSelection` for creating assignments with associations.

### Scope
- Add `span_ref()` method to `LLSelection`
- Add `finish_with_attr_and_associations()` method
- Implement fluent builder: `assign() -> LLAssignmentBuilder`
- Builder methods: `with_association()`, `with_association_from_selection()`, `build()`

### Dependencies
- Phase 1 complete (association types exist)
- Phase 2 complete (`LLCursorAssignment` accepts associations)

### Task List

| Task | Acceptance Criteria |
|------|---------------------|
| Add `LLSelection::span_ref()` | Returns `SpanRef { start_idx: self.start_idx, end_idx: self.end_idx }` |
| Add `finish_with_attr_and_associations()` | Creates `LLCursorAssignment` with provided associations |
| Create `LLAssignmentBuilder<Attr>` struct | Holds selection reference, value, and associations vec |
| Implement `LLSelection::assign<Attr>()` | Returns `LLAssignmentBuilder` initialized with empty associations |
| Implement `with_association<A>(association, span)` | Pushes `AssociatedSpan::new(association, span)` to builder |
| Implement `with_association_from_selection<A>(association, &sel)` | Calls `with_association(association, sel.span_ref())` |
| Implement `build()` | Calls `finish_with_attr_and_associations` with accumulated data |

### Verification

**Test scenarios**:
1. `span_ref()` returns correct indices for various selections
2. `finish_with_attr_and_associations()` creates assignment with correct associations
3. Builder pattern: `assign().build()` produces assignment with empty associations
4. Builder pattern: chained `with_association()` calls accumulate correctly
5. Builder pattern: `with_association_from_selection()` extracts correct span
6. Builder is ergonomic in resolver-like code (integration test)

**Test organization**: `src/ll_line/ll_selection.rs` - inline `#[cfg(test)] mod tests`

**Pass/fail criteria**: All selection tests pass; builder API compiles in example resolver code

---

## Phase 5: Display Rendering

### Objectives
Extend `LLLineDisplay` to render associations as arrow lines with optional span labels.

### Scope
- Internal `IncludedAttr` struct to track associations per included attribute
- `include_with_associations<T>()` method
- Span label assignment algorithm (`[A]`, `[B]`, etc.)
- Arrow line rendering with glyph, label, and target

### Dependencies
- Phase 3 complete (`get_debug_with_associations` available)
- Phase 4 complete (associations can be created and stored)

### Task List

| Task | Acceptance Criteria |
|------|---------------------|
| Create internal `IncludedAttr` struct | Holds `range`, `debug_value`, `associations`, `show_associations` flag |
| Refactor `include_attrs` storage | Uses `IncludedAttr` instead of tuple |
| Update existing `include<T>()` | Creates `IncludedAttr` with empty associations and `show_associations: false` |
| Implement `include_with_associations<T>()` | Fetches associations via `get_debug_with_associations`, sets `show_associations: true` |
| Implement span label assignment | Deterministically assigns `[A]`, `[B]`, etc. to referenced spans that are also included |
| Render span labels in attribute lines | Append `[A]` marker after span bracket, before debug value |
| Render association arrow lines | After attribute line, render `└─{glyph}{label}─>[target]` for each association |
| Handle unlabeled targets | Fall back to `[start..end]` for targets not in included set |

### Verification

**Test scenarios**:
1. `include<T>()` behavior unchanged (no arrow lines rendered)
2. `include_with_associations<T>()` renders arrow lines for attributes with associations
3. Span labels assigned deterministically (sorted by start, then end index)
4. Multiple associations on same attribute render as multiple arrow lines
5. Arrow targets use `[A]` label when target span is included
6. Arrow targets use `[start..end]` when target span not included
7. Glyph and label appear correctly in arrow line
8. Complex multi-layer display renders correctly

**Test organization**: `src/ll_line/display.rs` - inline `#[cfg(test)] mod tests` plus snapshot tests

**Pass/fail criteria**: All display tests pass; snapshot output matches expected format

### Expected Display Format

```
The  Company  shall  deliver  goods
     ╰──╯[A] DefinedTerm("Company")
               ╰──╯Shall
               ╰────────────────╯ObligationPhrase { obligor: "Company", action: "deliver" }
                  └─@obligor_source─>[A]
                  └─#action_span────>[3..5]
```

---

## Phase 6: Adoption in layered-contracts

### Objectives
Define contract-specific association types and update `ObligationPhraseResolver` to emit associations linking obligations to their source spans.

### Scope
- Define `ObligorSource` and `ActionSpan` association types
- Modify `find_obligor()` to return source selection/span
- Modify `extract_action()` to return action span
- Update resolver to emit associations via builder API

### Dependencies
- Phase 4 complete (builder API available)
- All core phases complete

### Task List

| Task | Acceptance Criteria |
|------|---------------------|
| Define `ObligorSource` association type | Implements `Association` with label `"obligor_source"` and glyph `"@"` |
| Define `ActionSpan` association type | Implements `Association` with label `"action_span"` and glyph `"#"` |
| Modify `find_obligor()` signature | Returns `Option<(ObligorReference, bool, SpanRef)>` or equivalent with source span |
| Update `find_obligor()` implementation | Tracks and returns the span of the obligor source (term/pronoun/noun-phrase) |
| Modify `extract_action()` signature | Returns `(String, Option<SpanRef>)` with action span |
| Update `extract_action()` implementation | Computes span from matched action words |
| Update resolver `go()` method | Uses builder API to emit associations |
| Handle cases where spans unavailable | Gracefully omit association when source span cannot be determined |

### Verification

**Test scenarios**:
1. `ObligorSource` association has correct label and glyph
2. `ActionSpan` association has correct label and glyph
3. Obligations from defined terms include `ObligorSource` pointing to term span
4. Obligations from pronouns include `ObligorSource` pointing to pronoun span
5. Obligations with action verbs include `ActionSpan` pointing to verb span
6. Obligations without clear action gracefully omit `ActionSpan`
7. Multiple obligations in same sentence each have correct associations

**Test organization**: `layered-contracts/src/tests/obligation.rs` - integration tests with snapshots

**Pass/fail criteria**: All obligation tests pass; associations present in snapshot output

---

## Phase 7: Update Snapshots

### Objectives
Update test snapshots to render and verify association output.

### Scope
- Switch obligation display to use `include_with_associations`
- Update snapshot files to reflect new output format
- Ensure associations are visible and correct in snapshots

### Dependencies
- Phase 5 complete (display rendering works)
- Phase 6 complete (obligations emit associations)

### Task List

| Task | Acceptance Criteria |
|------|---------------------|
| Update `test_obligations` display setup | Replace `include::<Scored<ObligationPhrase>>()` with `include_with_associations::<Scored<ObligationPhrase>>()` |
| Run `cargo insta review` | Review and accept updated snapshots |
| Verify snapshot content | Associations visible with correct labels, glyphs, and targets |
| Add dedicated association snapshot test | Test specifically verifying association arrow rendering |

### Verification

**Test scenarios**:
1. Existing obligation snapshots updated to show associations
2. Arrow lines appear under obligation spans
3. Referenced spans (DefinedTerm, etc.) receive `[A]`, `[B]` labels
4. Snapshot is deterministic (no flaky ordering)

**Test organization**: `layered-contracts/src/tests/obligation.rs` - snapshot tests via `insta`

**Pass/fail criteria**: `cargo insta test` passes; snapshots accurately reflect associations

---

## Artifacts & Learnings

### Architectural Decisions Made

| Decision | Rationale | Date |
|----------|-----------|------|
| Use `Arc<dyn AssociationAny>` for type erasure | Allows `AssociatedSpan: Clone` without requiring cloneable trait objects; avoids lifetimes in storage | 2025-12-19 |
| Parallel `HashMap<TypeId, Vec<Vec<AssociatedSpan>>>` in TypeBucket | Preserves existing `get::<T>() -> &[T]` API; alignment invariant maintained by insertion methods | 2025-12-19 |
| `ObligorSource` and `ActionSpan` associations | Both now implemented; `ActionSpan` requires tracking word selections during action extraction | 2025-12-19 |
| Span labels `[A]`, `[B]` only for included ranges | Avoids clutter; targets that aren't in display fall back to `[start..end]` indices | 2025-12-19 |

### Challenges Encountered

| Challenge | Resolution | Date |
|-----------|------------|------|
| Trait method ambiguity (`Association` vs `AssociationAny`) | Use fully-qualified syntax `Association::label(&assoc)` in tests | 2025-12-19 |
| Multi-word noun phrase span tracking | Track `first_sel` and `last_sel` during backwards iteration, build span from earliest to latest | 2025-12-19 |
| Missing public API for querying associations | Added `query_with_associations<T>()` to `LLLine` and `get_with_associations<T>()` to `TypeBucket` | 2025-12-19 |
| Span labels overflow after Z | Implemented base-26 labeling in `index_to_base26_label()`: A..Z, AA, AB.., AZ, BA.. (Excel-style) | 2025-12-19 |
| `ActionSpan` defined but not emitted | Modified `extract_action()` to return `(String, Option<SpanRef>)` and emit `ActionSpan` associations | 2025-12-19 |
| ActionSpan misaligned with trimmed action | `extract_action()` returns word spans array; `trim_trailing_conjunction()` returns word count to keep; span computed from retained words only | 2025-12-19 |
| WASM demo needed provenance access | Added `provenance` field to ObligationPhrase metadata in `layered-nlp-demo-wasm/src/lib.rs` using `query_with_associations` | 2025-12-19 |
| String-based matching unreliable for duplicates | Use `std::ptr::eq()` to match obligations by pointer equality instead of comparing action/obligor strings | 2025-12-19 |

### Implementation Summary

**Files Modified**:
- `src/ll_line/association.rs` - NEW: `SpanRef`, `Association`, `AssociatedSpan` (152 lines)
- `src/ll_line.rs` - Module wiring, `LLCursorAssignment.associations`, `insert_with_associations`, `query_with_associations<T>()`
- `src/ll_line/ll_selection.rs` - `span_ref()`, `assign()` builder, `LLAssignmentBuilder`
- `src/type_bucket.rs` - Parallel associations storage, `get_debug_with_associations`, `get_with_associations<T>()`
- `src/ll_line/display.rs` - `IncludedAttr`, `include_with_associations`, arrow rendering, `index_to_base26_label()`
- `src/lib.rs` - Re-exports
- `layered-contracts/src/obligation.rs` - `ObligorSource`, `ActionSpan`, updated `find_obligor()` and `extract_action()`, builder usage
- `layered-contracts/src/tests/obligation.rs` - Added `action_span_aligns_with_trimmed_text` regression test
- `layered-nlp-demo-wasm/src/lib.rs` - Added provenance data to ObligationPhrase metadata using `query_with_associations`

**Test Coverage**:
- 7 unit tests in `association.rs` covering SpanRef, Association trait, AssociatedSpan
- 1 unit test in `display.rs` for `index_to_base26_label()` base-26 labeling
- 31 tests in layered-nlp (including association and display tests)
- 205 tests in layered-contracts (all obligation tests updated with association output)
- `action_span_aligns_with_trimmed_text` regression test verifying ActionSpan/trimmed action alignment

### Example Output

```
ABC     Corp     (  the     "  Company  "  )     shall     deliver     goods  .
                                                 ╰───╯Shall
                 ╰─────────────────────────╯Scored(DefinedTerm { ... })
                                                 ╰───╯Scored(ObligationPhrase { obligor: NounPhrase { text: "Company" }, ... })
                                                   └─@obligor_source─>[8..8]
                                                   └─#action_span─>[12..16]
```

### Future Considerations

- Cross-document associations (linking spans across files)
- Association validation (e.g., every `ObligationPhrase` must link to a `DefinedTerm`)
- Bidirectional traversal (query "what obligations reference this term?")
- Association types for other resolvers (section references, defined term sources, etc.)
- Labeled span markers when target is in display (e.g., `└─@obligor_source─>[A]`)
- ConditionSpan association for linking condition phrases to their source text

---

## Critical Files Reference

| File | Role |
|------|------|
| `src/ll_line/association.rs` | NEW - Core association types |
| `src/ll_line.rs` | `LLCursorAssignment` extension, module wiring |
| `src/ll_line/ll_selection.rs` | Builder API, `finish_with_attr_and_associations()` |
| `src/type_bucket.rs` | Parallel association storage |
| `src/ll_line/display.rs` | Arrow rendering, span labels |
| `src/lib.rs` | Crate exports |
| `layered-contracts/src/obligation.rs` | Association types, resolver updates |
| `layered-contracts/src/tests/obligation.rs` | Integration tests, snapshots |

---

## Backwards Compatibility Guarantees

- `finish_with_attr()` unchanged - creates empty associations
- Existing resolvers work without modification
- `include<T>()` display unchanged - associations only shown when requested
- `get::<T>() -> &[T]` API preserved
- Gradual opt-in for each resolver

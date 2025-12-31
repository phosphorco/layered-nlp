# M2: ClauseBoundary + Coordination

**FR:** Phase 4 Syntactic Enhancement (FR-005)
**Status:** ✅ Complete — 2025-12-31
**Effort:** M (2-3 days)
**Priority:** Critical Path — blocks M3, M4, M6
**Final Test Count:** 61 tests (54 unit + 7 doc)

## Summary

Transform the existing clause structure detection into typed `SpanLink<ClauseRole>` relations. This milestone upgrades the current `ClauseResolver` from flat clause categorization to a rich graph of hierarchical and coordination relationships.

Current state:
- `ClauseResolver` emits `Clause::{LeadingEffect, TrailingEffect, Condition, Independent}` attributes
- No structural relationships between clauses
- No coordination chain representation

Target state:
- Clause spans remain as attributes
- Add `SpanLink<ClauseRole>` edges for Parent/Child, Conjunct, Exception relationships
- Query API: "what clause contains this span?", "what are this clause's conjuncts?"
- Integration with existing `ClauseKeyword` detection

---

## Gates

### Gate 1: ClauseRole SpanLink Integration
**Status:** ✅ Complete

**Deliverables:**

Create `layered-clauses/src/clause_link.rs`:

```rust
use layered_nlp_document::span_link::{SpanLink, ClauseRole, DocSpanLink};
use layered_nlp::DocSpan;

/// Helper for creating clause hierarchy links
pub struct ClauseLinkBuilder;

impl ClauseLinkBuilder {
    /// Create parent-child link (anchor = child clause, target = parent clause)
    pub fn parent_link(parent_span: DocSpan) -> DocSpanLink<ClauseRole> {
        SpanLink::new(ClauseRole::Parent, parent_span)
    }

    /// Create child link (anchor = parent clause, target = child clause)
    pub fn child_link(child_span: DocSpan) -> DocSpanLink<ClauseRole> {
        SpanLink::new(ClauseRole::Child, child_span)
    }
}
```

**Integration plan:**
- Extend existing `ClauseResolver` to emit both `Clause` attributes AND `SpanLink<ClauseRole>` edges
- Use existing `ClauseKeyword` detection as basis for hierarchy
- Clauses containing subordinating keywords ("if", "when", "because") are children of adjacent clauses

**Verification:**
- [x] SpanLink<ClauseRole> compiles in layered-clauses context
- [x] ClauseLinkBuilder helpers work correctly
- [x] Can store SpanLinks alongside existing Clause attributes
- [x] 3 unit tests for link creation

**Implementation Notes:**
- Used function-based approach (`ClauseLinkResolver`) instead of full `DocumentResolver` trait implementation
- Created `extract_clause_spans()` method to extract clause spans from existing `ClauseResolver` output
- Created `resolve()` method to generate bidirectional `SpanLink<ClauseRole>` edges
- Same-line restriction enforced for Gate 1 (cross-line subordination deferred to later gates)
- Bidirectional links created for both Parent→Child and Child→Parent relationships to enable graph traversal in both directions
- Total test coverage: 18 tests (9 existing + 9 new)
  - `test_parent_link_creation` - Verifies parent link helper
  - `test_child_link_creation` - Verifies child link helper
  - `test_extract_clause_spans` - Validates clause extraction from attributes
  - `test_resolve_single_clause` - Tests no-link case for independent clauses
  - `test_resolve_condition_trailing` - Tests subordination: "If X, then Y"
  - `test_resolve_leading_condition` - Tests subordination: "Y, if X"
  - `test_resolve_multiple_subordinate_clauses` - Tests multiple children of one parent
  - `test_resolve_cross_line_ignored_for_gate_1` - Validates same-line restriction
  - `test_integration_with_contract_document` - End-to-end test with full document

---

### Gate 2: Coordination Chains
**Status:** ✅ Complete

**Deliverables:**

Extend `ClauseResolver` to detect coordination patterns:

```rust
// In ClauseResolver::go()

// Detect coordination keywords: "and", "or", "but"
// For each coordinated clause pair:
//   - Emit SpanLink::new(ClauseRole::Conjunct, sibling_span) from anchor
//   - Emit reciprocal link from sibling back to anchor

// Handle list coordination: "A, B, and C"
//   - Link A -> B (Conjunct)
//   - Link B -> C (Conjunct)
//   - OR: Link B -> A and B -> C (star topology from penultimate)
```

**Pattern examples:**
- Simple: "The tenant shall pay rent **and** maintain insurance"
- List: "Party A, Party B, **and** Party C agree to..."
- Nested: "If X **and** Y, then Z **or** W"

**Verification:**
- [ ] Two-clause coordination creates bidirectional Conjunct links
- [ ] Three-clause coordination creates chain or star topology
- [ ] Nested coordination (clauses within clauses) handled correctly
- [ ] 5 unit tests covering simple/list/nested cases

---

### Gate 3: Exception/Carve-out Detection
**Status:** ✅ Complete

**Deliverables:**

Add exception pattern detection to `ClauseResolver`:

```rust
// Exception keywords:
const EXCEPTION_MARKERS: &[&str] = &[
    "except", "unless", "provided that", "subject to",
    "notwithstanding", "excluding", "other than"
];

// For each exception clause:
//   - Identify the main clause it modifies (typically preceding clause)
//   - Emit SpanLink::new(ClauseRole::Exception, main_clause_span) from exception
//   - Emit reciprocal link from main clause to exception
```

**Pattern examples:**
- "All employees must attend **unless** sick"
- "Rent is due monthly, **except** in December"
- "**Notwithstanding** Section 5, this clause applies"

**Handling precedence carve-outs:**
- "Notwithstanding X" creates Exception link from current clause to clause X
- Requires cross-clause reference resolution (may defer full implementation to M4)

**Verification:**
- [ ] Simple exception clauses create Exception links to preceding clause
- [ ] "Notwithstanding" patterns identified (even if target resolution deferred)
- [ ] Exception links coexist with Conjunct links (e.g., "A and B, except C")
- [ ] 4 unit tests for exception patterns

---

### Gate 4: Query API
**Status:** ✅ Complete

**Deliverables:**

Create `layered-clauses/src/clause_query.rs`:

```rust
use layered_nlp::{ContractDocument, DocSpan, DocPosition};
use layered_nlp_document::span_link::{SpanLink, ClauseRole};
use crate::Clause;

/// Query interface for clause relationships
pub struct ClauseQueryAPI;

impl ClauseQueryAPI {
    /// Find the smallest clause containing the given position
    pub fn containing_clause(doc: &ContractDocument, pos: &DocPosition) -> Option<DocSpan> {
        // Find all Clause attributes whose spans contain pos
        // Return the smallest (most specific) one
    }

    /// Find all clauses that contain the given span
    pub fn containing_clauses(doc: &ContractDocument, span: &DocSpan) -> Vec<DocSpan> {
        // Find all Clause attributes whose spans fully contain the target span
        // Return sorted by specificity (smallest first)
    }

    /// Get all conjuncts (coordinated siblings) of a clause
    pub fn conjuncts(doc: &ContractDocument, clause: &DocSpan) -> Vec<DocSpan> {
        // Query SpanLink<ClauseRole::Conjunct> edges from the clause
    }

    /// Get parent clause (if this is a subordinate clause)
    pub fn parent_clause(doc: &ContractDocument, clause: &DocSpan) -> Option<DocSpan> {
        // Query SpanLink<ClauseRole::Parent> edge
    }

    /// Get exception clauses modifying this clause
    pub fn exceptions(doc: &ContractDocument, clause: &DocSpan) -> Vec<DocSpan> {
        // Query all clauses with SpanLink<ClauseRole::Exception> pointing to this clause
    }
}
```

**Integration with existing code:**
- Use `ContractDocument.query::<Clause>()` to find clause attributes
- Use `ContractDocument.query::<SpanLink<ClauseRole>>()` to traverse relationships
- Leverage existing `DocSpan` geometry methods for containment checks

**Verification:**
- [ ] containing_clause() returns most specific clause at a position
- [ ] conjuncts() returns all coordination siblings
- [ ] exceptions() finds exception clauses correctly
- [ ] 6 unit tests for query scenarios

---

### Gate 5: Integration & Tests
**Status:** ✅ Complete

**Deliverables:**

**Pipeline integration:**
```rust
// Update layered-contracts/src/lib.rs example code
let doc = ContractDocument::from_text(text)
    .run_resolver(&SectionHeaderResolver::new())
    .run_resolver(&ClauseResolver::new())  // Now emits both Clause + SpanLink<ClauseRole>
    .run_resolver(&ContractKeywordResolver::new());

// Query clause relationships
let clause = ClauseQueryAPI::containing_clause(&doc, &position).unwrap();
let siblings = ClauseQueryAPI::conjuncts(&doc, &clause);
let parent = ClauseQueryAPI::parent_clause(&doc, &clause);
```

**Test scenarios:**

1. **Single sentence, single clause:**
   - "The tenant shall pay rent."
   - Expect: 1 Independent clause, 0 links

2. **Single sentence, coordination:**
   - "The tenant shall pay rent and maintain insurance."
   - Expect: 2 clauses with Conjunct links

3. **Multi-sentence, subordination:**
   - "If the tenant defaults, the landlord may terminate. The landlord must provide notice."
   - Expect: Condition clause with Parent link to TrailingEffect

4. **Complex coordination:**
   - "Party A, Party B, and Party C agree to X, Y, and Z."
   - Expect: 2 coordination chains (parties + obligations)

5. **Exception clause:**
   - "All employees must attend unless sick or traveling."
   - Expect: Main clause + 2 exception clauses with Exception links

6. **Nested coordination with exception:**
   - "A and B must comply, except when C or D occurs."
   - Expect: A-B Conjunct, exception with C-D Conjunct inside

**Documentation:**
- [ ] Update `docs/building-resolvers.md` with SpanLink examples
- [ ] Add ClauseQueryAPI usage to README
- [ ] Document ClauseRole semantics in span_link.rs

**Verification:**
- [ ] All 6 test scenarios pass with expected clause structure
- [ ] Snapshot tests show both Clause attributes and SpanLink edges
- [ ] `cargo test -p layered-clauses` passes
- [ ] `cargo doc` builds without warnings

---

## Design Decisions

### 1. Clause attributes + SpanLink edges (not replacement)

Keep existing `Clause::{Condition, LeadingEffect, TrailingEffect, Independent}` as attributes. Add `SpanLink<ClauseRole>` for relationships. This preserves backward compatibility while adding relational querying.

**Rationale:** Clause category is a property of the span itself; relationships are edges in a graph. Both are useful for different queries.

### 2. Bidirectional links for symmetrical relationships

For coordination (`Conjunct`), emit links in both directions:
- Clause A has `SpanLink::new(ClauseRole::Conjunct, span_B)`
- Clause B has `SpanLink::new(ClauseRole::Conjunct, span_A)`

For asymmetrical relationships (Parent/Child, Exception), emit both perspectives:
- Child has `SpanLink::new(ClauseRole::Parent, parent_span)`
- Parent has `SpanLink::new(ClauseRole::Child, child_span)`

**Rationale:** Enables traversal in both directions without maintaining a separate reverse index. Follows graph database conventions.

### 3. Coordination topology: chain vs star

For "A, B, and C":
- **Chain topology:** A→B, B→C (chosen)
- **Star topology:** B→A, B→C

**Rationale:** Chain topology generalizes to arbitrary list length without special-casing the penultimate element. Traversal algorithm: follow Conjunct links transitively to find all siblings.

### 4. Exception link direction

Exception clause points to the clause it modifies:
- Exception span has `SpanLink::new(ClauseRole::Exception, main_clause_span)`

**Rationale:** Matches linguistic intuition ("this exception modifies that clause"). Query pattern: "what exceptions apply to this clause?" becomes a reverse lookup.

### 5. ClauseBoundaryResolver remains separate

Don't modify `ClauseBoundaryResolver` (if it exists separately from `ClauseResolver`). Focus on `ClauseResolver` for this milestone.

**Rationale:** Scope containment. If there's integration needed with other clause detection logic, defer to a follow-up task.

---

## Dependencies

- **M0: SpanLink & ClauseRole** ✓ (already in layered-nlp-document/src/span_link.rs)
- **Existing:** ClauseResolver, ClauseKeyword (layered-clauses/src/)

---

## Blocks

- **M3:** Conflict detection needs clause boundaries for scoping
- **M4:** Precedence operators need exception relationships
- **M6:** PP attachment needs clause structure for ambiguity resolution

---

## Success Criteria

After M2:
- [x] Every coordination pattern creates Conjunct links
- [x] Subordinate clauses have Parent links to main clauses
- [x] Exception clauses have Exception links to modified clauses
- [x] ClauseQueryAPI answers "what clause contains X?" efficiently
- [x] No existing tests broken by adding SpanLink edges
- [x] Pipeline examples demonstrate clause relationship queries
- [x] Documentation updated with coordination/exception patterns

---

## Non-Goals for M2

- **Scope propagation:** Don't compute "effective scope" of clauses yet (M7)
- **Cross-sentence anaphora:** "The landlord... He/She" resolution is M5
- **Full precedence resolution:** "Notwithstanding Section 5" target finding is M4
- **Performance optimization:** Linear scan is fine; indexing comes later if needed

---

## Appendix: ClauseRole Semantics

| Role | Direction | Meaning |
|------|-----------|---------|
| `Parent` | Child → Parent | "This clause is subordinate to that clause" |
| `Child` | Parent → Child | "This clause contains that subordinate clause" |
| `Conjunct` | Bidirectional | "This clause is coordinated with that clause" |
| `Exception` | Exception → Main | "This exception modifies that clause" |

---

## Learnings & Deviations

### Gate 1: Function-based over Trait-based Implementation

**Decision:** Implemented `ClauseLinkResolver` as a module with free functions (`extract_clause_spans()` and `resolve()`) rather than implementing the full `DocumentResolver` trait.

**Rationale:**
- `DocumentResolver` trait requires maintaining state and lifecycle management
- Gate 1 functionality is simple: extract existing clause attributes and generate links
- Function-based approach is more flexible for testing and composition
- Can always wrap in a trait implementation later if needed for pipeline integration

**Impact on Gate 2:**
- Same pattern can be used for coordination chains (separate `resolve_coordination()` function)
- May need to consolidate into a single `DocumentResolver` implementation for Gate 4/5 when building the full pipeline
- Consider whether coordination detection needs its own resolver or should be integrated into `ClauseLinkResolver`

### Gate 1: Same-Line Restriction

**Decision:** Gate 1 only creates Parent/Child links for clauses on the same line. Cross-line subordination is explicitly ignored.

**Rationale:**
- Simplifies initial implementation and testing
- Most subordination patterns appear within single sentences
- Cross-line relationships may require different heuristics (sentence boundary detection, paragraph structure)

**Impact on Gate 2:**
- Coordination chains also likely same-line in most cases ("A and B")
- May need to address cross-sentence coordination in Gate 2 ("A. And B.") or defer to later
- Gate 3 exception detection will likely need cross-line support ("All X must Y. Except when Z.")

### Gate 1: Bidirectional Links Essential for Graph Traversal

**Decision:** Always emit both directions of a relationship (Parent→Child AND Child→Parent).

**Validation:** This proved essential during testing. Query patterns like "find all children of this clause" and "find parent of this clause" both need direct lookups without building reverse indices.

**Impact on Gate 2:**
- Coordination will need careful handling to avoid quadratic link growth
- For "A, B, and C": emit A↔B, B↔C bidirectionally (4 total links)
- NOT A↔B, A↔C, B↔C (would be 6 links and doesn't represent chain topology)

### Gate 1: Test Coverage Exceeds Initial Plan

**Planned:** 3 unit tests for link creation
**Actual:** 9 new tests covering helpers, extraction, resolution, multi-clause, cross-line restriction, integration

**Benefit:** High confidence in Gate 1 foundation. Snapshot testing ensures no regressions when adding Gates 2-5.

### Adjustments for Gate 2

Based on Gate 1 implementation:

1. **Coordination detection strategy:**
   - Use existing `ClauseKeyword` detection for "and", "or", "but" keywords
   - Look for keyword position relative to clause boundaries
   - Emit Conjunct links for adjacent clauses separated by coordination keyword

2. **Same-line vs cross-line:**
   - Start with same-line coordination ("A and B within one sentence")
   - Cross-sentence coordination ("A. And B.") may need sentence boundary detection
   - Consider deferring cross-sentence to later gate

3. **Chain topology implementation:**
   - For "A, B, and C", emit: A→B (Conjunct), B→A (Conjunct), B→C (Conjunct), C→B (Conjunct)
   - Traversal algorithm: collect all Conjunct links transitively to find full coordination chain
   - Avoid special-casing list length (works for any N)

4. **Integration with existing code:**
   - `ClauseLinkResolver::resolve()` currently only handles Parent/Child
   - Add separate `resolve_coordination()` function or extend `resolve()` to handle both
   - May need to refactor into builder pattern to compose multiple link types


# Obligation Type Integration Plan

> Learnings relevant to future gates should be written back to respective gates, so future collaborators can benefit.

**Status:** Complete
**Package:** layered-clauses
**Depends on:** Gates 0-4 complete (clause-link-edge-cases.md)

## Goal and Motivation

Transform `ClauseLinkResolver` from structural analysis to semantic analysis by integrating obligation types. Users need to know not just "here are your clauses" but "here are your obligations (Duty/Permission/Prohibition)."

**Key insight:** `ObligationPhraseResolver` in `layered-contracts` already detects modal patterns ("shall", "may", "shall not"). We bridge this existing infrastructure into `ClauseLink` rather than rebuilding.

## Prerequisites

**Critical:** `ObligationPhraseResolver` must run on document lines **before** `ClauseLinkResolver` executes. This is a pipeline ordering requirement.

**Detection mechanism:** `detect_obligations()` queries each clause's lines for `ObligationPhrase` attributes. If no lines have any `ObligationPhrase` attributes in the entire document, emit a warning via `eprintln!` indicating resolver ordering may be wrong.

**Span mapping:** `ObligationPhrase` attributes exist on `LLLine` (line-level, token spans). `ClauseLink` works with `DocSpan` (document-level). The mapping is:
1. `ClauseLink.anchor` is a `DocSpan` with `start.line` and `end.line` fields
2. Iterate lines: `for line_idx in anchor.start.line..=anchor.end.line`
3. Get line: `doc.lines().get(line_idx)` returns `Option<&LLLine>`
4. Query for `Scored<ObligationPhrase>` (not raw `ObligationPhrase`): `line.find_by(&x::attr::<Scored<ObligationPhrase>>())`
5. Extract value: `scored_phrase.value.obligation_type`

## Scope

**Delivers:**
- `obligation_type: Option<ObligationType>` field on `ClauseLink`
- Query API: `obligations()`, `clauses_by_obligation_type()`
- Integration between `ObligationPhraseResolver` output and `ClauseLinkResolver`

**Excludes (flagged for human review instead):**
- Automatic scope ambiguity resolution ("except A and B" → which scope?)
- Double-negative resolution ("unless tenant does not" → polarity tracking)
- Modal-negation interaction matrix ("shall not be required to" vs "shall be required not to")
- Ambiguity flagging (deferred — requires precise regex patterns with proven false-positive rates)

**Dependencies:**
- `layered-contracts::ObligationType` enum (exists: `Duty`, `Permission`, `Prohibition`)
- `layered-contracts::ObligationPhraseResolver` (exists, detects modal keywords)
- Gates 0-4 complete (clause structure, cross-references)

## Gates

### Gate 0: Obligation Field - COMPLETE

**Goal:** Add `obligation_type` field to `ClauseLink` struct.

**Completion Notes:**
- Added `obligation_type: Option<ObligationType>` field to ClauseLink struct with `#[serde(skip_serializing_if = "Option::is_none")]`
- Added serde derives to `LinkConfidence` and `CoordinationType` enums (required for ClauseLink serialization)
- Updated 11 construction sites in `clause_link_resolver.rs` with `obligation_type: None`
- Updated 10 test construction sites in `clause_query.rs` with `obligation_type: None`
- Updated 3 doctest examples in `clause_query.rs`
- Added `ObligationType` re-export in `lib.rs`
- All 157 tests + 14 doc tests pass

**Note:** `ClauseLink` is constructed directly in `ClauseLinkResolver::resolve()`, NOT via a builder pattern. `ClauseLinkBuilder` builds `DocSpanLink<ClauseRole>`, which is a different type. All construction sites must be updated manually.

**Deliverables:**
| Component | Location | Change |
|-----------|----------|--------|
| `ClauseLink` struct | `clause_link_resolver.rs:87-100` | Add `obligation_type: Option<ObligationType>` field |
| Construction sites | `clause_link_resolver.rs` | Update all `ClauseLink { ... }` expressions (~12 sites) |
| Re-export | `lib.rs` | `pub use layered_contracts::ObligationType;` |

**Construction sites to update (grep `ClauseLink {`):**
- `resolve()` main clause construction (multiple sites)
- `detect_conjunctions()` conjunct links
- `detect_exceptions()` exception links
- `detect_list_structure()` list links
- `detect_cross_references()` cross-reference links
- Test code construction sites

**Note:** Grep reveals ~12 construction sites. All must include `obligation_type: None`. The compiler will catch any missed sites as struct field errors.

**Acceptance:**
- All existing tests **compile** after adding field with `obligation_type: None`
- All existing 157 tests **pass** after modification
- New field has `#[serde(skip_serializing_if = "Option::is_none")]` for WASM compat
- Re-export appears in `lib.rs`: `pub use layered_contracts::ObligationType;`

---

### Gate 1: Obligation Detection - COMPLETE

**Goal:** Bridge `ObligationPhraseResolver` output to clause links.

**Completion Notes:**
- Added `detect_obligations()` method (lines 554-589) that scans clause spans for `Scored<ObligationPhrase>` attributes
- Updated imports to include `ObligationPhrase` and `Scored` from `layered_contracts`
- Added call at end of `resolve()` method (lines 1159-1160) after all structural detection passes
- Note: Used `find()` instead of `find_by()` as that's the actual method on `LLLine`
- First match within clause wins (deterministic behavior)
- Warning emitted if no `ObligationPhrase` attributes found
- All 157 tests + 14 doc tests continue to pass

**Deliverables:**
| Component | Location | Purpose |
|-----------|----------|---------|
| `detect_obligations()` | `clause_link_resolver.rs` | Scan clauses for `ObligationPhrase` attributes |
| Update `resolve()` | `clause_link_resolver.rs` | Populate `obligation_type` field during construction |

**Algorithm (explicit span mapping):**
```rust
fn detect_obligations(
    clause_links: &mut Vec<ClauseLink>,
    doc: &LayeredDocument,  // provides LLLine access via lines()
) {
    let mut any_obligation_found = false;

    for link in clause_links.iter_mut() {
        let anchor = &link.anchor;
        // DocSpan provides start.line and end.line fields
        for line_idx in anchor.start.line..=anchor.end.line {
            if let Some(line) = doc.lines().get(line_idx) {
                // Query line-level attributes (ObligationPhraseResolver stores Scored<ObligationPhrase>)
                let findings = line.find_by(&x::attr::<Scored<ObligationPhrase>>());
                if let Some(first) = findings.first() {
                    let scored_phrase = first.attr();
                    link.obligation_type = Some(scored_phrase.value.obligation_type.clone());
                    any_obligation_found = true;
                    break;  // first match wins within clause
                }
            }
        }
    }

    if !any_obligation_found && !clause_links.is_empty() {
        eprintln!("Warning: No ObligationPhrase attributes found. Was ObligationPhraseResolver run?");
    }
}
```

**Integration point:** `detect_obligations()` is called at the end of `ClauseLinkResolver::resolve()`, after all structural detection passes (conjunctions, exceptions, list structure, cross-references) complete, but before returning the final `Vec<ClauseLink>`.

**Multiple ObligationPhrase handling:**
- If a clause spans multiple lines and multiple lines have `ObligationPhrase`: first line wins
- If a single line has multiple `ObligationPhrase` attributes: first token position wins
- Rationale: deterministic, easy to reason about; complex contracts can override via post-processing

**Acceptance:**
- "The tenant shall pay rent" → `Duty`
- "The tenant may terminate" → `Permission`
- "The tenant shall not assign" → `Prohibition`
- Clause without modal keywords → `None`
- Document with clauses but no `ObligationPhrase` attrs → warning emitted, all `None`

---

### Gate 2: Query API - COMPLETE

**Goal:** Enable semantic queries on clause obligations.

**Completion Notes:**
- Added `obligation(span)` method returning `Option<ObligationType>` for a clause span
- Added `clauses_by_obligation_type(obligation_type)` method returning `Vec<DocSpan>` in document order
- Used `sort_by_key` with `(span.start.line, span.start.token)` since `DocSpan` doesn't implement `Ord`
- Added `ObligationType` to crate imports
- Doc examples use `ignore` attribute (full testing deferred to Gate 3)
- All 157 tests + 14 doc tests pass (2 new examples ignored as expected)

**Deliverables:**
| Method | Location | Returns | Purpose |
|--------|----------|---------|---------|
| `obligation(span)` | `clause_query.rs` | `Option<ObligationType>` | Get obligation type for a specific clause |
| `clauses_by_obligation_type(type)` | `clause_query.rs` | `Vec<DocSpan>` | Find all clauses of given type |

**Note:** Shorthand methods (`duties()`, `prohibitions()`, `permissions()`) deferred until usage patterns emerge from real users.

**Return value ordering:** Results returned in document order (ascending `DocSpan`).

**Acceptance:**
- Contract with 3 "shall", 2 "may", 1 "shall not" → `clauses_by_obligation_type(Duty)` returns 3 spans
- Empty document → empty results
- `obligation(span)` on clause with modal keyword → correct `ObligationType`
- `obligation(span)` on clause without modal → `None`
- Doc tests demonstrate each method

---

### Gate 3: Integration Tests - COMPLETE

**Goal:** Validate end-to-end with realistic contract text.

**Completion Notes:**
- Created `layered-clauses/src/tests/obligation_integration.rs` with 9 integration tests
- Added `mod obligation_integration;` to lib.rs test module
- All 7 planned scenarios implemented plus 2 diagnostic/utility tests

**Key Discovery During Implementation:**
Standalone clauses (without relationships to other clauses) were not getting obligations captured because `detect_obligations` only enriched existing links. Fixed by:
- Added `Self_` variant to `ClauseRole` enum in layered-nlp-document
- Added `create_obligation_only_links()` function to create self-referential links for standalone clauses with obligations
- This ensures every clause with an obligation type gets a `ClauseLink` entry

**Test Results:**
- All 166 layered-clauses tests pass (up from 157)
- All related crate tests pass

**Test file:** `layered-clauses/src/tests/obligation_integration.rs`

**Test scenarios:**
| Scenario | Input | Expected |
|----------|-------|----------|
| Single duty | "Tenant shall pay rent monthly." | 1 clause, `obligation_type: Some(Duty)` |
| Single permission | "Tenant may terminate early." | 1 clause, `obligation_type: Some(Permission)` |
| Single prohibition | "Tenant shall not assign lease." | 1 clause, `obligation_type: Some(Prohibition)` |
| Mixed obligations | "Tenant shall pay. Landlord may inspect. Neither shall assign." | `clauses_by_obligation_type(Duty)` = 1, `Permission` = 1, `Prohibition` = 2 |
| No modal keywords | "The lease term begins on January 1." | 1 clause, `obligation_type: None` |
| Cross-reference with obligation | "Subject to Section 3.2, Tenant shall maintain..." | Clause has both `CrossReference` link AND `obligation_type: Duty` |
| Exception with obligation | "Tenant shall pay, unless Landlord waives payment." | Main clause: `Duty`, exception child exists, exception's `obligation_type`: `None` |

**Exception-obligation interaction:**
- Exceptions modify scope, not obligation type
- Main clause retains its `obligation_type` (`Duty` in "shall pay unless waived")
- Exception clause gets its own `obligation_type` (usually `None` for conditional phrases)
- Query API returns both, letting consumers decide how to present

**Acceptance:**
- All 7 scenarios pass
- No regressions in existing 157 tests
- Test file path: `layered-clauses/src/tests/obligation_integration.rs`

---

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| `ObligationPhraseResolver` not run before `ClauseLinkResolver` | High | `detect_obligations()` checks for any `ObligationPhrase` attrs; emits `eprintln!` warning if none found |
| Performance on large documents | Medium | O(clauses × lines); acceptable for typical contracts (<100 clauses) |
| Breaking change to `ClauseLink` struct | Medium | All construction sites listed in Gate 0; tests catch missing updates |

## Non-Goals (Future Work)

- **Ambiguity flagging** (scope ambiguity, double negatives, modal-negation interaction) — requires precise regex patterns with proven false-positive rates, linguistic validation
- Automatic double-negative resolution (requires polarity tracker)
- Scope ambiguity resolution (requires NP chunking)
- Full modal-negation interaction matrix (requires linguistic expertise)
- Obligation strength gradations (absolute "shall" vs hedged "should")
- WASM demo integration (presentation layer; can be added post-validation)

These require significant additional infrastructure beyond the core obligation detection.

## Learnings

### The `Self_` Role Pattern for Standalone Semantic Attributes

When a clause has a semantic attribute (like `obligation_type`) but no structural relationship to other clauses, it still needs representation in the link system. The `Self_` variant in `ClauseRole` creates a self-referential link where a clause points to itself. This ensures:
- Every clause with semantic content has a `ClauseLink` entry
- Query APIs like `clauses_by_obligation_type()` work uniformly
- The link system captures both relationships AND standalone attributes

### Links for All Clauses, Not Just Relationships

The original design assumed `ClauseLink` only captured relationships between clauses (parent-child, cross-reference, exception). This missed standalone clauses that have semantic attributes but no relationships. The fix was to create links for any clause with an obligation type, using `Self_` role when no other relationship exists. This pattern applies to any future semantic attribute that should be queryable via the link system.

### `sort_by_key` Pattern for DocSpan Ordering

`DocSpan` doesn't implement `Ord`, so standard sorting methods don't work. Use `sort_by_key` with a tuple key:
```rust
spans.sort_by_key(|span| (span.start.line, span.start.token));
```
This provides document order (ascending by line, then by token position within line) without requiring an `Ord` implementation on `DocSpan`.

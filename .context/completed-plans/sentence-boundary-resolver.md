# Sentence Boundary Resolver

> Learnings relevant to future gates should be written back to respective gates, so future collaborators can benefit.

**Status:** Complete

**Goal:** Unify sentence boundary detection by moving existing infrastructure from `layered-clauses` to `layered-contracts`, enabling `pronoun.rs` and `obligation.rs` to use shared implementation.

## Motivation

- `pronoun.rs` has `has_sentence_boundary_between()` (`.!?` only, 24 lines)
- `obligation.rs` has `in_same_sentence()` (`.!?;`, inverted semantics, 34 lines)
- `layered-clauses` has battle-tested `SentenceBoundaryResolver` (134 lines, 10 tests, abbreviation handling)
- Duplication causes inconsistent behavior (semicolon handling differs)

## Scope

**In Scope:**
- Move `SentenceBoundary`, `SentenceConfidence`, `SentenceBoundaryResolver` from layered-clauses → layered-contracts
- Backward-compatible re-exports from layered-clauses
- Replace duplicated logic in pronoun.rs and obligation.rs
- Add `include_semicolons` configuration for obligation.rs compatibility

**Out of Scope:**
- Cross-line detection (ClauseLinkResolver already handles this at consumer level)
- New punctuation handling (em-dash, ellipsis, etc.)
- Caching layer

**Dependencies:**
- None (this unblocks clause-link-edge-cases.md Gate 0)

---

## Gate: Infrastructure Move [COMPLETE]

**Goal:** Move SentenceBoundary infrastructure from layered-clauses to layered-contracts without breaking existing consumers.

**Deliverables:**

| File | Action |
|------|--------|
| `layered-contracts/src/sentence_boundary.rs` | Create - move types + resolver from layered-clauses |
| `layered-contracts/src/lib.rs` | Export `SentenceBoundary`, `SentenceConfidence`, `SentenceBoundaryResolver` |
| `layered-clauses/src/sentence_boundary.rs` | Reduce to re-exports from layered-contracts |
| `layered-clauses/src/lib.rs` | Maintain existing exports (now re-exports) |
| `layered-contracts/Cargo.toml` | No changes needed (already has layered-nlp-document) |
| `layered-clauses/Cargo.toml` | Already depends on layered-contracts |

**Configuration Addition:**
```rust
pub struct SentenceBoundaryResolver {
    abbreviations: HashSet<&'static str>,
    include_semicolons: bool,  // NEW: for obligation.rs compatibility
}

impl SentenceBoundaryResolver {
    pub fn with_semicolons() -> Self { ... }  // NEW

    /// Check if there's a sentence boundary between two selections (on same line)
    pub fn has_boundary_between(&self, earlier: &LLSelection, later: &LLSelection) -> bool {
        // Scan tokens between selections for sentence-ending punctuation
    }
}
```

**Acceptance:**
- All 10 existing sentence_boundary tests pass in layered-clauses
- layered-contracts typecheck passes
- No API changes for existing layered-clauses consumers

**Completion Notes:**
- Created sentence_boundary.rs in layered-contracts (with include_semicolons config, with_semicolons() constructor, has_boundary_between() method)
- Added 10 tests for new functionality
- Re-exported from layered-clauses for backward compatibility
- All 166 layered-clauses + 559 layered-contracts tests pass

---

## Gate: Pronoun Integration [COMPLETE]

**Goal:** Replace `PronounResolver::has_sentence_boundary_between()` with shared resolver.

**Current:** `pronoun.rs:137-160` - private method scanning for `.!?`

**Target:** Use `SentenceBoundaryResolver::new()` (without semicolons, matching current behavior)

**Pattern:**
```rust
// Before: self.has_sentence_boundary_between(earlier, later)
// After:
let resolver = SentenceBoundaryResolver::new();
resolver.has_boundary_between(earlier, later)
```

**Acceptance:**
- Existing pronoun tests pass (especially `same_sentence` confidence tests)
- Private `has_sentence_boundary_between` deleted
- No behavior change (`.!?` only, no semicolons)

**Completion Notes:**
- Replaced has_sentence_boundary_between() at line 413 with SentenceBoundaryResolver::new()
- Deleted private method (26 lines removed)
- Fixed bug in has_boundary_between() that wasn't properly checking between selections

---

## Gate: Obligation Integration [COMPLETE]

**Goal:** Replace `ObligationPhraseResolver::in_same_sentence()` with shared resolver.

**Current:** `obligation.rs:517-551` - private method scanning for `.!?;` with inverted semantics

**Target:** Use `SentenceBoundaryResolver::with_semicolons()` with inverted return

**Pattern:**
```rust
// Before: self.in_same_sentence(sel_a, sel_b)
// After:
let resolver = SentenceBoundaryResolver::with_semicolons();
!resolver.has_boundary_between(sel_a, sel_b)
```

**Acceptance:**
- Existing obligation tests pass (especially `condition_scoped_to_same_sentence`)
- Private `in_same_sentence` deleted (`selection_is_before` retained - used by 11+ other methods)
- Semicolon behavior preserved (`;` is a boundary)

**Completion Notes:**
- Replaced in_same_sentence() call in find_conditions() with SentenceBoundaryResolver::new().with_semicolons()
- Deleted in_same_sentence method
- Kept selection_is_before helper (used by 11+ other methods)
- Semicolon behavior preserved

---

## Risks

| Risk | Mitigation |
|------|-----------|
| Abbreviation handling differs | Current pronoun/obligation don't handle abbreviations; shared resolver does - this is an improvement |
| Confidence levels unused | pronoun/obligation need boolean, not confidence - expose `has_boundary_between()` boolean API |
| Re-export churn | Keep layered-clauses exports identical, just backed by re-exports |

## Non-Goals

- Full sentence segmentation (paragraph → sentences)
- Legal prose abbreviation handling (e.g., "Corp.", "Inc." in party names)
- Caching/memoization

## Learnings

- `has_boundary_between()` must properly walk token-by-token and stop when reaching `later` selection
- The original implementation had a bug using split_with instead of proper forward iteration
- `selection_is_before` cannot be deleted from obligation.rs - it serves multiple purposes beyond sentence detection

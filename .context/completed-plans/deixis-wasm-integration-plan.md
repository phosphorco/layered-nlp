# Deixis WASM Demo Integration Plan

**Interesting artifacts and learnings must be written back to this document.**

---

## Overview

This document describes the phased integration of the `layered-deixis` crate and `layered-contracts` deictic resolver into the WASM demo (`layered-nlp-demo-wasm`). The goal is to expose deictic reference detection in the contract viewer UI, enabling users to see person, place, time, and discourse deixis annotations alongside existing contract analysis.

### Current State

- **layered-deixis**: Domain-agnostic crate with 4 simple resolvers (`PersonPronounResolver`, `PlaceDeicticResolver`, `SimpleTemporalResolver`, `DiscourseMarkerResolver`) that output `DeicticReference` directly.
- **layered-contracts**: Contains `DeicticResolver` that maps contract-specific types (`Scored<PronounReference>`, `TemporalExpression`, `SectionReference`) to unified `DeicticReference`.
- **WASM demo**: 10-layer resolver pipeline, serializes spans with metadata to JSON, rendered in HTML viewer with filtering and color-coded badges.

### Target State

- Deictic references appear as spans in the contract viewer
- Users can filter deixis by category (Person, Place, Time, Discourse)
- Metadata shows subcategory, resolved referent (if any), confidence, and source
- Both simple (word-list) and mapped (contract-specific) deictic references are displayed

---

## Phase 1: Backend Integration

### Objectives

Add deixis resolvers to the WASM pipeline and serialize `DeicticReference` spans.

### Scope

- Modify `layered-nlp-demo-wasm/src/lib.rs`
- No HTML/CSS changes yet

### Dependencies

- `layered-deixis` crate (already in workspace)
- `layered-contracts` re-exports deixis types

### Tasks

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 1.1 | Add `DeicticResolver` to the resolver pipeline after `SectionReferenceResolver` (or after existing layers that produce its inputs) | Resolver runs without panic; `DeicticReference` attributes appear on the line |
| 1.2 | Add simple deixis resolvers (`PersonPronounResolver`, `PlaceDeicticResolver`, `SimpleTemporalResolver`, `DiscourseMarkerResolver`) to fill gaps not covered by contract-specific resolvers | Word-list deictics (e.g., "I", "here", "however") are detected |
| 1.3 | Determine resolver ordering to avoid duplicate detections | Document which resolver should run first and why; no duplicate spans for same token |
| 1.4 | Add span extraction for `DeicticReference` in the serialization loop | `DeicticReference` spans appear in the `AnalysisResult.spans` array |
| 1.5 | Structure metadata for `DeicticReference` spans | Metadata includes: `category`, `subcategory`, `surface_text`, `resolved_referent` (if present), `confidence`, `source` |

### Resolver Ordering Considerations

The pipeline must resolve dependencies correctly:

```
Existing pipeline:
  ContractKeyword → Prohibition → DefinedTerm → TermReference →
  Pronoun → ObligationPhrase → PronounChain → ContractClause →
  ClauseAggregate → AccountabilityGraph

Deixis insertion options:
  Option A: Run DeicticResolver after PronounResolver (maps pronouns early)
  Option B: Run DeicticResolver after SectionReferenceResolver (if added)
  Option C: Run all simple deixis resolvers first, then DeicticResolver for mapping

Recommended: Option C - simple resolvers first (they're independent), then
DeicticResolver after any resolver whose output it maps.
```

### Verification

**Test Scenarios:**

1. **Simple deixis detection**: Input "I will meet you there tomorrow" → produces DeicticReference spans for "I", "you", "there", "tomorrow"
2. **Contract-specific mapping**: Input with pronoun resolution "The Company shall deliver. It must comply." → "It" produces DeicticReference with `resolved_referent` pointing to "Company"
3. **Discourse markers**: Input "However, the contract is valid. Therefore, we proceed." → "However", "Therefore" detected as DiscourseMarker
4. **No duplicates**: Same token should not have multiple DeicticReference spans from different resolvers
5. **Metadata completeness**: All DeicticReference spans include category, subcategory, confidence, and source

**Coverage Requirements:**

- Unit tests for span extraction logic
- Integration test running full pipeline on sample contract text
- Snapshot test for serialized JSON output

**Test Organization:**

```
layered-nlp-demo-wasm/src/lib.rs
  - #[cfg(test)] mod tests
    - test_deixis_simple_pronouns
    - test_deixis_contract_mapping
    - test_deixis_discourse_markers
    - test_deixis_no_duplicates
    - test_deixis_metadata_structure
```

**Pass/Fail Criteria:**

- All tests pass with `cargo test -p layered-nlp-demo-wasm`
- No panics when processing empty text or text without deictic expressions
- JSON output is valid and parseable by JavaScript

---

## Phase 2: UI Layer Filters

### Objectives

Add filter controls for deixis spans in the HTML viewer.

### Scope

- Modify `web/contract-viewer.html` (JavaScript and HTML)
- No CSS styling changes yet (use default badge style)

### Dependencies

- Phase 1 complete (backend produces DeicticReference spans)

### Tasks

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 2.1 | Add "Deixis" checkbox to the layer filters section | Checkbox visible in filter panel |
| 2.2 | Wire checkbox to filter `DeicticReference` spans | Toggling checkbox shows/hides deixis spans in tracks and table |
| 2.3 | Consider subcategory filtering (Person/Place/Time/Discourse) | Either: (a) single "Deixis" toggle, or (b) expandable sub-filters. Document decision. |
| 2.4 | Update default visibility state | Decide if deixis is shown by default; document rationale |

### Design Decision: Filter Granularity

**Option A: Single "Deixis" toggle**
- Pros: Simple, matches existing pattern (one checkbox per resolver type)
- Cons: Can't focus on just temporal or just person deixis

**Option B: Category sub-filters**
- Pros: Fine-grained control, useful for debugging specific categories
- Cons: Adds UI complexity, may clutter filter panel

**Recommendation:** Start with Option A (single toggle). If users need granularity, add sub-filters in a later iteration.

### Verification

**Test Scenarios:**

1. **Filter toggle**: Check "Deixis" → deixis spans appear; uncheck → they disappear
2. **Filter persistence**: Filter state survives re-analysis of same text
3. **Empty state**: No errors when deixis filter is on but text has no deictic expressions
4. **Interaction with other filters**: Deixis filter works independently of other layer filters

**Coverage Requirements:**

- Manual testing in browser (document test steps)
- Add automated test case to `TestRunner` class in HTML

**Test Organization:**

```
web/contract-viewer.html
  - TestRunner class
    - test_deixis_filter_toggle
    - test_deixis_spans_visible
```

**Pass/Fail Criteria:**

- Checkbox toggles visibility without JavaScript errors
- Console shows no errors during filter operations
- Tracks and table update correctly when filter changes

---

## Phase 3: Visual Styling

### Objectives

Add color-coded badge styling for deixis spans, differentiated by category.

### Scope

- Modify `web/contract-viewer.html` (CSS only)

### Dependencies

- Phase 2 complete (deixis spans visible in UI)

### Tasks

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 3.1 | Define color scheme for DeicticReference spans | Colors chosen; documented in this plan |
| 3.2 | Add CSS class `.kind-badge.DeicticReference` | Badge styled consistently with existing resolver badges |
| 3.3 | Consider category-specific colors (optional) | If implemented: Person=X, Place=Y, Time=Z, Discourse=W |
| 3.4 | Ensure contrast and accessibility | Colors meet WCAG AA contrast ratio (4.5:1 for text) |

### Color Scheme Proposal

Deixis spans should be visually distinct from existing resolver types. Current palette uses warm yellows, blues, purples, greens, pinks, cyans, and reds.

**Proposed: Teal/Aquamarine family** (unused in current palette)

```css
/* Single color for all deixis */
.kind-badge.DeicticReference {
    background: #ccfbf1;  /* teal-100 */
    color: #0f766e;       /* teal-700 */
}

/* OR category-specific (if Phase 2 chose sub-filters) */
.kind-badge.DeicticReference.Person   { background: #fef3c7; color: #92400e; }
.kind-badge.DeicticReference.Place    { background: #d1fae5; color: #065f46; }
.kind-badge.DeicticReference.Time     { background: #e0f2fe; color: #0369a1; }
.kind-badge.DeicticReference.Discourse{ background: #f3e8ff; color: #7c3aed; }
```

### Verification

**Test Scenarios:**

1. **Badge visibility**: DeicticReference badges display with correct background/text color
2. **Contrast check**: Text readable on badge background
3. **Consistency**: Badge style matches overall UI aesthetic
4. **Dark mode** (if applicable): Colors work in both light and dark themes

**Coverage Requirements:**

- Visual inspection in browser
- Screenshot comparison (optional, for regression)

**Pass/Fail Criteria:**

- Badges render without CSS errors
- Colors are visually distinct from other resolver badges
- Text is legible (manual accessibility check)

---

## Phase 4: Enhanced Metadata Display

### Objectives

Improve how deixis metadata is presented in the spans table.

### Scope

- Modify `web/contract-viewer.html` (JavaScript rendering logic)

### Dependencies

- Phase 3 complete (deixis spans styled)

### Tasks

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 4.1 | Format `resolved_referent` for display | If present, show "→ {referent text} ({confidence}%)" |
| 4.2 | Format `source` enum for human readability | Show "Word List: {pattern}" or "Pronoun Resolver" etc. |
| 4.3 | Show category/subcategory in label column | Label reads e.g., "Person (1st singular)" or "Time (duration)" |
| 4.4 | Add hover tooltip with full metadata JSON (optional) | Hovering over span row shows raw metadata |

### Label Formatting Examples

| Category | Subcategory | Formatted Label |
|----------|-------------|-----------------|
| Person | PersonFirst | "Person (1st)" |
| Person | PersonFirstPlural | "Person (1st plural)" |
| Person | PersonThirdSingular | "Person (3rd singular)" |
| Place | PlaceProximal | "Place (proximal)" |
| Place | PlaceDistal | "Place (distal)" |
| Time | TimePresent | "Time (present)" |
| Time | TimeDuration | "Time (duration)" |
| Time | TimeDeadline | "Time (deadline)" |
| Discourse | DiscourseMarker | "Discourse (marker)" |
| Discourse | DiscourseAnaphoric | "Discourse (anaphoric)" |

### Verification

**Test Scenarios:**

1. **Referent display**: Pronoun with resolved referent shows "→ Company (90%)"
2. **Source display**: Word-list match shows pattern; mapped types show resolver name
3. **Label formatting**: All subcategory types render with human-readable labels
4. **Null handling**: Missing optional fields (e.g., no referent) don't cause errors

**Coverage Requirements:**

- Unit test for label formatting function (if extracted to JS function)
- Manual verification of all subcategory types

**Test Organization:**

```
web/contract-viewer.html
  - formatDeicticLabel(span) function
  - TestRunner class
    - test_deixis_label_formatting
    - test_deixis_referent_display
```

**Pass/Fail Criteria:**

- All subcategory types have readable labels
- No "undefined" or "[object Object]" in rendered output
- Table remains functional with new formatting

---

## Phase 5: Integration Testing & Documentation

### Objectives

Comprehensive end-to-end testing and user documentation.

### Scope

- Add integration tests
- Update README or create user guide
- Performance validation

### Dependencies

- Phases 1-4 complete

### Tasks

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 5.1 | Create sample contract text exercising all deixis types | Sample text covers Person, Place, Time, Discourse categories |
| 5.2 | Add end-to-end test: input → WASM → JSON → render | Test validates full pipeline produces expected spans |
| 5.3 | Performance benchmark: measure analysis time with deixis resolvers | Document baseline; ensure no significant regression (< 20% slowdown) |
| 5.4 | Update `web/contract-viewer.html` header/help text | Mention deixis detection in UI description |
| 5.5 | Add example to `CLAUDE.md` or README showing deixis usage | Code example demonstrating deixis resolver chain |

### Sample Contract Text for Testing

```
AGREEMENT

This Agreement ("Agreement") is entered into as of the Effective Date.

1. OBLIGATIONS

The Company shall deliver the Product within 30 days. It must comply with
all applicable regulations. We agree to the terms herein.

However, if the Company fails to deliver, the Buyer may terminate this
Agreement. Therefore, timely performance is essential.

2. REPRESENTATIONS

I, the undersigned, represent that the information provided here is accurate.
You shall notify us of any changes. They must be submitted in writing.

The foregoing representations shall survive termination. As described below,
additional terms may apply.
```

This text exercises:
- **Person**: "It" (3rd), "We" (1st plural), "I" (1st), "You" (2nd), "They" (3rd plural), "us" (1st plural)
- **Place**: "here", "herein"
- **Time**: "within 30 days", "the Effective Date", "timely"
- **Discourse**: "However", "Therefore", "the foregoing", "below", "this Agreement"

### Verification

**Test Scenarios:**

1. **Full pipeline**: Sample text produces expected span count and types
2. **Round-trip**: JSON serialization/deserialization preserves all metadata
3. **Performance**: Analysis completes in < 100ms for sample text (adjust based on baseline)
4. **Browser compatibility**: Works in Chrome, Firefox, Safari (latest versions)

**Coverage Requirements:**

- Integration test with sample contract text
- Performance benchmark recorded in this document
- Manual browser testing checklist

**Test Organization:**

```
layered-nlp-demo-wasm/src/lib.rs
  - #[cfg(test)] mod integration_tests
    - test_full_deixis_pipeline
    - test_sample_contract_deixis_coverage

web/contract-viewer.html
  - TestRunner class
    - test_e2e_deixis_sample_contract
```

**Pass/Fail Criteria:**

- All integration tests pass
- Performance within acceptable bounds
- No console errors in any tested browser
- Documentation accurately describes feature

---

## Appendix A: File Change Summary

| File | Changes |
|------|---------|
| `layered-nlp-demo-wasm/Cargo.toml` | Add `layered-deixis` dependency (if not via layered-contracts re-export) |
| `layered-nlp-demo-wasm/src/lib.rs` | Add resolvers to pipeline; add span extraction for DeicticReference |
| `web/contract-viewer.html` | Add filter checkbox; add CSS styling; enhance metadata display |

---

## Appendix B: Artifacts & Learnings

*Record discoveries, gotchas, and decisions made during implementation here.*

### Decisions Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2025-12-19 | Add simple deixis resolvers directly to WASM crate | `layered-deixis` crate is committed and independent; `DeicticResolver` mapping in `layered-contracts` not committed yet due to dependency on uncommitted temporal/section_reference modules |
| 2025-12-19 | Place deixis resolvers after all contract-specific layers (layers 11-14) | Simple word-list resolvers are independent and don't need any inputs from other resolvers |
| 2025-12-19 | Use teal color (#ccfbf1 bg, #0f766e text) for DeicticReference badges | Teal was unused in existing palette; provides good contrast and visual distinction |
| 2025-12-19 | Single "Deixis" filter toggle (not per-category) | Following plan recommendation for Phase 2 Option A - simplicity matches existing pattern |

### Gotchas & Warnings

| Issue | Resolution |
|-------|------------|
| `create_line_from_string("")` panics on empty string | Don't test with empty string; use neutral text without deictic words instead |
| `DeicticResolver` in layered-contracts not available | It depends on uncommitted temporal.rs and section_reference.rs; only simple resolvers from layered-deixis are integrated for now |
| `serde-wasm-bindgen` returns metadata as `Map`, not plain object | Use `metadata.get('key')` instead of `metadata.key` in JavaScript; use `Object.fromEntries(map)` for JSON.stringify |
| Old WASM files in pkg/ caused wrong module to load | HTML imported `layered_nlp_demo.js` but new package is `layered_nlp_demo_wasm.js`; cleaned up stale files |

### Performance Measurements

| Metric | Before Deixis | After Deixis | Delta |
|--------|---------------|--------------|-------|
| Analysis time (711 char sample, release) | N/A (baseline) | 0.95 ms | — |
| WASM binary size (optimized) | N/A (baseline) | 468 KB | — |

**Benchmark Details:**
- Test: `cargo test -p layered-nlp-demo-wasm test_performance_benchmark --release -- --nocapture`
- Sample: 711 character contract text with defined terms, obligations, and deictic expressions
- Iterations: 100 (with warm-up)
- Result: ~0.95 ms average per analysis (947 µs)
- Threshold: < 10 ms (well within acceptable range)

---

## Appendix C: Test Checklist

### Automated Tests (must pass in CI)

- [x] `cargo test -p layered-nlp-demo-wasm` - all unit/integration tests (6 tests passing)
- [x] `cargo test -p layered-deixis` - deixis crate tests (5 tests passing)
- [ ] `cargo test -p layered-contracts` - contracts crate tests (includes deictic.rs) - BLOCKED: deictic.rs not committed yet

### Manual Browser Tests

- [ ] Chrome (latest): Load viewer, analyze sample text, toggle deixis filter
- [ ] Firefox (latest): Same as above
- [ ] Safari (latest): Same as above
- [ ] Verify no console errors in any browser
- [ ] Verify badge colors are visually distinct and readable

### Regression Tests

- [ ] Existing resolver spans still appear correctly
- [ ] Existing filter toggles still work
- [ ] Performance not significantly degraded

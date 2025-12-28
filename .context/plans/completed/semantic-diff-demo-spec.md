# Semantic Diff WASM Demo Specification

> **Interesting artifacts and learnings must be written back to this document.**

## Motivation

Text diffs on contracts (e.g., `git diff` or Word's track changes) surface every textual change but don't tell you which changes actually matter legally. A "shall" → "may" buried in a paragraph of reformatting is easy to miss but fundamentally changes the obligation.

This demo showcases the `DocumentAligner` and `SemanticDiffEngine` capabilities through an interactive browser-based tool. Users paste two contract versions and see:
- **Aligned sections** with match confidence
- **Semantic changes** (obligation modals, term definitions, temporal expressions)
- **Risk classification** and **party impact** for each change

### Audience

- **Engineers** learning how to use the semantic diff pipeline
- **Design partners** evaluating our technology for contract comparison use cases

### Non-Goals

- Production-ready UI/UX
- CLI tooling
- External review/hint workflows (future enhancement)
- Token-level text highlighting within sections (future enhancement)

---

## Design Decisions

| Decision | Rationale |
|----------|-----------|
| Use `Pipeline::standard()` | Provides all resolvers used by `SemanticDiffEngine` (keywords, terms, obligations, headers) without the overhead of `Pipeline::enhanced()`. Pronouns, recitals, and section references are not currently consumed by the diff engine. |
| 50,000 character input limit | Keeps worst-case runtime/memory manageable in browser. Exceeding returns error, no truncation. |
| JS-side token diff (future) | No existing text diff in Rust. If added later, use a small JS diff library on the short snippets in `SemanticChange` payloads. |
| No new Rust dependencies | Gate 0 reuses existing `layered_contracts` crates. Keep gzipped WASM under ~2 MB. |
| Warnings displayed separately | Split/Merged/Moved alignments produce warnings, not semantic changes. Display in distinct panel. |
| `apply_hints` is stretch goal | API exists and works, but UI for interactive hints is beyond demo scope. |

---

## Implementation Status

| Gate | Status | Notes |
|------|--------|-------|
| Gate 0: WASM API | ✅ Complete | `compare_contracts()` exposed, 10 tests passing, 240KB gzipped WASM |
| Gate 1: Basic Demo Page | ✅ Complete | `web/contract-diff.html` with two-pane input, summary stats, change list |
| Gate 2: Section Comparison View | ✅ Complete | Two-column layout, collapsible input, expand/collapse all, scroll-to-section |
| Gate 3: Trust & Diff Highlighting | ✅ Complete | Word-level diff via diff-match-patch, ExactMatch relabeling, inline impact panels |
| Gate 4: Navigation & Polish | ✅ Complete | Section outline sidebar, risk filters, keyboard nav (j/k), URL hash deep links |

### Gate 3 Learnings

**What worked:**
- `diff-match-patch` CDN integration was straightforward; `diff_cleanupSemantic()` produces readable diffs
- Client-side relabeling via `getTrueAlignmentType()` cleanly overrides backend "ExactMatch" → "Modified"
- Inline impact panels connect semantic analysis to visual context

**Issues fixed (post-review):**
1. ✅ **Alignment badge count mismatch** — Added `recalculateAlignmentCounts()` to compute corrected counts client-side
2. ✅ **Dead code** — Removed unused `createSemanticBadges()` function and CSS
3. ✅ **CDN dependency** — Added fallback that creates no-op `diff_match_patch` if CDN fails
4. ✅ **Click handler on ExactMatch rows** — Moved onclick to header only, text is now selectable

### Gate 4 Learnings

**What worked:**
- CSS Grid layout for sidebar + main content works well at 220px + 1fr
- Intersection Observer scroll-sync provides smooth active state updates in sidebar
- Risk-colored dots provide instant visual scanning of contract health
- Keyboard navigation (j/k) with focus ring improves power-user workflow
- URL hash deep linking with `window.history.replaceState()` enables bookmarking

**Implementation details:**
- `populateSidebar()` iterates aligned_pairs, uses `getRiskLevelForSection()` to determine dot color
- `applyFilters()` uses CSS class `hidden-unchanged` for both section rows and sidebar items
- Filter checkboxes sync with "Hide Unchanged" toggle bidirectionally
- `navigateToSection()` handles focus ring, scroll, URL hash, and sidebar active state

## Related Documents

- **[UI Design Specification](semantic-diff-ui-design.md)** — Detailed mockups, visual patterns, and UX requirements from design review

---

## Current Codebase State

Based on Oracle review (December 2024):

| Component | Status |
|-----------|--------|
| `DocumentAligner` | ✅ Implemented with align, hints, confidence signals |
| `SemanticDiffEngine` | ✅ Core works (modal, term, temporal, structural changes) |
| `SemanticDiffEngine::apply_hints` | ✅ Implemented (not exposed in WASM for this demo) |
| `AlignmentType::Split/Merged` | ⚠️ Types exist, algorithm not implemented |
| WASM demo infrastructure | ✅ Exists (`layered-nlp-demo-wasm`, `web/contract-viewer.html`) |
| Semantic diff in WASM | ❌ Not exposed yet |

---

## Gate 0: WASM Semantic Diff API

### Objectives

Expose semantic diff functionality from the WASM module so it can be called from JavaScript.

### Scope

- New WASM function `compare_contracts(original: &str, revised: &str) -> JsValue`
- Returns JSON-serializable diff result or structured error
- Uses existing `Pipeline::standard()` → `DocumentAligner` → `SemanticDiffEngine` flow
- Enforces 50,000 character per-document input limit

### Dependencies

- Existing `layered-nlp-demo-wasm` crate
- `DocumentAligner`, `SemanticDiffEngine`, `DocumentStructureBuilder` from `layered-contracts`

### Task List

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 0.1 | Add `layered-contracts` dependency to `layered-nlp-demo-wasm/Cargo.toml` | Crate compiles with new dependency |
| 0.2 | Implement `compare_contracts(original, revised)` WASM function | Function is callable from JavaScript |
| 0.3 | Validate input length (≤ 50,000 chars each) | Oversize input returns `input_too_large` error |
| 0.4 | Run `Pipeline::standard()` on both documents | Both documents processed without panic |
| 0.5 | Build `DocumentStructure` for each document | Structures contain detected sections |
| 0.6 | Run `DocumentAligner::align()` | Returns `AlignmentResult` with aligned pairs |
| 0.7 | Run `SemanticDiffEngine::compute_diff()` | Returns `SemanticDiffResult` |
| 0.8 | Serialize result to JSON via `serde_wasm_bindgen` | JavaScript receives parseable object |
| 0.9 | Define error JSON structure | Errors contain `{ error: { code, message, details? } }` |
| 0.10 | Handle empty/invalid input gracefully | Returns `invalid_input` error, no panic |
| 0.11 | Catch panics and return `internal_error` | No unhandled panics escape to JS |

### Error Codes

| Code | Condition | Status |
|------|-----------|--------|
| `invalid_input` | Empty text, missing required input | ✅ Implemented |
| `input_too_large` | Either document exceeds 50,000 characters | ✅ Implemented |
| `alignment_failed` | Pipeline or structural alignment failed | ✅ Implemented |
| `diff_failed` | Semantic diff failed after alignment | ⚠️ Not triggered (SemanticDiffEngine doesn't fail currently) |
| `internal_error` | Unexpected panic or bug | ✅ Panics logged to console via `console_error_panic_hook` |

### Output JSON Schema (TypeScript)

```typescript
// Success response
interface SemanticDiffResult {
  changes: SemanticChange[];
  summary: DiffSummary;
  party_summaries: PartySummaryDiff[];
  warnings: string[];  // Split/Merged/Moved notes
}

interface SemanticChange {
  change_id: string;
  change_type: SemanticChangeType;
  risk_level: "Critical" | "High" | "Medium" | "Low";
  party_impacts: PartyImpact[];
  confidence: number;
  explanation: string;
}

interface DiffSummary {
  total_changes: number;
  by_risk: { critical: number; high: number; medium: number; low: number };
  by_type: Record<string, number>;
}

// Error response
interface DiffError {
  error: {
    code: string;
    message: string;
    details?: Record<string, unknown>;
  };
}
```

### Verification

**Test Scenarios:**
1. Two identical contracts → no semantic changes, all sections ExactMatch
2. Contract with "shall" → "may" change → `ObligationModal` change detected
3. Contract with term redefinition → `TermDefinition` change with affected references
4. Contract with section added → `SectionAdded` change
5. Empty original or revised → graceful error response
6. Non-contract text → processes without panic (may have empty results)

**Required Coverage:**
- All `SemanticChangeType` variants can be serialized to JSON
- Alignment types (ExactMatch, Modified, Deleted, Inserted, Renumbered) appear in output
- Risk levels and party impacts included in change objects

**Pass/Fail Criteria:**
- PASS: `wasm-pack build` succeeds
- PASS: JavaScript can call function and parse response
- PASS: All test scenarios produce expected output structure
- FAIL: Any panic during processing
- FAIL: JSON parsing fails in JavaScript

**Test Organization:**
- Unit tests: `layered-nlp-demo-wasm/src/lib.rs` (`#[cfg(test)]` module)
- Naming: `test_compare_*`

---

## Gate 1: Basic Demo Page

### Objectives

Create a new HTML page for semantic diff demonstration with two text areas and diff output.

### Scope

- New `web/contract-diff.html` file
- Two-pane layout: original (left) and revised (right)
- "Compare" button triggers WASM diff
- Results displayed in structured format below

### Dependencies

- Gate 0 complete (WASM API available)

### Task List

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 1.1 | Create `web/contract-diff.html` with basic structure | Page loads in browser |
| 1.2 | Add two textarea inputs (original, revised) with character counter | Both textareas visible, counter shows chars/50000 |
| 1.3 | Add "Compare" button | Button triggers JavaScript handler |
| 1.4 | Load WASM module on page load | Console shows successful module load |
| 1.5 | Display loading indicator during WASM processing | Spinner/message appears while computing |
| 1.6 | Call `compare_contracts()` on button click | WASM function executes, loading indicator hides on completion |
| 1.7 | Display alignment summary | Shows count of ExactMatch, Modified, Added, Removed sections |
| 1.8 | Display semantic changes list | Each change shows type, location, description |
| 1.9 | Display diff summary statistics | Total changes, breakdown by risk level |
| 1.10 | Display alignment warnings panel | Shows Split/Merged/Moved notes separately from changes |
| 1.11 | Display error messages from WASM | Shows user-friendly error for invalid_input, input_too_large, etc. |
| 1.12 | Add sample contract button(s) | Populates textareas with example contracts |
| 1.13 | Style with existing CSS patterns from contract-viewer.html | Consistent look and feel |

### Verification

**Test Scenarios:**
1. Load page → no JavaScript errors in console
2. Click Compare with empty inputs → shows `invalid_input` error message
3. Paste contract exceeding 50,000 chars → shows `input_too_large` error
4. Paste two contracts, click Compare → loading indicator appears, then results
5. Sample button → populates both textareas with valid contracts
6. Modify one textarea, Compare again → new results replace old
7. Warnings present → warnings panel visible with count badge
8. No warnings → warnings panel hidden or shows "None"

**Required Coverage:**
- All alignment types render without error
- All semantic change types render without error
- Summary statistics match actual change counts
- Error messages display for all error codes
- Warnings panel displays when warnings present

**Pass/Fail Criteria:**
- PASS: Page loads and WASM initializes
- PASS: Compare produces visible output for sample contracts
- PASS: No console errors during normal operation
- FAIL: Page fails to load WASM module
- FAIL: Compare button does nothing

**Test Organization:**
- Manual browser testing (documented checklist)
- Optional: Add to `TestRunner` class pattern from contract-viewer.html

---

## Gate 2: Section Comparison View

### Objectives

Replace the current "list of changes" output with an interactive section-by-section comparison view that shows aligned sections side-by-side with semantic annotations.

### Design Rationale

The current output shows **what** changed but not **where**. Users see "Section Added: SECTION:2.3" but can't see the actual text or its context. A section-based comparison view:

1. **Anchors changes in context** - see the change alongside surrounding text
2. **Shows alignment visually** - matched sections appear side-by-side
3. **Reduces cognitive load** - unified view for identical sections, expanded view for changes
4. **Connects semantics to text** - clicking a change scrolls to and highlights the relevant section

### Layout

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ [Input area - collapsible after comparison]                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│ DIFF SUMMARY: 2 changes (0 critical, 0 high, 1 medium, 1 low)              │
├────────────────────────────────┬────────────────────────────────────────────┤
│ ORIGINAL                       │ REVISED                                    │
├────────────────────────────────┼────────────────────────────────────────────┤
│ ARTICLE I: DEFINITIONS         │ ARTICLE I: DEFINITIONS                     │
│ ────────────────────────────── │ ──────────────────────────────             │
│ Section 1.1 "Confidential..."  │ Section 1.1 "Confidential..."  [MODIFIED]  │
│ (unified - identical)          │ + "technical" ← Term Narrowed (LOW)        │
├────────────────────────────────┼────────────────────────────────────────────┤
│ Section 1.2 "Receiving..."     │ Section 1.2 "Receiving..."                 │
│ (unified - identical)          │ (unified - identical)                      │
├────────────────────────────────┼────────────────────────────────────────────┤
│ ARTICLE II: OBLIGATIONS        │ ARTICLE II: OBLIGATIONS                    │
│ ────────────────────────────── │ ──────────────────────────────             │
│ Section 2.1 ... shall protect  │ Section 2.1 ... may protect    [MODIFIED]  │
│                                │ ← Modal Weakened (HIGH)                    │
├────────────────────────────────┼────────────────────────────────────────────┤
│ Section 2.2 ... shall not...   │ Section 2.2 ... shall not...               │
│ (unified - identical)          │ (unified - identical)                      │
├────────────────────────────────┼────────────────────────────────────────────┤
│ [empty]                        │ Section 2.3 ... shall return   [INSERTED]  │
│                                │ ← New Obligation (MEDIUM)                  │
├────────────────────────────────┼────────────────────────────────────────────┤
│ ARTICLE III: TERM              │ ARTICLE III: TERM                          │
│ Section 3.1 ... two (2) years  │ Section 3.1 ... three (3) years [MODIFIED] │
│                                │ ← Temporal Change (MEDIUM)                 │
└────────────────────────────────┴────────────────────────────────────────────┘
```

### Scope

**In Scope:**
- Side-by-side section rendering based on alignment pairs
- Visual distinction: ExactMatch (unified/collapsed), Modified (side-by-side), Inserted (right only), Deleted (left only)
- Semantic change badges inline with affected sections
- Click-to-expand for collapsed identical sections
- Collapsible input area after comparison

**Out of Scope:**
- Word-level diff highlighting within sections (future: use JS diff library)
- Drag-to-resize panes
- Synchronized scroll between panes

### Dependencies

- Gate 1 complete (WASM API + basic page structure)
- Need to expose alignment pairs with section text excerpts from WASM

### API Enhancement Required

Current `compare_contracts()` returns semantic changes but not the aligned section texts. We'll use the simpler **text excerpts approach** (not offsets) since:
- `AlignmentCandidate` already uses this pattern with `original_excerpts`/`revised_excerpts`
- `DocumentAligner::extract_excerpt()` already exists and works
- Avoids UTF-8 vs UTF-16 offset complexity in JS

```typescript
interface FrontendAlignedPair {
  alignment_type: "ExactMatch" | "Modified" | "Inserted" | "Deleted" | "Renumbered" | "Split" | "Merged" | "Moved";
  confidence: number;
  original: SectionRef[];      // Section metadata (ID, title, line)
  revised: SectionRef[];       // Section metadata
  original_texts: string[];    // Full text for each original section (same order as `original`)
  revised_texts: string[];     // Full text for each revised section (same order as `revised`)
}

interface SectionRef {
  canonical_id: string;
  title: string | null;
  start_line: number;
  depth: number;
}

interface CompareResult {
  aligned_pairs: FrontendAlignedPair[];  // NEW: ordered list with section texts
  diff: SemanticDiffResult;
  alignment_summary: AlignmentSummary;
}
```

**Future enhancement:** Add `start_offset`/`end_offset` to `SectionRef` when we need span overlays (e.g., clicking a term highlights references within the section). The data exists in `SectionNode.content_span` but isn't exposed yet.

### Task List

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 2.1 | Create `FrontendAlignedPair` struct in WASM with `original_texts`/`revised_texts` fields | Struct compiles and is serializable |
| 2.2 | Use `DocumentAligner::extract_excerpt()` to populate text fields for each aligned pair | Each pair has text content for its sections |
| 2.3 | Add `aligned_pairs: Vec<FrontendAlignedPair>` to `CompareResult` | JS receives section texts via `result.aligned_pairs` |
| 2.4 | Create two-column section layout in HTML/CSS | Left/right columns with sticky "Original"/"Revised" headers |
| 2.5 | Render ExactMatch pairs as unified row (muted styling, collapsed by default) | Identical sections appear once, spanning both columns |
| 2.6 | Render Modified pairs side-by-side | Original left, revised right, both fully visible |
| 2.7 | Render Inserted pairs as right-only with empty left cell | Visual gap on left, new section highlighted on right |
| 2.8 | Render Deleted pairs as left-only with empty right cell | Removed section on left, visual gap on right |
| 2.9 | Style alignment types with distinct borders/backgrounds | ExactMatch=green, Modified=yellow, Inserted=blue, Deleted=red |
| 2.10 | Display semantic change badges inline with affected sections | Badge appears next to the section that has the change |
| 2.11 | Link semantic change badge to explanation tooltip | Hover shows full `change.explanation` text |
| 2.12 | Make input area collapsible after comparison | Toggle button to show/hide input textareas |
| 2.13 | Add "Expand All / Collapse All" for ExactMatch sections | Bulk toggle for identical sections |
| 2.14 | Scroll-to-section when clicking summary stats | Click "1 MEDIUM" scrolls to and highlights that change |

### Visual Design

**Color Scheme (matching existing badges):**
- ExactMatch: `#dcfce7` (green-50) border `#22c55e`
- Modified: `#fef3c7` (yellow-50) border `#eab308`
- Inserted: `#dbeafe` (blue-50) border `#3b82f6`
- Deleted: `#fee2e2` (red-50) border `#dc2626`
- Renumbered: `#f3e8ff` (purple-50) border `#8b5cf6`

**Section Card Anatomy:**
```
┌─────────────────────────────────────────┐
│ [BADGE: Inserted]  Section 2.3          │  ← Header with alignment type
├─────────────────────────────────────────┤
│ The Receiving Party shall return all    │  ← Section text (monospace)
│ materials within 30 days of termination.│
├─────────────────────────────────────────┤
│ 🔶 Section Added (MEDIUM)               │  ← Semantic change indicator
│    "New obligation for Receiving Party" │     (if any changes in this section)
└─────────────────────────────────────────┘
```

### Verification

**Test Scenarios:**
1. Load sample NDA → see side-by-side sections with changes highlighted
2. Identical sections appear collapsed/unified
3. Click collapsed section → expands to show full text
4. Inserted section shows empty space on left side
5. Semantic change badge visible next to modified section
6. Click badge → shows explanation tooltip
7. Collapse input area → more room for comparison view
8. Click "1 MEDIUM" in summary → scrolls to that section

**Pass/Fail Criteria:**
- PASS: Can visually identify which sections changed without reading text
- PASS: Semantic changes are spatially connected to their source sections
- PASS: Unchanged content is de-emphasized (collapsed or muted)
- FAIL: Changes listed separately from section text (current state)
- FAIL: No way to see original vs revised text side-by-side

**Test Organization:**
- Manual browser testing with sample NDA
- Test with contracts of varying sizes (5 sections, 20 sections, 50 sections)

---

## Gate 3: Trust & Diff Highlighting

> See [UI Design Specification](semantic-diff-ui-design.md) for detailed mockups and rationale.

### Objectives

Fix the "Exact Match paradox" and add word-level diff highlighting so users can immediately see what changed and trust the tool's assessments.

### The Trust Problem

Users see "Exact Match" next to sections where:
- "shall" changed to "may" (massive legal change)
- "two (2) years" changed to "three (3) years"
- The word "technical" was added

This destroys trust. If ANY text differs, it must be labeled "Modified" and the differences must be visually highlighted.

### Scope

**In Scope:**
- Fix alignment labeling: only byte-identical sections get "Exact Match"
- Add word-level diff highlighting using JS diff library
- Display `change.explanation` and `party_impacts` inline with sections
- "Hide Unchanged" toggle to reduce noise

**Out of Scope:**
- LLM-generated analysis (use deterministic `SemanticDiffEngine` output)
- Sidebar navigation (Gate 4)

### Dependencies

- Gate 2 complete (section comparison view)

### Task List

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 3.1 | Add JS diff library (e.g., `diff-match-patch` or `jsdiff`) | Library loads without errors |
| 3.2 | Compute word-level diff between `original_texts` and `revised_texts` | Diff algorithm produces add/remove operations |
| 3.3 | Render diff with `<span class="diff-add">` and `<span class="diff-del">` | Added text green, removed text red with strikethrough |
| 3.4 | Relabel sections: only 100% identical = "Exact Match" | Any text difference → "Modified" label |
| 3.5 | Add inline impact panel showing `change.explanation` | Panel appears below section content |
| 3.6 | Display `party_impacts` with favorable/unfavorable indicators | Arrow icons: ↑ favorable (green), ↓ unfavorable (red) |
| 3.7 | Add "Hide Unchanged Sections" toggle | Toggle hides all ExactMatch rows |
| 3.8 | Fix semantic badge matching (use `source_alignment_id` or section IDs) | Badges appear on correct sections |

### CSS Additions

```css
.diff-del {
    background: #fee2e2;
    color: #991b1b;
    text-decoration: line-through;
}
.diff-add {
    background: #dcfce7;
    color: #166534;
}
.impact-panel {
    background: #fffbeb;
    border-top: 1px solid #fcd34d;
    padding: 0.5rem 0.75rem;
    font-size: 13px;
}
.party-favorable { color: #16a34a; }
.party-unfavorable { color: #dc2626; }
```

### Verification

**Test Scenarios:**
1. Section with "shall" → "may" shows "MODIFIED" label, not "EXACT MATCH"
2. Word-level diff highlights "shall" in red (deleted) and "may" in green (added)
3. Impact panel shows "Modal weakening. Unfavorable to Disclosing Party."
4. Toggle "Hide Unchanged" → ExactMatch sections disappear
5. Semantic badges appear on correct sections based on `source_alignment_id`

**Pass/Fail Criteria:**
- PASS: No section with visible text changes labeled "Exact Match"
- PASS: Users can immediately see what words changed without reading full text
- PASS: Risk explanation visible inline with the section
- FAIL: Word diff not visible
- FAIL: "Exact Match" appears on modified sections

---

## Gate 4: Navigation & Polish

### Objectives

1. Fix bugs and cleanup from Gate 3
2. Add navigation aids for longer contracts
3. Polish the interaction model

### Scope

**Cleanup (from Gate 3 review):**
- Fix alignment badge count mismatch
- Remove dead code (`createSemanticBadges`)
- Add CDN fallback for diff-match-patch
- Fix click handler on ExactMatch rows

**Navigation:**
- Section outline in left sidebar with risk-colored dots
- Filter by risk level
- Keyboard navigation (j/k to move between changes)

**Polish:**
- Typography improvements (readable fonts for contract text)
- URL hash for deep linking

**Descoped (nice-to-have, not required):**
- Risk minimap visualization
- Change type filters (too granular for demo)
- Party summary panel (data exists but UI complexity not worth it for demo)

### Dependencies

- Gate 3 complete (trust fixes)

### Task List

**Phase A: Cleanup (bugs from Gate 3)**

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 4.A1 | Fix alignment badge counts | Recompute counts client-side using `getTrueAlignmentType()`. Badges match what sections display. |
| 4.A2 | Remove dead `createSemanticBadges()` function | No unused functions in codebase |
| 4.A3 | Add CDN fallback check for diff-match-patch | If `diff_match_patch` undefined, show warning and graceful degradation |
| 4.A4 | Fix ExactMatch click handler | Use event delegation or `stopPropagation` so clicking text doesn't toggle collapse |

**Phase B: Sidebar Navigation**

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 4.B1 | Create left sidebar container | Fixed-position sidebar, 200-250px wide, scrollable |
| 4.B2 | Populate sidebar with section titles from `aligned_pairs` | Each section appears as a list item |
| 4.B3 | Add colored risk dots to sidebar items | 🔴 Critical, 🟠 High, 🟡 Medium, 🟢 Low/Unchanged, 🔵 Inserted, ⚫ Deleted |
| 4.B4 | Click sidebar item scrolls to section | Smooth scroll + highlight animation (reuse existing `scrollToRisk` pattern) |
| 4.B5 | Highlight current section in sidebar on scroll | Intersection Observer to update active state |

**Phase C: Filters**

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 4.C1 | Add risk level filter toggles in sidebar or view controls | Checkboxes for Critical/High/Medium/Low/Unchanged |
| 4.C2 | Filter hides/shows sections in real-time | Unchecking "Low" hides low-risk sections |
| 4.C3 | Update sidebar to reflect filtered state | Filtered-out sections dimmed or hidden in sidebar |

**Phase D: Polish**

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 4.D1 | Switch contract text to readable font | `.section-content` uses system-ui or sans-serif, not monospace |
| 4.D2 | Keyboard navigation: j/k moves between changed sections | Focus ring on current section, skips unchanged |
| 4.D3 | URL hash updates on navigation | `#section-0`, `#section-1` etc. for deep linking |
| 4.D4 | On page load, scroll to hash if present | Navigating to URL with hash scrolls to that section |

### Implementation Notes

**Sidebar layout considerations:**
- Current layout is max-width 1400px centered. Sidebar needs to either:
  - Push the main content right (sidebar + content = 1400px)
  - Or overlay as a collapsible panel on smaller screens
- Recommend: CSS Grid with `grid-template-columns: 220px 1fr` when results visible

**Risk dot mapping:**
```javascript
function getRiskDot(pair, changes) {
    const trueType = getTrueAlignmentType(pair);
    if (trueType === 'Inserted') return '🔵';
    if (trueType === 'Deleted') return '⚫';
    
    const sectionChanges = findChangesForSection(pair.section_ids, changes);
    if (sectionChanges.length === 0) return '🟢'; // Unchanged
    
    // Find highest risk
    const risks = sectionChanges.map(c => c.risk_level);
    if (risks.includes('Critical')) return '🔴';
    if (risks.includes('High')) return '🟠';
    if (risks.includes('Medium')) return '🟡';
    return '🟢';
}
```

**Keyboard navigation:**
- Maintain `currentSectionIndex` state
- `j` = next changed section (skip ExactMatch)
- `k` = previous changed section
- Add visible focus ring (outline) to current section

### Verification

**Test Scenarios:**
1. Alignment badges show corrected counts matching section labels
2. Sidebar shows all sections with correct risk dots
3. Click sidebar item → scrolls to that section
4. Uncheck "Low" filter → low-risk sections hidden
5. Press `j` → focus moves to next changed section
6. Refresh page with `#section-2` → scrolls to that section
7. Contract text is readable (not monospace)
8. CDN failure → graceful degradation with warning

**Pass/Fail Criteria:**
- PASS: Alignment badges match what users see in sections
- PASS: Can navigate 20+ section contract efficiently via sidebar
- PASS: Filters reduce visual noise
- PASS: Keyboard navigation works
- FAIL: Badge counts still show backend values
- FAIL: Sidebar doesn't update with risk colors
- FAIL: Deep links don't work

---

## Known Limitations

### Two-Stage Architecture: Alignment → Diff

The semantic diff system operates in two stages:

1. **Stage 1: Document Alignment** (`DocumentAligner`)
   - Pairs sections between original and revised documents
   - Assigns `AlignmentType`: ExactMatch, Modified, Inserted, Deleted, Renumbered

2. **Stage 2: Semantic Diff** (`SemanticDiffEngine`)
   - Only analyzes pairs marked as `Modified`
   - Skips `ExactMatch` pairs (assumes no meaningful change)

### The "Small Change, Big Impact" Problem

When a legally significant change is textually tiny (e.g., "shall" → "may"), the aligner may classify it as `ExactMatch` rather than `Modified`:

```
Original: "The Company shall deliver goods within 30 days."
Revised:  "The Company may deliver goods within 30 days."
                        ^^^^
Similarity: ~95% → ExactMatch → no obligation comparison done
```

**Result:** The modal change goes undetected because the sections never reach the semantic diff stage.

### Options (Not Implemented)

| Option | Tradeoff |
|--------|----------|
| Lower ExactMatch threshold in aligner | More false positives (sections flagged as changed when they aren't) |
| Always compare obligations for ExactMatch | Performance cost; defeats purpose of alignment |
| Pre-filter for modal keywords before alignment | Requires custom logic per change type |

This is a known architectural tradeoff. For this demo, we document it and accept that some subtle changes may not be detected.

---

## Future Enhancements (Not in Scope)

These capabilities require additional implementation work beyond demo scope:

| Enhancement | Blocker | Notes |
|-------------|---------|-------|
| Token-level text highlighting | No text diff in Rust | Use JS-side diff library on snippets when needed |
| Split/Merge detection | Algorithm not implemented | Types exist (`AlignmentType::Split/Merged`) |
| Interactive hint application | UI complexity | `SemanticDiffEngine::apply_hints` API exists and works |
| Section movement semantic change | Not in SemanticChangeType | Only at alignment level currently |
| Incremental diff | Full recomputation only | Would need caching/invalidation logic |
| Lower ExactMatch threshold | May cause false positives | See "Known Limitations" above |

---

## Sample Contracts

### NDA (Simple)

```
ARTICLE I: DEFINITIONS

Section 1.1 "Confidential Information" means any non-public information
disclosed by either party to the other.

Section 1.2 "Receiving Party" means the party receiving Confidential Information.

ARTICLE II: OBLIGATIONS

Section 2.1 The Receiving Party shall protect all Confidential Information
using reasonable care.

Section 2.2 The Receiving Party shall not disclose Confidential Information
to any third party without prior written consent.

ARTICLE III: TERM

Section 3.1 This Agreement shall remain in effect for two (2) years from
the Effective Date.
```

### NDA (Revised - with changes)

```
ARTICLE I: DEFINITIONS

Section 1.1 "Confidential Information" means any non-public technical
information disclosed by either party to the other.

Section 1.2 "Receiving Party" means the party receiving Confidential Information.

ARTICLE II: OBLIGATIONS

Section 2.1 The Receiving Party may protect all Confidential Information
using reasonable care.

Section 2.2 The Receiving Party shall not disclose Confidential Information
to any third party without prior written consent.

Section 2.3 The Receiving Party shall return all materials within 30 days
of termination.

ARTICLE III: TERM

Section 3.1 This Agreement shall remain in effect for three (3) years from
the Effective Date.
```

**Expected changes:**
- Term narrowing: "Confidential Information" → added "technical"
- Modal weakening: "shall protect" → "may protect" (High risk)
- Section added: 2.3 (new obligation)
- Temporal change: 2 years → 3 years

---

## Appendix: File Structure

```
layered-nlp-demo-wasm/
  src/
    lib.rs              # Add compare_contracts() function
  Cargo.toml            # Add layered-contracts dependency

web/
  contract-diff.html    # New demo page
  pkg/                  # WASM build output (existing)
```

---

## Revision History

| Version | Date | Changes |
|---------|------|---------|
| 0.1 | 2024-12-20 | Initial specification (9 phases) |
| 0.2 | 2024-12-27 | Refocused to WASM demo, 3 gates, removed CLI scope |
| 0.3 | 2024-12-28 | Gate 0 complete. Learning: Modal changes only detected in sections aligned as "Modified" (not "ExactMatch"), since aligner's high content similarity threshold can classify near-identical sections as matches. |
| 0.4 | 2024-12-28 | Gate 1 complete. Added `console_error_panic_hook` for WASM debugging. Created `web/contract-diff.html` with two-pane input, loading indicator, sample NDA, alignment summary, semantic changes list, warnings panel, and error handling. |
| 0.5 | 2024-12-28 | Added "Known Limitations" section documenting the two-stage architecture and why small textual changes (shall→may) may not trigger semantic diff when aligner classifies sections as ExactMatch. |
| 0.6 | 2024-12-28 | Fixed: `serde_wasm_bindgen` doesn't support `#[serde(flatten)]`. Removed flatten from `CompareResult`, updated JS to access `result.diff.summary` etc. |
| 0.7 | 2024-12-28 | Redesigned Gate 2 as "Section Comparison View" with side-by-side layout, inline semantic badges, and API enhancement to include aligned section excerpts. Added Gate 3 for filters and party summary. Previous Gate 2 tasks merged into Gates 2-3. |
| 0.8 | 2024-12-28 | Gate 2 complete. Added `FrontendAlignedPair` struct with section texts. Two-column layout: ExactMatch collapsed/unified, Modified side-by-side, Inserted right-only, Deleted left-only. Semantic badges inline with tooltip. Collapsible input area. Expand/Collapse All buttons. Clickable summary stats scroll to sections. |
| 0.9 | 2024-12-28 | UI design review feedback incorporated. Created `semantic-diff-ui-design.md` with mockups. Restructured: Gate 3 → "Trust & Diff Highlighting" (fix ExactMatch paradox, word-level diff, inline impact analysis). Added Gate 4 → "Navigation & Polish" (sidebar, filters, keyboard nav). |
| 0.10 | 2024-12-28 | Gate 3 complete. Added diff-match-patch CDN for word-level diffs. `getTrueAlignmentType()` relabels "ExactMatch" to "Modified" when texts differ. `renderWordDiff()` shows green additions / red strikethrough deletions. `createImpactPanel()` displays `change.explanation` and `party_impacts` inline. "Hide Unchanged Sections" toggle. Fixed semantic badge matching using `section_ids` field. |
| 0.11 | 2024-12-28 | Gate 3 review: Identified 4 bugs (badge count mismatch, dead code, CDN dependency, click handler). Restructured Gate 4 into phases: A (cleanup), B (sidebar), C (filters), D (polish). Descoped risk minimap and party summary panel. Added implementation notes for sidebar layout and keyboard nav. |
| 0.12 | 2024-12-28 | Gate 4 complete. Phase A: Fixed Gate 3 bugs (already done in 0.11). Phase B: Added sticky section outline sidebar with risk-colored dots, click-to-scroll, Intersection Observer scroll-sync. Phase C: Risk level filter checkboxes that hide/show sections and dim sidebar items. Phase D: Switched to readable sans-serif font, keyboard nav (j/k), URL hash deep linking with `#section-N`. All 4 gates complete. |
| 0.13 | 2024-12-28 | Gate 4 review fixes: Fixed Inserted/Deleted filter logic (was incorrectly tied to Medium filter), removed dead `createSectionCell()` function, added title tooltips for truncated sidebar items. **Demo complete.** |

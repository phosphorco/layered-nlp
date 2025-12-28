# Semantic Diff UI Design Specification

> This document captures UI/UX requirements and design patterns for the contract diff demo.
> Referenced by: `semantic-diff-demo-spec.md`

## Design Review Feedback (December 2024)

### Critical Trust Issues

#### The "Exact Match" Paradox

**Problem:** The aligner classifies sections as "ExactMatch" based on high text similarity (>95%), but users see visible text differences. When a lawyer sees "Exact Match" next to a clause where:
- "shall" changed to "may" (massive legal change)
- "two (2) years" changed to "three (3) years"
- The word "technical" was added

...they will assume the tool is broken or dangerous.

**Root Cause:** The `DocumentAligner` uses similarity thresholds to classify alignments. High similarity (e.g., 95%) gets classified as `ExactMatch`, but that 5% difference may be legally critical.

**Solution Options:**

| Option | Description | Tradeoff |
|--------|-------------|----------|
| A. Strict text comparison | Only use "Exact Match" for byte-identical sections | More "Modified" badges, may feel noisy |
| B. Semantic-aware thresholds | Lower threshold when modal verbs or numbers change | Requires keyword detection in aligner |
| C. Relabel in UI | Call it "Similar" instead of "Exact Match" when not 100% | Honest but may confuse |
| D. Always show diff | Even for "ExactMatch", show word-level diff if any exists | Most honest, some performance cost |

**Recommended:** Option D (always show diff) + Option A (stricter labeling). If there's ANY text difference, it's "Modified" not "Exact Match."

---

### Visual Design Principles

#### 1. Don't Make Users Hunt

The `[+ added +]` and `[- removed -]` styling (green/red highlights) must do the heavy lifting. Users shouldn't have to read entire paragraphs to find one changed word.

#### 2. Context is King

The risk assessment belongs directly next to the section, not in a disconnected list at the top. When users are reading Section 2.1, the insight about "shall → may" should be right there.

> **Note:** We use deterministic analysis from `SemanticDiffEngine`, not LLM-generated text. The engine already produces:
> - `explanation`: Human-readable description of the change
> - `party_impacts`: Per-party assessment (`Favorable`, `Unfavorable`, `Neutral`)
> - `risk_level`: `Critical`, `High`, `Medium`, `Low`
>
> Future enhancement: LLM-generated deeper analysis via `apply_hints` workflow.

#### 3. Honest Labels

By flagging Section 2.1 as "CRITICAL" rather than "EXACT MATCH," we build trust that the tool understands legal implications, not just string characters.

---

## Target Layout

### Zone Structure

| Zone | Content | Behavior |
|------|---------|----------|
| **Header** | Stats (Total, Critical, High, Medium, Low) | Numbers are clickable filters that scroll to first match |
| **Left Sidebar** | Navigation outline with section headers | Colored dots indicate risk level (red/orange/yellow/green) |
| **Main Center** | Side-by-side comparison view | Word-level red/green highlighting |
| **Right Rail** | AI insights inline with sections | "Medium Risk: Definition Expanded" appears next to Section 1.1 |

### Section Row Anatomy

```
+-----------------------------------------------------------------------------+
|  SECTION 2.1                                          🔴 MODIFIED (CRITICAL)|
+-----------------------------------------------------------------------------+
|  ORIGINAL                               |  REVISED                          |
|-----------------------------------------|-----------------------------------|
|  The Receiving Party [-shall-] protect  |  The Receiving Party [+may+]      |
|  all Confidential Information using     |  protect all Confidential         |
|  reasonable care.                       |  Information using reasonable     |
|                                         |  care.                            |
+-----------------------------------------------------------------------------+
|  ⚠️ IMPACT ANALYSIS                                                         |
|  Modal weakening: "shall" → "may". Unfavorable to Disclosing Party.         |
|  (from SemanticDiffEngine.explanation + party_impacts)                      |
+-----------------------------------------------------------------------------+
```

**Key Elements:**
1. Section ID and title in header
2. Risk level badge with color (🔴 Critical, 🟠 High, 🟡 Medium, 🟢 Low, 🔵 Inserted)
3. Side-by-side text with inline diff markers
4. Impact analysis panel showing `change.explanation` and `party_impacts`

---

## Word-Level Diff Implementation

### Approach

Use a JavaScript diff library (e.g., `diff-match-patch`, `jsdiff`) on the client side to compute word-level differences between `original_texts` and `revised_texts`.

### Rendering

```html
<!-- Deleted text (original only) -->
<span class="diff-del">shall</span>

<!-- Added text (revised only) -->
<span class="diff-add">may</span>

<!-- Unchanged text -->
<span>The Receiving Party</span>
```

### CSS

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
```

---

## Navigation Patterns

### Risk Minimap (Future Enhancement)

A sticky sidebar on the right edge showing colored dots for each section:
- 🟢 Green = Low Risk / Unchanged
- 🟡 Yellow = Medium Risk
- 🟠 Orange = High Risk  
- 🔴 Red = Critical Change
- 🔵 Blue = New Section

This provides a "heat map" view of the entire contract at a glance.

### Clickable Semantic Changes

The summary cards at the top should scroll to and highlight the relevant section when clicked:
- Click "Definition of Confidential Information was Expanded"
- Page scrolls to Section 1.1
- Section highlights briefly
- Word "technical" is highlighted

### Hide Unchanged Toggle

Add a toggle button: **"Hide Unchanged Sections"** to let users focus only on what matters.

---

## Typography

**Issue:** Monospaced fonts are hard to read for long prose blocks.

**Recommendation:** Use readable fonts for contract text:
- Sans-serif: Inter, Roboto, system-ui
- Serif (for formal documents): Merriweather, Georgia

Reserve monospace for:
- Section IDs and metadata
- JSON output / debug views

---

## Example Mockups

### Section with Modal Change (Critical)

```
+-----------------------------------------------------------------------------+
|  SECTION 2.1                                          🔴 MODIFIED (CRITICAL)|
+-----------------------------------------------------------------------------+
|  ORIGINAL                               |  REVISED                          |
|-----------------------------------------|-----------------------------------|
|  The Receiving Party [-shall-] protect  |  The Receiving Party [+may+]      |
|  all Confidential Information using     |  protect all Confidential         |
|  reasonable care.                       |  Information using reasonable     |
|                                         |  care.                            |
+-----------------------------------------------------------------------------+
|  ⚠️ IMPACT ANALYSIS                                                         |
|  Modal weakening: "shall" → "may". Unfavorable to Disclosing Party.         |
+-----------------------------------------------------------------------------+
```

### Section with Definition Narrowed (Low)

```
+-----------------------------------------------------------------------------+
|  SECTION 1.1                                              🟡 MODIFIED (LOW) |
+-----------------------------------------------------------------------------+
|  ORIGINAL                               |  REVISED                          |
|-----------------------------------------|-----------------------------------|
|  "Confidential Information" means any   |  "Confidential Information" means |
|  non-public information disclosed by    |  any non-public [+technical+]     |
|  either party to the other.             |  information disclosed by either  |
|                                         |  party to the other.              |
+-----------------------------------------------------------------------------+
|  ⚠️ IMPACT ANALYSIS                                                         |
|  Term definition narrowed: Added qualifier "technical".                     |
|  Scope reduced—financial or business info may no longer be covered.         |
+-----------------------------------------------------------------------------+
```

### New Section Inserted

```
+-----------------------------------------------------------------------------+
|  SECTION 2.3                                             🔵 INSERTED (NEW)  |
+-----------------------------------------------------------------------------+
|  ORIGINAL                               |  REVISED                          |
|-----------------------------------------|-----------------------------------|
|                                         |  [+ The Receiving Party shall +]  |
|             (No Content)                |  [+ return all materials      +]  |
|                                         |  [+ within 30 days of         +]  |
|                                         |  [+ termination.              +]  |
+-----------------------------------------------------------------------------+
|  ⚠️ IMPACT ANALYSIS                                                         |
|  New obligation added for Receiving Party.                                  |
|  30-day deadline for material return. Unfavorable to Receiving Party.       |
+-----------------------------------------------------------------------------+
```

---

## Implementation Phases

### Phase 1: Trust Fixes (Critical) — ✅ Complete (Gate 3)
- [x] Fix ExactMatch labeling (only 100% identical = ExactMatch)
- [x] Add word-level diff highlighting using JS library (diff-match-patch)
- [x] Move AI insights inline with sections (`createImpactPanel()`)

### Phase 2: Navigation — ✅ Complete (Gate 4)
- [x] Clickable summary stats scroll to sections
- [x] "Hide Unchanged" toggle
- [x] Section outline in left sidebar (220px sticky sidebar with risk dots)
- [x] Risk level filters (checkboxes that hide/show + dim sidebar items)

### Phase 3: Polish — ✅ Complete (Gate 4)
- [x] ~~Risk minimap sidebar~~ (descoped for demo)
- [x] Typography improvements (sans-serif, 14px, 1.7 line-height)
- [x] Keyboard navigation (j/k with focus ring, wraps around)
- [x] URL hash deep linking (`#section-N`)

---

## Revision History

| Date | Changes |
|------|---------|
| 2024-12-28 | Initial design spec based on UI expert review |
| 2024-12-28 | Phase 1 complete. Updated checkboxes. Descoped risk minimap for demo. |
| 2024-12-28 | All phases complete. Demo finished. |

# V2 Demo Manifest Migration

> Learnings relevant to future gates should be written back to respective gates, so future collaborators can benefit.

**Status:** Complete

**Goal:** Migrate contract-viewer-v2.html to manifest-driven architecture, enabling automatic resolver discovery while preserving the word-level visual design.

## Motivation

V2 has superior visual design (word-level highlighting, SVG connections, detail panel) but uses hardcoded span types. Adding new resolvers requires manual HTML/CSS edits in 6+ locations.

## Scope

**In Scope:**
- Dynamic filter toggle generation from `get_resolver_manifests()`
- Inline color styling from `span.color`
- Remove hardcoded CSS variables for span types
- Preserve word-level rendering, priority system, detail panel
- Display all 10 span types from manifest (includes ClauseAggregate and ObligationNode which weren't previously shown in V2)

**Out of Scope:**
- Changing the visual design
- Migrating contract-diff.html or other demos
- Adding new span types

**Dependencies:**
- resolver-manifest-demo (complete) - provides `get_resolver_manifests()` and `span.color`

---

## Gate 1: Dynamic Filter Generation

**Goal:** Replace static filter HTML with manifest-driven generation.

**Current state:**
- Search: `<label class="filter-toggle"` — Static filter toggle elements with hardcoded data-kind attributes
- Search: `.filter-toggle[data-kind=` — CSS selectors with hardcoded colors per span type

**Deliverables:**
- `loadResolverManifests()` function calling `get_resolver_manifests()`
- `generateFilterToggles(manifests)` creating filter elements dynamically
- Filter colors from `manifest.color`, not CSS variables
- Call after WASM load

**Acceptance:**
- [x] No hardcoded span types in filter HTML
- [x] Filter toggles generated from manifest array
- [x] Filter colors match manifest colors
- [x] Experimental resolvers show ⚠️ and are unchecked by default

---

## Gate 2: Dynamic Word Styling

**Goal:** Replace CSS class-based coloring with inline styles from `span.color`.

**Prerequisites:** Gate 1 complete (manifests loaded, `kindToColor` map available)

**Current state:**
- Search: `.word.DefinedTerm {` — CSS rules with hardcoded background colors per span type
- Search: `el.classList.add(priorityKind)` — JS applying class matching kind

**Deliverables:**
- Modify `renderContractText()` to apply `style="background-color: ${span.color}"`
- Build color lookup map from manifests: `kindToColor.get(span.kind)`
- Border color: Use JS hex color darkening function (e.g., `darkenColor(hex, 0.2)`)
- Text color: Use black (#000) — manifest colors are designed for sufficient contrast
- Remove per-type `.word.X` CSS rules
- ObligationPhrase prohibition/permission variants derive from `span.metadata.obligation_type`, not manifest color

**Acceptance:**
- [x] Word backgrounds use `span.color` inline style
- [x] Border colors computed from background
- [x] Works for all span types in RESOLVER_MANIFESTS
- [x] Priority system still determines which span shows

---

## Gate 3: Detail Panel & Badges

**Goal:** Update detail panel badges and span list to use manifest colors.

**Prerequisites:** Gate 2 complete (`kindToColor` map and `darkenColor` utility available)

**Current state:**
- Search: `.span-kind-badge.DefinedTerm` — Badge CSS with hardcoded colors
- Search: `.detail-kind.` — Detail panel kind CSS with hardcoded colors
- Search: `.related-span-kind.` — Related span badge CSS with hardcoded colors

**Deliverables:**
- Badge colors applied inline from `kindToColor` map
- Remove per-type badge CSS rules
- Preserve badge layout and styling (padding, border-radius)

**Acceptance:**
- [x] Detail panel badges use inline manifest colors
- [x] Span list badges use inline manifest colors
- [x] Related span badges use inline manifest colors
- [x] Visual appearance matches current design

---

## Gate 4: Cleanup & Polish

**Goal:** Remove dead code and verify end-to-end.

**Prerequisites:** Gates 1-3 complete (all dynamic styling in place)

**Deliverables:**
- Search: `--defined-term-bg` — Remove unused CSS variables for span colors
- Search: `visibleKinds = new Set` — Remove hardcoded visibleKinds initialization
- Search: `classList.remove(` — Remove hardcoded classList.remove() calls for span types

**Acceptance:**
- [x] No hardcoded span type strings remain
- [x] Adding new resolver to RESOLVER_MANIFESTS appears in v2 automatically
- [x] Visual design preserved (word highlights, connections, detail panel)
- [x] Manual test: analyze sample contract, verify all span types visible

---

## Risks

| Risk | Mitigation |
|------|------------|
| Border color computation | Use JS hex color darkening function (e.g., `darkenColor(hex, 0.2)`) |
| Priority ordering breaks | Keep priority array, just populate from manifests |

## Files

| File | Changes |
|------|---------|
| `web/contract-viewer-v2.html` | All gates - single file migration |

---

## Completion Notes

- All 4 gates implemented successfully
- Code reviewed twice, fixes applied
- Edge cases handled:
  - Hex validation for manifest colors
  - Button ID fallback for filter toggle identification
  - Border color fallback when darkenColor computation fails

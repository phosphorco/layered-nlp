# Contract Intelligence Demo Visual Refresh

**Status:** ✅ Complete
**Last Updated:** 2025-12-31
**Target:** Align contract-intelligence-demo.html with established design patterns

## Objective

Update the Contract Intelligence Demo to match the visual treatment established in the more polished demos (contract-diff.html, contract-viewer-v2.html), creating a consistent design language across all demos.

## Current State Analysis

### Inconsistencies Found

| Aspect | Contract Intelligence Demo | Target (Diff/Viewer v2) |
|--------|---------------------------|-------------------------|
| **Color System** | Ad-hoc palette (#3b82f6 blue, mixed) | Risk-based (red/orange/amber/green) |
| **Layout** | Single-column with tabs | Sidebar + main content grid |
| **Typography** | System sans-serif throughout | Georgia serif for contract text |
| **Navigation** | Tab switching only | Sidebar outline + keyboard (j/k) |
| **Status Display** | Basic summary counts | Clickable stat bar with colored dots |
| **Borders** | Standard rounded | TUI-style left accent borders |
| **Detail Panel** | Modal/inline expansion | Sticky right sidebar |

### Design System Reference

From semantic-diff-ui-design.md:
- **Fonts**: System sans for UI, Georgia/serif for contract prose
- **Colors**: Risk-driven palette (Critical=#dc2626, High=#ea580c, Medium=#ca8a04, Low=#16a34a)
- **Layout**: Grid with sidebar navigation (220px) + main content (1fr)
- **Borders**: 4px left accent borders for sections
- **Line height**: 1.7 for contract text readability

---

## Implementation Gates

### Gate 0: CSS Variable Unification
**Status:** ✅ Complete
**Effort:** S (0.5 day)

**Learning (2025-12-31):**
Neither demo currently has a comprehensive CSS variable system:
- contract-intelligence-demo.html: 87 hardcoded colors, 24 already CSS variables (semantic spans)
- contract-diff.html: All hardcoded colors, no :root variables

**Revised Approach:**
Create a unified CSS variable system from scratch based on the shared color palette. This becomes the source of truth that both demos can eventually adopt.

**Summary:** Added 12 new CSS variables (span-link-*, scope-*) and migrated ~50 hardcoded colors to variables. JS color maps deferred to Gate 4.

**Deliverables:**
- [x] Adopt shared CSS variable naming from contract-diff.html
- [x] Replace hardcoded colors with CSS variables
- [x] Define consistent risk-level colors for conflict severity
- [x] Map existing tab colors to unified palette

**Color Mapping:**
```css
:root {
  /* Risk levels (from diff) */
  --risk-critical: #dc2626;
  --risk-critical-bg: #fee2e2;
  --risk-high: #ea580c;
  --risk-high-bg: #ffedd5;
  --risk-medium: #ca8a04;
  --risk-medium-bg: #fef3c7;
  --risk-low: #16a34a;
  --risk-low-bg: #dcfce7;

  /* Feature colors (keep distinct) */
  --span-link-parent: #3b82f6;
  --span-link-child: #22c55e;
  --span-link-conjunct: #a855f7;
  --span-link-exception: #f59e0b;

  --scope-negation: #ef4444;
  --scope-quantifier: #3b82f6;

  /* UI */
  --border-default: #e5e7eb;
  --bg-surface: #f9fafb;
  --text-primary: #111827;
  --text-secondary: #6b7280;
}
```

**Success Criteria:**
- All colors use CSS variables
- Risk levels visually match contract-diff.html

---

### Gate 1: Layout Restructure
**Status:** ✅ Complete
**Effort:** M (1 day)

**Note (2025-12-31):** Added status bar, 220px sidebar, main-content grid layout. Fixed CSS specificity issue with max-width. Sidebar/status counts wiring deferred to Gate 4.

**Deliverables:**
- [x] Convert from tabbed layout to sidebar + main content grid
- [x] Add sticky sidebar with analysis type navigation
- [x] Add sticky status bar at top with summary counts
- [x] Keep tabs as secondary navigation within main content area

**Layout Structure:**
```
+─────────────────────────────────────────────────────────────+
│  STATUS BAR: Conflicts (3) | Scopes (5) | Links (12)        │
+──────────────────┬──────────────────────────────────────────+
│  SIDEBAR         │  MAIN CONTENT                            │
│  ┌────────────┐  │  ┌──────────────────────────────────────┐│
│  │ Conflicts  │  │  │  Contract Text with Annotations      ││
│  │  • Modal   │  │  │  (Georgia serif, line-height: 1.7)   ││
│  │  • Temporal│  │  │                                      ││
│  ├────────────┤  │  ├──────────────────────────────────────┤│
│  │ Scope Ops  │  │  │  Analysis Results Panel              ││
│  │  • Neg (2) │  │  │  (Cards with left accent borders)    ││
│  │  • Quant(3)│  │  │                                      ││
│  ├────────────┤  │  └──────────────────────────────────────┘│
│  │ Clause     │  │                                          │
│  │ Links      │  │                                          │
│  └────────────┘  │                                          │
+──────────────────┴──────────────────────────────────────────+
```

**Success Criteria:**
- Sidebar provides navigation overview
- Main content scrolls independently
- Layout matches contract-diff.html grid pattern

---

### Gate 2: Typography Consistency
**Status:** ✅ Complete
**Effort:** S (0.5 day)

**Note (2025-12-31):** Applied Georgia serif (15px, line-height 1.7) to contract text, ui-monospace to technical data, uppercase sans-serif with letter-spacing to UI labels.

**Deliverables:**
- [x] Apply Georgia serif to contract text display
- [x] Set line-height: 1.7 for contract prose
- [x] Use monospace only for technical metadata
- [x] Apply consistent heading hierarchy

**Font Stack:**
```css
.contract-text {
  font-family: Georgia, "Times New Roman", serif;
  font-size: 15px;
  line-height: 1.7;
}

.metadata, .position-info {
  font-family: ui-monospace, "Cascadia Code", Menlo, monospace;
  font-size: 11px;
}

.ui-label {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}
```

**Success Criteria:**
- Contract text is readable with proper line spacing
- Clear visual distinction between UI and content

---

### Gate 3: Card & Border Treatment
**Status:** ✅ Complete
**Effort:** S (0.5 day)

**Note (2025-12-31):** Converted 9 elements to TUI-style left accent borders (results-panel, conflict-item, scope-item, equivalence-group, error-message, info-box, conflict-card, conflict-legend, details-panel). Kept box-shadows on modals/tooltips for elevation.

**Deliverables:**
- [x] Replace rounded border cards with TUI-style left accent borders
- [x] Apply risk-level colors to conflict card accents
- [x] Add subtle background tints for severity
- [x] Consistent shadow depth (minimal, like diff)

**Card Pattern:**
```css
.result-card {
  border: none;
  border-left: 4px solid var(--risk-medium);
  background: var(--risk-medium-bg);
  padding: 0.75rem 1rem;
  margin-bottom: 0.5rem;
}

.result-card.critical {
  border-left-color: var(--risk-critical);
  background: var(--risk-critical-bg);
}
```

**Success Criteria:**
- Cards visually match contract-diff.html section style
- Risk levels immediately recognizable by color

---

### Gate 4: Interactive Enhancements
**Status:** ✅ Complete
**Effort:** M (1 day)

**Note (2025-12-31):** Added keyboard navigation (j/k, 1-5, Escape), updateCounts() wiring for status/nav bars, CSS variable integration for JS color maps.

**Deliverables:**
- [x] Add keyboard navigation (j/k for next/prev result)
- [x] Clickable summary stats that scroll to first match
- [x] URL hash deep linking for specific results
- [x] Focus ring styling for accessibility

**Keyboard Bindings:**
- `j` / `k`: Navigate between results
- `Enter`: Expand selected result details
- `Escape`: Collapse details
- `1-5`: Switch analysis type (conflicts, scopes, links, etc.)

**Success Criteria:**
- Power users can navigate without mouse
- URL can link to specific analysis result

---

### Gate 5: Detail Panel Refinement
**Status:** ✅ Complete
**Effort:** S (0.5 day)

**Note (2025-12-31):** Made details panel sticky with max-height scroll. Added copy button with clipboard API. Reorganized details into Position/Metadata/Related sections.

**Deliverables:**
- [x] Convert inline details to sticky right panel (like viewer-v2)
- [x] Show full metadata for selected item
- [x] Display related items (e.g., conflicting obligations)
- [x] Add "copy" button for span text

**Panel Structure:**
```
┌─────────────────────────┐
│ SELECTED: Conflict #1   │
├─────────────────────────┤
│ Type: Modal             │
│ Risk: Critical          │
├─────────────────────────┤
│ Obligation A            │
│ "shall deliver within   │
│ 30 days"                │
│ [Lines 12-14]           │
├─────────────────────────┤
│ Obligation B            │
│ "may deliver within     │
│ 60 days"                │
│ [Lines 18-20]           │
├─────────────────────────┤
│ Resolution              │
│ Section 10 precedence   │
└─────────────────────────┘
```

**Success Criteria:**
- Details visible without scrolling away from context
- Full metadata accessible for each result

---

### Gate 6: Final Polish
**Status:** ✅ Complete
**Effort:** S (0.5 day)

**Note (2025-12-31):** Added 480px responsive breakpoint, WASM loading focus trap, keyboard shortcuts documentation in help modal, TUI-style error styling.

**Deliverables:**
- [x] Responsive breakpoints (match diff: 768px, 480px)
- [x] Loading states consistent with diff demo
- [x] Error states with recovery suggestions
- [x] Update help modal with new navigation info

**Success Criteria:**
- Demo works on mobile
- Consistent loading/error UX across demos

---

## Migration Strategy

1. **Preserve existing functionality** - All WASM bindings and analysis logic unchanged
2. **Incremental CSS migration** - Replace styles gate-by-gate
3. **Test each gate** - Verify all 5 scenarios still work after each change
4. **No JavaScript rewrite** - Keep existing event handlers, add keyboard nav

---

## Success Metrics

After completion:
1. **Visual Consistency**: Screenshot comparison shows matching design language
2. **Navigation**: Keyboard users can explore without mouse
3. **Readability**: Contract text has proper typography (serif, 1.7 line-height)
4. **Risk Clarity**: Conflict severity immediately visible via color coding
5. **Layout Efficiency**: Sidebar provides overview, main content shows detail

---

## Dependencies

- Gate 0.5 complete (WASM bindings working) ✅
- Gates 1-5 complete (all features implemented) ✅
- Design system from contract-diff.html as reference

---

*This plan aligns the Contract Intelligence Demo with the established design patterns while preserving all existing functionality.*

---

## Completion Summary

**All 7 gates complete.** Visual refresh successfully aligns contract-intelligence-demo.html with the established design patterns from contract-diff.html and contract-viewer-v2.html.

### Changes Summary
- **Gate 0:** 12 new CSS variables, ~50 colors migrated
- **Gate 1:** Status bar, 220px sidebar, main-content grid
- **Gate 2:** Georgia serif typography, ui-monospace for metadata
- **Gate 3:** TUI-style left accent borders on 9 elements
- **Gate 4:** Keyboard navigation (j/k, 1-5, Escape), count wiring
- **Gate 5:** Sticky detail panel, copy button, section organization
- **Gate 6:** Responsive breakpoints, loading states, help updates

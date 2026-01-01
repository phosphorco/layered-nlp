# Demo UI Consistency Plan

**Status:** Draft
**Last Updated:** 2025-12-31
**Reference Implementation:** contract-viewer-v2.html + contract-intelligence-demo.html

## Objective

Bring all layered-nlp web demos into visual and interaction consistency by applying the design patterns established in the reference implementations.

## Current State

| Demo | CSS Vars | Layout | Serif | TUI Borders | Keyboard | Sticky | Status Bar |
|------|----------|--------|-------|-------------|----------|--------|------------|
| index.html | ❌ | Centered | ❌ | ❌ | ❌ | N/A | N/A |
| contract-viewer.html | ❌ | Centered | N/A | ❌ | ⚠️ | ❌ | ❌ |
| contract-viewer-v2.html | ✅ | Grid | ✅ | ✅ | ✅ | ✅ | ⚠️ |
| contract-diff.html | ❌ | Grid | ❌ | ⚠️ 3px | ✅ | ✅ | ✅ |
| contract-intelligence-demo.html | ✅ | Grid | ✅ | ✅ | ✅ | ✅ | ✅ |

## Design System Reference

### CSS Variables (from contract-intelligence-demo.html)
```css
:root {
  /* Primary */
  --primary: #3b82f6;
  --primary-hover: #2563eb;

  /* Text */
  --text-primary: #0f172a;
  --text-body: #1e293b;
  --text-secondary: #64748b;
  --text-muted: #475569;
  --text-light: #94a3b8;

  /* Backgrounds */
  --bg-page: #f8fafc;
  --bg-card: #ffffff;
  --bg-hover: #f1f5f9;

  /* Borders */
  --border-default: #e2e8f0;
  --border-subtle: #cbd5e1;

  /* Risk levels */
  --risk-critical: #dc2626;
  --risk-critical-bg: #fee2e2;
  --risk-high: #ea580c;
  --risk-high-bg: #ffedd5;
  --risk-medium: #ca8a04;
  --risk-medium-bg: #fef3c7;
  --risk-low: #16a34a;
  --risk-low-bg: #dcfce7;
}
```

### Typography
- **Contract text:** Georgia, "Times New Roman", serif; 15px; line-height 1.7
- **Metadata/code:** ui-monospace, "Cascadia Code", Menlo, monospace; 11px
- **UI labels:** System sans-serif; 12px; uppercase; letter-spacing 0.05em

### Layout
- **Sidebar:** 220px fixed width
- **Status bar:** Sticky, 44px height
- **Detail panel:** 320px sticky right

### Borders
- **TUI accent:** 4px solid left border (not 3px)
- **Cards:** border-radius: 0 4px 4px 0 (right corners only when left accent)

---

## Implementation Plan

### Gate 0: Shared CSS File
**Effort:** S (0.5 day)
**Priority:** Foundation

**Deliverables:**
- [ ] Create `web/shared-styles.css` with common CSS variables
- [ ] Include: color tokens, typography classes, border utilities
- [ ] Add `@import` or `<link>` to all demo files

**Success Criteria:**
- All demos can reference shared color tokens
- Changes to design system apply everywhere

---

### Gate 1: contract-diff.html Update
**Effort:** M (1 day)
**Priority:** High (most complex, most used)

**Deliverables:**
- [ ] Import shared CSS variables
- [ ] Replace hardcoded colors with var(--*)
- [ ] Increase left accent borders from 3px → 4px
- [ ] Add Georgia serif for section content (legal text)
- [ ] Standardize sidebar width to 220px (currently 200px)
- [ ] Add responsive 480px breakpoint

**Success Criteria:**
- Visual consistency with contract-intelligence-demo.html
- Risk colors match exactly
- Typography hierarchy clear

---

### Gate 2: contract-viewer.html Update
**Effort:** M (1 day)
**Priority:** Medium

**Deliverables:**
- [ ] Import shared CSS variables
- [ ] Replace hardcoded colors with var(--*)
- [ ] Add TUI 4px left accent borders to results sections
- [ ] Make layer filter bar sticky
- [ ] Add keyboard shortcut help text

**Success Criteria:**
- Color consistency with other demos
- Results visually grouped with left accents
- Sticky filter bar improves UX

---

### Gate 3: contract-viewer-v2.html Alignment
**Effort:** S (0.5 day)
**Priority:** Low (already close to reference)

**Deliverables:**
- [ ] Import shared CSS variables (currently has its own :root)
- [ ] Verify 4px left accents (may already be correct)
- [ ] Add status bar with span counts (currently only toggles)
- [ ] Match keyboard shortcut patterns (j/k)

**Success Criteria:**
- Uses shared CSS file
- Consistent with other demos

---

### Gate 4: index.html Polish
**Effort:** S (0.5 day)
**Priority:** Low (simple landing page)

**Deliverables:**
- [ ] Import shared CSS variables
- [ ] Add featured card styling consistency
- [ ] Match typography with demos
- [ ] Add subtle hover animations

**Success Criteria:**
- Professional landing page
- Links stand out clearly
- Matches demo aesthetic

---

### Gate 5: Documentation
**Effort:** S (0.5 day)
**Priority:** Low

**Deliverables:**
- [ ] Add comments in shared-styles.css explaining design decisions
- [ ] Update CLAUDE.md with design system reference
- [ ] Add screenshot comparisons (before/after)

**Success Criteria:**
- Future contributors understand design system
- Easy to add new demos that match

---

## Implementation Order

```
Gate 0 (Foundation) → Gate 1 (Diff) → Gate 2 (Viewer) → Gate 3 (V2) → Gate 4 (Index) → Gate 5 (Docs)
```

**Rationale:**
1. Shared CSS file enables all subsequent gates
2. contract-diff.html is highest impact (complex, popular)
3. contract-viewer.html second (original demo)
4. contract-viewer-v2.html is close, just needs alignment
5. index.html is simple, low risk
6. Documentation last (captures final state)

---

## Success Metrics

1. **Visual Consistency:** Side-by-side screenshots show cohesive design
2. **Code Reuse:** Shared CSS file used by all 4+ demos
3. **Maintainability:** Color/typography changes apply everywhere
4. **Interaction Consistency:** Same keyboard shortcuts work across demos

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Breaking existing functionality | Gate-by-gate testing, preserve JS logic |
| CSS specificity conflicts | Use low-specificity shared classes, demos override as needed |
| Merge conflicts with ongoing work | Single shared file, clear ownership |

---

*This plan creates a unified design system for all layered-nlp demos while preserving each demo's unique functionality.*

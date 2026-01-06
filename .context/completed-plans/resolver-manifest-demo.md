# Resolver Manifest Demo Architecture

> Learnings relevant to future gates should be written back to respective gates, so future collaborators can benefit.

**Status:** Complete

**Goal:** Create a central ResolverManifest system that drives demo rendering from Rust, enabling automatic discovery of new span types and user-controlled visibility via tags.

## Motivation

Currently the WASM demo has:
- 14 hardcoded resolver runs in a specific order
- 10 separate extraction loops (one per span type) in lib.rs lines 240-500
- Hardcoded HTML/CSS for each span badge in contract-viewer.html
- No visibility into experimental resolvers

This creates drift where Rust work doesn't surface in demos. Adding a new span type requires changes in 4+ places.

## Scope

**In Scope:**
- ResolverManifest struct with metadata (name, color, tags, extractor)
- ResolverTag enum for stability classification
- Single extraction loop driven by manifest
- Dynamic UI generation from manifest metadata
- Tag-based filtering in UI
- Experimental resolver warnings

**Out of Scope:**
- Resolver trait implementations (utilities remain standalone)
- Automatic resolver discovery via macros/inventory (static manifest is sufficient)
- Associated span depth control (future enhancement)
- Semantic infrastructure demo integration (separate follow-on plan when utilities have resolver wrappers)

**Dependencies:**
- None

---

## Gate 1: Manifest Types

**Goal:** Define the core types for resolver metadata.

**Deliverables:**

| File | Content |
|------|---------|
| `layered-nlp-demo-wasm/src/manifest.rs` | `ResolverManifest`, `ResolverTag`, `RawSpan`, `RawAssociation` |
| `layered-nlp-demo-wasm/src/lib.rs` | Export manifest module |

**Types:**

```rust
/// Serializable projection of AssociatedSpan for WASM boundary
pub struct RawAssociation {
    pub label: String,
    pub glyph: Option<String>,
    pub target_start: usize,
    pub target_end: usize,
}

pub struct RawSpan {
    pub start: usize,
    pub end: usize,
    pub label: String,
    pub metadata: Option<serde_json::Value>,
    pub associations: Vec<RawAssociation>,  // Serializable version
}

pub struct ResolverManifest {
    pub name: &'static str,
    pub description: &'static str,
    pub color: &'static str,  // CSS hex color
    pub tags: &'static [ResolverTag],
    pub extract: fn(&LLLine) -> Vec<RawSpan>,
}

pub enum ResolverTag {
    Stable,
    Experimental,
}
// Note: Additional tags (granularity, domain) deferred until UI use case is proven
// Note: All current resolvers are Stable. The Experimental tag enables future experimental resolvers to show with ⚠️ warning. No experimental resolvers are defined in this plan.
```

**Acceptance:**
- [x] Types compile and are exported from crate
- [x] ResolverTag serializes to lowercase string for JS consumption
- [x] Unit test: manifest creation with all tag variants

**Learnings:**
- Changed positions from `usize` to `u32` for consistency with existing `Span` type in lib.rs

---

## Gate 2: Extractor Functions (10 Span Types)

**Goal:** Create extraction functions for all 10 existing span types.

**Note:** Extractors should use `u32` for span positions (matching RawSpan type).

**Pre-work:** Capture current extraction timing baseline using sample contract before implementing extractors

**Deliverables:**

| File | Content |
|------|---------|
| `layered-nlp-demo-wasm/src/extractors.rs` | 10 extraction functions |
| `layered-nlp-demo-wasm/src/manifest.rs` | Static RESOLVER_MANIFESTS array |

**Extraction functions to create:**
1. `extract_contract_keywords` -> ContractKeyword
2. `extract_defined_terms` -> Scored<DefinedTerm>
3. `extract_term_references` -> Scored<TermReference>
4. `extract_pronoun_references` -> Scored<PronounReference>
5. `extract_obligation_phrases` -> Scored<ObligationPhrase>
6. `extract_pronoun_chains` -> Scored<PronounChain>
7. `extract_contract_clauses` -> Scored<ContractClause>
8. `extract_clause_aggregates` -> Scored<ClauseAggregate>
9. `extract_obligation_nodes` -> Scored<ObligationNode>
10. `extract_deictic_references` -> DeicticReference (all categories in one extractor)

**Manifest color assignments:**
- ContractKeyword: `#ff6b6b`, [Stable]
- DefinedTerm: `#4ecdc4`, [Stable]
- TermReference: `#95e1d3`, [Stable]
- PronounReference: `#f38181`, [Stable]
- ObligationPhrase: `#45b7d1`, [Stable]
- PronounChain: `#aa96da`, [Stable]
- ContractClause: `#fcbad3`, [Stable]
- ClauseAggregate: `#a8d8ea`, [Stable]
- ObligationNode: `#ffd3b6`, [Stable]
- DeicticReference: `#dcedc1`, [Stable]

**Acceptance:**
- [x] All 10 extractors produce same output as current inline code
- [x] RESOLVER_MANIFESTS contains all 10 entries
- [x] Manual verification: extractors produce visually identical output when tested with sample contract in browser

**Learnings:**
- `format_deictic_label` must handle all 23 `DeicticSubcategory` variants exhaustively - use explicit match arms, not wildcards
- Use unicode arrow `→` (U+2192) not ASCII `->` for visual consistency
- Provenance associations via `query_with_associations()` deferred to Gate 3 - extractors return empty `associations: vec![]`
- Duplicate `format_deictic_label` exists in lib.rs and extractors.rs - Gate 3 should consolidate when replacing inline code

Note: Formal snapshot testing infrastructure (insta) may be added in future iteration

---

## Gate 3: Unified Extraction Loop

**Goal:** Replace 10+ extraction loops with single manifest-driven loop.

**Deliverables:**

| File | Change |
|------|--------|
| `layered-nlp-demo-wasm/src/lib.rs` | Replace lines 240-500 with single loop |
| `layered-nlp-demo-wasm/src/lib.rs` | Add `#[wasm_bindgen] get_resolver_manifests() -> JsValue` |

**New extraction pattern:**
```rust
let mut all_spans = Vec::new();
for manifest in RESOLVER_MANIFESTS {
    for raw in (manifest.extract)(ll_line) {
        all_spans.push(WasmSpan {
            kind: manifest.name.to_string(),
            color: manifest.color.to_string(),
            tags: serialize_tags(manifest.tags),
            start_offset: raw.start as u32,
            end_offset: raw.end as u32,
            label: raw.label,
            metadata: raw.metadata,
        });
    }
}
```

**WasmSpan extension:**
- Add `color: String` field
- Add `tags: Vec<String>` field

**Note:** When replacing lib.rs extraction loops, also remove the duplicate `format_deictic_label` function from lib.rs (keep the one in extractors.rs).

**Acceptance:**
- [x] Demo produces identical output to current implementation
- [x] `cargo test -p layered-nlp-demo-wasm` passes
- [x] Performance: extraction time within 10% of baseline (captured in Gate 2)

**Learnings:**
- Tag serialization: Use pattern matching (`match t { Stable => "stable" }`) not `serde_json::to_string()` which produces double-quoted strings
- Removed 260 lines of per-type loops, replaced with 14-line manifest-driven loop
- `format_deictic_label` consolidated - removed from lib.rs, now only in extractors.rs
- `WasmResolverManifest` provides serializable projection without function pointer

---

## Gate 4: Dynamic UI Generation

**Goal:** Generate UI elements from manifest metadata instead of hardcoding.

**Deliverables:**

| File | Change |
|------|--------|
| `web/contract-viewer.html` | Dynamic toggle generation |
| `web/shared-styles.css` | Remove hardcoded badge colors |

**UI changes:**
1. Manifest metadata exposed via `get_resolver_manifests()` WASM function
2. JS generates filter toggles from manifest array
3. Badge colors read from span.color, not CSS classes
4. Tags shown as filter buttons
5. Experimental resolvers show warning icon

**Filter behavior:**
- Default: show only Stable tags
- Tag buttons toggle visibility
- "Show Experimental" master toggle
- Per-resolver checkboxes within active tags

**Acceptance:**
- [x] No hardcoded span types in HTML
- [x] Adding new manifest entry -> toggle appears automatically
- [x] Experimental spans hidden by default, shown with warning when enabled
- [x] Manual verification: demo renders correctly in Chrome/Firefox with all toggles functional

**Learnings:**
- Use ⚠️ emoji prefix for experimental warning (more visible than "(exp)" suffix)
- Wrap manifest loading in try-catch for WASM error resilience
- `span.color` from manifest enables inline styles, removing need for per-type CSS classes
- Stable checked by default: `input.checked = manifest.tags.includes('stable')`
- Master "Show Experimental" toggle deferred - individual toggles sufficient for now

---

## Gate 5: Polish and Documentation

**Goal:** Final polish and documentation.

**Deliverables:**
- Update CLAUDE.md with manifest pattern documentation
- README section on adding new resolvers

**Adding a new resolver (documented pattern):**
1. Create extraction function in `extractors.rs`
2. Add entry to `RESOLVER_MANIFESTS` with name, color, tags
3. Done - appears in demo automatically

**Acceptance:**
- [x] Documentation complete
- [x] `mise run demo` works
- [x] End-to-end: add test resolver, verify it appears

**Learnings:**
- Documentation pattern: 3 simple steps (extraction function, manifest entry, done)
- The manifest architecture successfully eliminated 260 lines of per-type extraction loops
- Zero-boilerplate resolver addition verified: only 2 files need changes

---

## Risks

| Risk | Mitigation |
|------|------------|
| Extraction function drift | Snapshot tests comparing old vs new output |
| Performance regression | Benchmark before/after |
| JS color parsing issues | Use standard hex colors, test in browsers |

## Non-Goals

- Macro-based automatic registration (static manifest sufficient)
- Resolver trait implementations (semantic infra is utilities)
- Associated span visualization (future work, separate plan)

# FR-010: Dual-Layer Test Snapshot System

**Status: Planning Complete**  
**Created: December 2024**

> **Interesting artifacts and learnings must be written back to this document.**

---

## Overview

Introduce a dual-layer test snapshot system that separates **storage** (machine-readable RON) from **rendering** (human-readable views). This enables:

- **Precise regression testing** via canonical RON snapshots
- **Human-friendly review** via semantic summaries and visual overlays
- **Meaningful diffs** that highlight semantic changes, not formatting
- **Association visualization** showing span relationships

### Design Principles

1. **Storage is canonical** — RON format is the single source of truth
2. **Rendering is derived** — All human views are deterministically generated from storage
3. **Stable IDs** — Spans have deterministic IDs (`dt-0`, `ob-0`) for cross-referencing
4. **Diff-friendly** — Sorted, normalized output produces minimal diffs
5. **Scalable** — Large documents handled via filtering and elision

### Key Types (Preview)

```
Snapshot              — Canonical storage format (RON-serializable)
SpanData              — Individual span with ID, position, value, associations
SpanId                — Stable identifier (e.g., "dt-0", "ob-0")
DocDisplay            — Visual ASCII overlay renderer (extends LLLineDisplay)
SnapshotRenderer      — Semantic summary + graph renderer
assert_contract_snapshot! — Dual-output test macro
```

---

## The Test Experience

```rust
#[test]
fn obligation_with_term_reference() {
    let doc = ContractDocument::from_text(r#"
Section 1.1 Definitions
"Company" means Acme Corporation.
The Company shall deliver Products within 30 days.
    "#)
    .run_pipeline(&Pipeline::standard());

    // Single macro produces dual snapshots
    assert_contract_snapshot!("obligation_basic", doc);
}
```

**Produces two files:**

1. `obligation_basic_data.snap` — RON storage (canonical)
2. `obligation_basic_view.snap` — Human-readable rendering

### Rendered View Example

```
═══ CONTRACT ANALYSIS: obligation_basic ═══

📖 DEFINITIONS (1)
  [dt-0] "Company" → Acme Corporation
         (QuotedMeans, conf=0.90)

📎 REFERENCES (1)
  [tr-0] "The Company" →resolves_to→ [dt-0]
         (conf=0.85)

⚖️ OBLIGATIONS (1)
  [ob-0] Company SHALL deliver Products
         ├─ Obligor: @→[tr-0]
         ├─ Temporal: within 30 days ⏱→[te-0]
         └─ conf=0.80

🏗️ STRUCTURE (1)
  [sh-0] Section 1.1: Definitions
         (level=2, conf=0.95)

═══ ANNOTATED TEXT ═══

 1│ Section 1.1 Definitions
   │ ╰─────────────────────╯[sh-0] SectionHeader(1.1, "Definitions")
   │
 2│ "Company" means Acme Corporation.
   │  ╰─────╯[dt-0] DefinedTerm("Company") (0.90)
   │
 3│ The Company shall deliver Products within 30 days.
   │     ╰─────╯[tr-0] TermRef("Company") →[dt-0]
   │     ╰─────────────────────────────────────────╯[ob-0] Obligation
   │       └─@obligor→[tr-0]
   │                              ╰───────────────╯[te-0] Temporal(30 days)

═══ ASSOCIATION GRAPH ═══

[dt-0] DefinedTerm("Company")
    ↑ resolves_to
[tr-0] TermReference("Company")
    ↑ @obligor_source
[ob-0] ObligationPhrase(Duty, "deliver Products")
    └─⏱temporal_bound→ [te-0] TemporalExpression(30 days)
```

---

## Gate 0: Foundation — Core Storage Types ✅ COMPLETE

### Objectives

Define the canonical `Snapshot` data model with stable ID generation, RON serialization, and basic construction from `ContractDocument`.

### Scope

- `SnapshotSpanId` type with deterministic generation scheme
- `SnapshotDocPos` and `SnapshotDocSpan` for position representation
- `SpanData` struct with value, confidence, source, associations
- `AssociationData` struct for typed edges (structure only; population deferred to Gate 4)
- `Snapshot` struct with grouped spans
- `Snapshot::from_document()` construction (convenience wrapper around `SnapshotBuilder`)
- `SnapshotKind` trait for type-specific prefixes

### Dependencies

- None (new module)

### Task List

| # | Task | Status |
|---|------|--------|
| 0.1 | Create `layered-contracts/src/snapshot/mod.rs` module | ✅ |
| 0.2 | Define `SnapshotSpanId` newtype wrapping `String` | ✅ |
| 0.3 | Define `SnapshotDocPos` struct with `line: u32`, `token: u32` | ✅ |
| 0.4 | Define `SnapshotDocSpan` struct with `start`, `end` | ✅ |
| 0.5 | Define `AssociationData` with `label`, `target`, `glyph` | ✅ |
| 0.6 | Define `SpanData` with id, position, type_name, value, confidence, source, associations | ✅ |
| 0.7 | Define `InputSource` enum | ✅ |
| 0.8 | Define `Snapshot` struct with `BTreeMap` for deterministic ordering | ✅ |
| 0.9 | Define `SnapshotKind` trait (note: uses associated consts, not object-safe) | ✅ |
| 0.10 | Implement `SnapshotKind` for core types | ✅ |
| 0.11 | Implement `Snapshot::from_document()` via `SnapshotBuilder` | ✅ |
| 0.12 | Implement deterministic ID generation: `{prefix}-{index}` | ✅ |
| 0.13 | Implement `Snapshot` RON serialization via `serde` | ✅ |

### Verification

**Test Scenarios:** All implemented in `layered-contracts/src/snapshot/tests/gate0_storage.rs`

1. ✅ **ID stability** — Same document produces same IDs across multiple builds
2. ✅ **ID uniqueness** — No duplicate IDs within a snapshot
3. ✅ **Type grouping** — Spans grouped correctly by type name
4. ✅ **Position sorting** — Spans within each type sorted by position
5. ✅ **Association conversion** — DEFERRED to Gate 4; associations field exists but is empty
6. ✅ **RON round-trip** — `serialize -> deserialize -> serialize` produces identical output
7. ✅ **Empty document** — Empty input produces valid empty snapshot
8. ✅ **Input preservation** — Original text lines preserved exactly
9. ✅ **Confidence preservation** — Scored<T> confidence and source captured

**Implementation Notes:**

- Uses Debug representation for values (avoids adding Serialize to all types)
- `SnapshotBuilder` provides flexible type registration; `Snapshot::from_document()` is convenience wrapper
- ScoreSource serialization omits non-deterministic fields (pass_id, verifier_id) for stability
- Full redaction will be implemented in Gate 6

---

## Gate 1: Basic Rendering — Semantic Summary ✅ COMPLETE

### Objectives

Implement a basic semantic summary renderer that groups spans by category and produces diff-friendly output.

### Scope

- `SemanticCategory` enum for grouping
- `classify_type_name()` function
- `SnapshotRenderer` struct with configuration
- `render_semantic()` method producing grouped summary
- Large document handling (max spans per type)

### Dependencies

- Gate 0 complete ✅

### Task List

| # | Task | Status |
|---|------|--------|
| 1.1 | Define `SemanticCategory` enum with 6 categories | ✅ |
| 1.2 | Implement `classify_type_name()` | ✅ |
| 1.3 | Define category display info: name, emoji glyph | ✅ |
| 1.4 | Define `SnapshotRenderer` struct with config fields | ✅ |
| 1.5 | Implement `SnapshotRenderer::default()` | ✅ |
| 1.6 | Implement `render_semantic()` | ✅ |
| 1.7 | Add elision message when spans truncated (per-type) | ✅ |
| 1.8 | Add confidence display for low-confidence spans | ✅ |
| 1.9 | Association summary (deferred, shows when populated) | ✅ |

### Verification

**Test Scenarios:** All implemented in `layered-contracts/src/snapshot/tests/gate1_semantic.rs`

1. ✅ **Category grouping** — Spans appear under correct category headings
2. ✅ **Ordering** — Categories in consistent order, spans sorted by position
3. ✅ **Elision** — Per-type elision with message
4. ✅ **Confidence display** — Low confidence shown, high confidence hidden
5. ✅ **Association display** — Ready for associations (currently empty)
6. ✅ **Empty categories** — Categories with no spans are omitted
7. ✅ **Determinism** — Same snapshot produces identical render output
8. ✅ **Unicode handling** — Safe char-based truncation for non-ASCII text
9. ✅ **Multi-type elision** — Per-type limits preserve type diversity

**Implementation Notes:**

- Elision is per-type within each category (not per-category)
- Unicode truncation uses char count, not byte count, to avoid panics
- Builder pattern with `with_max_spans()`, `with_confidence_threshold()`, `with_show_confidence()`

---

## Gate 2: Dual Snapshot Macro ✅ COMPLETE

### Objectives

Implement the `assert_contract_snapshot!` macro that produces both RON storage and rendered view snapshots.

### Scope

- `assert_contract_snapshot!` macro
- Integration with `insta` crate
- Basic redaction for non-deterministic fields
- Dual file output (`_data.snap`, `_view.snap`)

### Dependencies

- Gate 0 complete ✅
- Gate 1 complete ✅

### Task List

| # | Task | Status |
|---|------|--------|
| 2.1 | Add `insta` as dev-dependency | ✅ (was already present) |
| 2.2 | Implement `Snapshot::redact(self) -> Self` | ✅ |
| 2.3 | Define `assert_contract_snapshot!` macro | ✅ |
| 2.4 | Macro produces `{name}_data.snap` with RON | ✅ |
| 2.5 | Macro produces `{name}_view.snap` with semantic render | ✅ |
| 2.6 | Add `#[macro_export]` for external use | ✅ |
| 2.7 | Document macro usage in module docs | ✅ |

### Verification

**Test Scenarios:** All implemented in `layered-contracts/src/snapshot/tests/gate2_macro.rs`

1. ✅ **RON validity** — RON data is valid and parseable
2. ✅ **View readability** — View contains semantic summary with categories and IDs
3. ✅ **Redaction** — LLMPass sources redacted, non-LLM sources unchanged
4. ✅ **Re-run stability** — Multiple builds produce identical output
5. ✅ **Type accessibility** — All types needed by macros are properly exported

**Implementation Notes:**

- Macro name parameter must be a string **literal** (not expression) for `concat!` to work
- RON snapshot uses redacted snapshot; view uses raw snapshot to preserve detail
- Three macros provided: `assert_contract_snapshot!`, `assert_contract_snapshot_data!`, `assert_contract_snapshot_view!`

---

## Gate 3: Visual ASCII Overlay — DocDisplay ✅ COMPLETE

### Objectives

Implement document-level visual annotation rendering that extends the `LLLineDisplay` pattern to multi-line documents.

### Scope

- `DocDisplay` struct with filtering and configuration
- ASCII overlay rendering for single-line spans
- Multi-line span rendering with continuation markers
- Association arrow rendering
- Span label system (`[A]`, `[B]`, etc.)

### Dependencies

- Gate 0 complete ✅
- Study existing `src/ll_line/display.rs` ✅

### Task List

| # | Task | Status |
|---|------|--------|
| 3.1 | Define `DocDisplay` struct with `snapshot` reference and config | ✅ |
| 3.2 | Implement `DocDisplay::new(snapshot)` constructor | ✅ |
| 3.3 | Implement `include(type_name) -> Self` filter | ✅ |
| 3.4 | Implement `include_with_associations(type_name) -> Self` | ✅ |
| 3.5 | Implement `verbose(self) -> Self` | ✅ |
| 3.6 | Build line-to-spans index from snapshot | ✅ |
| 3.7 | Implement span label assignment (`[A]`, `[B]`, ...) | ✅ |
| 3.8 | Implement single-line span rendering | ✅ |
| 3.9 | Implement multi-line span start marker (`╭`) | ✅ |
| 3.10 | Implement multi-line span continuation (`│`) | ✅ |
| 3.11 | Implement multi-line span end marker (`╰╯`) | ✅ |
| 3.12 | Implement association arrow rendering | ✅ |
| 3.13 | Implement overlapping span stacking | ✅ |
| 3.14 | Implement confidence display (compact vs verbose) | ✅ |
| 3.15 | Implement `Display` trait for `DocDisplay` | ✅ |
| 3.16 | Add `Snapshot::render_annotated() -> String` convenience method | ✅ |

### Verification

**Test Scenarios:** All implemented in `layered-contracts/src/snapshot/tests/gate3_display.rs`

1. ✅ **Single-line span** — Span within one line renders with underline markers
2. ✅ **Multi-line span** — Span across 3 lines shows `╭`, `│`, `╰` correctly
3. ✅ **Overlapping spans** — Two spans at same position stack vertically
4. ✅ **Nested spans** — Inner span renders inside outer span visually
5. ✅ **Association arrows** — `@obligor→[dt-0]` appears below obligation span
6. ✅ **Label assignment** — Spans referenced by associations get `[A]`, `[B]` labels
7. ✅ **Label filtering** — Labels only assigned to targets that are included in display
8. ✅ **Type filtering** — Only included types appear in output
9. ✅ **Line numbers** — Line numbers appear in left margin
10. ✅ **Long lines** — Lines > 80 chars handled gracefully
11. ✅ **Unicode handling** — Char-based truncation for Unicode safety
12. ✅ **Determinism** — Same snapshot produces identical output

**Implementation Notes:**

- Uses `╭` / `│` / `╰╯` for multi-line span markers
- `estimate_char_pos()` is a temporary heuristic; real token positions deferred to Gate 5/6
- Labels only assigned to targets that pass `should_include()` to avoid confusing references
- FileRef snapshots show placeholder message directing user to .ron file

---

## Gate 4: Association Graph View ✅ COMPLETE

### Objectives

Implement a dedicated association graph renderer showing relationships between spans.

### Scope

- `render_graph()` method on `Snapshot`
- Tree-style relationship visualization
- Bidirectional relationship display
- Integration into combined view

### Dependencies

- Gate 0 complete ✅
- Gate 1 complete ✅

### Task List

| # | Task | Status |
|---|------|--------|
| 4.1 | Build span lookup map: `SpanId -> &SpanData` | ✅ |
| 4.2 | Build reverse association index: target → sources | ✅ |
| 4.3 | Implement tree rendering for outgoing associations | ✅ |
| 4.4 | Implement reverse association indicator | ✅ |
| 4.5 | Sort associations for determinism | ✅ |
| 4.6 | Group graph by semantic category | ✅ |
| 4.7 | Implement `Snapshot::render_graph() -> String` | ✅ |
| 4.8 | Integrate graph into combined `_view` output | Deferred to Gate 5 |

### Verification

**Test Scenarios:** All implemented in `layered-contracts/src/snapshot/tests/gate4_graph.rs`

1. ✅ **Simple chain** — `TermReference → DefinedTerm` shows correctly
2. ✅ **Multiple outgoing** — Span with 3 associations shows all
3. ✅ **Reverse references** — Target span shows incoming `↑` indicator
4. ✅ **No associations** — Span with no associations omitted from graph
5. ✅ **Circular references** — Handles without infinite loop
6. ✅ **Self-reference** — Span referencing itself shows both outgoing and incoming
7. ✅ **Ordering** — Consistent ordering across runs
8. ✅ **Elision** — Per-category elision with message when max exceeded
9. ✅ **Glyph display** — Association glyphs rendered before label

**Implementation Notes:**

- `GraphRenderer` struct with configuration (max_spans_per_category, show_reverse_associations)
- Outgoing associations sorted by (label, target_id) for determinism
- Incoming associations sorted by (source_id, label) for determinism
- Only spans with associations (outgoing or incoming when enabled) appear in graph
- Value summaries truncated at 30 chars (char-based for Unicode safety)

---

## Gate 5: Combined View and Macro Enhancement ✅ COMPLETE

### Objectives

Combine all rendering modes into a unified view and enhance the macro with configuration options.

### Scope

- Combined `_view` output with all sections
- Macro configuration for filtering
- Verbose vs compact modes
- Section headers and navigation

### Dependencies

- Gates 1-4 complete ✅

### Task List

| # | Task | Status |
|---|------|--------|
| 5.1 | Define `SnapshotConfig` struct | ✅ |
| 5.2 | Implement `Snapshot::render_all(config) -> String` | ✅ |
| 5.3 | Add section headers with `═══` separators | ✅ |
| 5.4 | Implement filtering at storage level | ✅ |
| 5.5 | Enhance macro with config parameter | Deferred (existing macro works) |
| 5.6 | Add `SnapshotConfig::default()` | ✅ |
| 5.7 | Add `SnapshotConfig::minimal()` | ✅ |
| 5.8 | Add `SnapshotConfig::verbose()` | ✅ |
| 5.9 | Add `RenderMode` enum and `with_modes()` | ✅ |

### Verification

**Test Scenarios:** All implemented in `layered-contracts/src/snapshot/tests/gate5_combined.rs`

1. ✅ **Combined output** — `_view` contains semantic, annotated, and graph sections
2. ✅ **Config filtering** — Only specified types appear when filtered
3. ✅ **Minimal mode** — Only semantic summary in output
4. ✅ **Verbose mode** — Full confidence and source details
5. ✅ **Section navigation** — Headers make sections easy to find
6. ✅ **Default behavior** — No config produces reasonable default output
7. ✅ **Empty snapshot** — Shows "(no spans)", "(no text)", "(no associations)"
8. ✅ **Custom modes** — Can select subset of render modes
9. ✅ **Builder chaining** — All config methods chainable
10. ✅ **Determinism** — Same config produces identical output

**Implementation Notes:**

- `SnapshotConfig` with fields: `included_types`, `render_modes`, `verbose`, `max_spans_per_group`, `show_line_numbers`, `show_reverse_associations`
- `RenderMode` enum: `Semantic`, `Annotated`, `Graph`
- Presets: `default()`, `minimal()`, `verbose()`
- Type filtering sorts included_types for determinism across runs
- Empty states handled with placeholder messages

---

## Gate 6: Large Document Handling and Redaction

### Objectives

Handle large documents gracefully and implement comprehensive redaction for non-deterministic content.

### Scope

- Span elision for large documents
- Multi-line span collapse
- `RedactionConfig` with fine-grained control
- LLM content truncation

### Dependencies

- Gate 5 complete

### Task List

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 6.1 | Add `max_spans_per_line` to `DocDisplay` | Elides when exceeded |
| 6.2 | Implement line elision message | "... N more spans on this line" |
| 6.3 | Implement multi-line span collapse | Spans > N lines show `... (M lines)` |
| 6.4 | Add collapse threshold config | Default: 5 lines |
| 6.5 | Define `RedactionConfig` struct | `redact_llm_ids`, `redact_timestamps`, `truncate_llm_text` |
| 6.6 | Implement `Snapshot::apply_redactions(config)` | Applies redaction rules |
| 6.7 | Implement LLM text truncation | Truncates to N chars with `...` |
| 6.8 | Add header note for elided content | "NOTE: Some spans elided; see .ron for full details" |
| 6.9 | Test with 100+ span document | Verify manageable output size |

### Verification

**Test Scenarios:**

1. **Large document** — 100+ spans produces reasonable `_view` size
2. **Per-line elision** — 20+ spans on one line shows elision
3. **Multi-line collapse** — 10-line span collapses to 3 lines
4. **LLM ID redaction** — Pass IDs replaced with `[REDACTED]`
5. **Text truncation** — Long LLM text truncated
6. **Elision note** — Header shows when content elided
7. **RON completeness** — `_data.snap` still contains full data

**Test Organization:**
- Location: `layered-contracts/src/snapshot/tests/gate6_scale.rs`
- Naming: `test_large_doc_<scenario>`

**Pass Criteria:**
- Large documents produce manageable output
- No information loss in RON storage
- Redaction prevents non-deterministic failures

---

## Gate 7: Migration and Documentation

### Objectives

Add `SnapshotKind` to all resolvers, migrate existing tests, and document the system.

### Scope

- `SnapshotKind` impls for all attribute types
- Migration of key existing tests
- Documentation in `ARCHITECTURE.md`
- Example tests demonstrating patterns

### Dependencies

- Gate 6 complete

### Task List

| # | Task | Acceptance Criteria |
|---|------|---------------------|
| 7.1 | Implement `SnapshotKind` for all line-level attribute types | All types have unique prefixes |
| 7.2 | Implement `SnapshotKind` for all document-level attribute types | All types have unique prefixes |
| 7.3 | Create prefix registry documentation | Table of type → prefix mappings |
| 7.4 | Migrate 3-5 existing integration tests to new format | Tests produce dual snapshots |
| 7.5 | Verify migrated tests pass | No regressions |
| 7.6 | Add "Testing" section to `ARCHITECTURE.md` | Documents snapshot system |
| 7.7 | Create `docs/testing.md` with detailed guide | Examples, patterns, troubleshooting |
| 7.8 | Add example test demonstrating each feature | Filtering, verbose, associations |
| 7.9 | Document CI workflow for snapshot review | How to review and accept changes |

### Verification

**Test Scenarios:**

1. **All types covered** — Every attribute type has `SnapshotKind` impl
2. **Unique prefixes** — No prefix collisions
3. **Migration equivalence** — Migrated tests catch same regressions as before
4. **Documentation accuracy** — Examples in docs compile and work

**Test Organization:**
- Location: `layered-contracts/src/snapshot/tests/gate7_migration.rs`
- Add example tests: `layered-contracts/tests/snapshot_examples.rs`

**Pass Criteria:**
- All attribute types supported
- Existing test coverage preserved
- Documentation complete and accurate

---

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Rendering complexity for multi-line spans | Medium | Medium | Start simple; iterate based on real examples |
| Snapshot churn from minor changes | Medium | High | Strict determinism; sort everything |
| Large snapshots hard to review | Medium | Medium | Elision, filtering, collapsed spans |
| ID scheme proves insufficient | Low | Medium | Can migrate to content-hash later |
| Unicode rendering issues | Low | Low | ASCII fallback mode |

---

## Success Metrics

After Gate 7 completion:

- **Dual snapshots** for all new tests
- **Readable diffs** — semantic changes produce small, meaningful diffs
- **Association visibility** — relationships clear in `_view` output
- **Large document support** — 100+ page contracts produce manageable output
- **Migration path** — existing tests can adopt incrementally

---

## Appendix: File Structure

```
layered-contracts/
├── src/
│   ├── snapshot/
│   │   ├── mod.rs              # Module root, re-exports, macro
│   │   ├── types.rs            # SpanId, DocPos, DocSpan, SpanData, Snapshot
│   │   ├── construction.rs     # Snapshot::from_document()
│   │   ├── semantic.rs         # SemanticCategory, render_semantic()
│   │   ├── display.rs          # DocDisplay, render_annotated()
│   │   ├── graph.rs            # render_graph()
│   │   ├── redaction.rs        # RedactionConfig, apply_redactions()
│   │   ├── config.rs           # SnapshotConfig, SnapshotRenderer
│   │   └── tests/
│   │       ├── mod.rs
│   │       ├── gate0_storage.rs
│   │       ├── gate1_semantic.rs
│   │       ├── gate2_macro.rs
│   │       ├── gate3_display.rs
│   │       ├── gate4_graph.rs
│   │       ├── gate5_combined.rs
│   │       ├── gate6_scale.rs
│   │       └── gate7_migration.rs
│   └── lib.rs                  # Add `pub mod snapshot;`
├── tests/
│   ├── snapshot_integration.rs
│   └── snapshot_examples.rs
└── docs/
    └── testing.md              # Comprehensive testing guide
```

---

## Appendix: ID Prefix Registry

| Attribute Type | Prefix | Example ID |
|----------------|--------|------------|
| `DefinedTerm` | `dt` | `dt-0`, `dt-1` |
| `TermReference` | `tr` | `tr-0` |
| `ObligationPhrase` | `ob` | `ob-0` |
| `SectionHeader` | `sh` | `sh-0` |
| `TemporalExpression` | `te` | `te-0` |
| `ContractKeyword` | `kw` | `kw-0` |
| `PronounReference` | `pr` | `pr-0` |
| `Coordination` | `co` | `co-0` |
| `BridgingReference` | `br` | `br-0` |
| `Conflict` | `cf` | `cf-0` |
| `RecitalSection` | `rc` | `rc-0` |
| `AppendixBoundary` | `ab` | `ab-0` |
| `FootnoteBlock` | `fn` | `fn-0` |
| `PrecedenceRule` | `pc` | `pc-0` |
| (auxiliary text) | `txt` | `txt-0` |

---

## Learnings Log

> Record interesting discoveries, design decisions, and lessons learned during implementation.

| Gate | Learning |
|------|----------|
| Gate 0 | Using Debug representation for values instead of requiring Serialize avoids modifying 20+ existing types. Tradeoff: snapshots will change if Debug impls change. |
| Gate 0 | `SnapshotKind` uses associated consts, not methods, so it's not object-safe. This is fine since we always use it generically via `T: SnapshotKind`. Updated plan to remove "object-safe" claim. |
| Gate 0 | Association conversion deferred to Gate 4. The associations field exists in SpanData but remains empty until we implement the graph rendering gate. This avoids premature coupling to resolver internals. |
| Gate 0 | ScoreSource serialization omits pass_id and verifier_id for snapshot determinism. Full redaction control will be in Gate 6. |
| Gate 0 | Input preservation uses `original_text()` from ContractDocument rather than reconstructing from tokens. This ensures exact line fidelity. |
| Gate 1 | Elision is per-type, not per-category. This prevents a noisy type from hiding rarer but important types in the same category. |
| Gate 1 | Unicode truncation must use char count, not byte slicing. Byte slicing can panic on multi-byte UTF-8 characters. |
| Gate 1 | Categories sorted in a fixed order (Definitions, References, Obligations, Structure, Temporal, Other) for deterministic output. |
| Gate 2 | Macro `$name` parameter must be `:literal` not `:expr` because `concat!` requires string literals. |
| Gate 2 | RON snapshot uses redacted version; view uses raw version to preserve semantic detail. |
| Gate 2 | Redaction only affects LLMPass sources; RuleBased and other sources remain unchanged. |
| Gate 3 | Token-to-character position mapping (`estimate_char_pos`) is a heuristic; real token metadata deferred to Gate 5/6. |
| Gate 3 | Labels (`[A]`, `[B]`) should only be assigned to targets that are actually included in the display to avoid confusing references to invisible spans. |
| Gate 3 | Multi-line spans use `╭` / `│` / `╰╯` markers; continuation lines placed at column 0 for simplicity until proper token positions available. |
| Gate 3 | FileRef snapshots can't display annotated overlays meaningfully; show placeholder directing users to .ron file. |
| Gate 4 | Graph only shows spans that have associations (outgoing, or incoming when enabled); spans without any edges are excluded for cleaner output. |
| Gate 4 | Associations sorted by (label, target_id) outgoing and (source_id, label) incoming for determinism. |
| Gate 4 | Self-references and circular references handled naturally by non-recursive traversal; no special cases needed. |
| Gate 4 | Non-string RON values also get char-based truncation to prevent huge Debug output from blowing up graph lines. |
| Gate 5 | `SnapshotConfig::with_types()` replaces (not extends) the included types set; documented explicitly. |
| Gate 5 | Type filtering must sort included_types before iteration for determinism across process runs (HashSet iteration order varies). |
| Gate 5 | Verbose mode sets confidence threshold to 1.0, meaning all confidence scores are shown regardless of value. |
| Gate 5 | Empty render_modes produces empty output; could add guard in future if this becomes a footgun. |

---

*Update this document as implementation progresses.*

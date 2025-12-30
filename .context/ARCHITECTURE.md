# Layered-NLP Architecture

This document describes the crate structure and key architectural patterns for the layered-nlp workspace.

## Crate Dependency Graph

```
layered-nlp (core)
   ↓
layered-nlp-document (document infrastructure)
   ↓
layered-contracts (contract-specific logic)
   ↓
layered-nlp-demo-wasm (browser demo)
```

### Core Crate: `layered-nlp`

The foundation crate providing line-level tokenization and pattern matching.

**Key types:**
- `LLLine` — Single line of tokenized text with attached attributes
- `Resolver` trait — Interface for recognizers that analyze text
- `LLSelection` — Span within an LLLine for pattern matching
- Matchers (`x::attr`, `x::seq`, `x::token_text`, etc.)

**Philosophy:** Everything is a span. Each resolver adds one layer of understanding.

### Document Infrastructure: `layered-nlp-document`

Generic document-level abstractions extracted from layered-contracts.

**Key types:**
- `LayeredDocument` — Multi-line document with cross-line structure
- `DocPosition` / `DocSpan` — Position within documents
- `Scored<T>` — Values with confidence scores (0.0-1.0)
- `ScoreSource` — Origin of confidence: RuleBased, LLMPass, HumanVerified, Derived
- `ProcessResult<T>` — Result wrapper with errors and warnings

**Design principle:** Domain-agnostic infrastructure. No contract-specific logic.

### Contract Analysis: `layered-contracts`

Contract language analysis built on top of layered-nlp-document.

**Type alias for backward compatibility:**
```rust
pub type ContractDocument = layered_nlp_document::LayeredDocument;
```

**Key resolvers (line-level):**
- `SectionHeaderResolver` — Section headers (Section 3.1, Article IV)
- `ContractKeywordResolver` — Modal verbs (shall, may, must)
- `DefinedTermResolver` — Defined terms ("Company" means...)
- `TermReferenceResolver` — Links term references to definitions
- `ObligationPhraseResolver` — Obligation detection with obligor/action
- `TemporalExpressionResolver` — Time expressions (within 30 days)
- `PronounResolver` — Pronoun-to-antecedent resolution

**Key resolvers (document-level):**
- `DocumentStructureBuilder` — Hierarchical section tree
- `SectionReferenceLinker` — Cross-reference resolution

**Contract diff system:**
- `DocumentAligner` — Aligns sections between versions
- `SemanticDiffEngine` — Detects semantic changes with risk levels
- `TokenAligner` — Word-level diff for change highlighting

**Snapshot testing:**
- `Snapshot` — Canonical RON storage format
- `SnapshotBuilder` — Extracts spans from documents
- `SnapshotKind` trait — Type-specific ID prefixes (dt, tr, ob, sh, te, kw)

### Ancillary Crates

| Crate | Purpose |
|-------|---------|
| `layered-part-of-speech` | POS tagging using wiktionary data |
| `layered-amount` | Numeric amount recognition |
| `layered-clauses` | Clause boundary detection |
| `layered-deixis` | Deictic expression detection |
| `layered-nlp-demo-wasm` | Browser-based contract analyzer |

## Key Architectural Patterns

### 1. Two-Level Resolver Architecture

**Line Level:** `Resolver` trait → `LLLine.run()` → `LLCursorAssignment<Attr>`

**Document Level:** `DocumentResolver` trait → `ContractDocument.run_document_resolver()`

### 2. Spans Stack at Same Position

Multiple spans can exist at the same position. This enables:
- N-best interpretations (AI proposes alternatives)
- Multi-perspective views (different reviewers)
- Layered enrichment (each resolver adds without overwriting)

### 3. Confidence is Explicit

`Scored<T>` wraps semantic attributes with:
- `confidence: f64` — 0.0-1.0, where 1.0 = verified
- `source: ScoreSource` — provenance tracking

### 4. Re-export Pattern

`layered-contracts` re-exports from `layered-nlp-document`:
```rust
pub use layered_nlp_document::{
    DocPosition, DocSpan, LayeredDocument, ProcessError, ProcessResult,
    Scored, ScoreSource,
};
```

### 5. Type Alias for Backward Compatibility

```rust
pub type ContractDocument = layered_nlp_document::LayeredDocument;
```

Allows existing code using `ContractDocument` to continue working.

## File Locations

| Component | Location |
|-----------|----------|
| Core line-level types | `src/ll_line.rs` |
| Document types | `layered-nlp-document/src/document.rs` |
| Scoring infrastructure | `layered-nlp-document/src/scored.rs` |
| Contract resolvers | `layered-contracts/src/` |
| Snapshot system | `layered-contracts/src/snapshot/` |
| Contract diff | `layered-contracts/src/semantic_diff.rs`, `token_diff.rs` |
| Pipeline presets | `layered-contracts/src/pipeline/` |

---

*Last updated: December 2024*

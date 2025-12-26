# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Build the entire workspace
cargo build

# Run tests for the entire workspace
cargo test

# Run a single test
cargo test test_name

# Run tests for a specific crate
cargo test -p layered-nlp
cargo test -p layered-part-of-speech
cargo test -p layered-amount
cargo test -p layered-clauses
cargo test -p layered-contracts
cargo test -p layered-deixis
cargo test -p layered-nlp-demo-wasm

# Run performance benchmark (release mode)
cargo test -p layered-nlp-demo-wasm test_performance_benchmark --release -- --nocapture

# Run clippy lints
cargo clippy

# Run example
cargo run --example position_of_speech
```

## Architecture Overview

layered-nlp is a data-oriented NLP framework for incrementally building up recognizers that can produce multiple interpretations of text spans.

### Core Philosophy

The system follows five principles that should guide all development:

1. **Everything is a span** — `SpanRef` (line-local) / `DocSpan` (document) define positions. All semantic content has a position.

2. **Semantics live in attributes** — Each resolver introduces new typed structs attached to spans. Don't embed meaning in code paths; make it queryable data.

3. **Relationships are first-class** — `AssociatedSpan` / `DocAssociatedSpan` create typed edges between spans for provenance tracking and graph traversal.

4. **Confidence is explicit** — `Scored<T>` wraps semantic attributes with confidence (0.0-1.0) and source tracking (`RuleBased`, `LLMPass`, `HumanVerified`, `Derived`).

5. **Queries are type-driven** — `TypeId`-based lookup (`query::<T>()`, `query_all::<T>()`) for attributes; positional methods for geometry.

### Two-Level Resolver Architecture

**Line Level:** `Resolver` trait → `LLLine.run()` → `LLCursorAssignment<Attr>`
- Operates on single tokenized lines
- Uses `SpanRef` for token ranges within a line
- Pattern matching via `LLSelection` and matchers (`x::attr()`, `x::seq()`, etc.)

**Document Level:** `DocumentResolver` trait → `ContractDocument.run_document_resolver()` → `SemanticSpan`
- Operates across multiple lines
- Uses `DocSpan` for cross-line ranges
- Spans stored in `SpanIndex` (BTreeMap-based, O(log n) lookup)

### Key Architectural Insight: Spans Stack

**Multiple spans can exist at the same position.** This is not a limitation—it's the feature that enables:
- N-best interpretations (AI proposes alternatives)
- Multi-perspective views (different reviewers annotate independently)
- Layered enrichment (each resolver adds without overwriting)

This means the system preserves all interpretations rather than forcing one "truth."

### Mentality for Working in This Codebase

1. **Think in layers** — Each resolver adds one type of understanding. Don't try to do everything in one resolver. Chain them: `doc.run_resolver(&A).run_resolver(&B)`.

2. **Query, don't assume** — Use `query::<T>()` to find what previous resolvers produced. Don't hardcode dependencies; make them explicit via `dependencies()` method.

3. **Create associations** — When your resolver uses another span as input, create an `AssociatedSpan` pointing to it. This builds the provenance graph automatically.

4. **Use `Scored<T>` for semantic attributes** — Structural attributes (headers, boundaries) can be unscored. Semantic interpretations (obligations, references, conflicts) must carry confidence.

5. **Test with snapshots** — Use `insta` and `LLLineDisplay` to visualize what your resolver produces. Snapshot tests catch regressions in span coverage.

6. **Consult the architecture docs** — See `.context/ARCHITECTURE.md` for patterns, `.context/ARCHITECTURE-EXAMPLES.md` for worked examples, and `.context/plans/` for the implementation roadmap.

### Core Concepts

**LLLine** (`src/ll_line.rs`): The main data structure representing a line of tokenized text with attached attributes. Created via `create_line_from_string()` or `create_line_from_input_tokens()`.

**Resolver trait** (`src/ll_line.rs:304`): The interface for recognizers that analyze text and assign attributes. Implement `go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>>` to create custom recognizers. Resolvers are chained via `ll_line.run(&resolver)`.

**LLSelection** (`src/ll_line/ll_selection.rs`): A span within an LLLine used for pattern matching. Key methods:
- `find_by(matcher)` - Find all matches of a pattern
- `match_forwards(matcher)` / `match_backwards(matcher)` - Extend selection in a direction
- `split_by(matcher)` - Split selection on matches
- `trim(matcher)` - Remove matching tokens from edges
- `finish_with_attr(value)` - Create an assignment from the current selection

**Matchers** (`src/ll_line/x/`): Combinators for pattern matching on tokens:
- `x::attr::<T>()` - Match any attribute of type T
- `x::attr_eq(&value)` - Match attribute equal to value
- `x::token_text()` - Get the text of a token
- `x::token_has_any(&[chars])` - Match token containing any of given chars
- `x::seq((a, b))` - Match sequence of patterns
- `x::all((a, b))` - Match all patterns at same position
- `x::any_of((a, b))` - Match any of the patterns
- `x::whitespace()` - Match whitespace

**TextTag** (`src/ll_line.rs:22`): Built-in token classification assigned during tokenization: `NATN` (natural number), `PUNC` (punctuation), `SYMB` (symbol), `SPACE` (whitespace), `WORD` (word).

### Workspace Structure

- `layered-nlp/` - Core library
- `layered-part-of-speech/` - Part-of-speech tagging resolver using wiktionary data
- `layered-amount/` - Numeric amount recognition (handles localized number formats)
- `layered-clauses/` - Clause detection (splits text on clause keywords)
- `layered-contracts/` - Contract language analysis (keywords, terms, obligations, document diff)
- `layered-deixis/` - Deictic expression detection (pronouns, place, time, discourse markers)
- `layered-nlp-demo-wasm/` - WASM demo for browser-based contract analysis
- `examples/` - Usage examples
- `docs/` - Additional documentation
- `web/` - HTML contract viewer (uses WASM demo)

### Testing

Tests use `insta` for snapshot testing. The `LLLineDisplay` type provides visual debugging output showing token spans and their attributes:

```
$  1  ,  000  .  25
╰USDDollars
   ╰──────────────╯Amount(1000.25)
╰─────────────────╯CurrencyAmount(USDDollars, Amount(1000.25))
```

Update snapshots with: `cargo insta review`

### Building Resolvers

See [docs/building-resolvers.md](docs/building-resolvers.md) for patterns and best practices when implementing new resolvers.

### Deixis Detection

The `layered-deixis` crate provides resolvers for detecting deictic expressions (context-dependent references):

```rust
use layered_deixis::{
    DeicticReference, PersonPronounResolver, PlaceDeicticResolver,
    SimpleTemporalResolver, DiscourseMarkerResolver,
};
use layered_nlp::create_line_from_string;

let line = create_line_from_string("I will meet you there tomorrow. However, plans may change.")
    .run(&PersonPronounResolver::new())   // Detects: I, you
    .run(&PlaceDeicticResolver::new())    // Detects: there
    .run(&SimpleTemporalResolver::new())  // Detects: tomorrow
    .run(&DiscourseMarkerResolver::new()); // Detects: However

// Query DeicticReference attributes
for find in line.find(&x::attr::<DeicticReference>()) {
    let deictic = find.attr();
    println!("{}: {:?} ({:?})", deictic.surface_text, deictic.category, deictic.subcategory);
}
// Output:
// I: Person (PersonFirst)
// you: Person (PersonSecond)
// there: Place (PlaceDistal)
// tomorrow: Time (TimeFuture)
// However: Discourse (DiscourseMarker)
```

Categories: `Person`, `Place`, `Time`, `Discourse`, `Social`

### Contract Diff System

The `layered-contracts` crate includes a semantic diff system for comparing contract versions:

```rust
use layered_contracts::{
    ContractDocument, DocumentAligner, DocumentStructureBuilder,
    SemanticDiffEngine, SectionHeaderResolver, ContractKeywordResolver,
    DefinedTermResolver, TermReferenceResolver, ObligationPhraseResolver,
};

// Process both document versions
let original = ContractDocument::from_text(original_text)
    .run_resolver(&SectionHeaderResolver::new())
    .run_resolver(&ContractKeywordResolver::new())
    .run_resolver(&DefinedTermResolver::new())
    .run_resolver(&TermReferenceResolver::new())
    .run_resolver(&ObligationPhraseResolver::new());

let revised = ContractDocument::from_text(revised_text)
    .run_resolver(&SectionHeaderResolver::new())
    .run_resolver(&ContractKeywordResolver::new())
    .run_resolver(&DefinedTermResolver::new())
    .run_resolver(&TermReferenceResolver::new())
    .run_resolver(&ObligationPhraseResolver::new());

// Build document structure
let orig_struct = DocumentStructureBuilder::build(&original).value;
let rev_struct = DocumentStructureBuilder::build(&revised).value;

// Align sections between versions
let aligner = DocumentAligner::new();
let alignments = aligner.align(&orig_struct, &rev_struct, &original, &revised);

// Compute semantic diff
let engine = SemanticDiffEngine::new();
let diff = engine.compute_diff(&alignments, &original, &revised);

// Analyze changes
for change in &diff.changes {
    println!("{:?}: {} (risk: {:?})",
        change.change_type, change.explanation, change.risk_level);
}
```

**Key components:**
- `DocumentAligner` - Aligns sections between document versions using title/content similarity
- `SemanticDiffEngine` - Detects semantic changes (modal changes, term redefinitions, temporal changes)
- `RiskLevel` - Classifies changes as Critical/High/Medium/Low
- `PartyImpact` - Tracks how changes affect each party (Favorable/Unfavorable/Neutral)

### WASM Demo

Build and serve the browser-based contract analyzer:

```bash
cd layered-nlp-demo-wasm && wasm-pack build --target web --out-dir ../web/pkg
cd ../web && python3 -m http.server 8080
# Open http://localhost:8080/contract-viewer.html
```

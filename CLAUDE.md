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

# Run clippy lints
cargo clippy

# Run example
cargo run --example position_of_speech
```

## Architecture Overview

layered-nlp is a data-oriented NLP framework for incrementally building up recognizers that can produce multiple interpretations of text spans.

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
- `layered-contracts/` - Contract language analysis (keywords, terms, obligations)
- `examples/` - Usage examples
- `docs/` - Additional documentation

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

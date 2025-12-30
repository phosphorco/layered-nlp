# Building Resolvers

This guide covers patterns for building effective resolvers in layered-nlp.

## Resolver Basics

A resolver implements the `Resolver` trait:

```rust
pub trait Resolver {
    type Attr: std::fmt::Debug + 'static + Send + Sync;
    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>>;
}
```

The resolver receives the full line as an `LLSelection` and returns assignments that attach attributes to token spans.

## Pattern 1: Simple Token Matching

For matching individual tokens by text content:

```rust
impl Resolver for MyResolver {
    type Attr = MyKeyword;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        selection
            .find_by(&x::token_text())
            .into_iter()
            .filter_map(|(sel, text)| {
                let keyword = self.match_keyword(&text.to_lowercase())?;
                Some(sel.finish_with_attr(keyword))
            })
            .collect()
    }
}
```

**Key points:**
- `find_by(&x::token_text())` returns all tokens with their text
- Use `filter_map` to skip non-matches
- `sel.finish_with_attr(value)` creates the assignment

## Pattern 2: Multi-Token Phrases

For matching phrases like "subject to" or "shall not":

```rust
// Inside the resolver's go() method
for (sel, text) in selection.find_by(&x::token_text()) {
    if text.to_lowercase() == "subject" {
        // Skip whitespace, then check next token
        if let Some((ws_sel, _)) = sel.match_first_forwards(&x::whitespace()) {
            if let Some((extended_sel, next)) = ws_sel.match_first_forwards(&x::token_text()) {
                if next.to_lowercase() == "to" {
                    results.push(extended_sel.finish_with_attr(SubjectTo));
                    continue;
                }
            }
        }
        // "subject" alone doesn't match
        continue;
    }
}
```

**Key points:**
- Use `match_first_forwards` to extend the selection
- Must explicitly handle whitespace between tokens
- The final `extended_sel` spans from first token through last matched

## Pattern 3: Building on Previous Resolvers

Resolvers can query attributes from earlier passes:

```rust
// Find tokens already tagged as Shall, then check for "not"
selection
    .find_by(&x::attr_eq(&ContractKeyword::Shall))
    .into_iter()
    .filter_map(|(sel, _)| {
        sel.match_first_forwards(&x::whitespace())
            .and_then(|(ws_sel, _)| {
                ws_sel.match_first_forwards(&x::token_text())
                    .filter(|(_, text)| text.to_lowercase() == "not")
                    .map(|(not_sel, _)| not_sel.finish_with_attr(ShallNot))
            })
    })
    .collect()
```

**Key points:**
- `x::attr_eq(&value)` matches spans with exact attribute value
- `x::attr::<T>()` matches any span with attribute type T
- For low-level experiments, chain resolvers manually: `.run(&Resolver1).run(&Resolver2)`
- For production use, encode dependencies in the pipeline (see "Pipeline Integration" below)

## Pattern 4: Configurable Keyword Lists

Make resolvers configurable for different use cases:

```rust
pub struct KeywordResolver {
    shall_keywords: Vec<&'static str>,
    may_keywords: Vec<&'static str>,
}

impl Default for KeywordResolver {
    fn default() -> Self {
        Self {
            shall_keywords: vec!["shall", "must"],
            may_keywords: vec!["may", "can"],
        }
    }
}

impl KeywordResolver {
    fn match_keyword(&self, text: &str) -> Option<Keyword> {
        if self.shall_keywords.contains(&text) {
            Some(Keyword::Shall)
        } else if self.may_keywords.contains(&text) {
            Some(Keyword::May)
        } else {
            None
        }
    }
}
```

## Available Matchers

From `layered_nlp::x`:

| Matcher | Purpose |
|---------|---------|
| `x::token_text()` | Get token's text content |
| `x::attr::<T>()` | Match any attribute of type T |
| `x::attr_eq(&value)` | Match exact attribute value |
| `x::whitespace()` | Match whitespace tokens |
| `x::token_has_any(&[chars])` | Match token containing any char |
| `x::seq((a, b))` | Match sequential patterns |
| `x::all((a, b))` | Match all patterns at same position |
| `x::any_of((a, b))` | Match any of the patterns |

## Selection Methods

| Method | Purpose |
|--------|---------|
| `find_by(matcher)` | Find all matches in selection |
| `find_first_by(matcher)` | Find first match only |
| `match_first_forwards(matcher)` | Extend selection forward |
| `match_first_backwards(matcher)` | Extend selection backward |
| `split_by(matcher)` | Split selection on delimiter |
| `trim(matcher)` | Remove matching tokens from edges |
| `finish_with_attr(value)` | Create assignment from selection |

## Testing with Snapshots

Use insta for visual snapshot testing:

```rust
fn test_resolver(input: &str) -> String {
    let ll_line = create_line_from_string(input)
        .run(&MyResolver::default());

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<MyAttr>();
    format!("{}", display)
}

#[test]
fn basic_test() {
    insta::assert_snapshot!(test_resolver("input text"), @r###"
    input     text
    ╰───╯MyAttr
    "###);
}
```

**Key points:**
- `LLLineDisplay::include::<T>()` adds attribute type to output
- Inline snapshots with `@r###"..."###` for easy review
- Run `cargo insta test --accept` to update snapshots

## Common Pitfalls

1. **Forgetting whitespace**: Tokens are separated by whitespace tokens. Use `match_first_forwards(&x::whitespace())` before matching next word.

2. **Case sensitivity**: Use `.to_lowercase()` for case-insensitive matching.

3. **Attribute bounds**: Attributes must be `Debug + 'static + Send + Sync`.

4. **Selection spans**: When extending selections, the final span includes all intermediate tokens (including whitespace).

---

## Pipeline Integration

The pipeline orchestrator (`layered_contracts::pipeline`) provides automatic dependency ordering for resolvers.

### Adding a Resolver to a Pipeline

Wrap existing resolvers with `with_meta()` to declare their dependencies and outputs:

```rust
use layered_contracts::pipeline::{Pipeline, with_meta};
use layered_contracts::{ContractKeyword, DefinedTerm, Scored};

let pipeline = Pipeline::new()
    // Resolver with no dependencies
    .with_line_resolver(
        with_meta("keywords", MyKeywordResolver::new())
            .produces::<ContractKeyword>()
    )
    // Resolver that depends on keywords
    .with_line_resolver(
        with_meta("terms", MyTermResolver::new())
            .depends_on::<ContractKeyword>()
            .produces::<Scored<DefinedTerm>>()
    );
```

**Key points:**
- `with_meta(id, resolver)` wraps the resolver with metadata
- `.produces::<T>()` declares the attribute type(s) this resolver produces
- `.depends_on::<T>()` declares required dependencies
- `.optional_depends_on::<T>()` declares optional dependencies
- The pipeline automatically sorts resolvers topologically

### Using Preset Pipelines

For common use cases, use preset pipelines:

```rust
use layered_contracts::pipeline::Pipeline;

// Core analysis presets
let doc = Pipeline::structure_only().run_on_text("Section 1.1")?;  // Headers only
let doc = Pipeline::fast().run_on_text("Article I...")?;            // Headers + keywords
let doc = Pipeline::standard().run_on_text("The Company shall...")?; // Full semantic analysis

// Enhanced presets for document-level structure (recitals, appendices, footnotes)
let doc = Pipeline::enhanced().run_on_text("WHEREAS...\nSection 1...\nEXHIBIT A")?;
let doc = Pipeline::enhanced_minimal().run_on_text("WHEREAS...\nSection 1...")?;  // No obligation analysis
```

The `enhanced` and `enhanced_minimal` presets are used internally by
[`EnhancedDocumentStructureBuilder`](../layered-contracts/src/enhanced_structure.rs)
to orchestrate full document-structure analysis.

### Migration Patterns

**Wrapper-based migration** (recommended for gradual adoption):

```rust
// Before: Manual ordering
let doc = ContractDocument::from_text(text)
    .run_resolver(&KeywordResolver::new())
    .run_resolver(&TermResolver::new())  // Must come after keywords
    .run_resolver(&ReferenceResolver::new());  // Must come after terms

// After: Pipeline handles ordering automatically
let pipeline = Pipeline::new()
    .with_line_resolver(
        with_meta("keywords", KeywordResolver::new())
            .produces::<Keyword>()
    )
    .with_line_resolver(
        with_meta("terms", TermResolver::new())
            .depends_on::<Keyword>()
            .produces::<Term>()
    )
    .with_line_resolver(
        with_meta("refs", ReferenceResolver::new())
            .depends_on::<Term>()
            .produces::<Reference>()
    );

let doc = pipeline.run_on_text(text)?;
```

**Native implementation** (for new resolvers):

```rust
use std::any::TypeId;
use layered_contracts::pipeline::{ResolverMeta, PipelinePhase};

impl ResolverMeta for MyResolver {
    fn id(&self) -> &'static str { "my_resolver" }
    
    fn produces(&self) -> &[TypeId] {
        static PRODUCES: std::sync::OnceLock<[TypeId; 1]> = std::sync::OnceLock::new();
        PRODUCES.get_or_init(|| [TypeId::of::<MyOutput>()])
    }
    
    fn requires(&self) -> &[TypeId] {
        static REQUIRES: std::sync::OnceLock<[TypeId; 1]> = std::sync::OnceLock::new();
        REQUIRES.get_or_init(|| [TypeId::of::<SomeInput>()])
    }
    
    fn phase(&self) -> PipelinePhase {
        PipelinePhase::LINE
    }
}
```

### Configuration

Enable or disable resolvers at runtime:

```rust
use layered_contracts::pipeline::{Pipeline, PipelineConfig};

let config = PipelineConfig::new()
    .disable("expensive_resolver")
    .enable("optional_resolver");

let doc = Pipeline::standard()
    .with_config(config)
    .run_on_text(text)?;
```

### Introspection

Debug pipeline execution order:

```rust
let pipeline = Pipeline::standard();

// Structured view
let view = pipeline.inspect_plan()?;
for step in &view.line_steps {
    println!("{}: produces {:?}, requires {:?}", step.id, step.produces, step.requires);
}

// Graphviz DOT output
let dot = pipeline.to_dot()?;
println!("{}", dot);
```

### Troubleshooting

| Error | Cause | Fix |
|-------|-------|-----|
| `MissingProducer` | Resolver requires a type no one produces | Add a resolver that produces the required type, or remove the dependency |
| `Cycle` | Circular dependencies detected | Break the cycle by refactoring resolvers |
| `DuplicateId` | Two resolvers have the same ID | Use unique IDs for each resolver |
| `phase_mismatch: true` | Line resolver depends on doc-only type | Line resolvers can only depend on line-phase outputs |

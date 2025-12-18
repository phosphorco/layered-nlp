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
- Chain resolvers: `.run(&Resolver1).run(&Resolver2)`

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

# Pipeline Integration for Spec-Driven Testing

> Learnings relevant to future gates should be written back to respective gates, so future collaborators can benefit.

**Status**: Complete
**Depends on**: spec-driven-testing.md (Complete)

## Goal

Connect `run_fixture()` to the actual layered-nlp resolver chain so fixtures execute end-to-end. Currently returns empty `PipelineResult`; needs ~100-200 lines of bridge code.

## Scope

**Delivers**:
- Working `run_fixture()` that processes fixture text through resolver chain
- Span extraction populating `PipelineResult` with detected spans
- End-to-end fixture execution with real assertions

**Excludes**:
- Character-position mapping (not needed - `query()` returns text directly)
- New resolver implementations
- Fixture syntax changes (markers, references)

**Clarification**: Assertion adjustments to match resolver output are in-scope

## Context

### API Surface (from exploration)

| Operation | API |
|-----------|-----|
| Create document | `create_line_from_string(text)` or `LayeredDocument::from_text(text)` |
| Chain resolvers | `.run(&ResolverType::default())` |
| Extract spans | `line.find(&x::attr::<Scored<T>>())` → `Vec<Find<Scored<T>>>` |

### Resolver Chain Order

```rust
.run(&POSTagResolver::default())
.run(&ContractKeywordResolver::default())
.run(&ProhibitionResolver::default())
.run(&DefinedTermResolver::default())
.run(&TermReferenceResolver::default())
.run(&PronounResolver::default())
.run(&ObligationPhraseResolver::default())
```

### Key Insight

`find()` already returns the span text as a String - no position mapping needed. Just collect `(text, span)` tuples.

---

## Gate: Resolver Bridge (COMPLETE)

**Deliverable**: `run_fixture()` processes text through resolvers and populates `PipelineResult`

### Implementation

In `/Users/cole/phosphor/phosphor-copy/.context/layered-nlp/layered-nlp-specs/src/runner.rs`:

1. **Add dependency** to `Cargo.toml`:
   Add `layered-part-of-speech = { path = "../layered-part-of-speech" }` to Cargo.toml

2. **Add imports** from `layered_contracts` and `layered_nlp`:
   - Resolvers: `POSTagResolver`, `ContractKeywordResolver`, `ProhibitionResolver`, `DefinedTermResolver`, `TermReferenceResolver`, `PronounResolver`, `ObligationPhraseResolver`
   - Core: `create_line_from_string`, `Scored`

3. **Implement `run_fixture()`**:
   - Get normalized text from fixture
   - For single-paragraph: use `create_line_from_string()`
   - For multi-paragraph: process each paragraph separately
   - Chain all resolvers
   - Query for `Scored<ObligationPhrase>`, `Scored<PronounReference>`, `Scored<DefinedTerm>`
   - Collect `(text, span.value)` tuples into `PipelineResult`

   Note: Start with single-paragraph fixtures. Multi-paragraph uses same pattern (iterate paragraphs, process each as line).

4. **Handle `Scored<T>` wrapper consistently**:
   - Resolvers produce `Scored<T>` for all span types
   - Use `.find(&x::attr::<Scored<T>>())` pattern (matches extractors.rs)
   - Extract `.value` from `Scored<T>` for `obligations` and `pronouns`
   - Keep `Scored<DefinedTerm>` wrapped (matches existing type signature)

### Acceptance Criteria

- [x] `cargo check -p layered-nlp-specs` compiles with new imports
- [x] `run_fixture()` returns non-empty `PipelineResult` for fixtures with detectable spans
- [x] Existing test `test_check_fixture_with_injected_spans` still passes
- [x] New test: fixture with "shall pay" detects `ObligationType::Duty`
- [x] New test: fixture with `"Term" means` detects `DefinitionType::QuotedMeans`
- [x] New test: multi-paragraph fixture processes all paragraphs
- [x] `cargo test -p layered-nlp-specs` passes

### Learnings

- The `find()` API returns `(LLSelection, T)` tuples where the selection gives access to the text range
- Multi-paragraph processing works by iterating `fixture.paragraphs`
- The resolver chain successfully detects obligations (Duty), defined terms (QuotedMeans), and processes multiple paragraphs
- Tests now explicitly verify these detection patterns work end-to-end

### Test Scenarios

| Fixture | Expected Detection |
|---------|-------------------|
| `simple-obligation.nlp` | 1 ObligationPhrase (Duty) |
| `permission-simple.nlp` | 1 ObligationPhrase (Permission) |
| `defined-term-single.nlp` | 1 DefinedTerm (QuotedMeans) |
| `multi-paragraph-obligation.nlp` | Entities + Obligations across paragraphs |

---

## Gate: End-to-End Validation (COMPLETE)

**Deliverable**: Verify full assertion pipeline works with real resolver output

### Implementation

1. **Update existing fixtures** if assertions don't match actual resolver output:
   - Resolvers may detect slightly different text spans than fixture markers
   - Adjust assertions to match actual behavior

2. **Add integration test**: `test_full_pipeline_integration`
   - Load fixture, run pipeline, check assertions
   - Verify `MatchResult.all_passed()` for expected-pass fixtures

3. **Update `expected_failures.toml`** for known gaps:
   - Pronoun resolution may not work without document context
   - Some patterns may not be implemented in resolvers

### Acceptance Criteria

- [x] At least 2 fixtures pass end-to-end (load → run → check → pass)
- [x] Failures produce actionable error messages with field-level diagnostics
- [x] `expected_failures.toml` documents any known gaps
- [x] Exit code 0 when only expected failures present

### Results

**Passing Fixtures (2/7)**:
1. `simple-obligation.nlp` - 1/1 assertions passed
2. `permission-simple.nlp` - 1/1 assertions passed

**Failing Fixtures (5/7)**: Documented in `expected_failures.toml` with 10 known failure entries.

### Learnings

1. **Span anchoring semantics** - Resolver anchors attributes at semantic trigger points (modal keywords like "shall", "may"), not full phrases. The `action` field in `ObligationPhrase` provides the action text that can be combined with modal for span matching.

2. **Quote handling mismatch** - Fixtures use quoted text (e.g., `"shall pay"`) while resolver output excludes quotes. Span text construction needs to account for this difference.

3. **Multi-paragraph limitations** - Multi-paragraph and cross-paragraph references (pronoun resolution, term references across paragraphs) need additional work beyond single-paragraph detection. Current resolver chain processes paragraphs independently.

4. **Text reference syntax** - The `text: ref(...)` syntax for extracting text from marker references works but needs careful coordination with how the resolver actually constructs span text.

### Potential Issues

| Issue | Mitigation |
|-------|------------|
| Resolver detects different span boundaries | Assertions match by text content, not position |
| Multi-line not supported by LLLine | Process paragraphs individually |
| Some resolvers need document context | Start with line-level, document-level as follow-up |

---

## Files to Modify

| File | Change |
|------|--------|
| `layered-nlp-specs/src/runner.rs` | Implement `run_fixture()` with resolver chain |
| `layered-nlp-specs/Cargo.toml` | Add `layered-part-of-speech` dependency for POS |
| `layered-nlp-specs/fixtures/*.nlp` | Adjust assertions if needed |
| `layered-nlp-specs/fixtures/expected_failures.toml` | Document known gaps |

## Verification

After both gates:
```bash
cargo test -p layered-nlp-specs
# Should show fixtures executing end-to-end
```

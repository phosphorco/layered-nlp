# Spec-Driven Testing Infrastructure

> Learnings relevant to future gates should be written back to respective gates, so future collaborators can benefit.

**Status:** Complete

## Goal

Build specification-driven testing infrastructure where:
1. NLP behavior defined declaratively in `.nlp` fixtures with inline annotations
2. Test harness compares pipeline output against trait-based assertions
3. Failures tracked with lifecycle: pending → known → fixed

## Motivation

Current testing is code-centric and example-sparse:
- Tests written after code, not driving it
- Adding test cases requires Rust knowledge
- No distinction between "known gap" vs "regression"

Target: Adding a failing case is trivial; fixing follows structured loop.

## Scope

**In scope:**
- Multi-paragraph `.nlp` fixture format with inline entity/span markers
- Trait-based assertion system with per-type syntax
- Test harness crate loading fixtures, running pipelines, generating rich diffs
- Failure tracking with `expected_failures.toml`

**Deferred (follow-up plan):**
- Initial corpus migration (existing tests → fixtures)
- Agent-driven fix loop integration
- Claude Code skill for "fix failing fixture"
- Markdown → `.nlp` transpiler for documentation authoring

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Document format | Multi-paragraph markdown | Natural reading flow; cross-paragraph refs |
| Span marking | `«ID:text»` inline | Co-located with text; supports entity IDs |
| Assertion syntax | Blockquote `> [ref]: Type(...)` | Visually separated; markdown compatible |
| Cross-references | `§Entity` notation | Clear, concise document-wide references |
| Assertion validation | Trait-based per type | Each type owns its syntax; extensible |
| Per-type syntax | MVP: key=value + arrow | `-> §Tenant` for refs, `modal=shall` for modals |
| Complex assertions | S-expr fallback | **Deferred** to follow-up |
| Failure lifecycle | TOML tracking file | Git-diffable, simple tooling |

---

## Gate 1a: Single-Paragraph Fixtures (MVP)

### Prerequisites
- Understanding of existing Resolver trait and span types

### Deliverables

0. **Crate scaffold** (`layered-nlp-specs/`)
   - `Cargo.toml` with deps: layered-nlp, layered-contracts, serde, nom (for parsing)
   - `src/lib.rs` with module declarations
   - Add as workspace member in root `Cargo.toml`

1. **Single-paragraph document format** (`.nlp` files)

   Basic structure with numbered span markers only:

   ```markdown
   # Test: Simple Obligation Detection

   The Tenant «1:shall pay» rent of $2,000 to the Landlord monthly.

   > [1]: Obligation(modal=shall)
   ```

   **MVP Syntax elements:**

   | Element | Purpose | Example |
   |---------|---------|---------|
   | `«n:text»` | Mark numbered span | `«1:shall pay»` |
   | `> [n]: Type(...)` | Assertion (blockquote style) | `> [1]: Obligation(shall+)` |

2. **Parser for single-paragraph case** (`layered-nlp-specs/src/parser/`)
   - `span.rs` — `«n:text»` numbered span extraction
   - `assertion.rs` — `> [n]: Type(...)` parsing

3. **Rust types** (`layered-nlp-specs/src/fixture.rs`)
   - `NlpDocument` — parsed fixture with single paragraph, spans, assertions
   - `SpanMarker` — `{ id, text, char_range }`
   - `Assertion` — `{ ref_target, span_type, assertion_body }`
   - `RefTarget` — enum: `Span(n)`

4. **Example fixtures** (`fixtures/`)
   - `simple-obligation.nlp` — basic obligation detection
   - `defined-term-single.nlp` — single term definition
   - `permission-simple.nlp` — permission phrase detection

### Acceptance Criteria

- [x] `.nlp` fixtures parse single-paragraph documents correctly
- [x] Numbered span markers `«1:text»` extracted with positions
- [x] Assertions in blockquote format parse type and body
- [x] At least 3 simple example fixtures

---

## Gate 1b: Multi-Paragraph & Cross-References

### Prerequisites
- Gate 1a single-paragraph parser complete

### Deliverables

1. **Multi-paragraph document format**

   Extended structure with `---` separators and entity markers:

   ```markdown
   # Test: Cross-Paragraph Pronoun Resolution

   «T:The Tenant» shall pay «rent:$2,000» to «L:the Landlord» monthly.
   Payment «1:shall be» due on the first of each month.

   > ["Tenant"]: DefinedTerm(term_name="Tenant")
   > ["Landlord"]: DefinedTerm(term_name="Landlord")
   > [1]: Obligation(bearer=§T, modal=shall)

   ---

   If «T» «2:fails to» pay within five days, «T» «3:shall» pay a late fee.
   «L» «4:may» waive this fee at «5:their» sole discretion.

   > [3]: Obligation(bearer=§T, modal=shall)
   > [4]: Obligation(bearer=§L, modal=may)
   > [5]: PronounRef(-> §L)
   ```

   Note: `modal=may` maps to `ObligationType::Permission` within `ObligationPhrase`.
   Complex assertions like `[2..3]: Condition(...)` are deferred to follow-up.

   **Extended syntax elements:**

   | Element | Purpose | Example |
   |---------|---------|---------|
   | `«ID:text»` | Define entity with cross-ref ID | `«T:The Tenant»` |
   | `---` | Paragraph separator | |
   | `§ID` | Cross-paragraph entity ref | `§T`, `§L` |
   | `[n..m]` | Span range | `[2..3]` |
   | `["text"]` | Text-based span ref | `["Tenant"]` |

2. **Cross-reference syntax**
   - `§Entity` — document-wide entity reference (defined via `«ID:text»`)
   - `[n]` — current paragraph span (defined via `«n:text»`)
   - `[§P.n]` — specific paragraph span (when explicit paragraph needed)
   - `["text"]` — text-based span ref (first occurrence)
   - `["text"@2]` — text-based with occurrence index (0-indexed)

3. **Reference scoping rules**
   - `[n]` — current paragraph only (error if not found)
   - `[§P.n]` — specific paragraph P (0-indexed), span n
   - `§Entity` — document-wide entity, must have been defined in any prior paragraph
   - Parser reports error with suggestion if unqualified `[n]` is ambiguous

4. **Full parser with 5 modules** (`layered-nlp-specs/src/parser/`)
   - `document.rs` — multi-paragraph parsing, `---` splitting
   - `entity.rs` — `«ID:text»` extraction, entity registry
   - `span.rs` — `«n:text»` numbered span extraction (extended from 1a)
   - `assertion.rs` — `> [ref]: Type(...)` parsing (extended from 1a)
   - `references.rs` — `§Entity`, `[n]`, `["text"]` resolution

5. **Extended Rust types** (`layered-nlp-specs/src/fixture.rs`)
   - `NlpDocument` — parsed fixture with paragraphs, entities, assertions
   - `Paragraph` — text with entity markers and span markers extracted
   - `EntityDef` — `{ id, text, paragraph_idx, char_range }`
   - `RefTarget` — extended enum: `Entity(id)`, `Span(n)`, `SpanRange(n, m)`, `TextRef(text, occurrence)`

6. **Complex example fixtures** (`fixtures/`)
   - `pronoun-resolution.nlp` — cross-paragraph pronouns with `§Entity` refs
   - `obligation-chains.nlp` — conditional obligations with triggers
   - `defined-terms.nlp` — term definition and reference patterns

### Acceptance Criteria

- [x] `.nlp` fixtures parse multi-paragraph documents correctly
- [x] Entity markers `«T:text»` create cross-ref IDs in registry
- [x] `---` separator correctly splits paragraphs
- [x] Cross-paragraph `§Entity` references resolve to entity definitions
- [x] Reference scoping rules enforced with clear error messages
- [x] At least 3 complex example fixtures covering pronoun, obligation, defined-term types

---

## Gate 2: Trait-Based Assertion System

### Prerequisites
- Gate 1 fixture types and parser complete

### Deliverables

1. **Core traits** (`layered-nlp-specs/src/assertion/mod.rs`)
   ```rust
   /// Implemented by each span type that supports assertions
   trait SpanAssertion {
       /// The parsed assertion specification for this type
       type Assertion: AssertionSpec;

       /// Parse assertion syntax specific to this span type
       fn parse_assertion(input: &str) -> Result<Self::Assertion, ParseError>;

       /// Check if this span satisfies the assertion
       fn check(&self, assertion: &Self::Assertion) -> Result<(), AssertionMismatch>;

       /// Type name for error messages
       fn span_type_name() -> &'static str;
   }

   /// Describes an assertion for error reporting
   trait AssertionSpec {
       /// Human-readable description of what was asserted
       fn describe(&self) -> String;

       /// List of fields this assertion constrains
       fn constrained_fields(&self) -> Vec<&'static str>;
   }
   ```

2. **Error types** (`layered-nlp-specs/src/assertion/error.rs`)
   ```rust
   /// Error parsing assertion syntax
   struct ParseError {
       message: String,
       position: usize,
       suggestions: Vec<String>,      // valid alternatives
       valid_fields: Vec<&'static str>, // for this span type
   }

   /// Assertion check failed
   struct AssertionMismatch {
       span_text: String,
       assertion_source: String,
       fields: Vec<FieldMismatch>,
   }

   struct FieldMismatch {
       field: &'static str,
       expected: String,
       actual: String,
       severity: MismatchSeverity,
   }

   enum MismatchSeverity {
       Hard,  // semantic mismatch, definitely wrong
       Soft,  // threshold not met (e.g., confidence)
       Info,  // informational difference
   }
   ```

3. **Per-type implementations** (3 span types for MVP)

   **ObligationPhrase** (`layered-nlp-specs/src/assertion/obligation.rs`):
   - Modal checking: `modal=shall` (Duty), `modal=may` (Permission), `modal=must`
   - Fields: `bearer=§T` (maps to obligor)
   - Example: `Obligation(bearer=§T, modal=shall)`
   - Note: `Permission` is not a separate span type; it's `ObligationType::Permission` within ObligationPhrase

   **PronounReference** (`layered-nlp-specs/src/assertion/pronoun.rs`):
   - Arrow syntax: `-> §Tenant` (resolution target)
   - Fields: `confidence >= 0.8` (via Scored wrapper)
   - Example: `PronounRef(-> §L)`

   **DefinedTerm** (`layered-nlp-specs/src/assertion/defined_term.rs`):
   - Fields: `term_name=`, `definition_type=`
   - Example: `DefinedTerm(term_name="Tenant")`, `DefinedTerm(definition_type=Explicit)`

4. **Assertion syntax patterns (MVP)**

   | Pattern | Meaning | Example |
   |---------|---------|---------|
   | `key=value` | Exact field match | `bearer=§T`, `modal=shall` |
   | `-> target` | Reference points to target | `-> §Tenant` |
   | `modal=shall\|may\|must` | Modal checking | `modal=shall`, `modal=may` |
   | `confidence >= n` | Threshold comparison | `confidence >= 0.8` |

   **Deferred to follow-up plan:**

   | Pattern | Reason |
   |---------|--------|
   | `(!Type)` | Negative type assertions - needs deliverable |
   | `contains(...)` | Substring matching |
   | S-expr `(and (-> A) (chain=1))` | Complex assertions |
   | `[§P.n]` | Explicit paragraph refs |
   | `-> !`, `-> ?` | Null/any reference wildcards |
   | `key~"pat"` | Contains pattern |

5. **Field Mapping Conventions**

   Assertion syntax uses domain terms; implementation maps to actual Rust struct fields:

   | Assertion Syntax | Rust Struct | Rust Field |
   |------------------|-------------|------------|
   | `bearer=§T` | ObligationPhrase | `obligor: ObligorReference` |
   | `modal=shall` | ObligationPhrase | `obligation_type == ObligationType::Duty` |
   | `modal=may` | ObligationPhrase | `obligation_type == ObligationType::Permission` |
   | `-> §Entity` | PronounReference | `candidates[0].text` match |
   | `confidence >= 0.8` | Scored<T> | Scored wrapper (not a field on inner type) |
   | `term_name=` | DefinedTerm | `term_name: String` |
   | `definition_type=` | DefinedTerm | `definition_type: DefinitionType` |

   The `SpanAssertion::check()` implementation handles these mappings.

   **Removed (fields do not exist in codebase):**
   - `type=possessive` - PronounType has ThirdSingularNeuter/Masculine/Feminine, ThirdPlural, Relative, Other
   - `DefinedTerm(first)` - no "first occurrence" field
   - `DefinedTerm(alias=, defined_in=)` - these fields do not exist

6. **Scored<T> integration**
   - `SpanAssertion` implemented for inner types (e.g., `PronounReference`)
   - Harness unwraps `Scored<T>` → passes `value` to `check()`, `confidence` to threshold assertions
   - `confidence >= 0.8` checks `Scored<T>.confidence`, not a field on inner type

7. **Assertion registry** (`layered-nlp-specs/src/assertion/registry.rs`)
   - Map type names to parse/check functions
   - Manual registration (following existing patterns)
   - Fallback for unknown types with warning

### Acceptance Criteria

- [x] `SpanAssertion` trait implemented for 3 span types: ObligationPhrase, PronounReference, DefinedTerm
- [x] Arrow syntax works: `-> §Entity` (reference resolution)
- [x] Modal checking works: `modal=shall`, `modal=may`, `modal=must`
- [x] Field equality works: `key=value` pattern
- [x] Threshold syntax works: `confidence >= 0.8`
- [x] Errors include field-level expected/actual/severity
- [x] `describe()` produces readable assertion summaries
- [x] **Parser integration**: Gate 1 parser extracts raw body string, per-type `parse_assertion()` handles syntax

---

## Gate 3: Test Harness

### Prerequisites
- Gate 2 assertion system operational
- Example fixtures from Gate 1a/1b

### Deliverables

#### Core Execution

1. **Fixture loader** (`layered-nlp-specs/src/loader.rs`)
   - `load_fixture(path: &Path) -> Result<NlpDocument>`
   - `load_all_fixtures(dir: &Path) -> Result<Vec<NlpDocument>>`
   - Glob pattern: `fixtures/**/*.nlp`

2. **Document context** (`layered-nlp-specs/src/context.rs`)
   - `DocumentContext` — tracks entities, paragraphs, cross-refs during evaluation
   - Entity resolution: `resolve_entity("T") -> EntityDef`
   - Span resolution: `resolve_span(paragraph_idx, span_id) -> SpanMarker`
   - Text ref resolution: `resolve_text_ref("Tenant", occurrence) -> CharRange`

3. **Pipeline runner** (`layered-nlp-specs/src/runner.rs`)
   - `run_fixture(doc: &NlpDocument, config: &PipelineConfig) -> PipelineResult`
   - Process each paragraph through resolvers
   - Collect all spans with positions

4. **PipelineConfig** (`layered-nlp-specs/src/config.rs`)
   ```rust
   struct PipelineConfig {
       resolvers: Vec<String>,  // resolver names to run
       // "standard" = full contract pipeline
   }

   impl PipelineConfig {
       pub fn standard() -> Self {
           Self { resolvers: vec!["standard".into()] }
       }
   }
   ```

5. **Resolver registry** (`layered-nlp-specs/src/registry.rs`)
   - Map resolver names to instantiation functions
   - Support preset: `standard` (full contract pipeline)
   - Example: `"DefinedTerm" → || Box::new(DefinedTermResolver::new())`

#### Assertion Checking

6. **Span matcher** (`layered-nlp-specs/src/matcher.rs`)
   - `check_assertions(result: &PipelineResult, doc: &NlpDocument, ctx: &DocumentContext) -> Vec<AssertionResult>`
   - For each assertion:
     1. Resolve reference target to char range
     2. Find spans at that position
     3. Dispatch to `SpanAssertion::check()` for matching type
   - Return: `AssertionResult { passed, assertion, mismatch? }`

7. **Error formatter** (`layered-nlp-specs/src/formatter.rs`)
   - Rich error output using trait's `describe()`:
   ```
   FAIL: lease-test.nlp:24

     If «T» «2:fails to» pay within five days...
            ^^^^^^^^^^^

     assertion failed for Obligation span: "fails to"
       ✗ modal: expected `shall`, found `implicit`
       ○ bearer: expected §T, found §T (ok)

     Assertion was: Obligation(bearer=§T, modal=shall)

     hint: "fails to" may be a negative condition, not an obligation
   ```

8. **Integration with trait `check()` methods**
   - Wire `SpanAssertion::check()` into matcher
   - Collect `FieldMismatch` with severity levels
   - Generate hints based on mismatch patterns

#### Failure Tracking & CI

9. **`expected_failures.toml` format** (`layered-nlp-specs/fixtures/expected_failures.toml`)
   ```toml
   [[known]]
   fixture = "pronoun-resolution.nlp"
   assertion = "§3.[6]"  # paragraph 3, span 6
   reason = "Possessive pronoun resolution not yet implemented"
   added = "2025-01-06"

   [[pending]]
   fixture = "obligation-chains.nlp"
   assertion = "§2.[2..3]"
   issue = "https://github.com/.../issues/123"
   ```

10. **Lifecycle states**
    - `pending` — Known failure, awaiting fix (not blocking)
    - `known` — Acknowledged limitation (documented, won't fix soon)
    - (absence) — Expected to pass; failure = regression

11. **Harness integration** (`layered-nlp-specs/src/failures.rs`)
    - Load `expected_failures.toml` on startup
    - Classify each failure: regression vs expected
    - Exit code: 0 if only expected failures, 1 if regressions

12. **Output modes** (via environment variable)
    - Default: Show regressions only (expected failures summarized)
    - `SPEC_SHOW_ALL=1`: Show all failures including expected

### Acceptance Criteria

- [x] Fixtures load and parse without errors
- [x] Document context resolves entities and spans correctly
- [x] Pipeline runner processes fixtures through resolver chain
- [x] Resolver registry instantiates standard pipeline
- [x] Span matcher finds spans at assertion positions
- [x] `SpanAssertion::check()` dispatches to correct type implementations
- [x] Assertion failures show field-level expected/actual with hints
- [x] Error formatter produces readable, actionable output
- [x] `expected_failures.toml` lifecycle works (pending, known states)
- [x] Regressions (unexpected failures) exit non-zero
- [x] Expected failures pass CI (exit 0 with warning)
- [x] `SPEC_SHOW_ALL=1` displays all failures including expected
- [x] Test harness runs `.nlp` fixtures end-to-end
- [x] Cross-paragraph pronoun tests with `§Entity` refs work

---

## Deferred Work (Follow-Up Plan)

### Corpus Migration
- Convert existing insta tests → `.nlp` fixtures
- Extract test cases from `layered-contracts/src/tests/*.rs`
- Preserve snapshot compatibility during transition

### Agent Fix Loop Integration
- Claude Code skill: `/fix-fixture <fixture-id>`
- Reads failing fixture, analyzes resolver, proposes fix
- Runs targeted test subset, checks for regressions

### Advanced Features
- Markdown → `.nlp` transpiler for documentation authoring
- Coverage reports: which resolvers have fixture coverage
- Mutation testing: do fixtures catch intentional regressions?
- IDE support: syntax highlighting for `.nlp` files

### Deferred Assertion Syntax
- S-expression assertion syntax for complex composition (AND/OR/NOT)
- `(!Type)` negative type assertions
- `-> !` (no reference) and `-> ?` (any reference) wildcards
- `contains(...)` substring matching
- `key~"pat"` pattern matching
- `[§P.n]` explicit paragraph span refs
- Condition span type and trigger refs

---

## Risks

| Risk | Mitigation |
|------|------------|
| Resolver registry requires manual updates when new resolvers added | Manual registration following `pipeline/mod.rs` pattern; trait registration auto-discovery is optional future optimization |
| `.nlp` format unfamiliar to contributors | Provide clear syntax guide in README; defer transpiler for Markdown authoring |
| Cross-paragraph reference resolution complex | Start with single-paragraph tests; add cross-paragraph incrementally |
| Trait-based assertion system adds per-type maintenance burden | Start with 3 core types (ObligationPhrase, PronounReference, DefinedTerm); document pattern for adding new types |
| Syntax errors in fixtures hard to diagnose | Parser includes byte offsets; "did you mean" suggestions for common mistakes |
| Per-type syntax cognitive load | Start with key=value; add domain shortcuts based on usage |

---

## Verification

After all gates complete:

1. **Multi-paragraph smoke test**
   - Create fixture with 3 paragraphs
   - Entity defined in §1 (`«T:The Tenant»`) referenced in §3 (`§T`)
   - Pronoun in §2 resolving to §1 entity
   - Verify cross-paragraph resolution works

2. **Trait validation test**
   - Introduce wrong antecedent intentionally in fixture
   - Verify error shows expected vs actual with hint
   - Confirm trait's `describe()` produces readable output
   - Check that `FieldMismatch` severity is correct

3. **Syntax coverage test** (MVP patterns only)
   - Field equality: `key=value` (e.g., `bearer=§T`, `term_name="Tenant"`)
   - Arrow syntax: `-> §Tenant` (reference resolution)
   - Modal checking: `modal=shall`, `modal=may`, `modal=must`
   - Thresholds: `confidence >= 0.8`

4. **Failure lifecycle test**
   - Add fixture with intentional failure
   - Add to `expected_failures.toml` as `pending`
   - Verify CI passes (exit 0)
   - Remove from TOML, verify CI fails (regression)
   - Fix the resolver, verify CI passes

5. **Documentation**
   - README in `layered-nlp-specs/` explaining:
     - `.nlp` fixture format with syntax reference
     - How to add new span type assertions
     - Running tests locally
     - Interpreting failure output

6. **Coexistence test**
   - Existing `cargo test` passes unchanged
   - New fixture harness as separate `cargo test -p layered-nlp-specs`
   - Both run in CI without conflicts

---

## Progress Log

### 2025-01-06: Gate 1a Complete
- Created `layered-nlp-specs` crate with full scaffold
- Implemented core types: `NlpFixture`, `SpanMarker`, `Assertion`, `RefTarget`, `AssertionBody`, `FieldCheck`, `CompareOp`
- Implemented parser for `«n:text»` span markers with character range tracking
- Implemented assertion parser for `[n]: Type(field=value)` format
- Created 3 example fixtures demonstrating MVP syntax
- All 11 tests passing

### 2025-01-06: Gate 1b Complete
- Extended types: `Paragraph`, `EntityDef`, `MarkerId` enum (Numeric/Named)
- Extended `RefTarget` with `Entity(String)` and `TextRef { text, occurrence }`
- Parser now handles `---` paragraph separator
- Entity markers `«ID:text»` create entities in registry
- Cross-references `§Entity` resolve to entities in assertions
- Created 4 multi-paragraph fixtures demonstrating all patterns
- All 22 tests passing

### 2025-01-06: Gate 2 Complete
- `SpanAssertion` trait with `parse_assertion()`, `check()`, `span_type_name()`
- `AssertionSpec` trait for describing and constraining assertions
- Error types: `ParseError`, `AssertionMismatch`, `FieldMismatch`, `MismatchSeverity`
- Field parsing: `key=value`, `key>=value`, `-> §Entity` patterns
- ObligationPhrase assertions: `modal=shall|may|must`, `bearer=§T`, `action`
- PronounReference assertions: `-> §Entity`, `pronoun_type`, `confidence>=`
- DefinedTerm assertions: `term_name`, `definition_type`
- Registry: `check_obligation()`, `check_pronoun()`, `check_defined_term()`
- `MatchResult` for aggregating results with pass/fail summary
- All 68 tests passing

### 2025-01-06: Gate 3 Complete
- Fixture loader: `load_fixture()`, `load_all_fixtures()` with recursive `.nlp` glob
- Document context: `DocumentContext` with entity/span/text-ref resolution
- Pipeline config: `PipelineConfig` with `standard()` preset
- Pipeline runner: `run_fixture()`, `check_fixture_assertions()`, `PipelineResult`
- Error formatter: `format_failure()` with field-level diagnostics, text underlines, contextual hints
- Failure tracking: `ExpectedFailures` with `pending`/`known` states from TOML
- `HarnessResult` with `exit_code()`: 0 = expected failures only, 1 = regressions
- `expected_failures.toml` documenting lifecycle format
- All 95 tests passing

### Plan Complete!
All gates (1a, 1b, 2, 3) successfully implemented. The spec-driven testing infrastructure is ready for use.

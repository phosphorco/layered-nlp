# Resolver Design Exercise (Recipe + Pseudocode)

This is a conceptual exercise for designing new resolvers in the layered-nlp system, from two complementary angles:
1) a recipe-style workflow, and 2) Rust-like pseudocode that shows how new resolvers integrate with clarifications and diffing.

Important: The pseudocode is intentionally illustrative. It follows the current architecture, but some helper calls are conceptual. When implementing, align with the actual APIs in `layered-nlp-document` (e.g., `DocumentResolver::resolve`, `run_document_resolver`, `Scored::rule_based`).

---

## Angle 1: The Recipe

### 1) Define the semantic object
- What new attribute are you introducing?
- Is it line-level (single sentence) or document-level (cross-paragraph)?
- What is the minimal data structure that captures the meaning?

### 2) Choose scope + inputs
- Line-level resolver: use `LLSelection` + matchers (`x::seq`, `x::attr`, etc.).
- Document-level resolver: use `LayeredDocument` (or a crate alias like `ContractDocument`) and cross-line queries.

### 3) Declare dependencies (manual today)
- There is no `dependencies()` hook on resolvers in the codebase today.
- Order resolvers explicitly in the pipeline: `.run_resolver(...).run_document_resolver(...)`.

### 4) Use Scored<T> for semantic attributes
- Semantics must be `Scored<T>` with explicit confidence and source.
- Use `Scored::rule_based(value, confidence, "rule_name")` for deterministic extractors.

### 5) Create provenance edges (current + planned)
- Line-level provenance uses `AssociatedSpan` (exists today).
- Document-level associations are planned; until then, include spans directly on your attribute (e.g., `definition_span`, `mention_span`).

### 6) Add fixtures + expected failures
- Write minimal `.nlp` fixtures.
- If failing, add to `fixtures/expected_failures.toml` as `pending`.

### 7) Integrate into the pipeline
- For document-level changes, ensure the resolver runs before diff/align.

### 8) Clarification integration
- Ensure your attribute can be overridden by `Scored::verified_by(...)` so clarifications can re-run deterministically.

---

## Angle 2: Rust-like Pseudocode

### A) Line-level resolver: Negation + Exception Scope

```rust
#[derive(Debug, Clone)]
pub struct ProhibitionScope {
    pub action: String,
    pub exception: Option<String>,
}

pub struct NegationExceptionResolver;

impl Resolver for NegationExceptionResolver {
    type Attr = Scored<ProhibitionScope>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut out = Vec::new();

        for found in selection.find_by(&x::seq((x::token_text("shall"), x::token_text("not")))) {
            let action_span = found.match_forwards(&x::any_of((
                x::token_text("disclose"),
                x::token_text("use"),
            )));

            let exception = action_span.match_forwards(&x::seq((
                x::token_text("except"),
                x::token_text("as"),
                x::token_text("required"),
                x::token_text("by"),
                x::token_text("law"),
            )));

            let action = action_span.text().to_string();
            let exception_text = exception.map(|e| e.text().to_string());

            out.push(action_span.finish_with_attr(Scored::rule_based(
                ProhibitionScope { action, exception: exception_text },
                0.85,
                "negation_exception_scope",
            )));
        }

        out
    }
}
```

### B) Document-level resolver: Cross-paragraph term reference

```rust
#[derive(Debug, Clone)]
pub struct TermReference {
    pub term_name: String,
    pub definition_span: DocSpan,
    pub mention_span: DocSpan,
}

pub struct TermReferenceDocResolver;

impl DocumentResolver for TermReferenceDocResolver {
    type Attr = Scored<TermReference>;

    fn resolve(&self, doc: &LayeredDocument) -> Vec<Self::Attr> {
        let mut out = Vec::new();

        // Conceptual: gather defined terms by scanning line-level attrs.
        let mut defs: Vec<(String, DocSpan)> = Vec::new();
        for (line_idx, line) in doc.lines_enumerated() {
            for def in line.find(&x::attr::<DefinedTerm>()) {
                let span = DocSpan::single_line(line_idx, def.range());
                defs.push((def.attr().term_name.clone(), span));
            }
        }

        // Conceptual: find later mentions for each term.
        for (term, def_span) in defs {
            for (line_idx, line) in doc.lines_enumerated() {
                for mention in line.find(&x::token_text_eq(&term)) {
                    let mention_span = DocSpan::single_line(line_idx, mention.range());
                    if mention_span.start.line > def_span.start.line {
                        out.push(Scored::rule_based(
                            TermReference {
                                term_name: term.clone(),
                                definition_span: def_span.clone(),
                                mention_span,
                            },
                            0.7,
                            "term_reference_cross_paragraph",
                        ));
                    }
                }
            }
        }

        out
    }
}
```

### C) Clarifications (alignment hints + verified overrides)

```rust
// Conceptual: load hints and overrides from a manifest.
let clarifications = ClarificationManifest::load("clarifications.toml")?;

// Alignment hints feed DocumentAligner::apply_hints.
let alignment_hints = clarifications.alignment_hints.clone();

// Semantic overrides can be injected as verified Scored<T> values.
for override in clarifications.semantic_overrides {
    doc.add_doc_attr(Scored::verified_by(override.value, "reviewer"));
}
```

### D) Diff pipeline usage (current APIs)

```rust
let original = ContractDocument::from_text(a)
    .run_resolver(&SectionHeaderResolver::new())
    .run_resolver(&DefinedTermResolver::new())
    .run_resolver(&ObligationPhraseResolver::new())
    .run_resolver(&NegationExceptionResolver)
    .run_document_resolver(&TermReferenceDocResolver);

let revised = ContractDocument::from_text(b)
    .run_resolver(&SectionHeaderResolver::new())
    .run_resolver(&DefinedTermResolver::new())
    .run_resolver(&ObligationPhraseResolver::new())
    .run_resolver(&NegationExceptionResolver)
    .run_document_resolver(&TermReferenceDocResolver);

let structure_a = DocumentStructureBuilder::build(&original).value;
let structure_b = DocumentStructureBuilder::build(&revised).value;

let aligner = DocumentAligner::new();
let candidates = aligner.compute_candidates(&structure_a, &structure_b, &original, &revised);
let alignment = aligner.apply_hints(candidates, &alignment_hints);

let diff = SemanticDiffEngine::new().compute_diff(&alignment, &original, &revised);
```

---

## Feasibility Checklist
- Are dependencies explicit (by pipeline ordering)?
- Does the attribute use `Scored<T>`?
- Is provenance captured (via spans or `AssociatedSpan` where available)?
- Is the pattern minimal and testable with a fixture?
- Can clarifications override the attribute deterministically?

---

## Next Step (optional)
If this is useful, consider adding a short section to `docs/versioned-diff-architecture.md` linking to this exercise as a design playbook for new resolvers.

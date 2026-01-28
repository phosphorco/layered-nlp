[![Layered NLP](https://raw.githubusercontent.com/storyscript/layered-nlp/main/assets/layered-nlp.svg)](https://github.com/storyscript/layered-nlp)

Incrementally build up recognizers over an abstract token that combine to create _multiple_ possible interpretations.

Key features:

- Abstract over token type to support "rich" tokens like we have at Story.ai.
- May generate multiple interpretations of the same token span.
- Produces a set of ranges over the input token list with different attributes.

## Quick Start (Humans)

```bash
# Build workspace
cargo build

# Run all tests
cargo test

# Run fixture harness (most common during development)
cargo test -p layered-nlp-specs runner::tests::test_full_pipeline_integration -- --nocapture
```

## Repository Map

- `src/` — core line-level engine (LLLine, matchers, selection)
- `layered-contracts/` — contract NLP resolvers (terms, obligations, diff)
- `layered-clauses/` — clause/linking resolvers (list items, cross-refs)
- `layered-nlp-specs/` — fixture harness + assertions
- `docs/` — architecture notes and resolver patterns
- `examples/` — runnable examples
- `web/` + `layered-nlp-demo-wasm/` — demo UI + WASM build

## Development Workflow (Humans)

1. Pick a lane:
   - Expansion: add fixtures (`workflows/expansion.md`)
   - Investigation: root-cause failures (`workflows/investigation.md`)
   - Implementation: fix resolvers (`workflows/implementation.md`)
2. Run the fixture harness and inspect failures.
3. Update `layered-nlp-specs/fixtures/expected_failures.toml` for new, known gaps.
4. Keep changes narrow and layered; avoid overfitting a single fixture.

## Demos

```bash
cd layered-nlp-demo-wasm && wasm-pack build --target web --out-dir ../web/pkg
cd ../web && python3 -m http.server 8080
# Open http://localhost:8080/contract-viewer.html
```

## Troubleshooting

- **Fixtures fail but resolver looks correct**: check `layered-nlp-specs/src/runner.rs` span extraction rules.
- **Snapshot updates**: run `cargo insta review`.
- **Expected failures**: ensure failures are recorded in `layered-nlp-specs/fixtures/expected_failures.toml`.

### Layering

The key idea here is to enable starting from a bunch of vague tags and slowly building meaning up through incrementally adding information that builds on itself.

Simplification: `Money = '$' + Number`

```
    $   123   .     00
                    ╰Natural
              ╰Punct
        ╰Natural
        ╰Amt(Decimal)╯
    ╰Money($/£, Num)─╯
```

Simplification:

- `Location(NYC) = 'New' + 'York' + 'City'`
- `Location(AMS) = 'Amsterdam'`
- `Address(Person, Location) = Person + Verb('live') + Predicate('in') + Location`

```
    I     live      in      New York City
                                     ╰Noun
                                ╰Noun
                            ╰Adj
                    ╰Predicate
          ╰Verb
    ╰Noun
    ╰Person(Self)
                            ╰──Location─╯
    ╰────Address(Person, Location)─────╯
```

### Contributor Workflows

If you are working on fixture coverage or implementation, start here:

- Expansion (fixtures + coverage gaps): `workflows/expansion.md`
- Investigation (failure forensics): `workflows/investigation.md`
- Implementation (make fixtures pass): `workflows/implementation.md`

**Agent loop (short version):**
Pick a lane, complete the full checklist in that workflow, run tests, and hand off to the next lane.

**Agent loop (super simple):**
Pick lane → do the full lane checklist → run tests → hand off.

### Architecture Notes

- Versioned diff design: `docs/versioned-diff-architecture.md`
- Resolver design playbook (recipe + pseudocode): `docs/resolver-design-exercise.md`

[![MIT licensed][mit-badge]][mit-url]
[![APACHE licensed][apache-2-badge]][apache-2-url]

[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE-MIT
[apache-2-badge]: https://img.shields.io/badge/license-APACHE%202.0-blue.svg
[apache-2-url]: LICENSE-APACHE

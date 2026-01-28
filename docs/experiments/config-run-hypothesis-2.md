# Hypothesis 2: Pipeline Introspection CLI Improves Debugging Throughput

## Hypothesis
Providing a CLI entrypoint for pipeline inspection (plan + DOT graph) will reduce debugging time when resolver ordering causes fixture failures.

## Motivation
- `docs/building-resolvers.md` shows code-only introspection APIs (`Pipeline::inspect_plan`, `Pipeline::to_dot`).
- There is no CLI that exposes these for quick debugging or CI artifacts.

## Evidence (repo scan)
- Introspection examples are only in docs, not in a CLI or script:
  - `docs/building-resolvers.md`
- No `bin/` or `main.rs` exists under `layered-contracts` or `layered-nlp-specs` for pipeline inspection.

## Proposed change
- Add a CLI command (new binary or subcommand) to print:
  - resolver plan (line + document phases)
  - DOT graph for visual inspection
- Optionally write the DOT to `target/pipeline.dot` for CI artifacts.

## Experiment design
- Control: debugging requires writing a short Rust snippet or adding debug prints.
- Variant: `cargo run -p layered-contracts --bin pipeline-inspect -- --dot`.
- Measure: time-to-first-inspection, number of code changes required, and how often a debugging run requires recompilation.

## Test (lightweight)
- Verify no existing CLI or script provides the pipeline plan.
- Verify docs encourage introspection but only via code snippets.

## Test execution notes (current repo)
- `rg --files -g "*main.rs" layered-contracts layered-nlp-specs` returns no CLI entrypoints.
- `rg --files -g "src/bin/*" layered-contracts layered-nlp-specs` returns no binaries.
- `docs/building-resolvers.md` shows introspection code but no CLI invocation.

## Result (current)
Supported: docs show introspection APIs but there is no CLI or script wired to them, indicating a clear UX gap for debugging.

## Next actions
- Implement a `pipeline-inspect` binary in `layered-contracts`.
- Add doc snippet showing how to invoke it and where the DOT file is written.

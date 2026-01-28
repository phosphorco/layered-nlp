# Hypothesis 1: Config-Driven Fixture Runner Unlocks Resolver Flexibility

## Hypothesis
Adding a config-driven runner for `layered-nlp-specs` will reduce friction when experimenting with resolver chains and make fixture workflows more repeatable without code edits.

## Motivation
- The fixture workflows repeatedly instruct running the same command, but there is no documented way to vary resolver sets without editing code.
- `PipelineConfig` exists but is not used in `run_fixture`.

## Evidence (repo scan)
- Repeated fixture workflow commands:
  - `workflows/expansion.md`
  - `workflows/investigation.md`
  - `workflows/implementation.md`
  - `layered-nlp-specs/CONTRIBUTING-FIXTURES.md`
- `PipelineConfig` is passed into `run_fixture` but currently ignored:
  - `layered-nlp-specs/src/runner.rs`

## Proposed change
- Add a small CLI binary in `layered-nlp-specs` (e.g., `layered-nlp-specs/src/bin/runner.rs`).
- Support `--config <path>` to load a TOML/YAML config.
- Map config to `PipelineConfig` and choose resolver chain before running fixtures.

## Experiment design
- Control: current workflow requires code edits for resolver changes.
- Variant: config file selects resolver chain and optional features.
- Measure: number of steps to switch resolver set (count edits + commands) and ability to reproduce results across runs.

## Test (lightweight)
- Confirm `PipelineConfig` is unused by searching for `_config` in `run_fixture` and checking that no resolver selection uses it.
- Confirm docs do not mention config-driven runs.

## Test execution notes (current repo)
- `layered-nlp-specs/src/runner.rs` defines `run_fixture(fixture, _config)` and does not reference the config inside the function.
- `rg -n "PipelineConfig" README.md docs workflows layered-nlp-specs/src` shows only structural usage, not dynamic resolver selection.

## Result (current)
Supported: `PipelineConfig` is unused in `run_fixture`, and docs only show fixed commands with no config option. This indicates a workflow gap that a config-driven runner could fill.

## Next actions
- Implement a config loader (TOML via `toml` crate) and a small CLI command.
- Update docs to include `layered-nlp-specs` config examples and precedence (defaults < config < CLI flags).

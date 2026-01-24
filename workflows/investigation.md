# Workflow: Investigation (Failure Forensics)

Purpose: Diagnose why fixtures fail and turn noisy failures into a clear root-cause map. This is the bridge between Expansion and Implementation.

## When to use
- The failure set is growing but root causes are unclear.
- Failures are inconsistent or surprising.
- You need a prioritized fix list before implementing.

## Inputs
- Runner output (use `--nocapture`).
- Failing fixtures and their assertions.
- Any recent resolver changes.

## Outputs
- A short root-cause report.
- A prioritized list of fixture clusters.
- Suggested next fixes or new fixtures.

## Steps
1. Run `cargo test -p layered-nlp-specs runner::tests::test_full_pipeline_integration -- --nocapture` and capture output.
2. Group failures by symptom (e.g., "No ObligationPhrase found", modal mismatches, cross-paragraph not found).
3. Map each group to a probable resolver stage.
4. Identify minimal repro fixtures (one per group).
5. Produce a short report with: cluster name, affected fixtures, likely cause, suggested fix path.

## Guardrails
- Avoid implementing in this workflow; focus on clarity.
- Prefer minimal repro fixtures over broad ones.
- Record any ambiguous cases explicitly.

## Report Template
```
## Cluster: <name>
Symptoms: <what the runner shows>
Fixtures: <list>
Likely cause: <resolver / stage>
Suggested fix: <short idea>
Next fixture: <optional new fixture>
```

## Definition of Done
- Failure clusters are grouped with a clear root-cause hypothesis.
- Each cluster lists affected fixtures and a suggested fix path.
- At least one minimal repro fixture exists per cluster (if missing, add it).

## Handoff
- If the fix path is clear, switch to `workflows/implementation.md`.
- If missing coverage is discovered, switch to `workflows/expansion.md`.

## Notes
Investigation keeps the team aligned on why failures happen. Treat it as a shared diagnostic log.

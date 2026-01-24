# Workflow: Implementation (Make Fixtures Pass)

Purpose: Turn failing fixtures into passing ones by implementing or improving resolvers. This is the delivery loop.

## When to use
- You are fixing known gaps in `expected_failures.toml`.
- You are making resolver changes or adding new span types.
- You are paying down failing fixtures.

## Inputs
- Failing fixtures and their assertions.
- `fixtures/expected_failures.toml` for prioritized gaps.
- Resolver architecture docs and existing resolver patterns.

## Outputs
- Resolver code changes.
- Updated tests or fixtures as needed.
- Fewer entries in `expected_failures.toml`.

## Steps
1. Pick a small cluster of related failing fixtures (same root cause).
2. Identify the minimal resolver change that addresses the cluster.
3. Implement and run `cargo test -p layered-nlp-specs`.
4. Remove or update entries from `expected_failures.toml` for fixtures now passing.
5. If the fix changes interpretation boundaries, update fixture assertions accordingly.

## Guardrails
- Fix the root cause, not just the fixture.
- Keep resolver changes minimal and layered.
- Avoid broad heuristics that would overfit a single fixture.

## Checklist
- [ ] Root cause identified across multiple fixtures.
- [ ] Resolver change documented in code and/or README.
- [ ] Passing fixtures removed from `expected_failures.toml`.
- [ ] No regressions in unrelated fixture groups.

## Definition of Done
- The targeted failing fixtures now pass.
- `fixtures/expected_failures.toml` updated to remove cleared entries.
- No regressions in previously passing fixtures.
- Any changed boundary interpretation is reflected in fixture assertions.

## Handoff
- If failures remain unclear, switch to `workflows/investigation.md`.
- If new gaps are discovered, switch to `workflows/expansion.md`.

## FAQ

Q: If a resolver change shifts span boundaries, which workflow updates assertions?
A: Implementation. Update the fixture assertions as part of the fix so the new behavior is captured.

## Notes
This workflow reduces the failing surface area. Treat passing fixtures as non-negotiable behavior.

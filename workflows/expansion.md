# Workflow: Expansion (Fixtures + Coverage Gaps)

Purpose: Expand the fixture surface area and document gaps without changing core implementation. This is how we scale coverage and keep gaps visible.

## When to use
- You are adding new .nlp fixtures.
- You are reviewing `expected_failures.toml` or coverage goals.
- You are defining new patterns or edge cases.

## Inputs
- Fixture directories under `fixtures/line/`, `fixtures/document/`, `fixtures/integration/`.
- `fixtures/expected_failures.toml`.
- Coverage group READMEs.

## Outputs
- New or updated fixtures.
- Updated group READMEs if a new edge case/pattern is introduced.
- Updated `expected_failures.toml` for newly failing fixtures.

## Steps
1. Scan existing fixtures in the target group. Identify patterns missing or underrepresented.
2. Create minimal fixtures that isolate one pattern per file.
3. Run `cargo test -p layered-nlp-specs runner::tests::test_full_pipeline_integration -- --nocapture` to see which new assertions fail.
4. Add new failures to `fixtures/expected_failures.toml` with a clear reason and date.
5. If you introduced a new pattern class or edge case, update the group README.

## Guardrails
- Do not implement resolvers in this workflow.
- Keep fixtures minimal; avoid multi-pattern fixtures.
- Add new fixtures even if you expect them to fail.

## Checklist
- [ ] Fixture name is descriptive and searchable.
- [ ] One pattern per fixture.
- [ ] Assertions are precise and minimal.
- [ ] Failures are recorded in `expected_failures.toml`.
- [ ] README updated if this is a new edge case.

## Definition of Done
- New fixtures are added and run through the runner.
- All newly failing assertions are recorded in `fixtures/expected_failures.toml`.
- Coverage group README updated when a new pattern or edge case was introduced.

## Handoff
- If failures cluster by root cause, switch to `workflows/investigation.md`.
- If a single fix would clear multiple fixtures, switch to `workflows/implementation.md`.

## FAQ

Q: When do I mark a failure as `pending` vs `known`?
A: Use `pending` for gaps you intend to fix; use `known` for accepted limitations you do not plan to address soon.

Q: Should I update the group README before or after adding fixtures?
A: Add fixtures first, run the runner, record failures, then update the README if a new pattern or edge case was introduced.

Q: For mixed-scope patterns, do I add both line and document fixtures?
A: Add the minimal scope needed to isolate the pattern. Add both only if there is distinct line-level behavior and cross-paragraph behavior worth locking down.

## Notes
This workflow intentionally grows the failing set. It is about clarity and scope, not pass rate.

# COORDINATION.md

Guidance for orchestrating multiple agents working in parallel on layered-nlp.

## Goals

- Reduce duplicated work and conflicting edits.
- Keep changes minimal, layered, and fixture-driven.
- Make agent output auditable and easy to merge.

## Preflight (Orchestrator)

- Provide absolute repo path: `/Users/cole/phosphor/phosphor-copy/.repos/layered-nlp`.
- Provide a short command set agents can copy:
  - `rg -n "pattern" /Users/cole/phosphor/phosphor-copy/.repos/layered-nlp/...`
  - `sed -n 'START,ENDp' /Users/cole/phosphor/phosphor-copy/.repos/layered-nlp/...`
  - `cd /Users/cole/phosphor/phosphor-copy/.repos/layered-nlp && cargo ...`
- Use `./ntm.sh` (repo root) to launch sessions rooted in this repo.
- Disable LSP prompts if possible; they interrupt flow.

## Task Slicing

- Assign one **root cause** per agent (e.g., list-item clause links, defined-term patterns).
- Avoid having two agents edit the same file unless explicitly coordinated.
- Keep fixtures and resolver changes in the same agent lane.

## Evidence & Handoff

Each agent should report:
- Summary of changes
- Files changed
- Tests run (with command)
- Issues or blockers
- Tool ideas / improvements wished for

## Safety & Cleanliness

- Avoid ad-hoc debug files; prefer unit tests or temporary files that are deleted before handoff.
- If a temp file is created, delete it and mention it in the report.
- Keep expected failures in `layered-nlp-specs/fixtures/expected_failures.toml` accurate.

## Known Friction Points

- **Large files**: use `rg` + `sed` slices rather than full-file reads.
- **Fixture harness**: `layered-nlp-specs/src/runner.rs` defines span extraction—check it when fixtures “should pass” but don’t.
- **CWD resets**: use absolute paths for commands and file access.

## Conflict Avoidance

- When touching `layered-clauses/src/clause_link_resolver.rs`, avoid parallel edits.
- When touching `layered-contracts/src/defined_term.rs`, avoid parallel edits.
- Coordinate changes that update fixture expectations or shared assertions.

## Suggested Checkpoints

- After analysis, before edits: post a brief plan.
- After edits: run a targeted test (or explain why not).
- Before handoff: provide summary + files + tests + issues.

## Next Phase Plan (Preflight + Loop)

### Preflight (always)
1. Run the fixture harness once and capture the failing fixture list.
2. Compare failures against `layered-nlp-specs/fixtures/expected_failures.toml`.
3. Identify 1–2 **actual** root-cause areas to assign (avoid stale tasks).

### Loop (repeat per root cause)
1. **Assign** one root cause per agent with an explicit "stop if already passing" clause.
2. **Inspect** the minimal repro fixture and the resolver responsible.
3. **Fix** the resolver with narrow, layered changes (no ad-hoc debug files).
4. **Verify** via a targeted test run (or explain why not).
5. **Record** outcome in `COORDINATION_LOG.md` (success, drift, or stale task).

### Safety Rules
- Disable LSP prompts in agent panes to avoid blocking automation.
- Use explicit staging by file path; never rely on `git add .` in this repo.
- If the preflight shows a task is stale, stop and log it rather than editing.

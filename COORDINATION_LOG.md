# Coordination Log

## 2026-01-28
- ntm send only pasted prompts; agents did not run until a manual Enter was sent via tmux. This is a repeatable workflow gap.
- Claude Code LSP install prompts blocked automation; had to select "No"/"Disable all" to proceed.
- Task staleness: both agents found integration tests already passing. Preflight checks (run the target test + scan expected_failures) should happen before assignment.
- Agent drift when tasks are stale: one agent created a debug binary and edited examples/Cargo.toml without need, then had to revert.
- Useful agent tooling wishes: fixture-level runner, smarter test selection by fixture tag, and a fast "are these failing?" precheck.

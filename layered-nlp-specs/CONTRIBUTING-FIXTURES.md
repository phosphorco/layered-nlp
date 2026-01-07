# Contributing Fixtures: The NLP Fixture Flywheel

This document defines the systematic methodology for improving layered-nlp through fixture-driven development. Every fixture is executable documentation; every failure is a roadmap.

---

## 1. The Fixture Flywheel

The Fixture Flywheel is a continuous improvement cycle:

```
Identify Gap → Write Fixture → Fixture Fails → Fix Resolver → Fixture Passes → Coverage Updates → Identify Next Gap
     ↑                                                                                                    |
     └────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Key Principles

1. **Each rotation informs the next** - Failures cluster by category. When three fixtures fail on the same root cause, you have found an architectural gap worth fixing.

2. **Coverage metrics provide momentum and visibility** - Numbers create accountability. Watching 73% become 85% motivates continued effort.

3. **Fixtures are executable documentation** - A fixture that passes proves the capability works. A fixture that fails documents exactly what is missing.

4. **Agent delegation enables scaling** - Clear fixture specifications allow spawning agents to implement fixes or expand coverage in parallel.

---

## 2. Coverage Groups

Organize fixtures by NLP capability. Each group has a target coverage percentage based on importance and feasibility.

| Group | Directory | Capability | Target Coverage |
|-------|-----------|------------|-----------------|
| Obligations | `fixtures/obligations/` | Duty detection (shall, must, will) | 90% |
| Permissions | `fixtures/permissions/` | Permission detection (may, permitted) | 90% |
| Prohibitions | `fixtures/prohibitions/` | Prohibition detection (shall not, cannot) | 85% |
| Defined Terms | `fixtures/defined-terms/` | Definition recognition ("Term" means) | 90% |
| Term References | `fixtures/term-references/` | Subsequent term usage | 85% |
| Pronouns | `fixtures/pronouns/` | Pronoun resolution (it, they to antecedent) | 80% |
| Cross-Paragraph | `fixtures/cross-paragraph/` | Multi-section context | 75% |
| Conflicts | `fixtures/conflicts/` | Contradiction detection | 80% |
| Conditions | `fixtures/conditions/` | Conditional parsing (if, unless, provided) | 85% |
| Semantic Roles | `fixtures/semantic-roles/` | Who-does-what extraction | 80% |

---

## 3. Fixture Authoring Guidelines

### 3.1 File Naming Convention

```
{group}/{pattern}-{variant}.nlp

Examples:
obligations/shall-simple.nlp
obligations/shall-with-condition.nlp
obligations/shall-cross-paragraph.nlp
defined-terms/quoted-means-simple.nlp
defined-terms/parenthetical-definition.nlp
```

Names should be:
- **Descriptive** - The pattern being tested is obvious from the name
- **Searchable** - `grep -r "shall-with"` finds related fixtures
- **Sortable** - Variants group together alphabetically

### 3.2 Fixture Structure

```nlp
# Title: [Pattern Being Tested]
# Group: [obligations|permissions|defined-terms|...]
# Pattern: [Brief description of linguistic pattern]
# Complexity: [simple|compound|edge-case]
# Source: [real-world|synthetic|edge-case]

@paragraph
The text with «ID:marked spans» for assertions.
|ID| SpanType(field=value, field2=value2)

---
@paragraph
Second paragraph if testing cross-paragraph.
|ID2| SpanType(field=value)
```

The header metadata enables:
- Filtering fixtures by group or complexity
- Understanding fixture intent without reading assertions
- Tracing patterns back to their real-world sources

### 3.3 Authoring Principles

1. **One pattern per fixture** - Test one linguistic pattern. If a fixture tests both modal detection AND condition extraction, split it into two fixtures.

2. **Minimal reproducer** - Use the shortest text that exhibits the pattern. Long paragraphs obscure what is being tested.

3. **Progressive complexity** - Start with `{pattern}-simple.nlp`, then add `-with-condition`, `-with-temporal`, `-cross-paragraph` variants.

4. **Real-world grounded** - Patterns should come from actual contracts. Synthetic edge cases are acceptable but should be marked as such.

5. **Named for discoverability** - Clear, searchable names. When debugging, you want to find related fixtures quickly.

6. **Document edge cases** - Note WHY an edge case matters. What contract language triggered this fixture?

### 3.4 Assertion Syntax Reference

```nlp
# Numeric span reference
|1| Obligation(modal=shall, obligation_type=Duty)

# Named entity reference
|T| DefinedTerm(term_name=Tenant, definition_type=QuotedMeans)

# Text reference (for inline matches)
|["Tenant"]| DefinedTerm(term_name=Tenant)

# Entity cross-reference in assertions
|1| Obligation(bearer=§T)

# Confidence threshold
|1| PronounReference(confidence>=0.8)
```

The syntax supports:
- **Numeric IDs** for positional spans within a paragraph
- **Named IDs** for entities that will be referenced elsewhere
- **Text literals** for exact match assertions
- **Cross-references** with `§` prefix to link entities
- **Thresholds** for probabilistic assertions

---

## 4. Prioritization Framework

Not all fixtures are equally important. Use this framework to prioritize fixes.

### 4.1 Priority Score Formula

```
Priority = (Impact × Frequency) / Effort
```

Higher scores indicate higher priority.

### 4.2 Scoring Rubric

**Impact (1-5):**
| Score | Meaning |
|-------|---------|
| 5 | Blocks other capabilities (dependency) |
| 4 | User-visible errors in production |
| 3 | Missing expected capability |
| 2 | Edge case handling |
| 1 | Polish/completeness |

**Frequency (1-5):**
| Score | Meaning |
|-------|---------|
| 5 | Every contract has this pattern |
| 4 | Most contracts (>70%) |
| 3 | Common (~50%) |
| 2 | Occasional (~20%) |
| 1 | Rare (<10%) |

**Effort (1-5):**
| Score | Meaning |
|-------|---------|
| 5 | Architectural change required |
| 4 | New resolver needed |
| 3 | Significant resolver modification |
| 2 | Minor resolver tweak |
| 1 | Fixture/assertion adjustment only |

### 4.3 Priority Tiers

| Tier | Score | Action |
|------|-------|--------|
| **P0** | >= 15 | Fix immediately, blocks progress |
| **P1** | 8-14 | Fix this week, high value |
| **P2** | 4-7 | Fix when convenient |
| **P3** | < 4 | Backlog, fix opportunistically |

**Example calculation:**
- `shall-with-condition.nlp` fails
- Impact: 4 (missing expected capability)
- Frequency: 5 (every contract has conditional obligations)
- Effort: 3 (significant resolver modification)
- Score: (4 × 5) / 3 = 6.67 → P2

---

## 5. Daily Workflow

### 5.1 Morning Review (30 min)

```bash
# Run current test suite
cargo test -p layered-nlp-specs

# Check failure status
cat fixtures/expected_failures.toml | grep -c "pending"

# Review high-priority pending items
grep -A3 "priority = \"P0\"" fixtures/expected_failures.toml
```

This establishes current state and surfaces blockers.

### 5.2 Work Session (2-4 hrs)

**If fixture exists but fails:**
1. Diagnose root cause (resolver issue vs fixture issue)
2. Spawn agent to implement fix
3. Verify fixture passes
4. Update expected_failures.toml

**If pattern is uncovered:**
1. Write new fixture following guidelines
2. Run to verify it fails (expected)
3. Add to expected_failures.toml as pending
4. Spawn agent to implement fix (or mark for later)

**Batch similar fixes:**
- If 3+ fixtures fail on same root cause, fix once
- Group by resolver, not by fixture

### 5.3 End of Day (15 min)

```bash
# Stage passing fixtures
git add fixtures/

# Update expected_failures.toml
# - Remove fixed items
# - Add new pending items with priority

# Commit with coverage summary
git commit -m "test(fixtures): X fixtures now passing

- Fixed: [list fixed patterns]
- Added: [list new fixtures]
- Coverage: obligations 80%, permissions 90%"
```

### 5.4 Weekly Review (1 hr)

1. **Coverage by group** - Which groups are below target?
2. **Velocity** - How many fixtures fixed this week?
3. **Blockers** - What architectural issues are blocking progress?
4. **Priorities** - Adjust P0/P1 based on learnings

---

## 6. Coverage Checklists

Use these checklists to track progress toward coverage targets.

### 6.1 Obligations (Target: 90%)

- [ ] shall-simple
- [ ] shall-with-bearer
- [ ] shall-with-condition-if
- [ ] shall-with-condition-unless
- [ ] shall-with-temporal
- [ ] shall-cross-paragraph
- [ ] shall-in-list-item
- [ ] must-simple
- [ ] must-with-bearer
- [ ] will-simple
- [ ] will-with-bearer

### 6.2 Permissions (Target: 90%)

- [ ] may-simple
- [ ] may-with-bearer
- [ ] may-with-condition
- [ ] is-permitted-to
- [ ] has-the-right-to
- [ ] is-entitled-to

### 6.3 Prohibitions (Target: 85%)

- [ ] shall-not-simple
- [ ] must-not-simple
- [ ] cannot-simple
- [ ] may-not-simple
- [ ] is-prohibited-from
- [ ] is-not-permitted-to

### 6.4 Defined Terms (Target: 90%)

- [ ] quoted-means-simple
- [ ] quoted-means-with-article
- [ ] parenthetical-definition
- [ ] hereinafter-definition
- [ ] inline-definition
- [ ] multi-word-term
- [ ] cross-paragraph-reference

### 6.5 Pronouns (Target: 80%)

- [ ] it-simple
- [ ] they-simple
- [ ] such-party
- [ ] the-foregoing
- [ ] cross-paragraph-antecedent
- [ ] ambiguous-antecedent

### 6.6 Conditions (Target: 85%)

- [ ] if-simple
- [ ] unless-simple
- [ ] provided-that
- [ ] subject-to
- [ ] in-the-event-that
- [ ] nested-conditions

---

## 7. Working with Agents

Agents can parallelize fixture work. Provide clear specifications.

### 7.1 Spawning a Fix Agent

When you have a failing fixture, spawn an agent with:

```
Fix the failing fixture at fixtures/obligations/shall-with-condition.nlp

The fixture expects: Obligation(modal=shall, bearer=§T)
Current error: NotFound: No ObligationPhrase found for 'shall pay'

Root cause hypothesis: [your analysis]

Files to investigate:
- layered-contracts/src/obligation.rs
- layered-nlp-specs/src/runner.rs

Success criteria: cargo test -p layered-nlp-specs test_full_pipeline_integration passes
```

### 7.2 Spawning a Coverage Agent

To expand coverage for a group:

```
Add fixtures to reach 90% coverage for the obligations group.

Current fixtures: shall-simple.nlp, shall-with-condition.nlp
Missing patterns: must-simple, will-simple, shall-with-temporal

For each new fixture:
1. Follow CONTRIBUTING-FIXTURES.md guidelines
2. Verify it fails initially (expected)
3. Add to expected_failures.toml with priority score
4. Do NOT implement fixes - just add the fixtures
```

### 7.3 Spawning a Batch Fix Agent

When multiple fixtures share a root cause:

```
Fix cross-paragraph context propagation affecting these fixtures:
- multi-paragraph-obligation.nlp
- pronoun-reference.nlp
- defined-term-complex.nlp

All fail because entities defined in paragraph 0 are not visible in paragraph 1.

This is an architectural issue in run_fixture() which processes paragraphs independently.

Options:
A) Use document-level resolvers instead of line-level
B) Pass entity context between paragraph processing
C) Change fixture expectations to single-paragraph only

Recommend option and implement.
```

---

## 8. The expected_failures.toml Format

Track known and pending failures in a structured format.

```toml
# Pending: Known failures we intend to fix
[[pending]]
fixture = "obligations/shall-with-condition.nlp"
assertion = "S0.[1]"
reason = "Condition extraction not implemented"
priority = "P1"
impact = 4
frequency = 4
effort = 3
added = "2025-01-07"
blocked_by = []  # Optional: other fixtures that must pass first

# Known: Failures we accept (architectural limitations)
[[known]]
fixture = "cross-paragraph/deep-nesting.nlp"
assertion = "S3.[1]"
reason = "4+ paragraph depth not supported by design"
added = "2025-01-07"
wont_fix = true
```

Fields:
- `fixture` - Path relative to fixtures directory
- `assertion` - Which assertion fails (for multi-assertion fixtures)
- `reason` - Why it fails, briefly
- `priority` - P0/P1/P2/P3 tier
- `impact`, `frequency`, `effort` - Scores from prioritization rubric
- `added` - Date added to registry
- `blocked_by` - Other fixtures that must pass first
- `wont_fix` - True for known architectural limitations

---

## 9. Success Metrics

### 9.1 Coverage Dashboard (Future)

```bash
mise run fixture-coverage

# Output:
# obligations:     8/11 (73%) [========  ] Target: 90%
# permissions:     5/6  (83%) [========= ] Target: 90%
# prohibitions:    0/6  (0%)  [          ] Target: 85%
# defined-terms:   2/7  (29%) [===       ] Target: 90%
# ...
```

### 9.2 Weekly Velocity

Track these metrics weekly:
- **Fixtures added** - How many new test cases created
- **Fixtures fixed** - How many previously-failing fixtures now pass
- **Coverage delta** - Net change in coverage percentage

### 9.3 Quality Indicators

- **Zero regressions** - Passing fixtures stay passing
- **Test before merge** - All new resolvers have fixtures before merge
- **Coverage monotonic** - Coverage never decreases

---

## Quick Reference

```bash
# Run all fixture tests
cargo test -p layered-nlp-specs

# Run specific fixture test
cargo test -p layered-nlp-specs test_full_pipeline_integration

# Check what is failing
cargo test -p layered-nlp-specs 2>&1 | grep "NotFound"

# Add new fixture (then run to verify it fails)
vim fixtures/obligations/must-simple.nlp

# Update failures registry
vim fixtures/expected_failures.toml
```

---

*This document defines the systematic approach to improving layered-nlp through fixture-driven development. Follow the flywheel, and each rotation makes the next easier.*

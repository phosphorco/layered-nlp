# Feedback Request: CONTRIBUTING-FIXTURES.md

## Context

We've created a comprehensive guide for systematic NLP development through fixture-driven testing. The document defines:

- **The Fixture Flywheel**: A continuous improvement cycle for NLP capabilities
- **Coverage Groups**: 10 NLP capability categories with target coverage percentages
- **Authoring Guidelines**: How to write effective .nlp fixtures
- **Prioritization Framework**: How to decide what to work on (Impact x Frequency / Effort)
- **Daily Workflow**: Practical processes for contributors
- **Agent Integration**: Templates for spawning agents to fix or expand coverage

The goal is to create a sustainable development flywheel where:
1. Gaps are identified through coverage metrics
2. Fixtures document expected behavior
3. Agents implement fixes systematically
4. Progress is measurable and visible

**Before we rely on this guide**, we want feedback to ensure it's accurate, usable, and complete.

---

## Feedback Dimensions

We're interested in feedback across six dimensions:

### 1. Alignment with Reality (Priority: HIGH)

Does the document match what actually exists in the codebase?

**Specific questions:**
- Does the assertion syntax (`|1| Obligation(modal=shall)`) match what the parser supports?
- Are the resolver names (ObligationPhraseResolver, DefinedTermResolver, etc.) accurate?
- Does the expected_failures.toml format described match the actual implementation?
- Are the file paths correct?

**How to check:** Read the parser.rs, matcher.rs, and failures.rs files and compare to examples in the guide.

### 2. Usability - "Can I Follow This?" (Priority: HIGH)

Can someone actually use this guide to contribute?

**Specific questions:**
- If I want to add a new fixture, are the instructions complete?
- Is the daily workflow realistic or aspirational?
- Are there steps where I'd get stuck without additional information?
- Is the priority formula actually useful or just overhead?

**How to check:** Attempt to add one new fixture (e.g., `line/prohibitions/modal/shall-not-simple.nlp`) following the guide exactly. Note where you get stuck.

### 3. Coverage Group Completeness (Priority: MEDIUM)

Are the 10 coverage groups and their pattern checklists comprehensive?

**Specific questions:**
- Are there NLP capabilities used in contract analysis that aren't covered?
- Are there common linguistic patterns missing from the checklists?
- Are some groups over-specified while others are under-specified?
- Do the target percentages (75-90%) make sense?

**How to check:** Review the existing resolvers in layered-contracts/ and layered-nlp/ to see if all capabilities are represented.

### 4. Workflow Realism (Priority: MEDIUM)

Will people actually follow this workflow?

**Specific questions:**
- Is 30 minutes for morning review realistic?
- Will contributors actually update expected_failures.toml with priority scores?
- Is the weekly review cadence appropriate?
- What friction will cause people to skip steps?

**How to check:** Simulate a week of following the workflow. What feels burdensome?

### 5. Missing Infrastructure (Priority: LOW - but important to document)

What does the guide reference that doesn't exist yet?

**Known gaps:**
- `mise run fixture-coverage` dashboard doesn't exist
- Priority/impact/effort fields aren't in current expected_failures.toml
- No fixture linter exists
- Some coverage group directories are still empty

**Question:** Should we build this infrastructure before finalizing the guide, or document it as "future work"?

### 6. Agent Usability (Priority: MEDIUM)

Can an agent effectively use the spawning templates?

**Specific questions:**
- Are the agent spawning templates complete enough?
- Is sufficient context provided for an agent to succeed?
- Should we include example successful agent outputs?

**How to check:** Spawn an agent using one of the templates and evaluate the result.

---

## Priority Order for Review

If you have limited time, review in this order:

1. **Alignment Check** - Verify examples actually work (30 min)
2. **Try to Follow It** - Add one fixture following the guide (1 hr)
3. **Coverage Completeness** - Review against actual resolvers (30 min)
4. **Workflow Simulation** - Imagine a week of use (15 min)

---

## How to Provide Feedback

### Option A: Inline Comments
Add comments directly to CONTRIBUTING-FIXTURES.md using this format:
```
<!-- FEEDBACK: [dimension] - [your comment] -->
```

### Option B: Structured Response
Create a feedback file with sections:
```markdown
## Alignment Issues
- [issue 1]
- [issue 2]

## Usability Gaps
- [gap 1]

## Suggestions
- [suggestion 1]
```

### Option C: PR with Fixes
If you find issues, feel free to fix them directly and submit changes.

---

## Specific Validation Tasks

If you want concrete tasks to validate the guide:

### Task 1: Syntax Validation (15 min)
Read `layered-nlp-specs/src/parser.rs` and verify:
- [ ] The fixture header syntax (`# Title:`, `# Group:`, etc.) is supported
- [ ] The span marker syntax (`<<ID:text>>`) is correct
- [ ] The assertion syntax (`|ID| Type(field=value)`) is accurate
- [ ] Entity cross-references (`@T`) work as described

### Task 2: Add a Fixture (30 min)
Following the guide exactly, add `fixtures/line/prohibitions/modal/shall-not-simple.nlp`:
- [ ] Create the file with correct structure
- [ ] Run the test suite
- [ ] Verify it fails as expected
- [ ] Add to expected_failures.toml
- [ ] Note any steps where the guide was unclear

### Task 3: Priority Calculation (15 min)
For 3 existing failures in expected_failures.toml:
- [ ] Calculate priority score using the formula
- [ ] Assign P0/P1/P2/P3 tier
- [ ] Evaluate: Was this useful or busy work?

### Task 4: Coverage Gap Analysis (30 min)
Compare the coverage checklists against:
- [ ] `layered-contracts/src/` - what resolvers exist?
- [ ] `layered-nlp/src/` - what capabilities are built in?
- [ ] Note any resolvers without corresponding coverage groups

---

## Timeline

- **Feedback requested by:** [No hard deadline - ongoing]
- **Planned revisions:** After significant feedback received
- **Infrastructure buildout:** Based on feedback about what's blocking usability

---

## Questions for Reviewers

1. Is this guide **too comprehensive** (overwhelming) or **not comprehensive enough** (leaves gaps)?

2. Should we **build the missing infrastructure first** (directories, coverage dashboard) or finalize the guide first?

3. Is the **priority formula** actually useful, or should we simplify to just P0/P1/P2 tiers without the math?

4. Are there **existing patterns** in the layered-nlp codebase we should follow instead of inventing new ones?

5. What's the **minimum viable version** of this guide that would enable contributions?

---

*Thank you for reviewing. Feedback helps ensure this guide actually enables the flywheel we're trying to create.*

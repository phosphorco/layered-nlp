# SemanticDiffEngine & DocumentAligner Demonstration Specification

> **Interesting artifacts and learnings must be written back to this document.**

## Executive Summary

This document specifies a phased demonstration of the `DocumentAligner` and `SemanticDiffEngine` components. The goal is to provide concrete, runnable examples that showcase the semantic contract comparison capabilities, validate the implementation against real-world scenarios, and establish patterns for integration with external systems (LLMs, expert review queues, UI).

---

## System Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           DEMONSTRATION PIPELINE                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────┐     ┌─────────────────┐     ┌──────────────────────────┐  │
│  │  Original   │────▶│  Per-Line       │────▶│  DocumentStructure       │  │
│  │  Contract   │     │  Resolvers      │     │  (hierarchical sections) │  │
│  └─────────────┘     └─────────────────┘     └────────────┬─────────────┘  │
│                                                           │                 │
│  ┌─────────────┐     ┌─────────────────┐     ┌────────────▼─────────────┐  │
│  │  Revised    │────▶│  Per-Line       │────▶│  DocumentStructure       │  │
│  │  Contract   │     │  Resolvers      │     │  (hierarchical sections) │  │
│  └─────────────┘     └─────────────────┘     └────────────┬─────────────┘  │
│                                                           │                 │
│                                              ┌────────────▼─────────────┐  │
│                                              │    DocumentAligner       │  │
│                                              │    (section matching)    │  │
│                                              └────────────┬─────────────┘  │
│                                                           │                 │
│                                              ┌────────────▼─────────────┐  │
│                                              │   SemanticDiffEngine     │  │
│                                              │   (change detection)     │  │
│                                              └────────────┬─────────────┘  │
│                                                           │                 │
│                                              ┌────────────▼─────────────┐  │
│                                              │   SemanticDiffResult     │  │
│                                              │   (JSON exportable)      │  │
│                                              └──────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Demo Infrastructure & Sample Contracts

### Objectives
- Establish demonstration infrastructure (test harness, sample contracts, output formatting)
- Create a curated set of contract pairs that exercise all alignment and diff scenarios
- Define output formats for human review and machine consumption

### Scope
- Sample contract corpus creation
- Demo runner infrastructure
- Output formatting utilities

### Dependencies
- None (foundational phase)

### Task List

| ID | Task | Acceptance Criteria |
|----|------|---------------------|
| 1.1 | Create sample contract: "Simple NDA" original | Contains: 5+ sections, 3+ defined terms, 5+ obligations, 2+ temporal expressions |
| 1.2 | Create sample contract: "Simple NDA" revised (minor changes) | Same structure as original, 2-3 obligation modal changes, 1 term redefinition |
| 1.3 | Create sample contract: "Service Agreement" original | Contains: 10+ sections with nesting, 5+ defined terms, 10+ obligations |
| 1.4 | Create sample contract: "Service Agreement" revised (structural changes) | Section renumbering, 1 section split, 1 section merged, 2 sections added |
| 1.5 | Create demo runner binary/example | Accepts two file paths, runs full pipeline, outputs to stdout and JSON file |
| 1.6 | Create human-readable output formatter | Produces markdown report with change summary, risk breakdown, party impacts |
| 1.7 | Create machine-readable JSON schema documentation | Documents all fields in SemanticDiffResult with examples |

### Verification

**Test Scenarios:**
1. Demo runner accepts valid contract files and produces output
2. Demo runner handles empty files gracefully with appropriate warning
3. Demo runner handles malformed input (non-text, binary) with error message
4. Output formatter produces valid markdown that renders correctly
5. JSON output validates against documented schema

**Required Coverage:**
- All sample contracts must parse without errors
- All sample contracts must produce non-empty DocumentStructure
- Demo runner must handle all error paths without panicking

**Pass/Fail Criteria:**
- PASS: All sample contracts produce expected section counts (documented per contract)
- PASS: JSON output is valid and parseable
- PASS: Markdown output contains all required sections (Summary, Changes, Party Impacts)
- FAIL: Any panic or unhandled error during demo execution

**Test Organization:**
- Directory: `layered-contracts/tests/demo/`
- Naming: `demo_phase1_*.rs`
- Sample contracts: `layered-contracts/tests/fixtures/contracts/`

---

## Phase 2: DocumentAligner — Basic Alignment

### Objectives
- Demonstrate exact section matching by canonical ID
- Demonstrate section matching by title when IDs differ (renumbering)
- Demonstrate modified section detection (same ID, different content)

### Scope
- ExactMatch, Renumbered, and Modified alignment types
- Confidence scoring visibility
- Signal breakdown in output

### Dependencies
- Phase 1 (sample contracts, demo infrastructure)

### Task List

| ID | Task | Acceptance Criteria |
|----|------|---------------------|
| 2.1 | Demo: Exact match alignment | Given two identical sections with same ID, alignment type is ExactMatch with confidence >= 0.95 |
| 2.2 | Demo: Renumbered section alignment | Given "Section 1.1" → "Section 2.1" with identical content, alignment type is Renumbered |
| 2.3 | Demo: Modified section alignment | Given same section ID with changed content, alignment type is Modified with similarity score |
| 2.4 | Demo: Signal breakdown display | Output shows individual signal contributions (id_match, title_match, semantic_similarity, text_similarity) |
| 2.5 | Demo: Confidence threshold effects | Show how varying `match_threshold` in SimilarityConfig affects alignment decisions |
| 2.6 | Document: Alignment type decision tree | Flowchart showing how alignment type is determined from signals |

### Verification

**Test Scenarios:**
1. Two sections with identical ID and content → ExactMatch
2. Two sections with different ID but identical title and content → Renumbered
3. Two sections with same ID but 50% different content → Modified
4. Two sections with same ID but 90% different content → still Modified (not Deleted+Inserted)
5. Section with unique ID in original, no match in revised → Deleted
6. Section with unique ID in revised, no match in original → Inserted

**Required Coverage:**
- Each alignment type (ExactMatch, Renumbered, Modified, Deleted, Inserted) demonstrated
- Confidence scores validated against expected ranges
- Signal weights shown and explained

**Pass/Fail Criteria:**
- PASS: All expected alignment types produced for test contract pair
- PASS: Confidence scores are in range [0.0, 1.0]
- PASS: Signal scores sum contribution matches total score (within floating point tolerance)
- FAIL: Wrong alignment type for any test case
- FAIL: Missing alignment for any section

**Test Organization:**
- Directory: `layered-contracts/tests/demo/`
- Naming: `demo_phase2_alignment_*.rs`
- Fixtures: `layered-contracts/tests/fixtures/alignment/`

---

## Phase 3: DocumentAligner — Complex Alignment (Split/Merge)

### Objectives
- Demonstrate split detection (one original → multiple revised)
- Demonstrate merge detection (multiple original → one revised)
- Show how coverage thresholds affect split/merge detection

### Scope
- Split and Merged alignment types
- Multi-section alignment relationships
- Coverage percentage reporting

### Dependencies
- Phase 2 (basic alignment working)

### Task List

| ID | Task | Acceptance Criteria |
|----|------|---------------------|
| 3.1 | Demo: Section split detection | Original "Section 5" split into "Section 5.1" and "Section 5.2" detected with coverage >= 80% |
| 3.2 | Demo: Section merge detection | Original "Section 3.1" + "Section 3.2" merged into "Section 3" detected |
| 3.3 | Demo: Partial split (incomplete coverage) | When split coverage < 80%, show as Modified + Inserted rather than Split |
| 3.4 | Demo: Split with content changes | Split where content also changed shows combined Split + content differences |
| 3.5 | Demo: Merge with content changes | Merge where content was also edited shows combined Merge + modifications |
| 3.6 | Document: Split/merge algorithm explanation | Describe the content overlap calculation and coverage threshold logic |

### Verification

**Test Scenarios:**
1. Clean split: 1 section → 2 sections, 100% content preserved → Split detected
2. Clean merge: 2 sections → 1 section, 100% content preserved → Merged detected
3. Partial split: 1 section → 2 sections, only 60% content matched → NOT detected as Split
4. Split with additions: 1 section → 2 sections + new content → Split with modification notes
5. Three-way split: 1 section → 3 sections → Split with all three targets listed

**Required Coverage:**
- Split detection with 2, 3, and 4 target sections
- Merge detection with 2 and 3 source sections
- Boundary cases at exactly 80% coverage threshold

**Pass/Fail Criteria:**
- PASS: Split detected when coverage >= 80%
- PASS: Merge detected when combined sources cover >= 80% of target
- PASS: AlignedPair.original and .revised vectors correctly populated for multi-section alignments
- FAIL: Split/Merge detected below coverage threshold
- FAIL: Missing sections in split/merge relationships

**Test Organization:**
- Directory: `layered-contracts/tests/demo/`
- Naming: `demo_phase3_split_merge_*.rs`
- Fixtures: `layered-contracts/tests/fixtures/split_merge/`

---

## Phase 4: DocumentAligner — External Hint Integration

### Objectives
- Demonstrate exporting uncertain alignments for external review
- Demonstrate importing hints from external systems (LLM, expert)
- Show the round-trip workflow: compute → export → (external) → import → finalize

### Scope
- AlignmentCandidates export
- AlignmentHint import
- HintType effects (ForceMatch, ForceNoMatch, AdjustConfidence, OverrideType)

### Dependencies
- Phase 3 (all alignment types working)

### Task List

| ID | Task | Acceptance Criteria |
|----|------|---------------------|
| 4.1 | Demo: Export uncertain candidates | Alignments with confidence < 0.75 exported to JSON with excerpts |
| 4.2 | Demo: ForceMatch hint application | External hint forces alignment between two sections that wouldn't otherwise match |
| 4.3 | Demo: ForceNoMatch hint application | External hint prevents alignment that would otherwise occur |
| 4.4 | Demo: AdjustConfidence hint application | External hint raises/lowers confidence by specified delta |
| 4.5 | Demo: OverrideType hint application | External hint changes alignment type (e.g., Modified → Renumbered) |
| 4.6 | Demo: LLM integration pattern | Pseudo-code/documentation showing how to call LLM API with candidates, parse response to hints |
| 4.7 | Demo: Expert queue integration pattern | Documentation showing database schema for queuing candidates for human review |

### Verification

**Test Scenarios:**
1. Export candidates → JSON is valid and contains expected fields
2. Round-trip: export → no changes → import empty hints → same result as direct align()
3. ForceMatch: two unrelated sections, apply ForceMatch → now aligned
4. ForceNoMatch: two matching sections, apply ForceNoMatch → now both Deleted/Inserted
5. AdjustConfidence +0.2: low-confidence alignment now passes review threshold
6. OverrideType: Modified → Renumbered changes the classification but not the pairing

**Required Coverage:**
- All HintType variants exercised
- JSON serialization/deserialization round-trip verified
- Confidence clamping to [0.0, 1.0] after adjustment

**Pass/Fail Criteria:**
- PASS: Exported JSON deserializes to identical structure
- PASS: Each hint type produces documented effect
- PASS: Multiple hints on same candidate combine correctly
- FAIL: Hint application changes unexpected alignments
- FAIL: Confidence outside [0.0, 1.0] after adjustment

**Test Organization:**
- Directory: `layered-contracts/tests/demo/`
- Naming: `demo_phase4_hints_*.rs`
- Fixtures: `layered-contracts/tests/fixtures/hints/`

---

## Phase 5: SemanticDiffEngine — Obligation Changes

### Objectives
- Demonstrate obligation modal change detection (shall → may, etc.)
- Show risk level classification for different modal transitions
- Display party impact analysis (who benefits, who is harmed)

### Scope
- ObligationModal change type
- RiskLevel classification
- PartyImpact with direction

### Dependencies
- Phase 2 (basic alignment needed for section pairing)

### Task List

| ID | Task | Acceptance Criteria |
|----|------|---------------------|
| 5.1 | Demo: shall → may detection | "Company shall pay" → "Company may pay" detected as ObligationModal change |
| 5.2 | Demo: may → shall detection | Permission upgraded to duty detected and classified as favorable to beneficiary |
| 5.3 | Demo: shall → shall not detection | Duty to prohibition detected as Critical risk |
| 5.4 | Demo: Risk level assignment | shall→may is High risk, condition addition is Medium risk |
| 5.5 | Demo: Party impact display | For each change, show affected parties with Favorable/Unfavorable/Neutral |
| 5.6 | Demo: Bidirectional impact | For obligation weakening, show unfavorable to beneficiary AND favorable to obligor |
| 5.7 | Document: Modal transition risk matrix | Table showing from→to modal and resulting risk level |

### Verification

**Test Scenarios:**
1. shall → may: High risk, unfavorable to beneficiary, favorable to obligor
2. may → shall: Medium risk (more certainty), favorable to beneficiary
3. shall → must: Low risk (synonym), neutral to both parties
4. shall not → may: Critical risk (prohibition removed)
5. Multiple modal changes in same section: all detected and reported separately
6. No modal change (identical obligations): no ObligationModal changes generated

**Required Coverage:**
- All ObligationType transitions tested
- Risk levels verified for each transition type
- Party impact direction verified for each transition

**Pass/Fail Criteria:**
- PASS: All modal transitions detected with correct from/to types
- PASS: Risk levels match documented matrix
- PASS: Party impacts show correct direction for affected parties
- FAIL: Modal change missed
- FAIL: Wrong risk level for known transition
- FAIL: Missing party in impact list

**Test Organization:**
- Directory: `layered-contracts/tests/demo/`
- Naming: `demo_phase5_obligations_*.rs`
- Fixtures: `layered-contracts/tests/fixtures/obligations/`

---

## Phase 6: SemanticDiffEngine — Term & Temporal Changes

### Objectives
- Demonstrate term definition change detection
- Show downstream reference impact for changed terms
- Demonstrate temporal expression change detection

### Scope
- TermDefinition change type with reference count
- Temporal change type with value comparison
- Downstream impact reporting

### Dependencies
- Phase 5 (semantic diff infrastructure)

### Task List

| ID | Task | Acceptance Criteria |
|----|------|---------------------|
| 6.1 | Demo: Term narrowing detection | "Confidential Information" definition narrowed, change classified appropriately |
| 6.2 | Demo: Term expansion detection | Definition expanded to include more, classified as scope change |
| 6.3 | Demo: Reference count in term changes | Term change shows count of downstream references affected |
| 6.4 | Demo: Temporal value change | "30 days" → "45 days" detected with value and unit |
| 6.5 | Demo: Temporal unit change | "30 days" → "1 month" detected as unit change |
| 6.6 | Demo: Temporal removal | Deadline removed entirely, flagged as high risk |
| 6.7 | Document: Term change classification guide | Explain narrowing vs. expansion vs. complete replacement |

### Verification

**Test Scenarios:**
1. Term narrowing: definition with exclusion added → detected as narrowing
2. Term expansion: definition with inclusion added → detected as expansion
3. Term reference count: changed term with 5 references → change shows affected_references = 5
4. Temporal increase: 30 days → 60 days → detected with direction "extended"
5. Temporal decrease: 60 days → 30 days → detected with direction "shortened"
6. Temporal addition: no deadline → "within 30 days" → detected as addition
7. Temporal removal: "within 30 days" → no deadline → detected as removal, high risk

**Required Coverage:**
- Term narrowing, expansion, and replacement
- Temporal increase, decrease, addition, and removal
- Reference impact counts verified against actual term usage

**Pass/Fail Criteria:**
- PASS: All term changes detected with correct classification
- PASS: Reference counts accurate (verified by grep of term in document)
- PASS: Temporal changes detected with correct direction
- FAIL: Term change missed
- FAIL: Wrong reference count
- FAIL: Temporal direction incorrect

**Test Organization:**
- Directory: `layered-contracts/tests/demo/`
- Naming: `demo_phase6_terms_temporal_*.rs`
- Fixtures: `layered-contracts/tests/fixtures/terms_temporal/`

---

## Phase 7: SemanticDiffEngine — Structural Changes

### Objectives
- Demonstrate section addition detection
- Demonstrate section removal detection with obligation analysis
- Show broken reference detection when sections are removed

### Scope
- SectionAdded and SectionRemoved change types
- Obligation inventory for removed sections
- Broken reference warnings

### Dependencies
- Phase 6 (term and temporal detection)

### Task List

| ID | Task | Acceptance Criteria |
|----|------|---------------------|
| 7.1 | Demo: Section addition with obligations | New section with 2 obligations, both reported in change |
| 7.2 | Demo: Section removal with obligations | Removed section's obligations inventoried in change |
| 7.3 | Demo: Section removal risk classification | Removal of obligation-heavy section is High risk |
| 7.4 | Demo: Broken reference warning | Section removed that was referenced by another section, warning generated |
| 7.5 | Demo: Section reordering (not removal) | Section moved to different position but content preserved, classified as Moved not Removed+Added |
| 7.6 | Document: Structural change impact guide | Explain how section changes affect overall contract balance |

### Verification

**Test Scenarios:**
1. Add empty section: detected as SectionAdded, low risk
2. Add section with 3 obligations: detected with obligation count = 3
3. Remove section with 0 obligations: detected as SectionRemoved, low risk
4. Remove section with 2 duties: detected as SectionRemoved, lists lost obligations
5. Remove section referenced by Section 9.1: warning includes "Section 9.1 references removed section"
6. Move section from position 3 to position 7: classified as Moved, not Removed+Added

**Required Coverage:**
- Section addition with 0, 1, and 3+ obligations
- Section removal with 0, 1, and 3+ obligations
- Broken reference detection for direct references
- Section movement vs. removal disambiguation

**Pass/Fail Criteria:**
- PASS: All additions and removals detected
- PASS: Obligation counts accurate for added/removed sections
- PASS: Broken references produce warnings
- FAIL: Section movement misclassified as removal+addition
- FAIL: Obligation count wrong
- FAIL: Broken reference not warned

**Test Organization:**
- Directory: `layered-contracts/tests/demo/`
- Naming: `demo_phase7_structural_*.rs`
- Fixtures: `layered-contracts/tests/fixtures/structural/`

---

## Phase 8: End-to-End Integration Demo

### Objectives
- Demonstrate complete pipeline from raw text to semantic diff result
- Show JSON export suitable for UI consumption
- Demonstrate review candidate workflow
- Provide realistic contract comparison scenario

### Scope
- Full pipeline execution
- JSON output validation
- Review workflow demonstration
- Performance baseline

### Dependencies
- All previous phases

### Task List

| ID | Task | Acceptance Criteria |
|----|------|---------------------|
| 8.1 | Demo: Complete NDA comparison | Two NDA versions, full diff with all change types |
| 8.2 | Demo: Complete Service Agreement comparison | Complex contracts with structural changes |
| 8.3 | Demo: JSON export for UI | Export includes all fields needed for UI rendering (as documented in Phase 1) |
| 8.4 | Demo: Review candidates export | Low-confidence items exported with excerpts for review |
| 8.5 | Demo: Party perspective view | Filter/display changes from Company perspective vs. Contractor perspective |
| 8.6 | Demo: Summary statistics | Total changes by risk level, changes by type, party balance shift |
| 8.7 | Benchmark: Pipeline performance | Document execution time for 10-page, 50-page, 100-page contracts |
| 8.8 | Demo: Incremental update pattern | Show how to re-diff after user verification of one item |

### Verification

**Test Scenarios:**
1. NDA comparison produces complete result with expected change counts
2. Service Agreement comparison handles nested sections correctly
3. JSON output validates against schema
4. Review candidates include only items below threshold
5. Party perspective correctly filters favorable/unfavorable
6. Summary statistics sum correctly (total = critical + high + medium + low)
7. 50-page contract completes in < 5 seconds
8. Incremental update preserves verified items

**Required Coverage:**
- End-to-end with real-world contract structures
- All output formats (human, JSON, review candidates)
- Party perspective views for both parties
- Performance with varying document sizes

**Pass/Fail Criteria:**
- PASS: Complete pipeline runs without error on all sample contracts
- PASS: JSON validates against documented schema
- PASS: Summary statistics are mathematically consistent
- PASS: Performance within documented bounds
- FAIL: Any pipeline step fails
- FAIL: Inconsistent statistics
- FAIL: Performance exceeds 10x documented bounds

**Test Organization:**
- Directory: `layered-contracts/tests/demo/`
- Naming: `demo_phase8_e2e_*.rs`
- Fixtures: `layered-contracts/tests/fixtures/e2e/`
- Benchmarks: `layered-contracts/benches/` (using criterion)

---

## Phase 9: CLI & Interactive Demo

### Objectives
- Provide command-line interface for contract comparison
- Enable interactive exploration of results
- Support integration with editor/IDE

### Scope
- CLI binary with subcommands
- Interactive result navigation
- Editor integration hooks

### Dependencies
- Phase 8 (complete pipeline)

### Task List

| ID | Task | Acceptance Criteria |
|----|------|---------------------|
| 9.1 | CLI: `contract-diff compare <original> <revised>` | Produces JSON output to stdout |
| 9.2 | CLI: `--format` flag for output format | Supports `json`, `markdown`, `summary` |
| 9.3 | CLI: `--party` flag for perspective | Filters output to specified party's view |
| 9.4 | CLI: `--risk-level` flag for filtering | Shows only changes at or above specified risk |
| 9.5 | CLI: `--export-candidates` for review workflow | Exports uncertain items to file |
| 9.6 | CLI: `--import-hints` to apply external feedback | Applies hints file before computing final diff |
| 9.7 | Demo: VS Code integration pattern | Documentation for using CLI from VS Code task |
| 9.8 | Demo: CI/CD integration pattern | GitHub Actions workflow for contract change review |

### Verification

**Test Scenarios:**
1. `compare` with valid files produces output
2. `compare` with missing file produces helpful error
3. `--format json` produces valid JSON
4. `--format markdown` produces valid markdown
5. `--party Company` filters to Company-relevant changes
6. `--risk-level high` shows only High and Critical
7. `--export-candidates` creates valid JSON file
8. `--import-hints` with valid hints modifies result

**Required Coverage:**
- All CLI flags tested
- Error messages tested for all error conditions
- Output format tests for each format

**Pass/Fail Criteria:**
- PASS: CLI runs with documented flags
- PASS: Error messages are actionable
- PASS: Output formats are valid and complete
- FAIL: CLI panics on any input
- FAIL: Flag combination produces unexpected result

**Test Organization:**
- Directory: `layered-contracts/tests/cli/`
- Naming: `cli_*.rs`
- Integration: Use `assert_cmd` crate for CLI testing

---

## Appendix A: Sample Contract Templates

### A.1 Simple NDA Structure
```
ARTICLE I: DEFINITIONS
  Section 1.1: "Confidential Information" means...
  Section 1.2: "Disclosing Party" means...
  Section 1.3: "Receiving Party" means...

ARTICLE II: OBLIGATIONS
  Section 2.1: The Receiving Party shall protect...
  Section 2.2: The Receiving Party shall not disclose...
  Section 2.3: The Receiving Party may disclose if...

ARTICLE III: TERM
  Section 3.1: This Agreement shall remain in effect for...
  Section 3.2: The obligations shall survive for...

ARTICLE IV: MISCELLANEOUS
  Section 4.1: Governing Law
  Section 4.2: Entire Agreement
```

### A.2 Service Agreement Structure
```
ARTICLE I: DEFINITIONS (10+ terms)
ARTICLE II: SERVICES (scope, deliverables)
ARTICLE III: COMPENSATION (payment terms, invoicing)
ARTICLE IV: TERM AND TERMINATION
ARTICLE V: INTELLECTUAL PROPERTY
ARTICLE VI: CONFIDENTIALITY
ARTICLE VII: REPRESENTATIONS AND WARRANTIES
ARTICLE VIII: INDEMNIFICATION
ARTICLE IX: LIMITATION OF LIABILITY
ARTICLE X: MISCELLANEOUS
```

---

## Appendix B: Test Naming Conventions

| Pattern | Purpose |
|---------|---------|
| `demo_phase{N}_{feature}_basic` | Basic happy path |
| `demo_phase{N}_{feature}_edge_*` | Edge cases |
| `demo_phase{N}_{feature}_error_*` | Error handling |
| `demo_phase{N}_{feature}_perf_*` | Performance tests |

---

## Appendix C: Fixture Organization

```
layered-contracts/tests/fixtures/
├── contracts/
│   ├── nda_original.txt
│   ├── nda_revised_minor.txt
│   ├── nda_revised_major.txt
│   ├── service_original.txt
│   └── service_revised_structural.txt
├── alignment/
│   ├── exact_match_pair.json
│   ├── renumbered_pair.json
│   └── split_merge_pair.json
├── hints/
│   ├── force_match.json
│   ├── force_no_match.json
│   └── adjust_confidence.json
├── obligations/
│   ├── modal_shall_to_may.txt
│   └── modal_may_to_shall.txt
├── terms_temporal/
│   ├── term_narrowing.txt
│   └── temporal_extension.txt
├── structural/
│   ├── section_added.txt
│   └── section_removed.txt
└── e2e/
    ├── nda_complete_original.txt
    ├── nda_complete_revised.txt
    └── expected_output.json
```

---

## Appendix D: Learnings & Artifacts

> This section will be updated as implementation progresses.

### Discovered Issues

*(To be filled in during implementation)*

### Performance Observations

*(To be filled in during benchmarking)*

### Integration Patterns Validated

*(To be filled in during Phase 4 and Phase 9)*

### User Feedback

*(To be filled in during demo reviews)*

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2024-12-20 | Claude | Initial specification |

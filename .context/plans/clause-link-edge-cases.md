# Clause Link Edge Cases Resolution Plan

**Interesting artifacts and learnings must be written back to this document.**

**Status:** Gate 4 Complete ✅ (Cross-Reference Integration)
**Created:** 2026-01-01
**Package:** layered-clauses

## Overview

This plan addresses edge cases in `ClauseLinkResolver` that limit its practical usefulness for real contract analysis. The current implementation has a same-line restriction that silently drops cross-line relationships, and lacks support for complex linguistic structures common in legal prose.

**Current Limitations:** *(Gate 0 addressed the first item)*
- ~~Same-line restriction drops multi-line clause relationships~~ ✅ Fixed
- Exceptions only link to immediate predecessor
- Only "and" operator recognized (not "or", "but")
- No nested conditional support
- No cross-reference integration

**Goal:** Transform ClauseLinkResolver from a single-line demo into production-ready contract analysis infrastructure.

---

## Review Findings (2026-01-01)

Five parallel reviews identified critical issues:

### Dependency Graph Correction

**Original (INCORRECT):**
The plan had Gate 1 (Exception Scope) before Gate 2 (Operators), but exception scope detection requires knowing which clauses are coordinated first.

**Corrected dependency graph:**
```
Gate 0 (Foundation)
├── Gate 2 (Operators) → Gate 1 (Exception Scope) → Gate 3 (Nesting)
├── Gate 4 (Cross-Ref) [parallel after Gate 0]
├── Gate 5 (Semantics) [parallel after Gate 0, soft dep on Gate 2]
└── Gate 6 (Linguistics) [parallel after Gate 0]
```

**Critical path reduced:** 7 sequential gates → 4 gates (0→2→1→3)

### Architecture Concerns

| Gate | Rating | Issue |
|------|--------|-------|
| Gate 0 | 🟡 | Missing `SentenceBoundaryResolver` infrastructure |
| Gate 2 | 🟡 | Precedence parsing may need expression AST, not just keywords |
| Gate 3 | 🔴 | Nested clauses conflict with flat-clause architecture - consider descoping |
| Gate 5-6 | 🟡 | Full anaphora/modal scope requires syntactic parsing beyond current capabilities |

### Recommended Descoping

1. **Gate 3**: Accept flat clauses with depth=1; defer tree-based clause model
2. **Gate 5-6**: Frame as "flag for review" rather than "resolve automatically"

### Missing Infrastructure

- `SentenceBoundaryResolver` - Required before Gate 0 begins
- Link indexing by anchor span - Performance for large documents
- Multi-signal sentence detection - Legal prose uses semicolons, section headers

---

## Gate 0: Cross-Line Clause Relationships

**Priority:** Critical (Foundation)
**Dependency:** None
**Scope:** Remove same-line restriction with proper sentence-boundary heuristics

### Objectives

1. Detect condition-effect relationships that span multiple lines
2. Implement sentence-boundary detection to scope relationship search
3. Handle multi-line condition blocks (lists, paragraphs)
4. Maintain precision by adding confidence scores

### Tasks

- [ ] Add `SentenceBoundaryResolver` to detect sentence breaks (period + capital)
- [ ] Modify `detect_parent_child_links()` to search within sentence, not line
- [ ] Add `DocSpan` support for multi-line clause ranges
- [ ] Implement backward search from "then" to find preceding "if/when" within N sentences
- [ ] Add `Confidence` field to `ClauseLink` (High for same-line, Medium for cross-line)
- [ ] Create multi-line condition block detection for list patterns
- [ ] Add warning/log when relationships are dropped due to ambiguity
- [ ] Update `ClauseQueryAPI` to handle multi-line spans

### Acceptance Criteria

- Multi-line "When X,\n then Y" patterns produce Parent/Child links
- Same-line relationships maintain High confidence
- Cross-line relationships have Medium confidence with provenance
- Sentence boundary prevents false positives across unrelated sentences
- No silent dropping of relationships

### Verification

**Test Scenarios:**

1. `test_cross_line_simple_condition_effect` - "When X fails,\nthen Y terminates" produces 2 links
2. `test_cross_line_multi_sentence` - Two unrelated sentences do NOT link
3. `test_paragraph_condition_block` - Multi-line list "If: (a) X\n(b) Y\nthen Z" links correctly
4. `test_confidence_scoring` - Same-line = High, cross-line = Medium
5. `test_backward_search_limits` - "then" without preceding "if" in sentence produces no link
6. `test_sentence_boundary_detection` - Period + capital = new sentence

**Coverage Requirements:**
- Unit tests for sentence boundary detection
- Integration tests for cross-line patterns
- Snapshot tests for confidence values
- Property test: cross-line links ⊆ intra-sentence links

**Directory:** `layered-clauses/src/tests/cross_line/`

---

## Gate 1: Exception Scope Propagation

**Priority:** High
**Dependency:** Gate 0 (cross-line support enables multi-clause exception scope)
**Scope:** Exceptions apply to all coordinated clauses, not just immediate predecessor

### Objectives

1. "A and B, unless C" creates exception links from C to BOTH A and B
2. Handle chained exceptions: "A, unless B, except when C"
3. Implement exception scope boundaries

### Tasks

- [x] Modify `detect_exceptions()` to identify exception scope (all preceding coordinated clauses) ✅
- [x] Add transitive exception linking for coordinated clause chains ✅
- [ ] Implement `ExceptionScope` enum: `Immediate`, `AllCoordinated`, `Explicit(Vec<DocSpan>)` (deferred - using coordination graph instead)
- [x] Handle chained exceptions with precedence: "unless" before "except" before "notwithstanding" ✅
- [ ] Add `ClauseLink::exception_scope` field (deferred - scope derived from coordination graph)
- [x] Create exception boundary detection (comma, semicolon, period) ✅
- [x] Update `ClauseQueryAPI::exceptions()` to return transitive closures ✅

### Acceptance Criteria

- "A and B, unless C" produces 2 exception links (C→A, C→B)
- Chained exceptions maintain proper nesting
- Exception scope is explicit and queryable
- No false positives from unrelated clauses

### Verification

**Test Scenarios:**

1. `test_exception_applies_to_all_conjuncts` - "A and B, unless C" → 2 exception links
2. `test_exception_applies_to_chain` - "A and B and C, unless D" → 3 exception links
3. `test_chained_exceptions_precedence` - "A, unless B, except C" → proper nesting
4. `test_exception_boundary_comma` - Comma before exception limits scope
5. `test_exception_scope_query` - `exceptions(clause_a)` returns all applicable exceptions
6. `test_no_exception_cross_sentence` - Exception doesn't apply across sentence boundary

**Coverage Requirements:**
- Unit tests for each exception keyword
- Integration tests for chained patterns
- Snapshot tests for complex exception graphs

**Directory:** `layered-clauses/src/tests/exception_scope/`

---

## Gate 2: Extended Operators and Precedence

**Priority:** High
**Dependency:** Gate 1 (exception scope uses operator precedence)
**Scope:** Support "or", "but", "nor" with proper precedence

### Objectives

1. Recognize "or", "but", "nor" as coordination operators
2. Implement basic operator precedence: "and" binds tighter than "or"
3. Handle mixed operators: "A or B and C" parsed correctly
4. Distinguish coordination types semantically

### Tasks

- [x] Extend `ClauseKeyword` enum with `Or`, `But`, `Nor` variants ✅
- [x] Add `CoordinationType` enum: `Conjunction`, `Disjunction`, `Adversative`, `NegativeAlternative` ✅
- [x] Modify `ClauseKeywordResolver` to detect all coordination keywords ✅
- [x] Implement precedence parser for mixed operators ✅
- [x] Add `ClauseLink::coordination_type` field ✅
- [x] Add `ClauseLink::precedence_group` field ✅
- [x] Create `assign_precedence_groups()` for partitioning chains ✅
- [x] Add query helpers: `top_level_operator()`, `precedence_group_members()`, `precedence_groups()` ✅
- [ ] Handle Oxford comma ambiguity: "A, B, and C" vs "A, B and C" (deferred)
- [ ] Update visualization to distinguish "and" vs "or" links (deferred)

### Acceptance Criteria

- "A or B" produces Disjunction link
- "A but B" produces Adversative link
- "A or B and C" parses as "A or (B and C)" with correct precedence
- Coordination type is queryable and affects semantic interpretation

### Verification

**Test Scenarios:**

1. `test_or_coordination` - "A or B" produces Disjunction link
2. `test_but_coordination` - "A but B" produces Adversative link
3. `test_precedence_and_over_or` - "A or B and C" → A or (B and C)
4. `test_mixed_operators_chain` - "A and B or C and D" → (A and B) or (C and D)
5. `test_oxford_comma_handling` - Both comma patterns handled consistently
6. `test_coordination_type_query` - Can filter by coordination type

**Coverage Requirements:**
- Unit tests for each operator
- Property tests for precedence consistency
- Integration tests with exceptions

**Directory:** `layered-clauses/src/tests/operators/`

---

## Gate 3: Nested and Complex Structures

**Priority:** Medium
**Dependency:** Gates 0-2 (needs multi-line + operators + exception scope)
**Scope:** Handle nested conditionals, parentheticals, numbered lists

### Objectives

1. Support nested conditionals: "If A, then if B, then C"
2. Handle parenthetical clauses as modifiers
3. Parse numbered lists as coordinated clause groups
4. Build tree-structured clause relationships

### Tasks

- [ ] Implement recursive clause detection (clauses containing clauses) ⏸️ DEFERRED
- [ ] Add `ClauseSpan::depth` field for nesting level ⏸️ DEFERRED
- [ ] Create `ClauseRole::Parenthetical` for parenthetical modifiers ⏸️ DEFERRED
- [x] Add `ClauseRole::ListItem` with ordinal and parent reference ✅
- [x] Implement list detection: (a)/(b)/(c), (i)/(ii)/(iii), 1./2./3. patterns ✅
- [ ] Modify ClauseLink to support tree structure (parent chain) ⏸️ DEFERRED
- [ ] Add `ClauseQueryAPI::ancestors()` and `descendants()` for tree traversal ⏸️ DEFERRED
- [x] Handle list-final "and" vs clause-internal "and" ✅ (already works via coordination algorithm)

### Acceptance Criteria

- "If A, then if B, then C" produces tree: A → (B → C)
- Parenthetical "(who must...)" links to containing clause
- List items link to list-introducing clause
- Tree queries return correct ancestors/descendants

### Verification

**Test Scenarios:**

1. `test_nested_if_then` - Depth 2 nesting produces tree structure
2. `test_triple_nesting` - "If A, if B, if C, then D" → depth 3
3. `test_parenthetical_attachment` - "(who must...)" attaches to subject clause
4. `test_numbered_list_linking` - "(a), (b), (c)" all link to parent clause
5. `test_list_final_and` - "(a), (b), and (c)" not confused with clause "and"
6. `test_ancestor_query` - Deeply nested clause returns full parent chain
7. `test_descendant_query` - Parent returns all nested children

**Coverage Requirements:**
- Unit tests for each structure type
- Snapshot tests for tree shapes
- Integration tests combining nesting with exceptions

**Directory:** `layered-clauses/src/tests/nesting/`

---

## Gate 4: Cross-Reference Integration

**Priority:** Medium
**Dependency:** Gate 0 (multi-line needed for cross-section references)
**Scope:** Integrate SectionReference with clause structure

### Objectives

1. "Subject to Section 3.2" creates clause reference link
2. "As defined in Article IV" links definition to usage
3. "Notwithstanding Exhibit A" creates override relationship
4. Enable document-wide clause graph

### Tasks

- [x] Create integration layer between ClauseLinkResolver and SectionReferenceResolver ✅
- [x] Add `ClauseRole::CrossReference` with target section identifier ✅
- [ ] Implement `ReferencePurpose` enum: `SubjectTo`, `AsDefinedIn`, `Notwithstanding`, `PursuantTo` (deferred - uses existing `ReferencePurpose` from contracts)
- [ ] Link clause to target section's clause structure (deferred - requires DocumentStructureBuilder integration)
- [ ] Handle forward references (reference before section appears) (deferred - requires two-pass resolution)
- [x] Add `ClauseQueryAPI::referenced_sections()` and `referencing_clauses(section)` ✅
- [ ] Integrate with DocumentStructureBuilder for section resolution (deferred)

### Acceptance Criteria

- "Subject to Section 3.2" produces CrossReference link
- Reference purpose is explicit and queryable
- Forward references resolve after full document parse
- Document-wide clause graph is navigable

### Verification

**Test Scenarios:**

1. `test_subject_to_reference` - "Subject to Section X" creates link
2. `test_notwithstanding_reference` - Creates override-type link
3. `test_pursuant_to_reference` - Creates dependency-type link
4. `test_forward_reference_resolution` - Reference to later section resolves
5. `test_reference_purpose_query` - Can filter by reference purpose
6. `test_cross_section_graph` - Navigate from clause to referenced section's clauses

**Coverage Requirements:**
- Unit tests for each reference pattern
- Integration tests with full document structure
- End-to-end test with multi-section contract

**Directory:** `layered-clauses/src/tests/cross_reference/`

---

## Gate 5: Semantic Disambiguation

**Priority:** Medium
**Dependency:** Gates 0-2 (needs full operator support)
**Scope:** Handle modal-negation interaction, scope ambiguity, double negatives

### Objectives

1. Distinguish "shall not be required to" vs "shall be required not to"
2. Resolve scope ambiguity: "except widgets and gadgets"
3. Handle double negatives: "unless tenant does not pay"
4. Track polarity through clause structure

### Tasks

- [ ] Implement `ModalNegationInteraction` analyzer
- [ ] Add `ObligationType` enum: `Duty`, `Prohibition`, `Permission`, `Discretion`
- [ ] Create `PolarityTracker` for negation composition (even=positive, odd=negative)
- [ ] Implement scope ambiguity detection with N-best candidates
- [ ] Add `ClauseLink::obligation_type` derived from modal+negation
- [ ] Create `AmbiguityFlag` for user review of unclear scope
- [ ] Handle "unless not" → double negative resolution

### Acceptance Criteria

- "shall not be required to" classified as Discretion (no obligation)
- "shall be required not to" classified as Prohibition
- "unless X does not" resolves to "if X does"
- Ambiguous scope flagged for review

### Verification

**Test Scenarios:**

1. `test_modal_negation_required_not_to` - Prohibition classification
2. `test_modal_negation_not_required_to` - Discretion classification
3. `test_double_negative_resolution` - "does not fail to" → "does"
4. `test_unless_not_polarity` - "unless X does not" → "if X does"
5. `test_scope_ambiguity_detection` - "except A and B" flagged as ambiguous
6. `test_obligation_type_query` - Can filter clauses by obligation type

**Coverage Requirements:**
- Unit tests for modal-negation matrix
- Property tests for polarity composition
- Integration tests with full clause structure

**Directory:** `layered-clauses/src/tests/semantics/`

---

## Gate 6: Linguistic Phenomena

**Priority:** Low (enhances but not required for core functionality)
**Dependency:** Gate 0 (needs cross-line support)
**Scope:** Handle anaphora, relative clauses, cataphora

### Objectives

1. Integrate PronounChainResolver with clause structure
2. Handle relative clauses as clause modifiers
3. Support cataphoric (forward) references
4. Link pronouns to clause participants

### Tasks

- [ ] Create integration layer between PronounChainResolver and ClauseLinkResolver
- [ ] Add `ClauseParticipant` with pronoun resolution
- [ ] Implement `ClauseRole::Relative` for relative clause attachment
- [ ] Add bidirectional pronoun search for cataphora
- [ ] Handle "Before it expires" cataphoric patterns
- [ ] Add `ClauseQueryAPI::participants()` returning resolved entities
- [ ] Implement salience model for pronoun resolution

### Acceptance Criteria

- "If tenant fails, they..." links "they" to "tenant" in clause structure
- Relative clauses attach to modified noun phrase
- Cataphoric pronouns resolve to following referent
- Participants queryable from clause

### Verification

**Test Scenarios:**

1. `test_anaphora_cross_clause` - Pronoun in clause 2 resolves to clause 1 subject
2. `test_relative_clause_attachment` - "party who breaches" properly structured
3. `test_cataphora_before_it` - "Before it expires, the contract..." resolves
4. `test_participant_query` - Clause returns resolved participants
5. `test_salience_ordering` - Nearest appropriate antecedent preferred

**Coverage Requirements:**
- Unit tests for each linguistic pattern
- Integration tests with full document
- Snapshot tests for resolution chains

**Directory:** `layered-clauses/src/tests/linguistics/`

---

## Implementation Order

**Corrected dependency graph:**
```
Gate 0 (Foundation)
├── Gate 2 (Operators) → Gate 1 (Exception Scope) → Gate 3 (Nesting)
├── Gate 4 (Cross-Ref) [parallel]
├── Gate 5 (Semantics) [parallel]
└── Gate 6 (Linguistics) [parallel]
```

**Critical path:** Gate 0 → Gate 2 → Gate 1 → Gate 3 (4 gates)
**Parallel tracks:** Gates 4, 5, 6 can start after Gate 0 completes

**Rationale:**
1. Gate 0 unlocks multi-line support (foundation for all)
2. Gate 2 (Operators) must precede Gate 1 - exception scope requires knowing coordinated clauses
3. Gate 1 (Exception Scope) uses operator detection from Gate 2
4. Gate 3 adds structural complexity on solid foundation
5. Gates 4-6 only depend on Gate 0, enabling parallelization

**Natural shipping breakpoints:**
1. After Gate 0: Multi-line clause linking (core value)
2. After Gate 0+2: Operator support ("and", "or", "but")
3. After Gates 0-3: Full clause structure
4. After Gate 4: Document-wide cross-reference graph

---

## Success Metrics

1. **Coverage**: Multi-line contracts produce meaningful links (not empty)
2. **Precision**: False positive rate < 5% on test corpus
3. **Recall**: Detect > 80% of human-annotated clause relationships
4. **Confidence**: Every link has explicit confidence score
5. **Queryability**: All relationships navigable via ClauseQueryAPI

---

## Risks and Mitigations

| Risk | Severity | Mitigation | Contingency |
|------|----------|------------|-------------|
| Cross-line creates false positives | High | Multi-signal sentence boundary detection + confidence scores | Fallback to line-level only; add manual break annotation |
| Ambiguous operator precedence | Medium | Explicit precedence rules + N-best output + ambiguity flags | "Show all interpretations" mode; let user select |
| Performance on large documents | Medium | BFS limits (max 100 clauses/line, 1000 links/doc) + lazy evaluation | Emit `PerformanceBudgetExceeded` with partial results |
| Breaking existing tests | Low | All changes additive; existing tests remain | Gate-by-gate regression suite |
| Complexity explosion | Medium | Gate structure ensures incremental progress | Integration test checkpoints between gates |
| **WASM bundle size explosion** | High | Budget 500KB per gate; monitor after each gate | Gates 5-6 become compile-time optional features |
| **Sentence boundary fails on legal prose** | Critical | Multi-signal detection: period+capital, section headers, list patterns | `BoundaryConfidence` enum with fallback |
| **ClauseLink struct changes break WASM** | High | `#[serde(default)]` on new fields; version WASM API | Maintain deprecated endpoints for 2 releases |
| **Gate 3 architecture mismatch** | High | Flat clauses with depth=1 initially; tree model deferred | Accept limitation; document in API |
| **No graceful degradation** | Medium | `UnrecognizedPattern` warnings; provenance on all links | "Explain this link" UI pattern |

---

## Test Coverage Gaps (from review)

### Missing Integration Tests

Add `layered-clauses/src/tests/integration/` directory with:
- `integration_gate_0_1_cross_line_exception` - Exception spanning lines
- `integration_gate_0_2_cross_line_operators` - Operators spanning lines
- `integration_gate_1_2_exception_with_disjunction` - "A or B, unless C"
- `integration_gate_2_3_nested_operators` - "If (A and B) or C, then D"
- `integration_full_pipeline` - Realistic multi-section contract

### High-Priority Missing Test Scenarios

| Gate | Test | Why Critical |
|------|------|--------------|
| 0 | `test_sentence_boundary_abbreviation` | "Dr.", "Inc.", "Corp." are common |
| 0 | `test_quoted_period_not_boundary` | Quoted periods shouldn't split |
| 1 | `test_exception_does_not_cross_semicolon` | Common in legal lists |
| 2 | `test_or_in_word_not_operator` | "Oregon" contains "or" |
| 3 | `test_max_nesting_depth` | Performance/stack overflow prevention |
| 4 | `test_reference_to_nonexistent_section` | Error handling |

### Property Tests to Add

```
forall (sentence: ValidSentence) =>
  cross_line_links(sentence) ⊆ intra_sentence_links(sentence)

forall (expr: OperatorExpr) =>
  parse(to_string(parse(expr))) == parse(expr)  // Round-trip

forall (negations: [Negation]) =>
  polarity(negations) == if even(|negations|) then Positive else Negative
```

---

## Learnings

### 2026-01-01: Gate 0 Exploration Complete

**Same-Line Restriction Analysis:**
- Located at 3 enforcement points in `clause_link_resolver.rs`:
  - Line 119: Coordination detection
  - Line 252: Exception detection
  - Line 324: Condition→Effect linking
- Pattern: `if current.span.start.line != next.span.start.line { continue; }`
- Comments indicate this is deliberate false-positive prevention

**Existing Infrastructure (90% complete for Gate 0):**

| Component | Location | Status |
|-----------|----------|--------|
| `TextTag::PUNC` | `ll_line.rs:20-38` | ✅ Already classifies `.`, `?`, `!` |
| `DocSpan` multi-line | `document.rs:38-105` | ✅ `is_single_line()`, `line_count()`, `contains()`, `overlaps()` |
| Punctuation splitting | `clauses.rs:23-26` | ✅ `split_by(&x::attr_eq(&TextTag::PUNC))` |
| DocumentResolver | `document.rs:156-165` | ✅ Document-level resolver trait |
| Cross-line iteration | `clause_link_resolver.rs:62-82` | ✅ `doc.lines_enumerated()` pattern |

**Gaps to Fill:**

1. **SentenceBoundaryResolver** - Mark sentence-ending punctuation
   - Needs abbreviation handling: "Dr.", "Inc.", "e.g.", "U.S."
   - Needs lookahead: "said Dr. Smith" vs "said goodbye."
   - Needs quoted sentence handling

2. **Cross-line keyword search** - `has_coordination_keyword_between()` only searches single line
   - Need `has_coordination_keyword_between_spanning(doc, span1, span2)`

3. **Confidence scoring** - `ClauseLink` struct needs `confidence: f32` field

**Architecture Decision: Two-Level Sentence Detection**

*Level 1 (Line-level):* `SentenceBoundaryResolver` marks potential sentence ends
*Level 2 (Document-level):* Assembly logic groups clauses within sentence boundaries

This matches the existing ClauseResolver → ClauseLinkResolver pattern.

**Implementation Priority:**

1. Start with `SentenceBoundaryResolver` (line-level)
2. Modify `ClauseLinkResolver` to use sentence boundaries instead of line boundaries
3. Add confidence scoring
4. Add tests for cross-line patterns

---

### 2026-01-01: Gate 0 Implementation Complete ✅

**Summary:** Cross-line clause linking is now functional.

**Files Created/Modified:**
- `src/sentence_boundary.rs` - New SentenceBoundaryResolver (114 lines)
- `src/clause_link_resolver.rs` - Added `in_same_sentence()`, `has_coordination_keyword_between_spanning()`, `has_exception_keyword_between_spanning()` (~200 lines added)
- `src/clause_link.rs` - Added `LinkConfidence` enum and builder methods
- `src/clause_query.rs` - Added `high_confidence_links()`, `links_with_confidence()` methods
- `src/clauses.rs` - Fixed single-clause case to handle `Then` keyword
- `src/tests/cross_line.rs` - 11 cross-line test scenarios

**Test Results:**
- 73 tests pass, 7 ignored (deferred to future gates)
- Core cross-line patterns work:
  - ✅ Cross-line condition→effect (with explicit "then")
  - ✅ Cross-line coordination ("and" spanning lines)
  - ✅ Cross-line exceptions ("unless" spanning lines)
  - ✅ Sentence boundary prevents false positives
  - ✅ Confidence scoring (High=same-line, Medium=cross-line)

**Known Limitations (documented in tests):**
- Implicit trailing effect requires explicit "then" keyword for cross-line patterns
- `ClauseResolver` is line-level; can't track condition state across lines
- These will be addressed in Gate 2 (DocumentResolver for clauses)

**Key Implementation Insights:**

1. **`match_first_forwards` doesn't skip whitespace** - Must use `after()` + `find_by()` pattern to search across whitespace tokens

2. **LLLineDisplay uses Debug, not Display** - Custom Debug impls needed for attribute visualization

3. **Spanning helpers follow consistent pattern:**
   ```rust
   fn has_X_keyword_between_spanning(doc, span1, span2) -> bool {
       if same_line { use existing single-line logic }
       else { iterate lines, check token ranges, respect boundaries }
   }
   ```

4. **Max line distance heuristic (10 lines)** prevents false positives on large documents

5. **Graceful degradation** - Works with or without `SentenceBoundaryResolver` run first

**Gate 0 Acceptance Criteria Status:**
- ✅ Multi-line "When X,\n then Y" patterns produce Parent/Child links
- ✅ Same-line relationships maintain High confidence
- ✅ Cross-line relationships have Medium confidence with provenance
- ✅ Sentence boundary prevents false positives across unrelated sentences
- ✅ No silent dropping of relationships (documented limitations)

---

### 2026-01-01: Gate 2 Exploration Complete

**Summary:** Comprehensive analysis of codebase structure for extended operators.

**Key Files Identified:**

| Component | File | Lines | Status |
|-----------|------|-------|--------|
| ClauseKeyword enum | `clause_keyword.rs` | 3-13 | Needs Or, But, Nor variants |
| ClauseKeywordResolver | `clause_keyword.rs` | 15-73 | Needs or, but, nor vector fields |
| ClauseResolver | `clauses.rs` | 60-80 | Needs match arms for new keywords |
| ClauseLink struct | `clause_link_resolver.rs` | 65-73 | Needs coordination_type field |
| ClauseRole enum | `../layered-nlp-document/src/span_link.rs` | 37-48 | `Conjunct` role already suitable |
| detect_coordination() | `clause_link_resolver.rs` | 166-220 | Chain topology works for all operators |
| has_coordination_keyword_between_spanning() | `clause_link_resolver.rs` | 279 | Needs to match all coordination keywords |

**Architectural Insights:**

1. **Chain Topology Already Works:** Current `A→B→C` chain pattern (not star) scales to Or/But/Nor without modification.

2. **Operator Semantic Distinctions:**
   - `And/Or`: Positive coordination (additive/alternative)
   - `But`: Adversative (contrast/qualification)
   - `Nor`: Negative coordination (negative alternative)

3. **Integration Pattern:** New operators follow same path:
   - ClauseKeywordResolver detects keyword → assigns ClauseKeyword attribute
   - ClauseResolver assigns clause types based on context
   - ClauseLinkResolver detects coordination via `has_coordination_keyword_between_spanning()`

4. **Test Patterns Established:**
   - Snapshot tests (`insta`) in `tests/clauses.rs` for visual regression
   - Query API tests in `tests/integration.rs` using `ClauseQueryAPI`
   - Cross-line tests in `tests/cross_line.rs` for multi-line scenarios

**Implementation Order:**

1. Add `Or`, `But`, `Nor` to ClauseKeyword enum
2. Add CoordinationType enum (`Conjunction`, `Disjunction`, `Adversative`)
3. Add vector fields to ClauseKeywordResolver
4. Update ClauseResolver match arms
5. Add coordination_type field to ClauseLink
6. Update `has_coordination_keyword_between_spanning()` to match all operators
7. Create tests for new operators
8. Handle operator precedence (And binds tighter than Or)

---

### 2026-01-01: Gate 2 Core Implementation Complete

**Summary:** Extended operators (Or, But, Nor) and CoordinationType now functional.

**Test Results:**
- 79 tests pass (7 ignored - deferred to future gates)
- 9 doc tests pass
- 6 new coordination type tests added and passing

**Files Modified:**

| File | Changes |
|------|---------|
| `clause_keyword.rs` | Added `Or`, `But`, `Nor` enum variants; added `or`, `but`, `nor` vector fields to resolver |
| `clause_link_resolver.rs` | Added `CoordinationType` enum; added `coordination_type` field to `ClauseLink`; added `detect_coordination_type_between_spanning()` |
| `clause_link.rs` | Added `conjunct_link_typed()` helper method |
| `clauses.rs` | Added match arms for `Or`, `But`, `Nor` in ClauseResolver |
| `lib.rs` | Exported `CoordinationType` |
| `tests/*.rs` | Updated ClauseKeywordResolver::new() calls to 6 parameters |

**Gate 2 Acceptance Criteria Progress:**
- ✅ "A or B" produces Disjunction link
- ✅ "A but B" produces Adversative link
- ⏳ "A or B and C" precedence parsing - NOT YET IMPLEMENTED
- ✅ Coordination type is queryable

**Remaining for Gate 2:**
1. Operator precedence parser (And binds tighter than Or)
2. Oxford comma disambiguation
3. CoordinationGroup for complex expressions

**Architectural Decision: Incremental Precedence**

The current implementation treats all operators equally (chain topology). For precedence, we have two options:

1. **Expression AST approach** - Parse "A or B and C" into `Or(A, And(B, C))` tree structure
2. **Grouping hint approach** - Mark coordination groups with precedence level

Recommend Option 2 for simplicity - add `precedence_group: u8` to ClauseLink rather than building full AST.

---

### 2026-01-01: Gate 2 Precedence Implementation Complete ✅

**Summary:** Operator precedence parsing now functional using group-based partitioning.

**Test Results:**
- 83 tests pass (7 ignored - deferred to future gates)
- 12 doc tests pass (3 new for query helpers)
- 4 new precedence tests added and passing

**Implementation Approach:** Group-based partitioning (not full AST)

The `assign_precedence_groups()` function partitions the chain by incrementing group IDs at each operator type change:
- "A or B and C" → Group 0 (OR), Group 1 (AND)
- "A and B or C and D" → Group 0 (AND), Group 1 (OR), Group 2 (AND)

**Precedence Levels:**
```rust
PRECEDENCE_AND: u8 = 2  // Tightest binding
PRECEDENCE_OR: u8 = 1
PRECEDENCE_BUT: u8 = 0  // Loosest binding
```

**New Query Helpers in ClauseQueryAPI:**
- `top_level_operator()` → Returns operator type of lowest-precedence group
- `precedence_group_members(span)` → Returns all clauses in same group as span
- `precedence_groups()` → Returns all distinct group IDs

**Gate 2 Acceptance Criteria - Final Status:**
- ✅ "A or B" produces Disjunction link
- ✅ "A but B" produces Adversative link
- ✅ "A or B and C" correctly groups with precedence (AND binds tighter)
- ✅ Coordination type is queryable
- ✅ Precedence groups enable structured queries

**Deferred to future work:**
- Oxford comma disambiguation (low priority)
- Visualization updates (nice-to-have)

---

### 2026-01-01: Gate 2 Review & Bug Fixes ✅

**Review identified critical bug:** `precedence_group` was storing sequential partition IDs (0,1,2...) instead of actual precedence values (AND=2, OR=1, BUT=0).

**Bugs fixed:**

| Issue | Fix |
|-------|-----|
| `precedence_group` stored partition ID not precedence | Now stores actual `PRECEDENCE_*` value |
| `top_level_operator()` used `max()` | Fixed to use `min()` for lowest precedence |
| 3 `top_level_operator` tests commented out | Enabled and passing |

**Test Results (post-fix):**
- 86 unit tests pass (7 ignored)
- 11 doc tests pass

**Known limitations (documented, not blocking):**
- "provided" in exception list may cause false positives (needs "provided that")
- "however" as BUT keyword may match parenthetical usage
- `precedence_group_members()` includes query span (inconsistent with `conjuncts()`)

---

### 2026-01-02: Gate 1 Implementation Complete ✅

**Summary:** Exception scope propagation now functional.

**Test Results:**
- 95 unit tests pass (7 ignored - deferred to future gates)
- 11 doc tests pass
- All Gate 1 acceptance criteria met

**Files Modified:**

| File | Changes |
|------|---------|
| `clause_link_resolver.rs` | Added `detect_exceptions_with_api()`, chained exception propagation, `has_semicolon_between()` helper, removed dead `detect_exceptions()` (87 lines) |
| `clause_query.rs` | Updated `exceptions()` to use BFS for transitive closure |
| `tests/exception_scope.rs` | 4 new exception scope tests |
| `tests/integration.rs` | 7 Gate 1 integration tests |

**Gate 1 Acceptance Criteria - Final Status:**
- ✅ "A and B, unless C" produces 2 exception links (C→A, C→B)
- ✅ Chained exceptions maintain proper nesting ("A, unless B, except C" → C→B, C→A transitive)
- ✅ Exception scope is explicit and queryable via `exceptions()` BFS
- ✅ Semicolon boundary prevents exception propagation
- ✅ No false positives from unrelated clauses

**Implementation Insights:**

1. **Exception scope uses coordination graph:** `detect_exceptions_with_api()` queries `api.conjuncts()` to find all coordinated clauses, then creates exception links to each.

2. **Chained exceptions via single-pass transitivity:** After creating direct exception links, the algorithm scans for chains (C→B where B→A exists) and creates transitive links (C→A).

3. **Semicolon boundary detection:**
   - Same-line: `has_semicolon_between()` iterates tokens between spans looking for `;`
   - Cross-line: Conservatively returns `true` (treats line break as potential boundary)

4. **`exceptions()` mirrors `conjuncts()` pattern:** BFS traversal with visited set to find all exceptions transitively.

**Review Findings (not blocking, documented for future work):**

| Issue | Severity | Notes |
|-------|----------|-------|
| Transitive propagation only goes 1 level | Important | For very deep chains (>3 levels), may miss D→A link. Fixpoint loop could address. |
| `exceptions()` returns exception tree | Minor | Semantically returns "exceptions to exceptions" - may need separate `direct_exceptions()` |
| Transitive confidence not degraded | Minor | C→A transitive has same confidence as C→B direct |
| O(n²) visited set | Minor | `Vec::contains` is fine for typical document sizes |

**Architecture Note:**

The pattern `detect_X_with_api()` (passing a temporary `ClauseQueryAPI`) is reusable:
1. Resolve coordination links first (creates the coordination graph)
2. Pass graph to exception detection via temporary API
3. Exception detection queries `conjuncts()` to find propagation targets

This ensures exception scope correctly respects operator precedence established in Gate 2.

---

### 2026-01-02: Gate 3 Exploration Complete

**Summary:** Comprehensive analysis of Gate 3 requirements with recommended descoping.

**Key Findings from Exploration:**

| Component | Location | Status |
|-----------|----------|--------|
| ClauseRole enum | `layered-nlp-document/src/span_link.rs:37-48` | Needs ListItem, ListContainer variants |
| TextTag (PUNC, NATN, WORD) | `layered-nlp/src/ll_line.rs:24-38` | ✅ Already supports list marker tokens |
| Pattern matching x::seq() | `layered-nlp/src/ll_line/x/` | ✅ Multi-token span detection works |
| "A, B, and C" coordination | `clause_link_resolver.rs:192-259` | ✅ Already implemented! |
| Exception scope propagation | `clause_link_resolver.rs:665-683` | ✅ Already propagates to all conjuncts |

**Critical Insight: List-Final "And" Already Works**

The two-pass coordination algorithm already handles "A, B, and C" patterns:
1. Pass 1: Detect if ANY coordination keyword exists in sequence
2. Pass 2: Commas create implicit links when coordination keyword exists anywhere

This means task "Handle list-final 'and' vs clause-internal 'and'" is already complete!

**Review Recommendation Accepted: Flat Clauses Only**

Per the review finding:
> **Gate 3**: Accept flat clauses with depth=1; defer tree-based clause model

**Scoped Gate 3 Implementation:**

1. ✅ **List-final "and"** - Already implemented in coordination algorithm
2. 🔧 **ListMarkerResolver** - NEW: Detect `(a)`, `(i)`, `1.` patterns as line-level resolver
3. 🔧 **ClauseRole::ListItem** - Add variant with ordinal tracking
4. 🔧 **ListItem linking** - Link list items to their containing clause
5. ⏸️ **Recursive nesting (depth > 1)** - DEFERRED
6. ⏸️ **ancestors()/descendants()** - DEFERRED (only useful with depth > 1)
7. ⏸️ **Parenthetical attachment** - DEFERRED

**Implementation Architecture:**

```
ListMarkerResolver (line-level)
  ↓
ListMarker attribute on tokens: (a), (i), 1.
  ↓
ClauseLinkResolver (document-level)
  ↓
ListItem links: marker → containing clause
```

**ListMarker Types to Detect:**

| Pattern | Example | Detection |
|---------|---------|-----------|
| Parenthesized letter | `(a)`, `(b)` | `seq((PUNC '('), WORD, (PUNC ')'))` |
| Parenthesized roman | `(i)`, `(ii)` | Same + validate content |
| Parenthesized digit | `(1)`, `(2)` | `seq((PUNC '('), NATN, (PUNC ')'))` |
| Numbered period | `1.`, `2.` | `seq(NATN, (PUNC '.'))` |

---

### 2026-01-02: Gate 3 Implementation Complete ✅

**Summary:** List structure support implemented with scoped approach (flat lists only).

**Test Results:**
- 130 unit tests pass (7 ignored - deferred to future gates)
- 11 doc tests pass
- 18 new tests added for list functionality

**Files Created:**

| File | Lines | Purpose |
|------|-------|---------|
| `src/list_marker.rs` | ~200 | `ListMarker` enum + `ListMarkerResolver` |
| `src/tests/list_marker.rs` | ~100 | List marker detection tests |

**Files Modified:**

| File | Changes |
|------|---------|
| `layered-nlp-document/src/span_link.rs` | Added `ListItem`, `ListContainer` to ClauseRole |
| `layered-nlp-demo-wasm/src/lib.rs` | Match arms for new ClauseRole variants |
| `src/clause_link_resolver.rs` | `detect_list_relationships()`, ~100 lines added |
| `src/clause_link.rs` | `list_item_link()`, `list_container_link()` builders |
| `src/clause_query.rs` | `list_container()`, `list_items()`, `is_list_item()`, `is_list_container()` |
| `src/tests/integration.rs` | 6 new list integration tests |
| `src/lib.rs` | Exported `ListMarker`, `ListMarkerResolver` |

**Gate 3 Acceptance Criteria - Final Status:**

| Criterion | Status | Notes |
|-----------|--------|-------|
| List marker detection | ✅ | `(a)`, `(i)`, `1.` patterns |
| List item → container linking | ✅ | Bidirectional `ListItem`/`ListContainer` links |
| Marker type grouping | ✅ | Same marker type = same list group |
| Query API for lists | ✅ | `list_container()`, `list_items()`, etc. |
| Nested lists (depth > 1) | ⏸️ | DEFERRED per review recommendation |
| Parenthetical attachment | ⏸️ | DEFERRED |
| ancestors()/descendants() | ⏸️ | DEFERRED (only needed for deep nesting) |

**Implementation Insights:**

1. **ListMarker priority order:** Roman numerals checked before single letters. Without this, `(i)` becomes `ParenthesizedLetter` instead of `ParenthesizedRoman`.

2. **Marker grouping algorithm:** Sequential list items with same marker type form a group. The clause immediately before the first item becomes the container.

3. **Bidirectional links:** Both `ListItem` (item → container) and `ListContainer` (container → item) links created, matching the Parent/Child pattern.

4. **Double-dereference pattern:** When cloning from `LLLineFind<&ListMarker>`, use `(*find.attr()).clone()` to properly clone the value.

5. **Integration with existing detection:** List detection runs as Gate 4 after coordination and exception detection, using the established pattern.

**Deferred Items (documented for future work):**

| Item | Reason | Future Gate |
|------|--------|-------------|
| Nested lists (depth > 1) | Architecture mismatch with flat clause model | Gate 3.1 |
| Parenthetical clauses | Requires syntactic parsing | Gate 3.2 |
| Tree traversal (ancestors/descendants) | Only useful with deep nesting | Gate 3.1 |

---

### 2026-01-02: Gate 4 Implementation Complete ✅

**Summary:** Cross-reference integration now functional. Section references within clauses create `CrossReference` links.

**Test Results:**
- 157 unit tests pass (7 ignored - deferred to future gates)
- 14 doc tests pass (3 new for cross-reference query methods)
- All typecheck and test gates pass

**Files Modified:**

| File | Changes |
|------|---------|
| `layered-nlp-document/src/span_link.rs` | Added `ClauseRole::CrossReference` variant |
| `layered-nlp-demo-wasm/src/lib.rs` | Added match arm for `CrossReference` → "CrossRef" |
| `layered-clauses/src/clause_link_resolver.rs` | Added `detect_cross_references()` (~75 lines), import for `SectionReference` |
| `layered-clauses/src/clause_link.rs` | Added `cross_reference_link()` builder method |
| `layered-clauses/src/clause_query.rs` | Added `referenced_sections()`, `referencing_clauses()`, `has_cross_references()` (~120 lines) |
| `layered-clauses/src/tests/cross_reference.rs` | 4 new tests + integration tests (~200 lines) |
| `layered-clauses/Cargo.toml` | Added `layered-contracts` dependency |

**Gate 4 Acceptance Criteria - Final Status:**

| Criterion | Status | Notes |
|-----------|--------|-------|
| "Subject to Section 3.2" produces CrossReference link | ✅ | Works for any `SectionReference` within clause span |
| Reference purpose is explicit and queryable | ⏸️ | Deferred - reuses `ReferencePurpose` from contracts |
| Forward references resolve | ⏸️ | Deferred - requires DocumentStructureBuilder |
| Document-wide clause graph navigable | ✅ | Query API enables traversal |

**Implementation Insights:**

1. **Dependency on layered-contracts:** Added `layered-contracts` as dependency to access `SectionReference` type. This creates a coupling but is the cleanest way to detect section references.

2. **Span containment logic:** The key challenge is determining if a `SectionReference` span is "within" a clause span:
   ```rust
   // Single-line clause: ref must be within [clause_start, clause_end]
   // Multi-line first line: ref_start >= clause_start
   // Multi-line last line: ref_end <= clause_end
   // Multi-line middle: always included
   ```

3. **Confidence scoring:** Same-line references get `High` confidence; cross-line get `Medium`.

4. **Snapshot test proved implementation works:** The snapshot showed `CrossReference` links being created, validating the core algorithm before fixing individual assertion tests.

5. **Query API follows existing patterns:** `referenced_sections()` and `referencing_clauses()` mirror the bidirectional pattern of `list_items()` / `list_container()`.

**Deferred Items (documented for future work):**

| Item | Reason | Effort |
|------|--------|--------|
| `ReferencePurpose` integration | Existing enum in contracts crate sufficient | Low |
| Forward reference resolution | Requires two-pass document resolution | Medium |
| Cross-section graph navigation | Needs DocumentStructureBuilder integration | High |

---

### 2026-01-02: Gates 4, 5, 6 Exploration Complete

**Summary:** Parallel exploration of remaining gates to assess feasibility and prioritize.

#### Gate 4 (Cross-Reference Integration) - SELECTED FOR IMPLEMENTATION

**Infrastructure Readiness: 95%**

| Component | Location | Status |
|-----------|----------|--------|
| `SectionReference` | `layered-contracts/src/section_reference.rs` | ✅ Complete - detects section references |
| `SectionReferenceLinker` | `layered-contracts/src/section_reference_linker.rs` | ✅ Complete - resolves references |
| `ReferencePurpose` enum | `section_reference.rs` | ✅ `Condition`, `Definition`, `Override`, `Conformity`, `Exception`, `Authority` |
| `ClauseLinkResolver` document-level | `clause_link_resolver.rs` | ✅ Can access both Clause and SectionReference attributes |
| `ClauseQueryAPI` traversal | `clause_query.rs` | ✅ BFS pattern reusable |

**Implementation Plan:**
1. Add `ClauseRole::CrossReference` variant
2. Implement `detect_cross_references()` in ClauseLinkResolver (~50-75 lines)
3. Add `referenced_sections()`, `referencing_clauses()` to ClauseQueryAPI
4. Write tests following established patterns

**Estimated Complexity: MEDIUM (250-300 LOC, 1-2 weeks)**

---

#### Gate 5 (Semantic Disambiguation) - DEFERRED

**Infrastructure Readiness: 70%**

| Component | Status | Notes |
|-----------|--------|-------|
| Modal detection | ✅ | `ContractKeyword::Shall/May/ShallNot` exists |
| `ObligationType` enum | ✅ | `Duty`, `Permission`, `Prohibition` in obligation.rs |
| `ProhibitionResolver` | ✅ | Detects "shall not" patterns |
| Polarity tracking | ❌ | No infrastructure for negation composition |
| Scope ambiguity | ❌ | No NP chunking for "except A and B" |
| Double negatives | ❌ | No resolver for "unless tenant does not" |

**Blockers:**
- Scope ambiguity detection requires NP (noun phrase) chunker
- Double-negative resolution requires polarity tracker
- Full modal-negation interaction matrix needs ~12-17 days

**Recommendation:** Defer until Gate 4 complete; consider descoping to "flag for review" approach

---

#### Gate 6 (Linguistic Phenomena) - DEFERRED

**Infrastructure Readiness: 80%**

| Component | Status | Notes |
|-----------|--------|-------|
| `PronounResolver` | ✅ | Full implementation in layered-contracts |
| `PronounChainResolver` | ✅ | Builds coreference chains |
| Deixis framework | ✅ | `layered-deixis` crate has `DeicticReference` types |
| Clause participants | ❌ | No `ClauseParticipant` struct exists |
| Cataphora support | ❌ | Current resolver only searches backward |
| Relative clause attachment | ❌ | Design decision needed |

**Design Decisions Needed:**
1. What is a `ClauseParticipant`? (just pronouns or all NPs?)
2. How to handle cataphora forward-search?
3. How to distinguish relative clauses from conditional clauses?

**Recommendation:** Good second choice after Gate 4; ~9-13 days estimated

---

#### Priority Decision

**Selected: Gate 4 (Cross-Reference Integration)**

**Rationale:**
1. Highest infrastructure readiness (95%)
2. Most contained scope - reuses existing SectionReference
3. Clear implementation path following `detect_*` pattern
4. Immediately useful for document-wide graph navigation
5. Low technical risk

---

*This plan transforms ClauseLinkResolver from a same-line demo into production-ready contract analysis infrastructure.*

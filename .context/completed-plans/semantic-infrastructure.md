# Semantic Infrastructure

> Learnings relevant to future gates should be written back to respective gates, so future collaborators can benefit.

**Status:** Complete

**Goal:** Build foundational semantic infrastructure to enable clause-link-edge-cases Gates 5-6 with "flag for review" approach rather than automatic resolution.

## Motivation

Gates 5-6 of clause-link-edge-cases require:
- Polarity tracking for double-negative detection
- Modal-negation interaction classification
- Cataphora (forward pronoun) resolution
- Clause participant linking

Current infrastructure gaps prevent progress. This plan builds incremental, testable components.

## Scope

**In Scope:**
- Polarity tracking with negation composition
- Modal-negation interaction flagging (add Discretion to ObligationType)
- Cataphora support in pronoun resolution
- ClauseParticipant type for pronoun-clause linking
- Relative clause attachment (ClauseRole::Relative)

**Out of Scope (Future Work):**
- NP (noun phrase) chunking - requires syntactic parser
- Full scope ambiguity resolution - needs NP boundaries
- Automatic double-negative resolution - flag only, user reviews

**Dependencies:**
- ScopeDimension::Negation exists (scope_operator.rs)
- NegationDetector exists (scope_operators.rs)
- PronounResolver/PronounChainResolver exist (pronoun.rs, pronoun_chain.rs)
- DeicticSubcategory::DiscourseCataphoric defined (layered-deixis)

**Unblocks:**
- clause-link-edge-cases.md Gate 5 (Semantic Disambiguation)
- clause-link-edge-cases.md Gate 6 (Linguistic Phenomena)

## Gate Dependencies

```
Polarity Tracking ──┐
                    ├──► Modal-Negation Classification
                    │
Cataphora Resolution ──► Clause Participant Integration
                                        │
                    Relative Clause ◄───┘
```

Polarity feeds into modal-negation. Cataphora feeds into participants.

---

## Completed Gates

### Gate 1: Polarity Tracking ✓

**Goal:** Track polarity (positive/negative) through clause structure for double-negative detection.

**Deliverables:**

| File | Content | Status |
|------|---------|--------|
| `layered-contracts/src/polarity.rs` | `PolarityTracker`, `Polarity` enum, `PolarityContext`, `PolarityResolver` | ✓ Complete |
| `layered-contracts/src/lib.rs` | Export polarity module | ✓ Complete |

**Implementation Summary:**
- `Polarity` enum: `Positive | Negative | Ambiguous`
- `PolarityContext` struct: span, negation_count, confidence, needs_review flag
- `DoubleNegativePattern` enum for patterns like "unless not", "cannot fail to", etc.
- `PolarityTracker` with `add_negation()`, `add_double_negative()`, `polarity()` methods
- `PolarityResolver` with `detect_double_negative_patterns()` and `is_negation_word()`

**Test Coverage:** 11 unit tests passing, covering:
- "shall not" → Negative, count=1
- "shall not fail to" → Positive (double negative), count=2
- "unless tenant does not pay" → flags Ambiguous
- Various double-negative pattern detection

**Learnings:**
- The `needs_review` flag approach works well for ambiguous cases - keeps the system conservative while enabling human oversight
- Confidence scoring based on negation count is straightforward: single negations are high confidence, double negatives reduce confidence

---

### Gate 2: Modal-Negation Classification ✓

**Goal:** Classify modal+negation combinations into obligation types with ambiguity flagging.

**Deliverables:**

| File | Content | Status |
|------|---------|--------|
| `layered-contracts/src/modal_negation.rs` | `ModalNegationClassifier`, `ModalObligationType`, `DiscretionPattern`, `ModalNegationClassification` | ✓ Complete |
| `layered-contracts/src/contract_keyword.rs` | Added `Must`, `Can`, `Will`, `MustNot`, `Cannot`, `WillNot` variants | ✓ Complete |
| `layered-contracts/src/lib.rs` | Export modal_negation module | ✓ Complete |

**Implementation Summary:**
- `ModalObligationType` enum: `Duty | Permission | Prohibition | Discretion`
- `DiscretionPattern` enum: patterns like "shall not be required to", "need not", "may decline to"
- `ModalNegationClassification` struct: span, obligation_type, modal (String), polarity, ambiguity flags
- `ModalNegationClassifier` with `classify()` and `classify_with_discretion()` methods
- Extended `ContractKeyword` with `Must`, `Can`, `Will`, `MustNot`, `Cannot`, `WillNot` variants

**Classification matrix (implemented):**

| Modal | Negation | Result |
|-------|----------|--------|
| shall | none | Duty |
| shall | not | Prohibition |
| shall | not be required | Discretion |
| may | none | Permission |
| may | not | Prohibition |
| need | not | Discretion |

**Test Coverage:** 13 unit tests passing, covering:
- "shall pay" → Duty
- "shall not pay" → Prohibition
- "shall not be required to pay" → Discretion
- "need not pay" → Discretion
- "may decline to" → Discretion
- Ambiguous cases flagged with `needs_review: true`

**Learnings:**
- `ContractKeyword` does not implement serde traits, so `ModalNegationClassification.modal` stores the `String` representation rather than the enum variant directly
- Discretion patterns are flagged for review rather than auto-resolved, following the established "flag for review" approach from Gate 1
- The `classify_with_discretion()` method enables explicit discretion pattern detection separate from standard modal classification

---

### Gate 3: Cataphora Resolution ✓

**Goal:** Enable bidirectional pronoun resolution via document-level two-pass approach.

**Deliverables:**

| File | Content | Status |
|------|---------|--------|
| `layered-contracts/src/pronoun.rs` | `CataphoraDirection` enum, `CataphoraCandidate` struct, `DocumentPronounResolver`, `DocumentPronounReference` | ✓ Complete |

**Implementation Summary:**
- `CataphoraDirection` enum: `Anaphoric | Cataphoric | Ambiguous` with `is_forward()`/`is_backward()` methods
- `CataphoraCandidate` struct: extends AntecedentCandidate with direction, salience, needs_review fields
- `DocumentPronounResolver` for two-pass document-level resolution with configurable scoring
- `DocumentPronounReference` struct for resolution results with best_anaphoric/best_cataphoric tracking
- Helper methods: `is_cataphoric_trigger()`, `calculate_salience()`, `is_direction_ambiguous()`
- Updated `PronounType` to add serde derives
- Backward compatibility via `from_antecedent()` and `from_pronoun_reference()` converters

**Test Coverage:** 10 unit tests passing, covering:
- "Before it expires, the Contract..." → "it" resolves to "Contract" (cataphoric)
- Backward anaphora resolution still works
- Cataphora candidates flagged with `is_forward: true`
- Salience scoring based on mention frequency
- Direction ambiguity detection

**Learnings:**
- Cataphoric references are always flagged for review (uncommon pattern requiring human verification)
- Salience scoring based on mention frequency improves candidate ranking
- Backward compatibility maintained through converter methods (`from_antecedent()`, `from_pronoun_reference()`)
- Two-pass architecture cleanly separates entity collection from bidirectional resolution

---

### Gate 4: Clause Participant Integration ✓

**Goal:** Link resolved pronouns and obligors to clause structure.

**Deliverables:**

| File | Content | Status |
|------|---------|--------|
| `layered-clauses/src/clause_participant.rs` | `ParticipantRole` enum, `ClauseParticipant` struct, `ClauseParticipants` collection, `ParticipantDetector` | ✓ Complete |
| `layered-clauses/src/clause_query.rs` | `participants()` method on ClauseQueryAPI | ✓ Complete |
| `layered-clauses/src/lib.rs` | Export new types | ✓ Complete |

**Implementation Summary:**
- `ParticipantRole` enum: `Subject | Object | Obligor | IndirectObject`
- `ClauseParticipant` struct: span, text, role, resolved_to, confidence, needs_review fields
- `ClauseParticipants` collection with query methods: `subjects()`, `objects()`, `obligor()`, `by_role()`, `primary_subject()`, etc.
- `ParticipantDetector` for pattern-based role detection (common contract parties: tenant, landlord, lessee, lessor, etc.)
- `ClauseQueryAPI::participants()` method returns stub for now; full integration needs ParticipantResolver pipeline

**Test Coverage:** 11 unit tests passing, covering:
- ParticipantRole enum variants and display
- ClauseParticipant construction and field access
- ClauseParticipants collection queries (subjects, objects, by_role)
- ParticipantDetector pattern-based detection
- Common contract party recognition (tenant, landlord, company, etc.)

**Learnings:**
- Kept participant tracking separate from ClauseLink (cleaner separation of concerns)
- `participants()` returns stub for now - full integration needs ParticipantResolver pipeline
- Pattern-based detection works well for common contract parties (tenant, landlord, etc.)

---

### Gate 5: Relative Clause Attachment ✓

**Goal:** Detect and link relative clauses to their head nouns.

**Deliverables:**

| File | Content | Status |
|------|---------|--------|
| `layered-nlp-document/src/span_link.rs` | Add `Relative` variant to `ClauseRole` enum | ✓ Complete |
| `layered-clauses/src/relative_clause.rs` | `RelativePronoun`, `RelativeClauseType`, `RelativeClauseAttachment`, `RelativeClauseDetector` | ✓ Complete |
| `layered-clauses/src/clause_query.rs` | `relative_clause()` and `all_relative_clauses()` methods on ClauseQueryAPI | ✓ Complete |
| `layered-clauses/src/lib.rs` | Export relative_clause module and types | ✓ Complete |
| `layered-nlp-demo-wasm/src/lib.rs` | Add `Relative` match arm to demo | ✓ Complete |

**Implementation Summary:**
- `RelativePronoun` enum: `Who | Whom | Whose | Which | That | Where | When | Zero`
- `RelativeClauseType` enum: `Restrictive | NonRestrictive | Ambiguous`
- `RelativeClauseAttachment` struct: clause_span, head_span, pronoun, clause_type, confidence, needs_review fields
- `RelativeClauseDetector` with methods:
  - `is_relative_marker()` - identifies relative pronouns
  - `is_conditional_marker()` - distinguishes "if/when/unless" from relative clauses
  - `detect_clause_type()` - classifies restrictive vs non-restrictive
- `ClauseQueryAPI::relative_clause(span)` returns `Option<RelativeClauseAttachment>`
- `ClauseQueryAPI::all_relative_clauses()` returns all detected relative clause attachments

**Patterns detected:**
- "the tenant who fails to pay" → relative clause "who fails to pay" modifies "tenant"
- "the property which is located" → relative clause "which is located" modifies "property"
- "the contract, which expires" → non-restrictive (comma-bounded)
- "the contract that expires" → restrictive (no comma)

**Test Coverage:** 9 unit tests passing, covering:
- RelativePronoun enum variants and from_str parsing
- RelativeClauseType classification (restrictive vs non-restrictive)
- RelativeClauseAttachment construction and field access
- is_relative_marker() detection for who/whom/whose/which/that/where/when
- is_conditional_marker() for if/when/unless (distinguishing from relative "when")
- Clause type detection based on comma presence
- Confidence scoring (non-restrictive higher than ambiguous)

**Learnings:**
- Distinguishing "that" (relative vs complementizer) requires context heuristics - "that" after a noun is likely relative, after "believe/think/know" is likely complementizer
- Non-restrictive clauses (comma-bounded) are easier to detect with higher confidence
- "which" + comma is almost always non-restrictive; "that" is almost always restrictive (English convention)
- Zero relative pronoun ("the man I saw") is hardest to detect - flagged for review with lower confidence
- The `needs_review` flag approach established in earlier gates works well for ambiguous relative clause attachments

---

## Risks

| Risk | Mitigation |
|------|-----------|
| NP chunking still needed for full scope | Plan explicitly defers; flag ambiguous cases |
| Cataphora less reliable than anaphora | Lower confidence scores; flag uncertain |
| Relative clause boundary detection tricky | Use pronoun position heuristics first |

## Non-Goals

- Full NP chunking / syntactic parsing
- Automatic double-negative resolution (flag only)
- Scope ambiguity resolution (needs NP boundaries)
- PP (prepositional phrase) attachment disambiguation

# Semantic Linguistic Integration

> Learnings relevant to future gates should be written back to respective gates, so future collaborators can benefit.

**Status:** Complete

## Goal and Motivation

Complete linguistic understanding pipeline for legal contract analysis. Enable extraction of:
- Who (party identification via pronoun resolution)
- Must do what (obligation with modal type)
- Under what scope (negation/quantifier boundaries)
- With what certainty (explicit confidence + ambiguity flags)

Continues deferred Gates 5-6 from `clause-link-edge-cases` plan. Exploration revealed infrastructure is ~85% complete; this plan wires existing components together.

## Scope

**Delivers:**
- `ReviewableResult<T>` wrapper for consistent uncertainty handling
- Document-level pronoun resolution with cataphora support
- Scope ambiguity flagging (without NP chunker)
- Modal-scope semantic analysis
- Obligation→Party linking

**Excludes:**
- NP chunker (high effort, defer unless flagging insufficient)
- Relative clause detection (defer to Tier 3)
- Automatic resolution (flag-for-review approach instead)

**Dependencies:**
- Completed: M7 polarity/negation infrastructure
- Completed: PronounResolver (line-level, backward)
- Completed: ClauseParticipant types (not wired)

## Existing Infrastructure

| Component | Location | Status |
|-----------|----------|--------|
| PolarityTracker | `layered-contracts/src/polarity.rs` | ✅ Complete (even/odd negation) |
| DoubleNegativePatterns | `layered-contracts/src/polarity.rs` | ✅ 5 patterns + needs_review |
| ModalNegationClassification | `layered-contracts/src/modal_negation.rs` | ✅ Duty/Permission/Prohibition/Discretion |
| ScopeBoundaryDetector | `layered-contracts/src/scope_operators.rs` | ✅ Clause boundaries |
| PronounResolver | `layered-contracts/src/pronoun.rs:100-435` | ✅ Backward/anaphoric |
| DocumentPronounResolver | `layered-contracts/src/pronoun.rs:564-665` | ⚠️ Code exists, needs trait impl |
| CataphoraCandidate | `layered-contracts/src/pronoun.rs:443-562` | ⚠️ Designed, tested, not integrated |
| ClauseParticipant | `layered-clauses/src/clause_participant.rs` | ⚠️ Types exist, not wired |
| Ambiguous<T> | `layered-nlp-document/src/ambiguity.rs` | ✅ Complete |
| Scored<T> | `layered-nlp-document/src/scored.rs` | ✅ Complete |

## New Deliverables Location

| Deliverable | Crate | File |
|-------------|-------|------|
| ReviewableResult<T> | layered-nlp-document | src/reviewable.rs |
| DocumentPronounResolver impl | layered-contracts | src/pronoun.rs (extend existing) |
| ScopeAmbiguityFlagger | layered-contracts | src/scope_ambiguity.rs |
| ModalScopeAnalyzer | layered-contracts | src/modal_scope.rs |
| ObligationPartyLinker | layered-contracts | src/obligation_linker.rs |

---

## Gate: Reviewable Result Foundation

**Purpose:** Standardize uncertainty representation across all resolvers.

**Deliverables:**
- `ReviewableResult<T>` struct in `layered-nlp-document`
- Confidence composition helpers
- Integration with existing `Ambiguous<T>` and `Scored<T>`

**Type signature:**
```rust
pub struct ReviewableResult<T> {
    pub ambiguous: Ambiguous<T>,  // reuses existing best/alternatives/flag
    pub needs_review: bool,
    pub review_reason: Option<String>,
}
```

**Acceptance:**
- [x] Type composes with existing Ambiguous<T> (which already contains Scored<T>, alternatives, AmbiguityFlag)
- [x] `ReviewableResult::certain(value)` for high-confidence results
- [x] `ReviewableResult::uncertain(value, alternatives, reason)` for flagged results
- [x] `compose_confidence(scores: &[f64]) -> f64` helper
- [x] compose_confidence() uses multiplication with 0.1 floor: max(product, 0.1)
- [x] Unit tests for confidence composition (multiply with floor)

**Test scenarios:**
| Input | Expected |
|-------|----------|
| Single 0.9 confidence | 0.9 |
| 0.8 × 0.9 | 0.72 |
| 0.8 × 0.9 × 0.7 | 0.504 |
| Any score < 0.6 | `needs_review = true` |

---

## Gate: Document Pronoun Resolution

**Purpose:** Enable cross-line pronoun resolution with cataphora support and document-level coreference chains.

**Deliverables:**
- `DocumentPronounResolver` implements `DocumentResolver` trait (~200-250 LOC)
- Cataphora (forward reference) detection integrated
- Cross-line antecedent search
- Document-level coreference chain building
- Salience scoring for chain resolution

**Note:** Includes document iteration, cross-line search, cataphora integration, and chain building.

**Leverages:**
- Existing `DocumentPronounResolver` code (pronoun.rs:564-665)
- Existing cataphora tests
- `CataphoraDirection` enum
- `is_cataphoric_trigger()` for: before, until, unless, if, when, once, while

**Note:** Existing code provides scoring helpers only; resolve logic is new implementation.

**Acceptance:**
- [x] DocumentPronounResolver implements DocumentResolver trait with document iteration logic
- [x] resolve() method iterates document lines, collects pronoun references
- [x] Backward search: finds antecedents in prior lines
- [x] Forward search: finds referents after cataphoric triggers
- [x] Aggregates mentions across all lines into coreference chains
- [x] Salience: defined terms > recent mentions > distant mentions
- [x] Returns `ReviewableResult<DocumentPronounReference>`
- [x] Returns `ReviewableResult<Vec<PronounChain>>`
- [x] Flags cataphoric references as `needs_review` (less certain)

**Test scenarios:**
| Input | Expected |
|-------|----------|
| "The Company shall... It must..." | "It" → "Company" (anaphoric, high confidence) |
| "Before it expires, the Agreement..." | "it" → "Agreement" (cataphoric, flagged) |
| "They shall notify..." (no antecedent) | Unresolved, flagged |
| L1: "Acme Corp ('Company')" L5: "The Company shall" L8: "It must" | Single chain: Acme Corp → Company → It |
| Multiple entities, interleaved pronouns | Separate chains, ambiguous pronouns flagged |
| Ambiguous "they" with two parties | Flag as needs_review |
| Defined term used inconsistently | Chain broken, flagged |

---

## Gate: Scope Ambiguity Flagging

**Purpose:** Flag uncertain scope boundaries without NP chunker.

**Deliverables:**
- `ScopeAmbiguityFlagger` resolver
- Integration with `ScopeBoundaryDetector`
- `ScopeAmbiguityFlag` variants

**Leverages:**
- Existing `ScopeBoundaryDetector`
- Note: ScopeBoundaryDetector provides basic boundary detection only; confidence scoring and multi-boundary tracking are new in this gate

**Flag conditions:**
- Boundary heuristic confidence < 0.7
- Multiple plausible boundaries within 10 tokens
- Negation + quantifier in same clause
- Exception keyword + conjunction ("except A and B")

**Acceptance:**
- [x] `ScopeAmbiguityFlagger` processes `ScopeOperator<T>` spans
- [x] Emits `ScopeAmbiguityFlag` when conditions met
- [x] Preserves alternative scope interpretations
- [x] Returns `ReviewableResult<ScopeOperator<NegationOp>>`

**Test scenarios:**
| Input | Expected |
|-------|----------|
| "shall not perform X." | Clear boundary (period), no flag |
| "shall not perform X and Y" | Ambiguous: scope over X only or X and Y? Flag. |
| "except widgets and gadgets" | Ambiguous: one exception or two? Flag. |
| "not all parties" | Quantifier-negation interaction. Flag. |

---

## Gate: Modal Scope Analyzer

**Purpose:** Combine modal, polarity, and scope into semantic obligation type.

**Deliverables:**
- `ModalScopeAnalyzer` resolver
- `ScopedObligation` output type
- Integration with polarity tracking

**Output type:**
```rust
pub struct ScopedObligation {
    pub obligation: Scored<ObligationPhrase>,
    pub modal_type: ModalObligationType,  // Duty/Permission/Prohibition/Discretion
    pub polarity: Polarity,
    pub scope: ScopeOperator<NegationOp>,  // outer ReviewableResult handles uncertainty
    pub scope_confidence: f64,
    pub scope_flag: Option<AmbiguityFlag>,
    pub overall_confidence: f64,
}
```

**Depends on:** Scope Ambiguity Flagging gate

**Acceptance:**
- [x] Combines ModalNegationClassification + PolarityTracker + ScopeBoundaryDetector
- [x] Distinguishes "shall not" (Prohibition) vs "shall not be required" (Discretion)
- [x] Confidence = modal_conf × polarity_conf × scope_conf
- [x] Returns `ReviewableResult<ScopedObligation>`

**Test scenarios:**
| Input | Modal Type | Polarity | Flagged? |
|-------|------------|----------|----------|
| "Tenant shall pay rent" | Duty | Positive | No |
| "Tenant shall not sublease" | Prohibition | Negative | No |
| "Tenant shall not be required to insure" | Discretion | Positive | No (known pattern) |
| "Unless tenant does not pay" | Duty | Positive (double neg) | Yes |
| "Tenant may not assign" | Prohibition | Negative | No |

---

## Gate: Obligation Party Linking

**Purpose:** Extract clause participants and link obligations to responsible parties in a single pass.

**Deliverables:**
- `ObligationPartyLinker` resolver
- `LinkedObligation` output type with participants extracted inline
- Passive voice detection: "shall be delivered by X" → Obligor=X
- Query API for obligation→party navigation

**Note:** ClauseParticipants extraction is an internal detail, not separately exposed.

**Depends on:** Modal Scope Analyzer + Document Pronoun Resolution gates

**Output type:**
```rust
pub struct LinkedObligation {
    pub obligation: ScopedObligation,
    pub obligor: ReviewableResult<ClauseParticipant>,
    pub beneficiary: Option<ReviewableResult<ClauseParticipant>>,
    pub overall_confidence: f64,
}
```

**Acceptance:**
- [x] For each clause, identifies Subject/Object/Obligor/IndirectObject
- [x] Pronouns resolved via `DocumentPronounResolver` chains
- [x] Explicit entities via `ParticipantDetector` patterns
- [x] ClauseParticipant.resolved_to populated from PronounChain resolution
- [x] Passive constructions identified via 'by' preposition after passive verb
- [x] Links `ScopedObligation` to `ClauseParticipant` via shared spans
- [x] Obligor = Subject with Duty/Prohibition; Beneficiary = Object/IndirectObject
- [x] Confidence compounds all upstream confidences
- [x] Returns `ReviewableResult<LinkedObligation>`
- [x] Query: `obligations_for_party(party_name) -> Vec<LinkedObligation>`

**Test scenarios:**
| Input | Obligor | Obligation | Confidence |
|-------|---------|------------|------------|
| "Tenant shall pay rent monthly" | Tenant | pay rent monthly (Duty) | High |
| "Tenant shall pay Landlord" | Tenant (obligor), Landlord (beneficiary) | pay (Duty) | High |
| "It shall deliver the goods" | (resolved pronoun) | deliver goods (Duty) | Maybe flagged |
| "Company shall not disclose" | Company | disclose (Prohibition) | High |
| "Payment shall be made" | (implicit) | made (Duty) | Low, flagged (passive) |
| "shall be delivered by Tenant" | Tenant (via 'by' prep) | delivered (Duty) | High |
| "It shall be delivered" | (unresolved) | delivered (Duty) | Low, flagged |

---

## Deferred (Tier 3)

### NP Chunker
**When:** If scope ambiguity flagging has >30% false positive rate
**Effort:** 5+ days
**Risk:** High—requires phrase boundary model

### Relative Clause Detector
**When:** If participant extraction misses "the party who breaches" patterns
**Effort:** 3 days
**Risk:** Medium—syntactic pattern matching

### Automatic Resolution Pass
**When:** If flag-for-review produces too many flags for practical use
**Effort:** 4+ days
**Risk:** High—may introduce false confidence

---

## Success Criteria

Plan complete when:
1. All Tier 0-2 gates pass acceptance criteria
2. End-to-end test: contract paragraph → `LinkedObligation` with parties identified
3. Ambiguous cases flagged with `needs_review = true`
4. No automatic resolution of genuinely ambiguous constructs

---

## Completion Summary

**Completed:** 2026-01-06

### Implementation Statistics
- **Total new tests:** 76
- **All tests passing:** 659 tests in layered-contracts, 58 in layered-nlp-document

### Files Created
1. `layered-nlp-document/src/reviewable.rs` - ReviewableResult<T> wrapper (15 tests)
2. `layered-contracts/src/scope_ambiguity.rs` - ScopeAmbiguityFlagger (13 tests)
3. `layered-contracts/src/modal_scope.rs` - ModalScopeAnalyzer (15 tests)
4. `layered-contracts/src/obligation_linker.rs` - ObligationPartyLinker (22 tests)

### Files Modified
1. `layered-nlp-document/src/lib.rs` - Added reviewable module export
2. `layered-contracts/src/pronoun.rs` - Added DocumentResolver impl + PronounChainResult (11 new tests)
3. `layered-contracts/src/lib.rs` - Added new module exports
4. `layered-contracts/src/obligation.rs` - Added serde derives
5. `layered-contracts/src/contract_keyword.rs` - Added serde derives
6. `layered-contracts/Cargo.toml` - Added regex dependency

### Key Deliverables
- `ReviewableResult<T>` with confidence composition (0.1 floor)
- Document-level pronoun resolution with anaphoric + cataphoric support
- Scope ambiguity flagging for boundary uncertainty detection
- Modal-scope semantic analysis distinguishing Prohibition vs Discretion
- Obligation-party linking with passive voice detection and query API

### Review Fixes (Post-Implementation)

Two review passes identified and fixed 10 issues:

**Round 1 Critical Fixes:**
- `pronoun.rs`: Fixed cross-line token distance calculation (anaphoric + cataphoric) - was ignoring source element's position within its line
- `scope_ambiguity.rs`: Changed `detect_exception_conjunction` to return `Vec<ScopeAmbiguityFlag>` to capture all patterns
- `scope_ambiguity.rs`: Fixed `compute_max_candidate_distance` to measure gap between non-overlapping spans
- `obligation_linker.rs`: Expanded passive voice regex to match `is|are|was|were|been` auxiliary verbs
- `obligation_linker.rs`: Fixed party name capture to support unlimited capitalized words
- `obligation_linker.rs`: Changed passive voice agent role from `Subject` to `Obligor`

**Round 2 Important Fixes:**
- `obligation_linker.rs`: Fixed regex over-matching with `(?-i:...)` for case-sensitive party name capture
- `pronoun.rs`: Fixed chain `canonical_name` to preserve original case instead of lowercase
- `scope_ambiguity.rs`: Updated `test_ambiguous_and_scope` with correct assertion and documentation

**Final Test Count:** 666 tests passing in layered-contracts, 58 in layered-nlp-document

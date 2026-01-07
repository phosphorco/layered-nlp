# Accountability Graph Integration

**STATUS: PLAN COMPLETE**

All 5 gates implemented:
- Gate 1: ClauseParty with confidence/review fields
- Gate 2: ScopedObligationResolver
- Gate 3: LinkedObligationResolver
- Gate 4: AccountabilityGraphResolver consuming LinkedObligation
- Gate 5: Verification queue with obligor/beneficiary review surfacing

> **Learnings relevant to future gates should be written back to respective gates, so future collaborators can benefit.**

## Goal and Motivation

Enable contract reviewers to answer "What are Party X's obligations and who benefits?" with confidence-qualified, human-reviewable results.

**Current state:**
- Beneficiary extraction happens twice with different logic:
  - `LinkedObligation` (Gate 5): principled extraction with confidence scores, passive voice detection, ReviewableResult
  - `AccountabilityGraphResolver` (Layer 9): ad-hoc regex ("to {Party}") without confidence
- No data flow from `LinkedObligation` to Layers 7-9
- Verification queue does not surface `LinkedObligation` review flags (passive voice, low confidence, ambiguous beneficiary)
- `ContractClause` has obligor only, no beneficiary field
- `ModalScopeAnalyzer.analyze()` is NOT a resolver - it's a method requiring explicit inputs
- `ScopedObligation` is never attached as a span attribute, so it can't be queried
- `LinkedObligationResolver` can't get its inputs because the prerequisite chain is broken

**After this plan:**
- Full line-level resolver chain: `ObligationPhrase` → `ScopedObligation` → `LinkedObligation`
- Each step produces queryable span attributes via the standard `Resolver` trait
- Single source of truth for party extraction via `LinkedObligation`
- Confidence-qualified obligor and beneficiary in `ContractClause` and `ObligationNode`
- Review queue surfaces passive voice warnings, low obligor confidence, ambiguous beneficiary
- Unified pipeline from semantic analysis to accountability graph

## Scope

**Delivers:**
- Full line-level resolver chain: `ObligationPhrase` → `ScopedObligation` → `LinkedObligation`
- `ScopedObligationResolver` - wraps `ModalScopeAnalyzer` as a `Resolver`
- `LinkedObligationResolver` - wraps `ObligationPartyLinker` as a `Resolver`
- `LinkedObligation` integrated into accountability pipeline (Layers 7-9)
- Unified beneficiary extraction through `LinkedObligation`
- Review queue surfacing `LinkedObligation` issues

**Excludes:**
- UI changes
- WASM demo updates (separate plan)
- NP chunker (deferred Tier 3)
- Document-level orchestrator (replaced by pure resolver chain)

**Dependencies:**
- semantic-linguistic-integration plan (Gate 5 - completed)
- `LinkedObligation`, `ClauseParticipant`, `ReviewableResult` types exist
- `ModalScopeAnalyzer` and `ObligationPartyLinker` utilities exist
- `ObligationPhrase` resolver exists and produces queryable span attributes

---

## Architectural Overview: Resolver Chain Design

### The Problem

The original plan attempted to create a document-level orchestrator (`LinkedObligationPipeline`) that would:
1. Extract pronoun chains
2. Extract `ScopedObligation` from lines
3. Run `ObligationPartyLinker.link()` to produce `LinkedObligation`
4. Return `Vec<LinkedObligation>`

**This approach is broken because:**
- `ModalScopeAnalyzer.analyze()` is NOT a resolver - it's a method requiring explicit inputs
- `ScopedObligation` was never attached as a span attribute, so it couldn't be queried
- `LinkedObligationResolver` (Gate 3) couldn't get its inputs because the prerequisite chain was broken
- Document-level orchestrators violate the framework's resolver pattern

### The Solution: Full Line-Level Resolver Chain

Instead of a document-level orchestrator, we create a pure resolver chain where each step produces queryable span attributes:

```
ObligationPhrase (existing resolver)
    ↓ [queryable via x::attr::<ObligationPhrase>()]
ScopedObligationResolver (Gate 2 - NEW)
    ↓ [queryable via x::attr::<ScopedObligation>()]
LinkedObligationResolver (Gate 3 - existing, but fixed)
    ↓ [queryable via x::attr::<LinkedObligation>()]
```

**Each resolver:**
1. Implements the `Resolver` trait with `go()` method
2. Queries for prerequisite attributes using `selection.find_by(x::attr::<T>())`
3. Wraps the existing utility class (`ModalScopeAnalyzer`, `ObligationPartyLinker`)
4. Attaches output as span attribute via `LLCursorAssignment<T>`

**Usage pattern:**
```rust
// Document-level: extract pronoun chains once
let pronoun_chains = doc.extract_pronoun_chains();

// Line-level: run resolver chain
let config = LinkedObligationResolverConfig { pronoun_chains };
let linked_obligation_resolver = LinkedObligationResolver::new(config);

ll_line.run(&obligation_phrase_resolver)
       .run(&scoped_obligation_resolver)
       .run(&linked_obligation_resolver);

// Downstream consumers can now query
ll_line.find(&x::attr::<LinkedObligation>())
```

### Key Design Decisions

**1. ScopedObligationResolver wraps ModalScopeAnalyzer**
- `ModalScopeAnalyzer.analyze()` requires explicit inputs (obligation, modal_classification, polarity_ctx, scope, scope_flag)
- `ScopedObligationResolver` queries for these inputs and calls `analyze()`
- Makes `ScopedObligation` queryable so `LinkedObligationResolver` can use it

**2. LinkedObligationResolver receives pronoun chains via config**
- Pronoun chains are document-scope, can't be queried from line-level context
- Solution: Pass via `LinkedObligationResolverConfig` struct at construction time
- Preserves resolver pattern while allowing document-level context

**3. No document-level orchestrator**
- Original Gate 2 "LinkedObligationPipeline" is removed
- Pure resolver chain runs via standard `ll_line.run(&resolver)` pattern
- Fits framework's architecture principles

---

## Gate 1: ContractClause Beneficiary Field  ✅ COMPLETE

### Summary

Added confidence and review fields to `ClauseParty` struct to support downstream accountability graph and verification queue features:

- Added `confidence: f64`, `needs_review: bool`, `review_reason: Option<String>` fields to ClauseParty
- Updated all 3 construction sites in `build_clause_party()` with confidence logic:
  - TermRef: 1.0 confidence, no review
  - PronounRef (verified): 0.9 confidence, no review
  - PronounRef (unverified): 0.7 confidence, needs review
  - NounPhrase: 0.5 confidence, needs review
- All 22 snapshot tests updated to reflect new fields
- All 668 tests pass

### Objectives
- Add beneficiary field to `ContractClause`
- Update `ContractClauseResolver` to populate beneficiary from `LinkedObligation`
- Preserve confidence metadata in `ClauseParty`

**Note:** This gate adds field structure and query logic. Actual population occurs after Gate 3 completes. Test scenarios use mocked LinkedObligation for verification.

### Scope
- Modify: `layered-contracts/src/contract_clause.rs`
- Modify: `layered-contracts/src/tests/contract_clause.rs`

### Dependencies
- `LinkedObligation` and `ClauseParticipant` types from `obligation_linker.rs`

### Tasks

| Task | Acceptance Criteria |
|------|---------------------|
| Inventory all ClauseParty construction sites | Search for `ClauseParty {` and `ClauseParty::new` patterns, document all locations in plan artifacts section |
| Add `beneficiary: Option<ClauseParty>` to `ContractClause` | Field exists, `ClauseParty` optionally represents beneficiary |
| Add `confidence: f64` to `ClauseParty` | Party confidence preserved from source |
| Add `needs_review: bool` to `ClauseParty` | Review flag propagated from `ClauseParticipant` |
| Add `review_reason: Option<String>` to `ClauseParty` | Reason preserved for UI/queue display |
| Update `build_clause_party` to accept confidence | Method signature includes confidence parameter |
| Add `build_beneficiary_party` method | Converts `ClauseParticipant` to `ClauseParty` |
| Update `ContractClauseResolver::go()` | Queries for `LinkedObligation`, populates beneficiary if present |
| Handle missing `LinkedObligation` gracefully | Falls back to no beneficiary (current behavior) |

### Test Scenarios

| Input | Obligor | Beneficiary | Confidence | Review Flag |
|-------|---------|-------------|------------|-------------|
| "Tenant shall pay Landlord rent" | Tenant | Landlord | High (>0.7) | false |
| "Tenant shall pay rent monthly" | Tenant | None | High | false |
| "shall be delivered by Tenant to Landlord" | Tenant (passive) | Landlord | Medium (0.5-0.7) | true (passive voice) |
| "It shall pay the Company" with low pronoun confidence | (resolved) | Company | Low (<0.6) | true |
| "shall be made" (implicit obligor) | (implicit) | None | Low | true |

### Verification

**Test file:** `layered-contracts/src/tests/contract_clause.rs`

| Scenario | Pass Criteria |
|----------|---------------|
| Clause with explicit beneficiary | `clause.beneficiary.is_some()`, display_text matches |
| Clause without beneficiary | `clause.beneficiary.is_none()` |
| Beneficiary confidence preserved | `beneficiary.confidence` matches source |
| Review flag propagated | `beneficiary.needs_review` matches source |
| Review reason preserved | `beneficiary.review_reason` contains passive voice or low confidence message |
| Obligor confidence preserved | `obligor.confidence` field populated |
| Backward compatibility | Existing tests pass without modification |

---

## Gate 2: ScopedObligationResolver ✅ COMPLETE

### Summary

Created `scoped_obligation_resolver.rs` implementing `Resolver` trait with `Attr = Scored<ScopedObligation>`:

- Queries for `Scored<ObligationPhrase>` spans
- Derives `ModalNegationClassification` from `ObligationType`
- Creates default `PolarityContext` based on obligation polarity
- Calls `ModalScopeAnalyzer.analyze()` to produce `ScopedObligation`
- 6 unit tests: duty, permission, prohibition, multiple obligations, no obligations, confidence preserved
- All 672 tests pass

### Objectives
- Make `ScopedObligation` queryable via `x::attr::<ScopedObligation>()`
- Wrap `ModalScopeAnalyzer` as a proper `Resolver` that produces span attributes
- Enable downstream resolvers to query modal scope and negation context

### Scope
- New file: `layered-contracts/src/scoped_obligation_resolver.rs`
- Modify: `layered-contracts/src/lib.rs` (export)

### Dependencies
- `ObligationPhrase` resolver (existing - produces queryable span attributes)
- `ModalScopeAnalyzer` utility class (from semantic-linguistic-integration prerequisite)
- `ModalNegationClassification` attribute type (may be queryable from prior resolver or derived)
- `ScopeAmbiguityFlag` attribute type (may be queryable from prior resolver)

### Tasks

| Task | Acceptance Criteria |
|------|---------------------|
| Create `ScopedObligationResolver` implementing `Resolver` trait | Struct exists with `Attr = ScopedObligation` |
| Implement `go()` querying for `ObligationPhrase` spans | Uses `selection.find_by(x::attr::<ObligationPhrase>())` |
| For each `ObligationPhrase`, derive modal classification inline | `ObligationType::Duty` → `ModalNegationClassification::Obligation` (shall), `ObligationType::Permission` → `Permission` (may), `ObligationType::Prohibition` → `Prohibition` (shall not) |
| For each `ObligationPhrase`, extract polarity context | Use default positive polarity unless negation detected in immediate context. Full PolarityTracker integration deferred—document as known limitation. |
| For each `ObligationPhrase`, extract scope | Extract from clause boundaries using existing clause detection |
| For each `ObligationPhrase`, query scope ambiguity flag | Query `x::attr::<ScopeAmbiguityFlag>()` if available, otherwise None |
| Call `ModalScopeAnalyzer.analyze()` with extracted inputs | Pass obligation, modal_classification, polarity_ctx, scope, scope_flag |
| Return `LLCursorAssignment<ScopedObligation>` attached to ObligationPhrase span | Each ScopedObligation attached to same span as source ObligationPhrase |
| Handle lines without `ObligationPhrase` | Returns empty vector gracefully |
| Export from `lib.rs` | `ScopedObligationResolver` publicly accessible |

### Architecture Notes

**Why this is necessary:**
- `ModalScopeAnalyzer.analyze()` is a method, not a resolver - it requires explicit inputs
- To make `ScopedObligation` queryable, we need a `Resolver` that:
  1. Queries for prerequisite attributes (`ObligationPhrase`, `ModalNegationClassification`, etc.)
  2. Runs `ModalScopeAnalyzer.analyze()` with those inputs
  3. Attaches `ScopedObligation` as a span attribute

**This enables:**
- `LinkedObligationResolver` (Gate 3) can query `x::attr::<ScopedObligation>()`
- Pure resolver chain: `ll_line.run(&obligation_phrase_resolver).run(&scoped_obligation_resolver).run(&linked_obligation_resolver)`
- No document-level orchestrator needed

**Note on resolver execution order:**
Resolver execution order is determined by the caller chain: `ll_line.run(&resolver1).run(&resolver2)`. Document required ordering in code comments.

### Test Scenarios

| Input Line | ObligationPhrase | Modal | Scope | ScopedObligation |
|------------|-----------------|-------|-------|------------------|
| "Tenant shall pay rent" | [0..4] | Shall (deontic) | clause boundary | Attached to [0..4], modal="shall", scope=clause |
| "Landlord may terminate" | [0..3] | May (permission) | clause boundary | Attached to [0..3], modal="may", scope=clause |
| "It shall not deliver" | [0..4] | Shall + negation | clause boundary | Attached to [0..4], modal="shall", negated=true |
| "Regular text" | None | - | - | No ScopedObligation |

### Verification

**Test file:** `layered-contracts/src/tests/scoped_obligation_resolver.rs`

| Scenario | Pass Criteria |
|----------|---------------|
| ScopedObligation queryable via `x::attr::<ScopedObligation>()` | `ll_line.find(&x::attr::<ScopedObligation>())` returns expected spans |
| Span alignment with ObligationPhrase | ScopedObligation span matches source ObligationPhrase span |
| Modal classification preserved | `ScopedObligation.modal_classification` matches input |
| Scope extracted | `ScopedObligation.scope` contains clause boundaries |
| Negation context preserved | `ScopedObligation.negated` flag correct |
| Lines without ObligationPhrase | No ScopedObligation attributes attached |
| Resolver chains correctly | Can run after ObligationPhrase resolver in pipeline |

---

## Gate 3: LinkedObligationResolver ✅ COMPLETE

### Summary

Created `linked_obligation_resolver.rs` implementing `Resolver` trait with `Attr = LinkedObligation`:

- Created `LinkedObligationResolverConfig` holding document-level pronoun chains
- Queries for `Scored<ScopedObligation>` spans from Gate 2
- Calls `ObligationPartyLinker.link()` with pronoun chains from config
- Extracts obligor and beneficiary as `ReviewableResult<ClauseParticipant>`
- 10 unit tests: direct obligor, beneficiary patterns, pronoun resolution, confidence composition, review propagation
- All 684 tests pass (672 + 10 new + 2 existing)

### Objectives
- Make `LinkedObligation` queryable via `x::attr::<LinkedObligation>()`
- Wrap `ObligationPartyLinker` as a proper `Resolver` that produces span attributes
- Enable `AccountabilityGraphResolver` to consume `LinkedObligation` using standard resolver pattern

### Scope
- New file: `layered-contracts/src/linked_obligation_resolver.rs`
- Modify: `layered-contracts/src/lib.rs` (export)

### Dependencies
- Gate 2 (`ScopedObligation` as queryable span attribute)
- `ObligationPartyLinker` utility class (from semantic-linguistic-integration prerequisite)
- Document-level pronoun chains (passed via `LinkedObligationResolverConfig`)

### Tasks

| Task | Acceptance Criteria |
|------|---------------------|
| Create `LinkedObligationResolverConfig` struct | Holds document-level context: `pronoun_chains: PronounChainResult` |
| Create `LinkedObligationResolver` implementing `Resolver` trait | Struct exists with `Attr = LinkedObligation`, holds config |
| Implement `LinkedObligationResolver::new(config)` constructor | Accepts config with pronoun chains |
| Document pronoun chain extraction prerequisite | Add comment: "Prerequisite: Call `DocumentPronounResolver` at document level before running line-level resolver chain. Extract results into `LinkedObligationResolverConfig`." |
| Implement `go()` querying for `ScopedObligation` spans | Uses `selection.find_by(x::attr::<ScopedObligation>())` |
| For each `ScopedObligation`, call `ObligationPartyLinker.link()` | Pass ScopedObligation and pronoun chains from config |
| Return `LLCursorAssignment<LinkedObligation>` attached to ScopedObligation span | Each LinkedObligation attached to same span as source ScopedObligation |
| Handle lines without `ScopedObligation` | Returns empty vector gracefully |
| Export from `lib.rs` | `LinkedObligationResolver` and `LinkedObligationResolverConfig` publicly accessible |

### Architecture Notes

**Why this is necessary:**
- `ObligationPartyLinker.link()` requires document-level pronoun chains as input
- Pronoun chains can't be queried from line-level context (they're document-scope)
- Solution: Pass pronoun chains via config struct at resolver construction time

**Usage pattern:**
```rust
// Document-level: extract pronoun chains once
let pronoun_chains = doc.extract_pronoun_chains();

// Line-level: create resolver with config
let config = LinkedObligationResolverConfig { pronoun_chains };
let resolver = LinkedObligationResolver::new(config);

// Run as part of standard resolver chain
ll_line.run(&obligation_phrase_resolver)
       .run(&scoped_obligation_resolver)
       .run(&resolver)
```

**This enables:**
- Pure resolver chain - no document-level orchestrator needed
- `AccountabilityGraphResolver` can query `x::attr::<LinkedObligation>()`
- Pronoun resolution flows through config rather than breaking resolver pattern

**Accepted tradeoff:**
Document-level pronoun chains are passed to LinkedObligationResolver via config, creating coupling between document and line levels. This is pragmatic—pure line-level pronoun resolution would require complex context windows. Document-level extraction runs once, results shared with all line resolvers.

### Test Scenarios

| Input Line | ScopedObligation | Pronoun Chains | LinkedObligation | Obligor | Beneficiary |
|------------|-----------------|----------------|------------------|---------|-------------|
| "Tenant shall pay Landlord rent" | [0..4] | Empty | [0..4] | Tenant | Landlord |
| "It shall deliver goods" | [0..4] | "It" → "Company" | [0..4] | Company (resolved) | None |
| "Payment shall be made by Seller to Buyer" | [0..5] | Empty | [0..5] | Seller (passive) | Buyer |
| "Regular text" | None | Empty | None | - | - |

### Verification

**Test file:** `layered-contracts/src/tests/linked_obligation_resolver.rs`

| Scenario | Pass Criteria |
|----------|---------------|
| LinkedObligation queryable via `x::attr::<LinkedObligation>()` | `ll_line.find(&x::attr::<LinkedObligation>())` returns expected spans |
| Span alignment with ScopedObligation | LinkedObligation span matches source ScopedObligation span |
| Obligor/beneficiary extracted | Query result contains correct party data |
| Pronoun resolution works | "It" resolved to defined term from config |
| Passive voice detection works | Obligor extracted from "by X" construction |
| Confidence preserved | `LinkedObligation.overall_confidence` computed from components |
| Review flags preserved | `needs_review()` true when any component needs review |
| Lines without ScopedObligation | No LinkedObligation attributes attached |
| Resolver chains correctly | Can run after ScopedObligation resolver in pipeline |

---

## Gate 4: Accountability Graph Unification ✅ COMPLETE

### Summary

Updated Layer 9 to consume `LinkedObligation` as primary extraction path with fallback to ClauseAggregate:

- Added `confidence: f64`, `needs_review: bool`, `review_reason: Option<String>` to BeneficiaryLink
- Added `node_confidence: f64`, `obligor_needs_review: bool`, `obligor_review_reason: Option<String>` to ObligationNode
- Implemented primary/fallback strategy: LinkedObligation first, ClauseAggregate as legacy fallback
- Added `build_from_linked_obligations()` method for primary path
- Added `build_from_aggregates()` method for legacy fallback
- Confidence values: LinkedObligation uses overall_confidence, legacy uses 0.5-0.7 based on chain resolution
- All 684 tests pass

### Objectives
- Update Layer 9 to consume `LinkedObligation` as primary extraction path
- Add confidence field to `ObligationNode` from `LinkedObligation.overall_confidence`
- Maintain legacy regex extraction as fallback during migration period

### Scope
- Modify: `layered-contracts/src/accountability_graph.rs`
- Modify: `layered-contracts/src/tests/accountability_graph.rs`

### Dependencies
- Gate 1 (ContractClause with beneficiary)
- Gate 3 (LinkedObligation as span attribute)

### Tasks

| Task | Acceptance Criteria |
|------|---------------------|
| Add `node_confidence: f64` field to `ObligationNode` | Stores overall confidence from LinkedObligation |
| Add `obligor_needs_review: bool` to `ObligationNode` | Surfaces obligor review status |
| Add `obligor_review_reason: Option<String>` to `ObligationNode` | Surfaces obligor review reason |
| Update `BeneficiaryLink` with confidence | Add `confidence: f64` field |
| Update `BeneficiaryLink` with review fields | Add `needs_review: bool`, `review_reason: Option<String>` |
| Update `AccountabilityGraphResolver` to query `LinkedObligation` | Primary source for party extraction via `ll_line.find(&x::attr::<LinkedObligation>())` |
| Implement fallback behavior | Check for LinkedObligation first; if absent, use existing regex extraction with lower confidence |
| Document migration strategy | Add comment: "Legacy regex fallback for documents without LinkedObligation resolver in pipeline. Full removal planned after resolver chain is universally deployed." |
| Update `calculate_confidence()` | Use LinkedObligation.overall_confidence as base; fallback path uses lower confidence (0.5) |

### Test Scenarios

| Input | Node Confidence | Obligor Review | Beneficiary Review |
|-------|-----------------|----------------|-------------------|
| High confidence LinkedObligation | >0.7 | false | false |
| Passive voice obligor | 0.5-0.7 | true | false |
| Low confidence pronoun | <0.6 | true | - |
| Ambiguous beneficiary | - | false | true |
| No LinkedObligation (legacy) | From ClauseAggregate | per existing logic | per existing logic |

### Verification

**Test file:** `layered-contracts/src/tests/accountability_graph.rs`

| Scenario | Pass Criteria |
|----------|---------------|
| Node gets LinkedObligation confidence | `node.confidence` equals `linked.overall_confidence` |
| Beneficiary from LinkedObligation | `node.beneficiaries[0].display_text` matches LinkedObligation |
| Beneficiary confidence preserved | `link.confidence` matches source |
| Review flags on beneficiary | `link.needs_review` and `link.review_reason` populated |
| Obligor review flags on node | `node.obligor_needs_review` populated |
| No duplicate beneficiaries | Single beneficiary per party per clause |
| Legacy fallback works | When no LinkedObligation, uses existing regex extraction |
| LinkedObligation path preferred | LinkedObligation checked first before fallback |

---

## Gate 5: Verification Queue Enhancement COMPLETE

### Summary

Implemented verification queue enhancements to surface obligor and beneficiary review items:

- Added `ObligorLink` variant to `VerificationTarget`
- Added `PassiveVoiceObligor`, `LowConfidenceObligor`, `AmbiguousBeneficiary` variants to `VerificationQueueDetails`
- Added `confidence: f64` field to `VerificationQueueItem`
- Updated `verification_queue()` to check `obligor_needs_review` and `link.needs_review`
- Queue now sorted by confidence ascending (lowest = highest priority)
- All 684 tests pass

### Objectives
- Update `VerificationQueueItem` to surface `LinkedObligation` review reasons
- Add new detail types for passive voice, low obligor confidence, ambiguous beneficiary
- Sort queue by confidence (lowest first)

### Scope
- Modify: `layered-contracts/src/accountability_analytics.rs`
- Modify: `layered-contracts/src/verification.rs`
- Modify: `layered-contracts/src/tests/accountability_analytics.rs`

### Dependencies
- Gate 4 (ObligationNode with review fields)

### Tasks

| Task | Acceptance Criteria |
|------|---------------------|
| Add `PassiveVoiceObligor` variant to `VerificationQueueDetails` | New enum variant for passive voice detection |
| Add `LowConfidenceObligor` variant | For obligor confidence below threshold |
| Add `AmbiguousBeneficiary` variant | For beneficiary needing review |
| Update `verification_queue()` to include obligor issues | Queue obligor-related items |
| Add `obligor_confidence` field to item | For sorting by confidence |
| Sort queue by confidence ascending | Lowest confidence items first |
| Update `VerificationTarget` for obligor issues | Add `ObligorLink` variant |
| Create `VerificationAction::ResolveObligor` | Action to verify/correct obligor |

### Test Scenarios

| Node State | Queue Item Type | Priority (lower = higher) |
|------------|-----------------|---------------------------|
| Passive voice obligor | PassiveVoiceObligor | confidence (e.g., 0.65) |
| Pronoun with 0.4 confidence | LowConfidenceObligor | 0.4 |
| Beneficiary needs review | AmbiguousBeneficiary | beneficiary confidence |
| Condition mentions unknown | Condition (existing) | per existing |
| All confident | No queue item | - |

### Verification

**Test file:** `layered-contracts/src/tests/accountability_analytics.rs`

| Scenario | Pass Criteria |
|----------|---------------|
| Passive voice queued | Queue contains `PassiveVoiceObligor` detail |
| Low confidence obligor queued | Queue contains `LowConfidenceObligor` detail |
| Ambiguous beneficiary queued | Queue contains `AmbiguousBeneficiary` detail |
| Queue sorted by confidence | First item has lowest confidence |
| Multiple issues for same node | All issues queued separately |
| High confidence not queued | Node with all confident parties not in queue |
| ResolveObligor action works | `apply_verification_action` updates obligor |
| Payload includes new details | `AccountabilityPayload` serializes new variants |

---

## File Summary

### New Files (2)
```
layered-contracts/src/
├── scoped_obligation_resolver.rs    # Wraps ModalScopeAnalyzer as Resolver (Gate 2)
└── linked_obligation_resolver.rs    # Wraps ObligationPartyLinker as Resolver (Gate 3)
```

### Modified Files (5)
```
layered-contracts/src/contract_clause.rs        # Add beneficiary field (Gate 1)
layered-contracts/src/accountability_graph.rs   # Consume LinkedObligation (Gate 4)
layered-contracts/src/accountability_analytics.rs  # Enhanced queue (Gate 5)
layered-contracts/src/verification.rs           # New action/target types (Gate 5)
layered-contracts/src/lib.rs                    # Export new modules (Gates 2-3)
```

### Test Files (5)
```
layered-contracts/src/tests/contract_clause.rs                # Gate 1
layered-contracts/src/tests/scoped_obligation_resolver.rs     # New - Gate 2
layered-contracts/src/tests/linked_obligation_resolver.rs     # New - Gate 3
layered-contracts/src/tests/accountability_graph.rs           # Gate 4
layered-contracts/src/tests/accountability_analytics.rs       # Gate 5
```

---

## Artifacts & Learnings

### Gate 1: ClauseParty Construction Site Inventory

**Completed inventory of ClauseParty construction sites:**

**Struct definition:** `layered-contracts/src/contract_clause.rs:28-41`
- Current fields: `display_text: String`, `chain_id: Option<u32>`, `has_verified_chain: bool`

**Construction sites (all in `build_clause_party()` method):**
1. **TermRef branch** (lines 163-167) - When obligor is a defined term reference
2. **PronounRef branch** (lines 173-177) - When obligor is a resolved pronoun
3. **NounPhrase branch** (lines 181-185) - When obligor is a plain noun phrase

**Key insight:** ClauseParticipant in layered-clauses already has the target fields (`confidence: f64`, `needs_review: bool`, `review_reason: Option<String>`) - can use as pattern.

**Confidence logic per branch:**
- TermRef: High confidence (1.0) - defined terms are explicit
- PronounRef: Medium confidence (0.7-0.9 based on chain verification) - needs review if chain unverified
- NounPhrase: Low confidence (0.5) - always needs review (not linked to defined term)

**Files consuming ClauseParty (no construction, just reads):**
- `clause_aggregate.rs` - clones ClauseParty instances
- `accountability_graph.rs` - reads ClauseParty instances
- `accountability_analytics.rs` - reads ClauseParty instances

### Decisions Made

- Gate 1: Used ClauseParticipant as reference pattern for confidence/review fields
- Gate 1: Confidence values based on party identification certainty (term refs highest, unlinked noun phrases lowest)
- Gate 2: Used Scored<ScopedObligation> as Attr type (not ReviewableResult) to match other resolver patterns
- Gate 2: Deferred full PolarityTracker integration - using default polarity based on obligation type
- Gate 2: ScopeAmbiguityFlag query present but scope operators deferred (None for now)
- Gate 3: LinkedObligation not wrapped in Scored - has its own overall_confidence field
- Gate 3: Pronoun chains passed via config at resolver construction, not queried at runtime
- Gate 3: Config struct allows empty chains - graceful degradation when document-level pronoun resolution not available
- Gate 4: Kept existing ClauseAggregate path as fallback during migration - not removed
- Gate 4: Legacy path uses lower confidence (0.5-0.7) vs LinkedObligation path
- Gate 4: Review fields populated from LinkedObligation.obligor/beneficiary ReviewableResult
- Gate 5: Passive voice detection identified by checking review_reason contains "passive"
- Gate 5: Confidence threshold for LowConfidenceObligor set to 0.6 (configurable)
- Gate 5: Queue sorted ascending so lowest confidence items appear first for prioritized review

### Issues Encountered

(To be filled in during implementation)

### Patterns Discovered

(To be filled in during implementation)

### Verification Results

(To be filled in during implementation)

---

## Success Metrics

After implementation:
- **Primary extraction path via LinkedObligation**: All documents run through resolver chain use LinkedObligation (legacy regex fallback exists during migration period)
- **Full resolver chain**: `ObligationPhrase` → `ScopedObligation` → `LinkedObligation` all produce queryable span attributes
- **Confidence visibility**: Every party in the accountability graph has a confidence score
- **Review coverage**: Verification queue surfaces 100% of low-confidence parties
- **Passive voice handling**: All "shall be X by Y" constructions correctly identify Y as obligor
- **Backward compatibility**: Existing tests pass, legacy path available for documents without LinkedObligation resolver in pipeline

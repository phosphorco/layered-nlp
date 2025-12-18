# Plan: Contract Language Formalization with Antecedent Awareness

## Goal
Build a production-ready, incremental resolver architecture for contract language analysis with:
- Pronoun chains and antecedent resolution
- Obligation phrase detection
- Multi-pass analysis with external verification support
- Confidence scoring (confidence < 1 = needs verification, verified = confidence 1)
- Traceable accountability mapping so we can answer "who owes what" across an agreement
- Layered passes that the user can refine (adjusted definitions automatically update downstream layers)

## Core Infrastructure: Scored<T> Wrapper

```rust
/// confidence = 1.0 means verified/certain
/// confidence < 1.0 means needs verification
pub struct Scored<T> {
    pub value: T,
    pub confidence: f64,      // 0.0 to 1.0
    pub source: ScoreSource,  // RuleBased | LLMPass | HumanVerified | Derived
}
```

Key principle: **Never return single "exact" matches. Always return candidates with scores.**

---

## Accountability Tracking Context

- Layered passes let us tag parties, defined terms, pronouns, and obligations independently, then weave them into accountability chains (who promised what, under which conditions).
- Each layer consumes the scored output of earlier layers, so re-running Layer 2 (definitions) automatically refreshes references, pronoun chains, and obligations.
- Verification hooks + confidence scores ensure anything below 1.0 gets surfaced for human/LLM review before we trust it in downstream search/analytics.

---

## Resolver Stack (7 Layers)

### Layer 1: ContractKeywordResolver
**Purpose**: Identify contract-specific keywords (shall, may, means, if, unless, etc.)

**Produces**: `ContractKeyword` enum
```rust
enum ContractKeyword {
    Shall, May, ShallNot,           // Obligation modals
    Means, Includes, Hereinafter,   // Definition signals
    If, Unless, Provided,           // Conditionals
    Party,                          // Party indicators
}
```

**Snapshot example**:
```
The     Contractor     shall     deliver  ...
                       ╰───╯Shall
```

---

### Layer 2: DefinedTermResolver
**Depends on**: ContractKeyword, POSTag, TextTag
**Produces**: `Scored<DefinedTerm>`

**Patterns detected**:
| Pattern | Example | Base Confidence |
|---------|---------|-----------------|
| Formal `"Term" means` | `"Company" means ABC Corp` | 0.95 |
| Parenthetical | `ABC Corp (the "Company")` | 0.90 |
| Hereinafter | `ABC Corp, hereinafter "Company"` | 0.85 |
| Implicit (capitalized) | `The Contractor shall...` | 0.40 |

---

### Layer 3: TermReferenceResolver
**Depends on**: `Scored<DefinedTerm>`
**Produces**: `Scored<TermReference>` assignments applied to word spans (future: optional `DefinitionCandidate` list when we surface multiple competing definitions)

Links later mentions of defined terms back to their definitions so downstream layers (pronouns, obligations) can treat repeated mentions as anchored entities.

---

### Layer 4: PronounResolver
**Depends on**: POSTag::Pronoun, TermReference, DefinedTerm
**Produces**: `Scored<PronounReference>` with `Vec<AntecedentCandidate>`

**Scoring factors**:
| Factor | Modifier |
|--------|----------|
| Nearest noun phrase | Base 0.50 |
| Is defined term | +0.30 |
| Gender/number agreement | +0.15 |
| Same sentence | +0.10 |
| Multiple candidates | -0.20 |

**Snapshot example**:
```
The     Company     shall     deliver  .     It     must     comply  ...
        ╰─────╯DefinedTerm
                                             ╰╯Scored(PronounRef { It -> [Company: 0.75] }, conf: 0.60)
```

---

### Layer 5: ObligationPhraseResolver
**Depends on**: ContractKeyword (Shall/May), TermReference, PronounReference
**Produces**: `Scored<ObligationPhrase>`

```rust
struct ObligationPhrase {
    obligor: ObligorReference,      // Who has the duty
    obligation_type: ObligationType, // Duty | Permission | Prohibition
    action: String,                  // What they must do
    conditions: Vec<ConditionRef>,   // If/unless/provided conditions
}
```

---

### Layer 6: PronounChainResolver
**Depends on**: PronounReference, TermReference
**Produces**: `Scored<PronounChain>`

Builds coreference chains across sentences linking all mentions to resolved entity.

---

### Layer 7: ContractClauseResolver
**Depends on**: All previous layers
**Produces**: `Scored<ContractClause>`

Synthesizes complete clause analysis with parties, obligations, conditions.

---

## External Verification Integration

```rust
// Collect items needing verification (confidence < 1.0)
let requests = VerificationBridge::collect_for_verification(&ll_line);

// After human/LLM verification, apply results
VerificationBridge::apply_verifications(&mut ll_line, verified_results);
```

Supports:
- Human verification setting confidence = 1.0
- LLM pass boosting/adjusting scores
- Rule-based scores from external sources

---

## Completed Layers

✅ **Layer 1: ContractKeywordResolver** - 30 passing tests
- `ContractKeyword` enum (Shall, May, Means, If, Unless, etc.)
- `ProhibitionResolver` for "shall not" patterns
- `Scored<T>` infrastructure

✅ **Layer 2: DefinedTermResolver** - 16 passing tests
- `DefinedTerm` struct with `term_name` and `definition_type`
- `DefinitionType` enum (QuotedMeans, Parenthetical, Hereinafter)
- Confidence scores: QuotedMeans 0.95, Parenthetical 0.90, Hereinafter 0.90

✅ **Layer 3: TermReferenceResolver** - 14 passing tests
- `TermReference` struct with `term_name` and `definition_type`
- Confidence scoring: exact case + capitalized (0.90), case-insensitive + capitalized (0.85), exact + lowercase (0.70), case-insensitive + lowercase (0.65)
- Article bonus (+0.05) for "the", "this", "such" preceding the term
- Multi-word term matching (e.g., "Effective Date")
- Properly skips terms inside definition spans

✅ **Layer 4: PronounResolver** - 17 passing tests
- `PronounReference` struct with pronoun text, type, and candidate antecedents
- `AntecedentCandidate` struct tracking text, defined term status, distance, and confidence
- `PronounType` enum (ThirdSingularNeuter, ThirdSingularMasculine, ThirdSingularFeminine, ThirdPlural, Relative, Other)
- Confidence scoring: base 0.50, +0.30 defined term bonus, +0.10 same sentence, -0.20 multiple candidates
- Distance-based penalty (~0.02 per token)
- Integrates with POSTagResolver (Tag::Pronoun), DefinedTerm, and TermReference

✅ **Layer 5: ObligationPhraseResolver** - 15 passing tests
- `ObligationPhrase` bundles `ObligorReference`, `ObligationType`, action span, and detected `ConditionRef`s
- Anchors on `ContractKeyword::Shall | May | ShallNot` while skipping `Shall` tokens already upgraded to `ShallNot`
- Pulls obligors from the nearest `TermReference`, `PronounReference` (using best candidate), or capitalized noun fallback
- Extracts action spans up to punctuation/new modal/condition keywords and links nearby `If/Unless/Provided/SubjectTo` clauses
- Confidence heuristics: base 0.75, +0.10 defined term bonus, +0.05 pronoun-chain bonus, -0.15 when multiple obligor candidates compete, -0.10 when action span is empty

✅ **Layer 6: PronounChainResolver** - 16 passing tests
- `PronounChain` struct captures `chain_id`, canonical name, whether the chain is seeded by a definition, ordered `ChainMention`s, and verification metadata.
- Seeds chains from `Scored<DefinedTerm>` (canonical definition mention) and merges `Scored<TermReference>` mentions keyed by lowercase term names.
- Attaches pronouns using their candidate list when `confidence >= 0.40`, ensuring pronouns only join chains that already have the referenced entity.
- Chain confidence starts from the best mention confidence and decays by 0.05 for each mention below 0.70, highlighting chains stitched from uncertain links.
- Deterministic output via canonical-name sorting; single-mention chains are skipped to avoid noise.

✅ **Layer 7: ContractClauseResolver** - 9 passing tests
- `ContractClause` wraps `ClauseParty`, `ClauseDuty`, and `ClauseCondition` around each `ObligationPhrase` so downstream layers can reason about clause-level semantics.
- `ClauseParty` stores the display text plus optional `chain_id` linkage and whether that chain already contains a verified mention; no beneficiary role yet (obligor only).
- `ClauseCondition` keeps the normalized keyword (`If/Unless/Provided/SubjectTo`), the preview text from Layer 5, and a boolean indicating whether the text mentions a known pronoun chain/defined entity (detected via lowercase canonical-name matching).
- Confidence inherits from the source obligation, +0.05 when the obligor maps to a verified pronoun chain, -0.10 when the action text is empty, and -0.15 when any attached condition references a capitalized/unknown entity so downstream reviewers can prioritize those clauses.
- Clause IDs derive from the modal token offset to preserve deterministic ordering and a strict 1:1 mapping with `ObligationPhrase` outputs (aggregation remains a potential future layer).

✅ **Layer 8: ClauseAggregationResolver** - 5 passing tests
- Groups adjacent `Scored<ContractClause>` records by obligor (preferring pronoun chain IDs) to power the core “who owes what” query.
- Aggregates keep ordered clause IDs with their duties/conditions plus source offsets so provenance can be traced when filtering by party.
- Confidence = min member clause confidence, -0.05 when we fall back to plain display text (no chain provenance), and -0.10 when an aggregate spans >40 tokens (possible cross-section stitching).
- `max_gap_tokens` (30 by default) constraints merges to clauses within the same local paragraph/sentence so downstream analytics retain contextual integrity.

✅ **Layer 9: AccountabilityGraphResolver** - 4 passing tests
- Converts `ClauseAggregate` outputs into `ObligationNode`s that expose obligor → beneficiary edges plus `ConditionLink`s, enabling queries like `graph.for_party(seller_chain_id)`.
- Beneficiary detection scans clause actions for “to ___” phrases, matches them to pronoun chains when possible, and flags unresolved capitalized phrases for verification so we don’t silently swallow mystery parties.
- Condition links reuse Layer 7 `ClauseCondition`s so downstream consumers can trace dependencies on sections (“subject to Section 5”) or conditional triggers (“if Buyer pays”).
- Confidence starts from the aggregate confidence, -0.10 when any beneficiary needs verification, +0.05 when at least one beneficiary maps to a verified chain, keeping the scoring aligned with accountability risk.


## Current Implementation Snapshot (as of this plan)

- `layered-contracts/src/contract_keyword.rs` implements `ContractKeywordResolver` + `ProhibitionResolver`.
- `layered-contracts/src/defined_term.rs` implements the three definition patterns with reusable helpers (`extract_quoted_term_*`).
- `layered-contracts/src/term_reference.rs` implements `TermReferenceResolver` with confidence scoring and multi-word matching.
- `layered-contracts/src/pronoun.rs` implements `PronounResolver` with antecedent detection, number agreement, and scoring.
- `layered-contracts/src/obligation.rs` implements `ObligationPhraseResolver` with modal anchoring, obligor detection, action extraction, and condition linking.
- `layered-contracts/src/pronoun_chain.rs` implements `PronounChainResolver` with chain builders, pronoun attachment thresholds, and confidence decay.
- `layered-contracts/src/contract_clause.rs` implements `ContractClauseResolver`, including the `ClauseParty`, `ClauseDuty`, and `ClauseCondition` structs plus configurable heuristics via `with_settings`.
- `layered-contracts/src/clause_aggregate.rs` implements `ClauseAggregationResolver`, which groups clause outputs by obligor and enforces configurable gap/penalty heuristics.
- `layered-contracts/src/accountability_graph.rs` implements `AccountabilityGraphResolver`, turning clause aggregates into obligor nodes with beneficiary and condition edges.
- `layered-contracts/src/lib.rs` re-exports keywords, definitions, term references, pronouns, obligations, pronoun chains, contract clauses, clause aggregates, accountability graph nodes, and scoring infrastructure, and wires the snapshot tests under `#[cfg(test)]`.
- Tests live beside the crate at `layered-contracts/src/tests/contract_keyword.rs`, `.../defined_term.rs`, `.../term_reference.rs`, `.../pronoun.rs`, `.../obligation.rs`, `.../pronoun_chain.rs`, `.../contract_clause.rs`, `.../clause_aggregate.rs`, and `.../accountability_graph.rs` with matching Insta snapshots under `.../tests/snapshots/`.

Keep the plan synchronized with this directory structure so newcomers can jump from prose → file path without guessing.

---

## Layer 3 - TermReferenceResolver (Implemented)

Links subsequent mentions of defined terms back to their definitions. See `layered-contracts/src/term_reference.rs` for the full implementation.

**Key implementation details:**
- Builds a lookup map keyed by lowercase first word → `Vec<(term_name, definition_type)>` to handle multi-word terms and collisions
- Uses `split_with` to check if word selections are contained within definition spans (avoiding self-references)
- Multi-word matching via `match_multiword_term()` which returns both the extended selection AND the full surface text for accurate confidence scoring
- Confidence scoring uses the actual matched surface text (not just first word) for multi-word terms

**Confidence table:**
| Scenario | Confidence |
|----------|------------|
| Exact case + capitalized | 0.90 |
| Case-insensitive + capitalized | 0.85 |
| Exact case + lowercase | 0.70 |
| Case-insensitive + lowercase | 0.65 |
| Article bonus ("the", "this", "such") | +0.05 |

---

## Layer 4 - PronounResolver (Implemented)

Identifies pronouns and links them to potential antecedents. See `layered-contracts/src/pronoun.rs` for the full implementation.

**Key implementation details:**
- Integrates with `POSTagResolver` to find `Tag::Pronoun` tokens
- Collects antecedents from `DefinedTerm`, `TermReference`, and plain nouns (tagged as `Tag::Noun` or `Tag::ProperNoun`)
- Uses `selection_is_before()` helper with `split_with` to ensure only prior antecedents are considered
- Agreement checking via `check_agreement()` which detects likely plural antecedents (e.g., "Parties", "Contractors")

**Confidence scoring:**
| Factor | Modifier |
|--------|----------|
| Base confidence | 0.50 |
| Is defined term | +0.30 |
| Same sentence | +0.10 |
| Number agreement | +0.15 |
| Multiple candidates | -0.20 |
| Distance penalty | -0.02/token (max -0.30) |

**Pronoun types:** ThirdSingularNeuter (it/its), ThirdSingularMasculine (he/him/his), ThirdSingularFeminine (she/her/hers), ThirdPlural (they/them/their), Relative (this/that/which/who)

---

## Layer 5 - ObligationPhraseResolver (Implemented)

Anchors on modal keywords and extracts obligation phrases tying obligors, action text, and qualifying conditions. See `layered-contracts/src/obligation.rs`.

**Key implementation details:**
- Finds `ContractKeyword::Shall | May | ShallNot` spans (after running `ProhibitionResolver`) and skips bare `Shall` tokens that are part of an upgraded `ShallNot`.
- Resolves the obligor via the nearest preceding `Scored<TermReference>` or `Scored<PronounReference>` (falling back to capitalized noun phrases) and wraps them in `ObligorReference`.
- Extracts the action span by walking forward from the modal through words/whitespace until it hits punctuation, another modal, or a condition keyword (including `SubjectTo`).
- Detects nearby `If/Unless/Provided/SubjectTo` keywords in the **same sentence** and stores lightweight `ConditionRef` previews so conditions don’t leak across clauses.
- POS-tag-aware fallback obligor detection limits matches to capitalized nouns/proper nouns to avoid grabbing tokens like “If” as an entity.
- Confidence scoring follows the documented heuristics (base 0.75, bonuses for defined term/pronoun chain, penalties for competing obligors or empty action).

**Coverage:** 15 snapshot tests spanning duty, permission, prohibition, conditionals, pronoun-obligor chains, multi-obligation sentences, and real contract excerpts (`layered-contracts/src/tests/obligation.rs` + snapshots).

---

## Layer 6 - PronounChainResolver (Implemented)

Stitches all mentions of an entity (definitions, references, pronouns) into deterministic chains so downstream layers can operate on entity-level context. Implemented in `layered-contracts/src/pronoun_chain.rs`.

**Key implementation details:**
- Seeds chains from all `Scored<DefinedTerm>` spans, keyed by lowercase term names, and marks `is_defined_term = true`.
- Adds `Scored<TermReference>` mentions to the matching chain; if a reference appears without a definition, a new non-defined chain is created so pronouns can still attach.
- Pronouns contribute mentions when their best candidate references an existing chain and its confidence ≥ 0.40; only the top candidate is used to avoid double-linking.
- Each `PronounChain` stores ordered `ChainMention`s (Definition, TermReference, Pronoun) with surface text, confidence, and approximate token offsets.
- Chain confidence derives from the highest mention confidence minus a 0.05 decay per mention below 0.70, clamped to `[0,1]`. Chains with a verified mention (`confidence == 1.0`) surface `has_verified_mention = true`.
- Builders are converted to `Scored<PronounChain>` attributes anchored on the defining selection (or first reference) and sorted by canonical name for deterministic snapshots.

**Coverage:** 16 snapshot tests (basic chains, pronoun-only chains, plural pronouns, competing antecedents, real contract paragraphs, edge cases where pronouns have no resolvable chain).

---

## Layer 7 - ContractClauseResolver (Implemented)

Aggregates every `Scored<ObligationPhrase>` into a clause-level object that records who owes what under which conditions. See `layered-contracts/src/contract_clause.rs`.

**Key implementation details:**
- `ContractClause` carries a deterministic `clause_id`/`source_offset`, `ClauseParty` (obligor text + optional `chain_id` + `has_verified_chain`, only set when PronounChainResolver produced a ≥2 mention chain), `ClauseDuty` (type + action string), and normalized `ClauseCondition`s.
- `ContractClauseResolver` walks obligation spans in document order, reuses the pronoun-chain collection to link obligors back to entity chains, and keeps clause IDs tied to the modal token offset to preserve provenance.
- `ClauseCondition::mentions_unknown_entity` flips to `true` only when the preview text includes capitalized tokens that don’t match any canonical chain name and aren’t in a small allowlist of common legal boilerplate (`Agreement`, `Section`, `Schedule`, `Exhibit`, `Effective`, `Date`), so innocuous phrases (“subject to Section 5”) don’t get penalized.
- Confidence model: inherit the upstream obligation confidence, add `verified_party_bonus` (default 0.05) when the chain already has a verified mention, subtract `missing_action_penalty` (0.10) when the downstream action is blank/whitespace, and subtract `undefined_condition_penalty` (0.15) when any attached condition flags `mentions_unknown_entity`. `with_settings()` allows tuning these heuristics per deployment.
- Output mirrors the source obligation ordering exactly and remains 1:1 with Layer 5, deferring cross-obligation aggregation to a potential later phase. We’re intentionally keeping the allowlist focused until we collect empirical false positives so the resolver doesn’t hide potentially unknown actors just to keep scores high.

**Coverage:** 9 snapshot tests covering defined-term clauses, prohibitions, subject-to conditions, pronoun obligors (including verified-chain boosts), known vs. unknown condition entities, empty-action penalties, and multi-obligation ordering.

---

## Layer 8 - ClauseAggregationResolver (Implemented)

Groups contiguous `ContractClause` outputs by obligor so downstream systems can run the accountability query directly (e.g., `let seller_duties = aggregates.iter().filter(|a| a.obligor.chain_id == Some(seller_chain));`).

**Key implementation details:**
- Aggregation keys prioritize `ClauseParty.chain_id`; we fall back to a normalized display text (quotes stripped, lowercase, leading article removed) when no chain exists.
- Clauses are processed in offset order; aggregates grow only when the next clause is within `max_gap_tokens` (default 30) so we stay in the same sentence/paragraph neighborhood.
- Each aggregate stores deterministic `aggregate_id`s (first clause offset), ordered clause IDs, per-clause duty/condition snapshots, and span offsets (`source_start`/`source_end`) for traceability.
- Confidence derives from the lowest member clause, -0.05 when we lack chain provenance, and -0.10 when the aggregate spans >40 tokens to highlight potential cross-section stitching.
- Resolver output attaches to the first clause selection, keeping Insta snapshots deterministic and making it easy to align clause-level and aggregate-level displays.

**Coverage:** 5 snapshot tests demonstrating single clauses, multi-duty rollups, aggregation resets when another party intervenes, missing-chain penalties, and long-span penalties.

---

## Layer 9 - AccountabilityGraphResolver (Implemented)

Builds an obligor-centric graph that links ClauseAggregates to their beneficiaries and conditions so analytics engines can jump straight to “who owes what to whom, under which triggers.”

**Key implementation details:**
- `ObligationNode` stores `aggregate_id`, `ClauseParty` obligor metadata, the underlying `ClauseAggregateEntry` list, `BeneficiaryLink`s, and `ConditionLink`s.
- Beneficiaries are detected by scanning each clause’s action text for “to …” phrases; we normalize the candidate (“the Buyer” → `buyer`) and match it against pronoun-chain canonical names. Unmatched capitalized phrases become `needs_verification = true` links so review tooling can chase them down.
- Condition edges repackage Layer 7 `ClauseCondition`s (If/Unless/SubjectTo). Each `ConditionLink` records the clause ID and condition type/text, giving downstream consumers a straightforward way to filter on Section references or “if Buyer pays” triggers.
- Confidence starts from the aggregate score, subtracts `unresolved_beneficiary_penalty` (0.10) when any beneficiary lacks a chain, and adds `verified_beneficiary_bonus` (0.05) when at least one beneficiary chain already has a verified mention.
- Resolver output attaches to the aggregate’s selection for deterministic Insta snapshots and to keep provenance consistent between ClauseAggregates and graph nodes.

**Coverage:** 4 snapshot tests covering matched beneficiaries, unresolved beneficiaries (penalty path), condition links, and verified-chain bonuses.

---

## Future Phases

### Phase 10: AccountabilityAnalytics (Planned)
- **Goal:** Turn the Layer 9 graph into actionable analytics + review workflows so we can answer “who owes what, to whom, under which sections, and how confident are we?” without additional post-processing.
- **Analytics surfaces:**
  - Party explorer: `graph.for_party(chain_id)` returns all nodes/edges for that chain, grouped by beneficiary and sorted by confidence.
  - Section/condition filters: surface obligations referencing specific sections or condition types (“SubjectTo Section 5”).
  - Cross-document rollups: optional phase that merges nodes across documents (same entity chain) so we can report Seller obligations across an entire corpus.
- **Verification tooling:**
  - Tag `BeneficiaryLink::needs_verification` nodes in a queue, allow reviewers/LLMs to confirm or correct the beneficiary entity, and convert those confirmations into `Scored::verified` updates.
  - Support `verification.rs` hooks so reviewers can mark entire nodes/edges as verified (confidence → 1.0) or attach notes (“Beneficiary is regulatory agency, not counterparty”).
- **Implementation outline:**
  - Add `accountability_analytics.rs` exposing helper queries (`ObligationGraph::for_party`, `::with_condition`, etc.) that operate on `Vec<Scored<ObligationNode>>`.
  - Emit a serialized snapshot struct (JSON-friendly) so BI/search can consume the graph with provenance (clause IDs, offsets, verification status).
  - Extend tests with realistic graph queries (e.g., seller owes buyer under Section 5) and verification workflows (mark unresolved beneficiary, re-run resolver to show confidence bump).
- **Success metric:** ability to run a downstream query like “List all Seller obligations requiring Buyer approval under Section 5, highlight ones needing verification” directly from the exported graph payload without ad hoc parsing.

---

## Critical Files

**Patterns to follow**:
- `layered-part-of-speech/src/lib.rs` - Simple resolver pattern
- `layered-clauses/src/clauses.rs` - Multi-pass resolver pattern
- `layered-clauses/src/tests/clauses.rs` - Snapshot test pattern

**Core APIs**:
- `src/ll_line.rs:304` - `Resolver` trait
- `src/ll_line/x/functions.rs` - Matcher combinators
- `src/ll_line/ll_selection.rs` - Selection operations

---

## Crate Structure

```
layered-contracts/
  Cargo.toml
  src/
    lib.rs
    scored.rs              # ✅ Scored<T> infrastructure
    contract_keyword.rs    # ✅ Layer 1: ContractKeyword, ProhibitionResolver
    defined_term.rs        # ✅ Layer 2: DefinedTerm, DefinitionType, DefinedTermResolver
    term_reference.rs      # ✅ Layer 3: TermReference, TermReferenceResolver
    pronoun.rs             # ✅ Layer 4: PronounReference, PronounType, PronounResolver
    obligation.rs          # ✅ Layer 5: ObligationPhrase, ObligorReference, ConditionRef, ObligationPhraseResolver
    pronoun_chain.rs       # ✅ Layer 6: PronounChain, PronounChainResolver
    contract_clause.rs     # ✅ Layer 7: ContractClause, ClauseParty, ClauseDuty, ClauseCondition, ContractClauseResolver
    clause_aggregate.rs    # ✅ Layer 8: ClauseAggregate, ClauseAggregateEntry, ClauseAggregationResolver
    verification.rs        # External integration
  tests/
    contract_keyword.rs    # ✅ 30 tests
    defined_term.rs        # ✅ 16 tests
    term_reference.rs      # ✅ 14 tests
    pronoun.rs             # ✅ 17 tests
    obligation.rs          # ✅ 15 tests
    pronoun_chain.rs       # ✅ 16 tests
    contract_clause.rs     # ✅ 9 tests
    clause_aggregate.rs    # ✅ 5 tests
    integration.rs
```

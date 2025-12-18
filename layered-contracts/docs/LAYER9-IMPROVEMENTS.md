# Layer 9 (AccountabilityGraphResolver) Improvements

This document captures planned improvements to the beneficiary detection and entity linking in Layer 9, based on analysis of false positives and coverage gaps.

---

## Overview

Layer 9 (`AccountabilityGraphResolver`) converts `ClauseAggregate` outputs into `ObligationNode`s with obligor→beneficiary edges. Current beneficiary detection scans action text for `" to "` phrases and matches against pronoun chains.

**Key files:**
- `src/accountability_graph.rs` — Layer 9 implementation
- `src/pronoun_chain.rs` — Layer 6, creates chains from mentions
- `src/contract_clause.rs` — Layer 7, has allowlist for legal terms
- `src/tests/accountability_graph.rs` — current tests (4 snapshots)

---

## High Priority Issues

### 1. "pursuant to Section" False Positives

**Problem:** The `" to "` beneficiary pattern matches document references that aren't parties:

```
Input:  "shall perform pursuant to Section 5"
Result: BeneficiaryLink { display_text: "Section 5", needs_verification: true }
        ← FALSE POSITIVE
```

**Affected patterns:**
- `pursuant to Section X`
- `subject to Article Y`
- `prior to Closing`
- `according to Schedule A`
- `in addition to Exhibit B`

**Fix:** Add a blocklist for non-party heads and filter before creating `BeneficiaryLink`:

```rust
impl AccountabilityGraphResolver {
    /// Heads that indicate structural/time references, not parties.
    const NON_PARTY_HEADS: &'static [&'static str] = &[
        "section",
        "article",
        "schedule",
        "exhibit",
        "annex",
        "appendix",
        "chapter",
        "closing",
        "effective date",
        "execution",
        "consummation",
        "termination",
        "expiration",
        "this agreement",
        "agreement",
    ];

    fn is_structural_or_timing_reference(normalized_candidate: &str) -> bool {
        let first_word = normalized_candidate
            .split_whitespace()
            .next()
            .unwrap_or("");
        Self::NON_PARTY_HEADS.contains(&first_word)
    }
}
```

Then in `detect_beneficiaries`, after normalizing:

```rust
// Skip structural/timing references like "Section 5", "Article 3", "Closing"
if Self::is_structural_or_timing_reference(&normalized) {
    continue;
}
```

**Tests to add:**
- `pursuant_to_section_is_not_beneficiary` — expects no beneficiary for `"Section 5"`
- `subject_to_article_is_not_beneficiary` — expects no beneficiary for `"Article III"`
- `prior_to_closing_is_not_beneficiary` — expects no beneficiary for `"Closing"`
- `deliver_to_buyer_still_detects_beneficiary` — positive control, real party still detected

**Confidence impact:** Fewer spurious `needs_verification = true` links → fewer `unresolved_beneficiary_penalty` hits → more accurate node confidence.

---

### 2. Single-Mention Defined Terms Have No chain_id

**Problem:** `PronounChainResolver` requires ≥2 mentions to emit a chain. Single-definition terms get no `chain_id`:

```rust
// From snapshot:
obligor: ClauseParty { display_text: "Seller", chain_id: None, ... }

// But "Seller" IS defined via: ABC Corp (the "Seller")
```

**Impact:** Queries like "show all obligations for Seller" miss clauses because `chain_id` is `None`.

**Fix:** In `pronoun_chain.rs`, relax the filter for defined terms only:

```rust
// Current:
if builder.mentions.len() < 2 {
    continue;
}

// Change to:
if !builder.is_defined_term && builder.mentions.len() < 2 {
    continue;
}
```

**Rationale:** A formal definition (`"Company" means ABC Corp`) is an authoritative anchor even without second mentions. Non-defined-term chains (pure references) still require ≥2 mentions to avoid noise.

**Tests to add:**
- `single_definition_yields_chain` in `tests/pronoun_chain.rs` — single `"Company" means` produces a chain
- `single_defined_obligor_has_chain_id` in `tests/contract_clause.rs` — obligor gets `chain_id`
- `single_mention_beneficiary_uses_chain` in `tests/accountability_graph.rs` — beneficiary links to chain

**Downstream effects:**
- `ContractClauseResolver`: More clauses get `verified_party_bonus` (+0.05)
- `AccountabilityGraphResolver`: More beneficiaries map to chains, fewer `unresolved_beneficiary_penalty`
- `ClauseCondition.mentions_unknown_entity`: Fewer false positives since `known_entity_names` is richer

---

## Medium Priority Issues

### 3. "for" Beneficiaries Not Detected

**Problem:** Common contract patterns missed:

```
"shall perform services for the Client"     ← Client not detected
"shall act as agent for the Company"        ← Company not detected
"shall maintain insurance for Buyer"        ← Buyer not detected
```

**Fix:** Refactor `extract_beneficiary_candidates` to support multiple prepositions:

```rust
impl AccountabilityGraphResolver {
    fn extract_beneficiary_candidates_for_prep(action: &str, prep: &str) -> Vec<String> {
        let mut candidates = Vec::new();
        let lower = action.to_lowercase();
        let needle = format!(" {} ", prep);
        let mut cursor = 0;

        while let Some(rel_idx) = lower[cursor..].find(&needle) {
            let start = cursor + rel_idx + needle.len();
            cursor = start;
            if start >= action.len() {
                break;
            }

            let candidate = action[start..].trim_start();
            if candidate.is_empty() {
                continue;
            }

            // Find end at delimiter
            let mut end = candidate.len();
            for delim in &[",", ";", ".", ":"] {
                if let Some(idx) = candidate.find(delim) {
                    end = end.min(idx);
                }
            }
            for kw in &[" and ", " or "] {
                if let Some(idx) = candidate.to_lowercase().find(kw) {
                    end = end.min(idx);
                }
            }

            let trimmed = candidate[..end].trim();
            if !trimmed.is_empty() {
                candidates.push(trimmed.to_string());
            }
        }
        candidates
    }

    fn extract_beneficiary_candidates(action: &str) -> Vec<String> {
        let mut all = Vec::new();
        all.extend(Self::extract_beneficiary_candidates_for_prep(action, "to"));
        all.extend(Self::extract_beneficiary_candidates_for_prep(action, "for"));
        all
    }
}
```

**Tests to add:**
- `services_for_client_detects_beneficiary` — `"for the Client"` yields beneficiary with chain
- `for_lowercase_noun_is_ignored` — `"for damages"` (lowercase) is not a beneficiary
- `for_section_is_ignored_by_blocklist` — `"for Section 10.2"` blocked by `NON_PARTY_HEADS`

**Note on false positives:** Phrases like `"for a period of"` and `"for the purpose of"` will be rejected by `looks_like_entity()` since `"a period of"` / `"the purpose of"` aren't capitalized.

---

### 4. Missing Test Coverage

**Current gaps:**

| Scenario | Status |
|----------|--------|
| Multiple beneficiaries in one action (`"to Buyer and to Auditor"`) | ❌ Missing |
| `"for"` beneficiary (`"services for Client"`) | ❌ Missing |
| `"pursuant to Section"` false positive | ❌ Missing |
| Infinitive `"to"` (`"required to perform"`) | ❌ Missing |
| Aggregate with multiple clauses, each with different beneficiary | ❌ Missing |
| Circular reference (obligor == beneficiary) | ❌ Missing |
| Beneficiary in condition text | ❌ Missing |

**Tests to add:**

```rust
#[test]
fn graph_multiple_beneficiaries_same_action() {
    // "to Buyer and to Auditor" should extract both
    insta::assert_snapshot!(test_graph(
        r#"ABC Corp (the "Seller") shall deliver goods to the Buyer and provide reports to the Auditor."#
    ));
}

#[test]
fn graph_pursuant_to_not_beneficiary() {
    // "Section 5" should NOT be a beneficiary
    insta::assert_snapshot!(test_graph(
        r#"ABC Corp (the "Company") shall perform pursuant to Section 5."#
    ));
}

#[test]
fn graph_obligor_equals_beneficiary() {
    // Edge case: circular reference
    insta::assert_snapshot!(test_graph(
        r#"ABC Corp (the "Company") shall pay amounts to the Company's subsidiaries."#
    ));
}

#[test]
fn graph_for_beneficiary_pattern() {
    // Documents the "for" pattern gap (will pass after fix #3)
    insta::assert_snapshot!(test_graph(
        r#"ABC Corp (the "Contractor") shall perform services for the Client."#
    ));
}

#[test]
fn graph_infinitive_to_not_beneficiary() {
    // "to perform" is infinitive, not a beneficiary
    insta::assert_snapshot!(test_graph(
        r#"ABC Corp (the "Company") shall be required to perform diligently."#
    ));
}
```

---

## Low Priority (Future Design)

### 5. Condition Party Extraction

**Problem:** Condition text often contains party references that aren't structured:

```rust
condition: ClauseCondition {
    text: "the Buyer provides written notice",
    mentions_unknown_entity: true,
}
```

You can't query "what conditions involve Buyer" without string matching.

**Future design:**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionPartyLink {
    pub display_text: String,
    pub chain_id: Option<u32>,
    pub has_verified_chain: bool,
    pub role: ConditionPartyRole,
    pub source_clause_id: u32,
    pub source_condition_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionPartyRole {
    Subject,     // party whose action triggers condition ("if Buyer pays")
    Beneficiary, // future role
}
```

Add `condition_party_links: Vec<ConditionPartyLink>` to `ObligationNode`.

**Effort:** Medium (1-2 days)

---

### 6. Edge-Based Graph Structure

**Current structure:**

```rust
ObligationNode {
    obligor: ClauseParty,
    beneficiaries: Vec<BeneficiaryLink>,
    condition_links: Vec<ConditionLink>,
    clauses: Vec<ClauseAggregateEntry>,
}
```

This is node-centric. For graph queries like "shortest path from Seller to Buyer obligations," an explicit edge model works better.

**Future design:**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ObligationEdge {
    pub from_node_id: u32,          // obligor node
    pub to_chain_id: Option<u32>,   // beneficiary party
    pub display_text: String,
    pub source_clause_id: u32,
    pub needs_verification: bool,
    pub has_verified_chain: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConditionEdge {
    pub from_node_id: u32,
    pub source_clause_id: u32,
    pub condition: ClauseCondition,
}

pub struct AccountabilityGraph {
    pub nodes: Vec<ObligationNode>,
    pub obligation_edges: Vec<ObligationEdge>,
    pub condition_edges: Vec<ConditionEdge>,
}
```

**Effort:** Large (2-3 days)

---

## Risks & Guardrails

### Over-filtering parties as structural references

**Risk:** Phrases like `"deliver to Closing Agent"` could be misinterpreted if `"Closing"` is in the blocklist.

**Guardrail:** The filter checks the **first word** after normalization. `"Closing Agent"` normalizes to `"closing agent"`, first word `"closing"` is blocked. If this becomes a real issue, refine to only block when the *entire* normalized candidate matches (e.g., just `"closing"` or `"the closing"`).

### More chains → more memory

Emitting single-mention defined-term chains slightly increases chain count. Acceptable for typical contract sizes; monitor in large documents.

### More "for" beneficiaries → more penalties

If data has unusual capitalized `"for X"` phrases without chains, you'll see more `needs_verification` penalties. This is intentional (surfaces unknown entities). Tune `unresolved_beneficiary_penalty` via `with_settings()` if needed.

---

## Implementation Checklist

- [ ] **High #1:** Add `NON_PARTY_HEADS` blocklist + `is_structural_or_timing_reference()` in `accountability_graph.rs`
- [ ] **High #2:** Relax ≥2 mention rule for defined terms in `pronoun_chain.rs`
- [ ] **Medium #3:** Add `"for"` pattern to `extract_beneficiary_candidates()`
- [ ] **Medium #4:** Add test coverage for edge cases
- [ ] **Low #5:** Design condition party extraction (future)
- [ ] **Low #6:** Design edge-based graph structure (future)

---

## References

- [PLAN-CONTRACT-LANGUAGE.md](../../PLAN-CONTRACT-LANGUAGE.md) — Overall architecture and layer summaries
- [accountability_graph.rs](../src/accountability_graph.rs) — Current Layer 9 implementation
- [pronoun_chain.rs](../src/pronoun_chain.rs) — Layer 6 chain building

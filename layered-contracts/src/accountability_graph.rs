//! Accountability graph construction (Layer 9).
//!
//! This resolver lifts `ClauseAggregate` records into `ObligationNode`s that
//! capture obligor → beneficiary relationships plus condition edges so
//! downstream systems can run accountability queries (e.g., "list everything
//! the Seller owes the Buyer under Section 5").

use std::collections::HashMap;

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};

use crate::clause_aggregate::{ClauseAggregate, ClauseAggregateEntry};
use crate::contract_clause::{ClauseCondition, ClauseParty};
use crate::obligation_linker::LinkedObligation;
use crate::pronoun_chain::PronounChain;
use crate::Scored;
use crate::utils::normalize_party_name;
use crate::verification::VerificationNote;

/// A link to a beneficiary (the party receiving the obligation/performance).
#[derive(Debug, Clone, PartialEq)]
pub struct BeneficiaryLink {
    /// Surface text captured from the clause (e.g., "Buyer").
    pub display_text: String,
    /// Pronoun chain ID if we resolved this beneficiary to a known entity.
    pub chain_id: Option<u32>,
    /// Whether the linked chain already contains a verified mention.
    pub has_verified_chain: bool,
    /// True when this link needs verification (unresolved party reference).
    pub needs_verification: bool,
    /// Clause ID the beneficiary link was extracted from.
    pub source_clause_id: u32,
    /// Confidence score for this beneficiary link (0.0-1.0).
    pub confidence: f64,
    /// Whether this beneficiary link needs human review.
    pub needs_review: bool,
    /// Reason for review if applicable.
    pub review_reason: Option<String>,
}

/// Edge capturing conditional relationships (Section references, If/Unless, etc.).
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionLink {
    /// Clause that generated this condition.
    pub source_clause_id: u32,
    /// Normalized condition metadata from Layer 7.
    pub condition: ClauseCondition,
}

/// Node describing all duties for a single obligor aggregate.
#[derive(Debug, Clone, PartialEq)]
pub struct ObligationNode {
    /// Deterministic identifier derived from the aggregate ID.
    pub node_id: u32,
    /// Underlying aggregate ID (from Layer 8).
    pub aggregate_id: u32,
    /// Copy of the obligor metadata.
    pub obligor: ClauseParty,
    /// Beneficiaries detected within this aggregate's clauses.
    pub beneficiaries: Vec<BeneficiaryLink>,
    /// Condition links derived from clause conditions.
    pub condition_links: Vec<ConditionLink>,
    /// Clause entries (duties + conditions) backing this node.
    pub clauses: Vec<ClauseAggregateEntry>,
    /// Reviewer-provided verification notes.
    pub verification_notes: Vec<VerificationNote>,
    /// Explanation of how the node confidence was derived.
    pub confidence_breakdown: Vec<String>,
    /// Overall confidence for the node (from LinkedObligation if available).
    pub node_confidence: f64,
    /// Whether the obligor needs human review.
    pub obligor_needs_review: bool,
    /// Reason for obligor review if applicable.
    pub obligor_review_reason: Option<String>,
}

/// Resolver that converts `ClauseAggregate` records into `ObligationNode`s.
///
/// Requires that the following resolvers have already run:
/// - Layers 1–8 (through `ClauseAggregationResolver`)
/// - `PronounChainResolver` (used for beneficiary linkage)
pub struct AccountabilityGraphResolver {
    /// Penalty when no pronoun chain backs a detected beneficiary.
    unresolved_beneficiary_penalty: f64,
    /// Bonus when at least one beneficiary maps to a verified chain.
    verified_beneficiary_bonus: f64,
}

impl Default for AccountabilityGraphResolver {
    fn default() -> Self {
        Self {
            unresolved_beneficiary_penalty: 0.10,
            verified_beneficiary_bonus: 0.05,
        }
    }
}

impl AccountabilityGraphResolver {
    /// Create a resolver with default heuristics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a resolver with custom heuristics.
    pub fn with_settings(
        unresolved_beneficiary_penalty: f64,
        verified_beneficiary_bonus: f64,
    ) -> Self {
        Self {
            unresolved_beneficiary_penalty,
            verified_beneficiary_bonus,
        }
    }

    fn extract_beneficiary_candidates(action: &str) -> Vec<String> {
        let mut candidates = Vec::new();
        let lower = action.to_lowercase();
        let mut cursor = 0;

        while let Some(rel_idx) = lower[cursor..].find(" to ") {
            let start = cursor + rel_idx + 4;
            cursor = start;
            if start >= action.len() {
                break;
            }
            let mut candidate = action[start..].trim_start();
            if candidate.is_empty() {
                continue;
            }

            let mut end = candidate.len();
            for delimiter in &[",", ";", ".", ":"] {
                if let Some(idx) = candidate.find(delimiter) {
                    end = end.min(idx);
                }
            }

            let lowercase_candidate = candidate.to_lowercase();
            for keyword in &[" and ", " or "] {
                if let Some(idx) = lowercase_candidate.find(keyword) {
                    end = end.min(idx);
                }
            }

            candidate = candidate[..end].trim();
            if candidate.is_empty() {
                continue;
            }
            candidates.push(candidate.to_string());
        }

        candidates
    }

    fn looks_like_entity(candidate: &str) -> bool {
        let trimmed = candidate
            .trim_matches(|c: char| matches!(c, '"' | '\'' | '(' | ')'))
            .trim();
        if trimmed.is_empty() {
            return false;
        }
        trimmed
            .split_whitespace()
            .next()
            .and_then(|word| word.chars().next())
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
    }

    fn detect_beneficiaries(
        &self,
        entry: &ClauseAggregateEntry,
        chain_map: &HashMap<String, ChainInfo>,
    ) -> Vec<BeneficiaryLink> {
        let mut results = Vec::new();
        for candidate in Self::extract_beneficiary_candidates(&entry.duty.action) {
            let normalized = normalize_party_name(&candidate);
            if let Some(chain) = chain_map.get(&normalized) {
                // Legacy path: regex extraction with pronoun chain linkage
                // Higher confidence when we can link to a verified chain
                let confidence = if chain.has_verified { 0.7 } else { 0.6 };
                results.push(BeneficiaryLink {
                    display_text: candidate.clone(),
                    chain_id: Some(chain.chain_id),
                    has_verified_chain: chain.has_verified,
                    needs_verification: false,
                    source_clause_id: entry.clause_id,
                    confidence,
                    needs_review: false,
                    review_reason: None,
                });
            } else if Self::looks_like_entity(&candidate) {
                // Legacy path: unlinked regex extraction
                // Lower confidence for unresolved party references
                results.push(BeneficiaryLink {
                    display_text: candidate.clone(),
                    chain_id: None,
                    has_verified_chain: false,
                    needs_verification: true,
                    source_clause_id: entry.clause_id,
                    confidence: 0.5,
                    needs_review: true,
                    review_reason: Some(format!(
                        "Beneficiary '{}' not linked to pronoun chain",
                        candidate
                    )),
                });
            }
        }

        results
    }

    fn build_condition_links(entry: &ClauseAggregateEntry) -> Vec<ConditionLink> {
        entry
            .conditions
            .iter()
            .map(|condition| ConditionLink {
                source_clause_id: entry.clause_id,
                condition: condition.clone(),
            })
            .collect()
    }

    fn calculate_confidence(
        &self,
        aggregate_confidence: f64,
        beneficiaries: &[BeneficiaryLink],
    ) -> (f64, Vec<String>) {
        let mut confidence = aggregate_confidence;
        let mut breakdown = vec![format!("Layer8 aggregate: {:.2}", aggregate_confidence)];
        if beneficiaries.iter().any(|b| b.needs_verification) {
            confidence -= self.unresolved_beneficiary_penalty;
            breakdown.push(format!(
                "Unresolved beneficiary penalty: -{:.2}",
                self.unresolved_beneficiary_penalty
            ));
        }
        if beneficiaries.iter().any(|b| b.has_verified_chain) {
            confidence += self.verified_beneficiary_bonus;
            breakdown.push(format!(
                "Verified beneficiary bonus: +{:.2}",
                self.verified_beneficiary_bonus
            ));
        }
        let final_conf = confidence.clamp(0.0, 1.0);
        breakdown.push(format!("Layer9 result: {:.2}", final_conf));
        (final_conf, breakdown)
    }
}

impl Resolver for AccountabilityGraphResolver {
    type Attr = Scored<ObligationNode>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        // Build pronoun chain map for legacy beneficiary detection
        let mut chain_map = HashMap::new();
        for (_, chain) in selection
            .find_by(&x::attr::<Scored<PronounChain>>())
            .into_iter()
        {
            chain_map.insert(
                normalize_party_name(&chain.value.canonical_name),
                ChainInfo {
                    chain_id: chain.value.chain_id,
                    has_verified: chain.value.has_verified_mention,
                },
            );
        }

        // Primary path: Try LinkedObligation first (Gate 3 output)
        let linked_obligations: Vec<_> = selection
            .find_by(&x::attr::<LinkedObligation>())
            .into_iter()
            .collect();

        if !linked_obligations.is_empty() {
            return self.build_from_linked_obligations(linked_obligations, &chain_map);
        }

        // Legacy fallback: Use ClauseAggregate when LinkedObligation not available
        self.build_from_aggregates(selection, &chain_map)
    }
}

impl AccountabilityGraphResolver {
    /// Build ObligationNodes from LinkedObligation (primary path).
    ///
    /// Uses LinkedObligation for party extraction with higher confidence
    /// than the legacy regex-based approach.
    fn build_from_linked_obligations(
        &self,
        linked_obligations: Vec<(LLSelection, &LinkedObligation)>,
        chain_map: &HashMap<String, ChainInfo>,
    ) -> Vec<LLCursorAssignment<Scored<ObligationNode>>> {
        let mut results = Vec::new();
        let mut node_id = 1u32;

        for (sel, linked) in linked_obligations {
            // Extract obligor from LinkedObligation
            let obligor_participant = &linked.obligor.ambiguous.best.value;
            let obligor_text = obligor_participant.resolved_entity_text();
            let normalized = normalize_party_name(obligor_text);
            let chain_info = chain_map.get(&normalized);

            let obligor = ClauseParty {
                display_text: obligor_text.to_string(),
                chain_id: chain_info.map(|c| c.chain_id),
                has_verified_chain: chain_info.map(|c| c.has_verified).unwrap_or(false),
                confidence: obligor_participant.confidence,
                needs_review: obligor_participant.needs_review,
                review_reason: obligor_participant.review_reason.clone(),
            };

            // Extract beneficiary from LinkedObligation if present
            let mut beneficiaries = Vec::new();
            if let Some(ref beneficiary_result) = linked.beneficiary {
                let beneficiary_participant = &beneficiary_result.ambiguous.best.value;
                let normalized = normalize_party_name(beneficiary_participant.resolved_entity_text());
                let chain_info = chain_map.get(&normalized);

                beneficiaries.push(BeneficiaryLink {
                    display_text: beneficiary_participant.text.clone(),
                    chain_id: chain_info.map(|c| c.chain_id),
                    has_verified_chain: chain_info.map(|c| c.has_verified).unwrap_or(false),
                    needs_verification: beneficiary_result.needs_review,
                    source_clause_id: 0, // LinkedObligation doesn't track clause ID
                    confidence: beneficiary_result.confidence(),
                    needs_review: beneficiary_result.needs_review,
                    review_reason: beneficiary_result.review_reason.clone(),
                });
            }

            // Build confidence breakdown
            let mut breakdown = vec![
                format!("LinkedObligation overall: {:.2}", linked.overall_confidence),
                format!("Obligor confidence: {:.2}", linked.obligor.confidence()),
            ];
            if let Some(ref ben) = linked.beneficiary {
                breakdown.push(format!("Beneficiary confidence: {:.2}", ben.confidence()));
            }

            let node = ObligationNode {
                node_id,
                aggregate_id: node_id, // No aggregate ID from LinkedObligation path
                obligor,
                beneficiaries,
                condition_links: Vec::new(), // Conditions not tracked in LinkedObligation
                clauses: Vec::new(),         // No clause entries from LinkedObligation path
                verification_notes: Vec::new(),
                confidence_breakdown: breakdown,
                node_confidence: linked.overall_confidence,
                obligor_needs_review: linked.obligor.needs_review,
                obligor_review_reason: linked.obligor.review_reason.clone(),
            };

            results.push(sel.finish_with_attr(Scored::derived(node, linked.overall_confidence)));
            node_id += 1;
        }

        results
    }

    /// Build ObligationNodes from ClauseAggregate (legacy fallback).
    ///
    /// Uses regex-based beneficiary extraction with lower confidence.
    /// This path is maintained for backward compatibility during migration.
    fn build_from_aggregates(
        &self,
        selection: LLSelection,
        chain_map: &HashMap<String, ChainInfo>,
    ) -> Vec<LLCursorAssignment<Scored<ObligationNode>>> {
        let mut aggregates: Vec<_> = selection
            .find_by(&x::attr::<Scored<ClauseAggregate>>())
            .into_iter()
            .collect();

        aggregates.sort_by_key(|(_, agg)| agg.value.source_start);

        let mut results = Vec::new();

        for (sel, aggregate) in aggregates {
            let mut beneficiaries = Vec::new();
            let mut condition_links = Vec::new();

            for entry in &aggregate.value.clauses {
                beneficiaries.extend(self.detect_beneficiaries(entry, chain_map));
                condition_links.extend(Self::build_condition_links(entry));
            }

            // Deduplicate beneficiaries by (clause_id, normalized name) to avoid repeated extractions.
            let mut seen = HashMap::new();
            beneficiaries.retain(|link| {
                let key = (
                    link.source_clause_id,
                    normalize_party_name(&link.display_text),
                );
                seen.insert(key, true).is_none()
            });

            let (confidence, breakdown) =
                self.calculate_confidence(aggregate.confidence, &beneficiaries);

            let node = ObligationNode {
                node_id: aggregate.value.aggregate_id,
                aggregate_id: aggregate.value.aggregate_id,
                obligor: aggregate.value.obligor.clone(),
                beneficiaries,
                condition_links,
                clauses: aggregate.value.clauses.clone(),
                verification_notes: Vec::new(),
                confidence_breakdown: breakdown,
                // Legacy path: use calculated confidence, obligor not reviewed
                node_confidence: confidence,
                obligor_needs_review: false,
                obligor_review_reason: None,
            };

            results.push(sel.finish_with_attr(Scored::derived(node, confidence)));
        }

        results
    }
}

struct ChainInfo {
    chain_id: u32,
    has_verified: bool,
}

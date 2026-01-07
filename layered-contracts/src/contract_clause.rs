//! Contract clause aggregation over obligation phrases.
//!
//! Layer 7 lifts [`ObligationPhrase`] outputs into clause-level summaries with
//! party metadata, normalized conditions, and confidence adjustments so
//! downstream systems can reason over "who owes what" at a clause granularity.

use std::cmp::Ordering;
use std::collections::HashSet;

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};

use crate::contract_keyword::ContractKeyword;
use crate::obligation::{ConditionRef, ObligationPhrase, ObligationType, ObligorReference};
use crate::pronoun_chain::PronounChain;
use crate::Scored;
use crate::utils::normalize_party_name;

const COMMON_CAPITALIZED_ALLOWLIST: &[&str] = &[
    "agreement",
    "effective",
    "date",
    "section",
    "article",
    "schedule",
    "exhibit",
];

/// A party participating in a clause-level obligation.
#[derive(Debug, Clone, PartialEq)]
pub struct ClauseParty {
    /// Display text for the party (defined term, resolved pronoun, or noun phrase).
    pub display_text: String,
    /// Identifier of the pronoun chain backing this party, if resolved.
    ///
    /// Chain IDs only appear when `PronounChainResolver` produced a chain
    /// (there must be at least two mentions), so single-mention parties will
    /// leave this as `None`.
    pub chain_id: Option<u32>,
    /// Whether the underlying chain already contains a verified mention.
    pub has_verified_chain: bool,
    /// Confidence score for this party identification (0.0-1.0).
    ///
    /// - 1.0: Direct term reference (highest confidence)
    /// - 0.9: Pronoun with verified chain
    /// - 0.7: Pronoun without verified chain
    /// - 0.5: Noun phrase not linked to defined term (lowest confidence)
    pub confidence: f64,
    /// Whether this party identification needs human review.
    pub needs_review: bool,
    /// Reason for review if applicable.
    pub review_reason: Option<String>,
}

/// The duty extracted from an obligation phrase.
#[derive(Debug, Clone, PartialEq)]
pub struct ClauseDuty {
    /// Whether this is a duty, permission, or prohibition.
    pub obligation_type: ObligationType,
    /// Plain-text action captured from the obligation phrase.
    pub action: String,
}

/// Normalized representation of a clause condition.
#[derive(Debug, Clone, PartialEq)]
pub struct ClauseCondition {
    /// The triggering keyword (If/Unless/Provided/SubjectTo).
    pub condition_type: ContractKeyword,
    /// Text preview captured by the lower layer.
    pub text: String,
    /// Flagged when the condition text references an unknown capitalized entity.
    pub mentions_unknown_entity: bool,
}

/// Clause-level aggregation over a single [`ObligationPhrase`].
#[derive(Debug, Clone, PartialEq)]
pub struct ContractClause {
    /// Deterministic identifier derived from the obligation's token offset.
    pub clause_id: u32,
    /// Token offset used for deterministic ordering + provenance.
    pub source_offset: usize,
    /// Party responsible for the duty.
    pub obligor: ClauseParty,
    /// Duty metadata (obligation type + action text).
    pub duty: ClauseDuty,
    /// Conditions attached to this clause.
    pub conditions: Vec<ClauseCondition>,
}

/// Aggregates [`ObligationPhrase`] outputs into [`ContractClause`] structs.
///
/// Requires that the following resolvers have already been run:
/// - `ContractKeywordResolver` + `ProhibitionResolver`
/// - `DefinedTermResolver`
/// - `TermReferenceResolver`
/// - `PronounResolver`
/// - `ObligationPhraseResolver`
/// - `PronounChainResolver`
pub struct ContractClauseResolver {
    /// Bonus when the obligor maps to a pronoun chain containing a verified mention.
    verified_party_bonus: f64,
    /// Penalty when the obligation action is empty/whitespace.
    missing_action_penalty: f64,
    /// Penalty when conditions reference unknown/capitalized entities.
    undefined_condition_penalty: f64,
}

impl Default for ContractClauseResolver {
    fn default() -> Self {
        Self {
            verified_party_bonus: 0.05,
            missing_action_penalty: 0.10,
            undefined_condition_penalty: 0.15,
        }
    }
}

impl ContractClauseResolver {
    /// Create a resolver with default heuristics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a resolver with custom heuristics.
    pub fn with_settings(
        verified_party_bonus: f64,
        missing_action_penalty: f64,
        undefined_condition_penalty: f64,
    ) -> Self {
        Self {
            verified_party_bonus,
            missing_action_penalty,
            undefined_condition_penalty,
        }
    }

    /// Estimate a deterministic offset for a selection by counting tokens.
    fn estimate_offset(&self, selection: &LLSelection) -> usize {
        let mut count = 0;
        let mut current = selection.clone();

        while let Some((prev_sel, _)) = current.match_first_backwards(&x::token_text()) {
            count += 1;
            current = prev_sel;
        }

        count
    }

    fn best_chain_match<'a>(
        &self,
        chains: &'a [Scored<PronounChain>],
        name: &str,
    ) -> Option<&'a PronounChain> {
        let target = normalize_party_name(name);
        chains
            .iter()
            .filter(|chain| normalize_party_name(&chain.value.canonical_name) == target)
            .max_by(|a, b| {
                a.confidence
                    .partial_cmp(&b.confidence)
                    .unwrap_or(Ordering::Equal)
            })
            .map(|chain| &chain.value)
    }

    fn build_clause_party(
        &self,
        obligor: &ObligorReference,
        chains: &[Scored<PronounChain>],
    ) -> ClauseParty {
        match obligor {
            ObligorReference::TermRef { term_name, .. } => {
                let chain = self.best_chain_match(chains, term_name);
                ClauseParty {
                    display_text: term_name.clone(),
                    chain_id: chain.map(|c| c.chain_id),
                    has_verified_chain: chain.map(|c| c.has_verified_mention).unwrap_or(false),
                    confidence: 1.0,
                    needs_review: false,
                    review_reason: None,
                }
            }
            ObligorReference::PronounRef {
                resolved_to, ..
            } => {
                let chain = self.best_chain_match(chains, resolved_to);
                let has_verified = chain.map(|c| c.has_verified_mention).unwrap_or(false);
                let (confidence, needs_review, review_reason) = if has_verified {
                    (0.9, false, None)
                } else {
                    (0.7, true, Some("Pronoun chain unverified".to_string()))
                };
                ClauseParty {
                    display_text: resolved_to.clone(),
                    chain_id: chain.map(|c| c.chain_id),
                    has_verified_chain: has_verified,
                    confidence,
                    needs_review,
                    review_reason,
                }
            }
            ObligorReference::NounPhrase { text } => {
                let chain = self.best_chain_match(chains, text);
                ClauseParty {
                    display_text: text.clone(),
                    chain_id: chain.map(|c| c.chain_id),
                    has_verified_chain: chain.map(|c| c.has_verified_mention).unwrap_or(false),
                    confidence: 0.5,
                    needs_review: true,
                    review_reason: Some("Not linked to defined term".to_string()),
                }
            }
        }
    }

    fn known_entity_names(&self, chains: &[Scored<PronounChain>]) -> HashSet<String> {
        chains
            .iter()
            .map(|chain| normalize_party_name(&chain.value.canonical_name))
            .collect()
    }

    fn condition_mentions_unknown_entity(
        &self,
        text: &str,
        known_entities: &HashSet<String>,
    ) -> bool {
        let lower = text.to_lowercase();
        if known_entities.iter().any(|entity| lower.contains(entity)) {
            return false;
        }

        self.capitalized_tokens(text)
            .into_iter()
            .any(|token| !COMMON_CAPITALIZED_ALLOWLIST.contains(&token.as_str()))
    }

    fn capitalized_tokens(&self, text: &str) -> Vec<String> {
        text.split_whitespace()
            .filter_map(|token| {
                let trimmed = token.trim_matches(|c: char| !c.is_alphanumeric());
                if trimmed.is_empty() {
                    return None;
                }
                let mut chars = trimmed.chars();
                if chars.next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    Some(trimmed.to_lowercase())
                } else {
                    None
                }
            })
            .collect()
    }

    fn convert_conditions(
        &self,
        conditions: &[ConditionRef],
        known_entities: &HashSet<String>,
    ) -> Vec<ClauseCondition> {
        conditions
            .iter()
            .map(|cond| ClauseCondition {
                condition_type: cond.condition_type.clone(),
                text: cond.text_preview.clone(),
                mentions_unknown_entity: self
                    .condition_mentions_unknown_entity(&cond.text_preview, known_entities),
            })
            .collect()
    }

    fn calculate_confidence(
        &self,
        base_confidence: f64,
        party: &ClauseParty,
        duty: &ClauseDuty,
        conditions: &[ClauseCondition],
    ) -> f64 {
        let mut confidence = base_confidence;

        if party.chain_id.is_some() && party.has_verified_chain {
            confidence += self.verified_party_bonus;
        }

        if duty.action.trim().is_empty() {
            confidence -= self.missing_action_penalty;
        }

        if conditions.iter().any(|cond| cond.mentions_unknown_entity) {
            confidence -= self.undefined_condition_penalty;
        }

        confidence.clamp(0.0, 1.0)
    }
}

impl Resolver for ContractClauseResolver {
    type Attr = Scored<ContractClause>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut assignments = Vec::new();

        let chains: Vec<Scored<PronounChain>> = selection
            .find_by(&x::attr::<Scored<PronounChain>>())
            .into_iter()
            .map(|(_, chain)| chain.clone())
            .collect();

        let known_entities = self.known_entity_names(&chains);

        let mut obligations: Vec<(LLSelection, Scored<ObligationPhrase>, usize)> = selection
            .find_by(&x::attr::<Scored<ObligationPhrase>>())
            .into_iter()
            .map(|(sel, obligation)| {
                let offset = self.estimate_offset(&sel);
                (sel, obligation.clone(), offset)
            })
            .collect();

        obligations.sort_by_key(|(_, _, offset)| *offset);

        for (sel, scored_obligation, offset) in obligations {
            let party = self.build_clause_party(&scored_obligation.value.obligor, &chains);
            let duty = ClauseDuty {
                obligation_type: scored_obligation.value.obligation_type,
                action: scored_obligation.value.action.clone(),
            };
            let conditions = self.convert_conditions(
                &scored_obligation.value.conditions,
                &known_entities,
            );

            let clause = ContractClause {
                clause_id: offset as u32,
                source_offset: offset,
                obligor: party,
                duty,
                conditions,
            };

            let confidence = self.calculate_confidence(
                scored_obligation.confidence,
                &clause.obligor,
                &clause.duty,
                &clause.conditions,
            );

            assignments.push(sel.finish_with_attr(Scored::derived(clause, confidence)));
        }

        assignments
    }
}

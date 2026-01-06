//! Obligation Party Linker for contract obligations.
//!
//! Gate 5: Links obligations to their parties (obligor and beneficiary) by
//! extracting party information from obligation phrases and pronoun chains.
//!
//! # Architecture
//!
//! The linker processes Gate 4 output (ScopedObligation) and produces:
//! - Obligor: The party with the duty (from ObligorReference)
//! - Beneficiary: The party receiving benefit (optional, extracted from action/conditions)
//!
//! # Passive Voice Detection
//!
//! Detects patterns like "shall be delivered by X" where X is the actual obligor.
//! Pattern: `{modal} be {past participle} by {agent}`
//!
//! # Review Flagging
//!
//! Results are flagged for human review when:
//! - Obligor is implicit (passive voice without "by" agent)
//! - Pronoun resolution has low confidence
//! - Passive voice penalty applied
//!
//! # Example
//!
//! ```ignore
//! use layered_contracts::{
//!     ObligationPartyLinker, ScopedObligation, PronounChainResult,
//! };
//!
//! let linker = ObligationPartyLinker::new();
//! let chains: Vec<PronounChainResult> = vec![];
//!
//! let linked = linker.link(scoped_obligation, &chains);
//!
//! if linked.obligor.needs_review {
//!     println!("Review needed: {:?}", linked.obligor.review_reason);
//! }
//! ```

use layered_nlp_document::{compose_confidence, DocSpan, ReviewableResult};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{ObligorReference, PronounChainResult, ScopedObligation};

// ============================================================================
// ClauseParticipant - Local definition to avoid circular dependency
// ============================================================================

/// Role of a participant within a clause.
///
/// This is a local copy of the type from layered-clauses to avoid
/// circular dependencies between layered-contracts and layered-clauses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParticipantRole {
    /// The subject/obligee of the clause - entity with duty or permission
    Subject,
    /// The object/deliverable of the clause - entity being acted upon
    Object,
    /// The party imposing the obligation (when distinct from subject)
    Obligor,
    /// Indirect object or beneficiary of the action
    IndirectObject,
}

/// A participant mention within a clause.
///
/// This is a local copy of the type from layered-clauses to avoid
/// circular dependencies between layered-contracts and layered-clauses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClauseParticipant {
    /// The span of this participant mention in the document (None for implicit participants)
    pub span: Option<DocSpan>,
    /// The text of this participant
    pub text: String,
    /// Role of this participant in the clause
    pub role: ParticipantRole,
    /// If this is a pronoun, the span it resolves to (antecedent)
    pub resolved_to: Option<DocSpan>,
    /// Text of what this resolves to
    pub resolved_text: Option<String>,
    /// Whether this is a pronoun that was resolved
    pub is_pronoun: bool,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Whether this needs human review
    pub needs_review: bool,
    /// Reason for review if applicable
    pub review_reason: Option<String>,
}

impl ClauseParticipant {
    /// Get the final resolved text (either direct text or resolved referent)
    pub fn resolved_entity_text(&self) -> &str {
        self.resolved_text.as_deref().unwrap_or(&self.text)
    }
}

// ============================================================================
// LinkedObligation - The main output type
// ============================================================================

/// A fully linked obligation with identified parties.
///
/// This is the output of Gate 5, combining:
/// - ScopedObligation (from Gate 4)
/// - Obligor ClauseParticipant (extracted and linked)
/// - Beneficiary ClauseParticipant (optional)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedObligation {
    /// The scoped obligation from Gate 4
    pub obligation: ScopedObligation,

    /// The party with the duty (obligor)
    pub obligor: ReviewableResult<ClauseParticipant>,

    /// The party receiving benefit (may be None for some obligations)
    pub beneficiary: Option<ReviewableResult<ClauseParticipant>>,

    /// Overall confidence from all components
    pub overall_confidence: f64,
}

impl LinkedObligation {
    /// Check if this linked obligation needs any human review.
    pub fn needs_review(&self) -> bool {
        self.obligor.needs_review
            || self
                .beneficiary
                .as_ref()
                .map(|b| b.needs_review)
                .unwrap_or(false)
    }

    /// Get all review reasons.
    pub fn review_reasons(&self) -> Vec<&str> {
        let mut reasons = Vec::new();
        if let Some(reason) = &self.obligor.review_reason {
            reasons.push(reason.as_str());
        }
        if let Some(beneficiary) = &self.beneficiary {
            if let Some(reason) = &beneficiary.review_reason {
                reasons.push(reason.as_str());
            }
        }
        reasons
    }

    /// Get the obligor's display name (resolved text if pronoun, otherwise text).
    pub fn obligor_name(&self) -> &str {
        self.obligor
            .ambiguous
            .best
            .value
            .resolved_entity_text()
    }

    /// Get the beneficiary's display name if present.
    pub fn beneficiary_name(&self) -> Option<&str> {
        self.beneficiary
            .as_ref()
            .map(|b| b.ambiguous.best.value.resolved_entity_text())
    }
}

// ============================================================================
// ObligationPartyLinkerConfig
// ============================================================================

/// Configuration for the ObligationPartyLinker.
#[derive(Debug, Clone)]
pub struct ObligationPartyLinkerConfig {
    /// Threshold below which results are flagged for review (default 0.6)
    pub review_threshold: f64,
    /// Penalty for passive voice obligor detection (default 0.2)
    pub passive_voice_penalty: f64,
    /// Base confidence for noun phrase obligors (default 0.7)
    pub noun_phrase_confidence: f64,
    /// Confidence for implicit obligors (passive without "by") (default 0.4)
    pub implicit_obligor_confidence: f64,
}

impl Default for ObligationPartyLinkerConfig {
    fn default() -> Self {
        Self {
            review_threshold: 0.6,
            passive_voice_penalty: 0.2,
            noun_phrase_confidence: 0.7,
            implicit_obligor_confidence: 0.4,
        }
    }
}

// ============================================================================
// ObligationPartyLinker
// ============================================================================

/// Links obligations to their parties (obligor and beneficiary).
///
/// Processes Gate 4 output (ScopedObligation) and extracts:
/// - Obligor from ObligorReference
/// - Beneficiary from action text patterns
#[derive(Debug, Clone)]
pub struct ObligationPartyLinker {
    config: ObligationPartyLinkerConfig,
    /// Compiled regex for passive voice detection
    passive_voice_regex: Regex,
    /// Compiled regex for beneficiary "to X" pattern
    beneficiary_to_regex: Regex,
}

impl ObligationPartyLinker {
    /// Create a new linker with default configuration.
    pub fn new() -> Self {
        Self::with_config(ObligationPartyLinkerConfig::default())
    }

    /// Create a linker with custom configuration.
    pub fn with_config(config: ObligationPartyLinkerConfig) -> Self {
        // Pattern: "{auxiliary verb} {past participle} by {agent}"
        // Matches "be/is/are/was/were/been X by Y" where X is any word (handles irregular participles)
        // Captures the agent after "by" - capitalized words (proper nouns for party names)
        // NOTE: Use (?-i:...) to turn off case-insensitivity for the party name capture group.
        // Without this, (?i) makes [A-Z] match lowercase letters too, causing over-matching
        // (e.g., "Smith and Jones" would incorrectly capture "and" as part of the name)
        let passive_voice_regex =
            Regex::new(r"(?i)(?:\b(?:be|is|are|was|were|been)\s+)(\w+)\s+by\s+(?:the\s+)?(?-i:([A-Z][a-zA-Z0-9_]*(?:\s+[A-Z][a-zA-Z0-9_]*)*))")
                .expect("Invalid passive voice regex");

        // Pattern: "to {beneficiary}" - captures capitalized entity after "to"
        let beneficiary_to_regex =
            Regex::new(r"(?i)\bto\s+(?:the\s+)?([A-Z][a-z]+(?:\s+[A-Z][a-z]+)?)")
                .expect("Invalid beneficiary regex");

        Self {
            config,
            passive_voice_regex,
            beneficiary_to_regex,
        }
    }

    /// Link an obligation to its parties.
    ///
    /// Extracts obligor from the ObligorReference and detects beneficiary
    /// from the action text.
    pub fn link(
        &self,
        scoped_obligation: ScopedObligation,
        chains: &[PronounChainResult],
    ) -> LinkedObligation {
        let obligor_ref = &scoped_obligation.obligation.value.obligor;
        let action = &scoped_obligation.obligation.value.action;

        // Try passive voice detection first
        let passive_obligor = self.detect_passive_voice_obligor(action);

        // Extract obligor (prefer passive voice if found)
        let obligor = if let Some(passive) = passive_obligor {
            passive
        } else {
            self.extract_obligor(obligor_ref, chains)
        };

        // Extract beneficiary from action text
        let beneficiary = self.extract_beneficiary(&scoped_obligation.obligation.value, chains);

        // Calculate overall confidence
        let obligor_confidence = obligor.confidence();
        let beneficiary_confidence = beneficiary
            .as_ref()
            .map(|b| b.confidence())
            .unwrap_or(1.0);

        let overall_confidence = compose_confidence(&[
            scoped_obligation.overall_confidence,
            obligor_confidence,
            beneficiary_confidence,
        ]);

        LinkedObligation {
            obligation: scoped_obligation,
            obligor,
            beneficiary,
            overall_confidence,
        }
    }

    /// Extract obligor ClauseParticipant from ObligorReference.
    fn extract_obligor(
        &self,
        obligor_ref: &ObligorReference,
        chains: &[PronounChainResult],
    ) -> ReviewableResult<ClauseParticipant> {
        match obligor_ref {
            ObligorReference::TermRef {
                term_name,
                confidence,
            } => {
                // Direct defined term reference
                let participant = ClauseParticipant {
                    span: None, // No span in ObligorReference
                    text: term_name.clone(),
                    role: ParticipantRole::Subject,
                    resolved_to: None,
                    resolved_text: None,
                    is_pronoun: false,
                    confidence: *confidence,
                    needs_review: *confidence < self.config.review_threshold,
                    review_reason: if *confidence < self.config.review_threshold {
                        Some(format!(
                            "Low term reference confidence ({:.2})",
                            confidence
                        ))
                    } else {
                        None
                    },
                };

                if *confidence >= self.config.review_threshold {
                    ReviewableResult::certain(participant)
                } else {
                    ReviewableResult::uncertain(
                        participant,
                        vec![],
                        format!("Low term reference confidence ({:.2})", confidence),
                    )
                }
            }

            ObligorReference::PronounRef {
                pronoun,
                resolved_to,
                is_defined_term,
                confidence,
            } => {
                // Look up chain for additional context
                let chain = chains
                    .iter()
                    .find(|c| c.canonical_name.to_lowercase() == resolved_to.to_lowercase());

                let final_confidence = if let Some(c) = chain {
                    // Boost confidence if chain has verified mentions
                    if c.has_verified_mention {
                        (*confidence).min(c.best_confidence).max(*confidence * 1.1)
                    } else {
                        *confidence
                    }
                } else {
                    *confidence
                };

                let needs_review = final_confidence < self.config.review_threshold;

                let participant = ClauseParticipant {
                    span: None,
                    text: pronoun.clone(),
                    role: ParticipantRole::Subject,
                    resolved_to: None, // No DocSpan available from ObligorReference
                    resolved_text: Some(resolved_to.clone()),
                    is_pronoun: true,
                    confidence: final_confidence,
                    needs_review,
                    review_reason: if needs_review {
                        Some(format!(
                            "Pronoun '{}' resolved to '{}' with low confidence ({:.2})",
                            pronoun, resolved_to, final_confidence
                        ))
                    } else if !is_defined_term {
                        Some(format!(
                            "Pronoun '{}' resolved to non-defined term '{}'",
                            pronoun, resolved_to
                        ))
                    } else {
                        None
                    },
                };

                if needs_review {
                    ReviewableResult::uncertain(
                        participant,
                        vec![],
                        format!(
                            "Pronoun '{}' resolved to '{}' needs verification",
                            pronoun, resolved_to
                        ),
                    )
                } else {
                    ReviewableResult::certain(participant)
                }
            }

            ObligorReference::NounPhrase { text } => {
                // Plain noun phrase - lower confidence
                let confidence = self.config.noun_phrase_confidence;
                let needs_review = confidence < self.config.review_threshold;

                let participant = ClauseParticipant {
                    span: None,
                    text: text.clone(),
                    role: ParticipantRole::Subject,
                    resolved_to: None,
                    resolved_text: None,
                    is_pronoun: false,
                    confidence,
                    needs_review,
                    review_reason: if needs_review {
                        Some(format!(
                            "Noun phrase '{}' not a defined term",
                            text
                        ))
                    } else {
                        None
                    },
                };

                if needs_review {
                    ReviewableResult::uncertain(
                        participant,
                        vec![],
                        format!("Noun phrase '{}' may not be a valid party", text),
                    )
                } else {
                    ReviewableResult::certain(participant)
                }
            }
        }
    }

    /// Detect passive voice and extract "by X" obligor.
    ///
    /// Pattern: "be {past participle}(ed|en) by {agent}"
    /// Example: "shall be delivered by Tenant" -> Tenant is the obligor
    fn detect_passive_voice_obligor(&self, action: &str) -> Option<ReviewableResult<ClauseParticipant>> {
        if let Some(captures) = self.passive_voice_regex.captures(action) {
            if let Some(agent_match) = captures.get(2) {
                let agent_text = agent_match.as_str().trim();

                // Apply passive voice penalty to confidence
                let base_confidence = 0.85;
                let confidence = base_confidence - self.config.passive_voice_penalty;
                let needs_review = confidence < self.config.review_threshold;

                let participant = ClauseParticipant {
                    span: None,
                    text: agent_text.to_string(),
                    role: ParticipantRole::Obligor, // Passive voice agent is the obligor
                    resolved_to: None,
                    resolved_text: None,
                    is_pronoun: false,
                    confidence,
                    needs_review,
                    review_reason: Some(format!(
                        "Obligor '{}' extracted from passive voice construction",
                        agent_text
                    )),
                };

                return Some(if needs_review {
                    ReviewableResult::uncertain(
                        participant,
                        vec![],
                        "Passive voice obligor detection",
                    )
                } else {
                    ReviewableResult::certain(participant)
                });
            }
        }

        // Check for passive voice without agent (implicit obligor)
        // Pattern: "be {word}" at start or end, where word is a past participle
        // Use a list of common passive participles to avoid false positives
        let common_passive_participles = [
            "made", "done", "given", "taken", "paid", "sent", "delivered",
            "provided", "completed", "performed", "executed", "submitted",
            "received", "approved", "signed", "returned", "obtained",
        ];

        let action_lower = action.to_lowercase();
        let has_implicit_passive = action_lower.starts_with("be ") && {
            // Check if the word after "be" is a common passive participle
            let rest = action_lower.strip_prefix("be ").unwrap_or("");
            let first_word = rest.split_whitespace().next().unwrap_or("");
            common_passive_participles.iter().any(|&p| first_word == p)
                || first_word.ends_with("ed")
                || first_word.ends_with("en")
        };

        if has_implicit_passive && !action_lower.contains(" by ") {
            // Passive voice without "by X" - implicit obligor, flag for review
            let participant = ClauseParticipant {
                span: None,
                text: "(implicit)".to_string(),
                role: ParticipantRole::Subject,
                resolved_to: None,
                resolved_text: None,
                is_pronoun: false,
                confidence: self.config.implicit_obligor_confidence,
                needs_review: true,
                review_reason: Some("Implicit obligor - passive voice without agent".to_string()),
            };

            return Some(ReviewableResult::uncertain(
                participant,
                vec![],
                "Passive voice without explicit agent - obligor is implicit",
            ));
        }

        None
    }

    /// Extract beneficiary from action text or conditions.
    ///
    /// Looks for patterns like:
    /// - "to {beneficiary}" in action: "deliver goods to Landlord"
    /// - Direct object patterns: "pay Landlord rent"
    fn extract_beneficiary(
        &self,
        obligation: &crate::ObligationPhrase,
        _chains: &[PronounChainResult],
    ) -> Option<ReviewableResult<ClauseParticipant>> {
        let action = &obligation.action;

        // Pattern 1: "to {beneficiary}"
        if let Some(captures) = self.beneficiary_to_regex.captures(action) {
            if let Some(beneficiary_match) = captures.get(1) {
                let beneficiary_text = beneficiary_match.as_str().trim();

                // Skip common non-party words
                let skip_words = ["the", "a", "an", "this", "that", "such", "any", "all"];
                if skip_words.contains(&beneficiary_text.to_lowercase().as_str()) {
                    return None;
                }

                let participant = ClauseParticipant {
                    span: None,
                    text: beneficiary_text.to_string(),
                    role: ParticipantRole::IndirectObject,
                    resolved_to: None,
                    resolved_text: None,
                    is_pronoun: false,
                    confidence: 0.75,
                    needs_review: false,
                    review_reason: None,
                };

                return Some(ReviewableResult::certain(participant));
            }
        }

        // Pattern 2: Direct object - look for capitalized words in action
        // that might be party names (not at the start, which is the verb)
        let words: Vec<&str> = action.split_whitespace().collect();
        for (i, word) in words.iter().enumerate() {
            // Skip first word (verb) and common articles
            if i == 0 {
                continue;
            }
            if ["the", "a", "an", "to", "from", "with", "by", "for"]
                .contains(&word.to_lowercase().as_str())
            {
                continue;
            }

            // Check if word is capitalized (potential party name)
            if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                // Check it's not a common non-party capitalized word
                let common_caps = [
                    "The", "A", "An", "This", "That", "Such", "Any", "All",
                    "Section", "Article", "Exhibit", "Schedule", "Agreement",
                    "Contract", "Notice", "Payment", "Service", "Services",
                ];
                if common_caps.contains(word) {
                    continue;
                }

                let participant = ClauseParticipant {
                    span: None,
                    text: word.to_string(),
                    role: ParticipantRole::IndirectObject,
                    resolved_to: None,
                    resolved_text: None,
                    is_pronoun: false,
                    confidence: 0.6,
                    needs_review: true,
                    review_reason: Some(format!(
                        "Potential beneficiary '{}' detected from direct object position",
                        word
                    )),
                };

                return Some(ReviewableResult::uncertain(
                    participant,
                    vec![],
                    format!("Beneficiary '{}' inferred from action text", word),
                ));
            }
        }

        None
    }

    /// Get all obligations for a specific party (by name).
    ///
    /// Filters obligations where the party appears as either obligor or beneficiary.
    pub fn obligations_for_party<'a>(
        party_name: &str,
        obligations: &'a [LinkedObligation],
    ) -> Vec<&'a LinkedObligation> {
        let party_lower = party_name.to_lowercase();

        obligations
            .iter()
            .filter(|o| {
                // Check obligor
                let obligor_matches = o
                    .obligor
                    .ambiguous
                    .best
                    .value
                    .resolved_entity_text()
                    .to_lowercase()
                    .contains(&party_lower);

                // Check beneficiary
                let beneficiary_matches = o
                    .beneficiary
                    .as_ref()
                    .map(|b| {
                        b.ambiguous
                            .best
                            .value
                            .resolved_entity_text()
                            .to_lowercase()
                            .contains(&party_lower)
                    })
                    .unwrap_or(false);

                obligor_matches || beneficiary_matches
            })
            .collect()
    }
}

impl Default for ObligationPartyLinker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModalObligationType, ObligationPhrase, ObligationType, Polarity};
    use layered_nlp_document::Scored;

    fn make_obligation(obligor: ObligorReference, action: &str) -> Scored<ObligationPhrase> {
        let phrase = ObligationPhrase {
            obligor,
            obligation_type: ObligationType::Duty,
            action: action.to_string(),
            conditions: Vec::new(),
        };
        Scored::rule_based(phrase, 0.85, "obligation_phrase")
    }

    fn make_scoped_obligation(obligor: ObligorReference, action: &str) -> ScopedObligation {
        ScopedObligation {
            obligation: make_obligation(obligor, action),
            modal_type: ModalObligationType::Duty,
            polarity: Polarity::Positive,
            scope: None,
            scope_confidence: 1.0,
            scope_flag: None,
            overall_confidence: 0.85,
        }
    }

    fn make_chain(canonical_name: &str, is_defined_term: bool, confidence: f64) -> PronounChainResult {
        PronounChainResult {
            chain_id: 1,
            canonical_name: canonical_name.to_string(),
            is_defined_term,
            mention_count: 3,
            has_verified_mention: confidence >= 0.9,
            best_confidence: confidence,
        }
    }

    // ========================================================================
    // Test cases from the plan
    // ========================================================================

    #[test]
    fn test_tenant_shall_pay_rent_monthly() {
        // "Tenant shall pay rent monthly" - Tenant (obligor), pay rent monthly (Duty), High confidence
        let linker = ObligationPartyLinker::new();
        let obligor = ObligorReference::TermRef {
            term_name: "Tenant".to_string(),
            confidence: 0.9,
        };
        let scoped = make_scoped_obligation(obligor, "pay rent monthly");
        let chains = vec![];

        let linked = linker.link(scoped, &chains);

        assert_eq!(linked.obligor_name(), "Tenant");
        assert!(!linked.obligor.needs_review);
        assert!(linked.overall_confidence > 0.7);
        assert!(linked.beneficiary.is_none());
    }

    #[test]
    fn test_tenant_shall_pay_landlord() {
        // "Tenant shall pay Landlord" - Tenant (obligor), Landlord (beneficiary), pay (Duty), High
        let linker = ObligationPartyLinker::new();
        let obligor = ObligorReference::TermRef {
            term_name: "Tenant".to_string(),
            confidence: 0.9,
        };
        let scoped = make_scoped_obligation(obligor, "pay Landlord rent");
        let chains = vec![];

        let linked = linker.link(scoped, &chains);

        assert_eq!(linked.obligor_name(), "Tenant");
        assert!(!linked.obligor.needs_review);

        // Should detect Landlord as beneficiary
        assert!(linked.beneficiary.is_some());
        let beneficiary = linked.beneficiary.as_ref().unwrap();
        assert_eq!(beneficiary.ambiguous.best.value.text, "Landlord");
    }

    #[test]
    fn test_pronoun_it_shall_deliver() {
        // "It shall deliver the goods" - (resolved pronoun), deliver goods (Duty), Maybe flagged
        let linker = ObligationPartyLinker::new();
        let obligor = ObligorReference::PronounRef {
            pronoun: "It".to_string(),
            resolved_to: "Company".to_string(),
            is_defined_term: true,
            confidence: 0.75,
        };
        let scoped = make_scoped_obligation(obligor, "deliver the goods");
        let chains = vec![make_chain("Company", true, 0.8)];

        let linked = linker.link(scoped, &chains);

        assert_eq!(linked.obligor_name(), "Company");
        assert!(linked.obligor.ambiguous.best.value.is_pronoun);
        // Confidence is decent, may or may not need review depending on threshold
        assert!(linked.overall_confidence > 0.5);
    }

    #[test]
    fn test_pronoun_low_confidence_flagged() {
        // Low confidence pronoun resolution should be flagged
        let linker = ObligationPartyLinker::new();
        let obligor = ObligorReference::PronounRef {
            pronoun: "It".to_string(),
            resolved_to: "something".to_string(),
            is_defined_term: false,
            confidence: 0.4,
        };
        let scoped = make_scoped_obligation(obligor, "deliver goods");
        let chains = vec![];

        let linked = linker.link(scoped, &chains);

        assert!(linked.obligor.needs_review, "Low confidence pronoun should be flagged");
    }

    #[test]
    fn test_company_shall_not_disclose() {
        // "Company shall not disclose" - Company (obligor), disclose (Prohibition), High
        let linker = ObligationPartyLinker::new();
        let obligor = ObligorReference::TermRef {
            term_name: "Company".to_string(),
            confidence: 0.95,
        };
        let scoped = ScopedObligation {
            obligation: make_obligation(obligor, "disclose"),
            modal_type: ModalObligationType::Prohibition,
            polarity: Polarity::Negative,
            scope: None,
            scope_confidence: 1.0,
            scope_flag: None,
            overall_confidence: 0.9,
        };
        let chains = vec![];

        let linked = linker.link(scoped, &chains);

        assert_eq!(linked.obligor_name(), "Company");
        assert!(!linked.obligor.needs_review);
        assert!(linked.overall_confidence > 0.8);
        // Prohibition typically has no beneficiary
        assert!(linked.beneficiary.is_none());
    }

    #[test]
    fn test_payment_shall_be_made_implicit() {
        // "Payment shall be made" - (implicit), made (Duty), Low, flagged (passive)
        let linker = ObligationPartyLinker::new();
        // Note: In real code, the ObligationPhraseResolver might detect this differently
        // Here we test the passive voice detection in action text
        let obligor = ObligorReference::NounPhrase {
            text: "Payment".to_string(),
        };
        let scoped = make_scoped_obligation(obligor, "be made");
        let chains = vec![];

        let linked = linker.link(scoped, &chains);

        // Should detect passive voice and flag for review
        assert!(linked.obligor.needs_review || linked.obligor.ambiguous.best.value.text == "(implicit)");
    }

    #[test]
    fn test_shall_be_delivered_by_tenant() {
        // "shall be delivered by Tenant" - Tenant (via 'by' prep), delivered (Duty), High
        let linker = ObligationPartyLinker::new();
        let obligor = ObligorReference::NounPhrase {
            text: "goods".to_string(),
        };
        // The action text contains the passive voice with agent
        let scoped = make_scoped_obligation(obligor, "be delivered by Tenant");
        let chains = vec![];

        let linked = linker.link(scoped, &chains);

        // Should extract "Tenant" from passive voice
        assert_eq!(linked.obligor_name(), "Tenant");
        // Passive voice detection applies a penalty but should still have decent confidence
        assert!(linked.overall_confidence > 0.4);
    }

    #[test]
    fn test_it_shall_be_delivered_unresolved() {
        // "It shall be delivered" - (unresolved), delivered (Duty), Low, flagged
        let linker = ObligationPartyLinker::new();
        let obligor = ObligorReference::PronounRef {
            pronoun: "It".to_string(),
            resolved_to: "unknown".to_string(),
            is_defined_term: false,
            confidence: 0.3,
        };
        let scoped = make_scoped_obligation(obligor, "be delivered");
        let chains = vec![];

        let linked = linker.link(scoped, &chains);

        // Low confidence pronoun and passive voice should both contribute to flagging
        assert!(linked.needs_review(), "Unresolved pronoun should be flagged");
    }

    // ========================================================================
    // Passive voice detection tests
    // ========================================================================

    #[test]
    fn test_passive_voice_detection_delivered_by() {
        let linker = ObligationPartyLinker::new();

        let result = linker.detect_passive_voice_obligor("be delivered by the Company");

        assert!(result.is_some());
        let participant = result.unwrap();
        assert_eq!(participant.ambiguous.best.value.text, "Company");
        // Passive voice agent is the obligor (the one doing the action)
        assert_eq!(participant.ambiguous.best.value.role, ParticipantRole::Obligor);
    }

    #[test]
    fn test_passive_voice_detection_paid_by() {
        let linker = ObligationPartyLinker::new();

        let result = linker.detect_passive_voice_obligor("be paid by Tenant");

        assert!(result.is_some());
        let participant = result.unwrap();
        assert_eq!(participant.ambiguous.best.value.text, "Tenant");
        assert_eq!(participant.ambiguous.best.value.role, ParticipantRole::Obligor);
    }

    #[test]
    fn test_passive_voice_auxiliary_verbs() {
        // Test that we detect various auxiliary verb forms, not just "be"
        let linker = ObligationPartyLinker::new();

        // "is delivered by"
        let result = linker.detect_passive_voice_obligor("is delivered by Vendor");
        assert!(result.is_some());
        assert_eq!(result.unwrap().ambiguous.best.value.text, "Vendor");

        // "was paid by"
        let result = linker.detect_passive_voice_obligor("was paid by Company");
        assert!(result.is_some());
        assert_eq!(result.unwrap().ambiguous.best.value.text, "Company");

        // "were completed by"
        let result = linker.detect_passive_voice_obligor("were completed by Contractors");
        assert!(result.is_some());
        assert_eq!(result.unwrap().ambiguous.best.value.text, "Contractors");

        // "are maintained by"
        let result = linker.detect_passive_voice_obligor("are maintained by Landlord");
        assert!(result.is_some());
        assert_eq!(result.unwrap().ambiguous.best.value.text, "Landlord");

        // "been approved by" (for "has been" / "had been" constructions)
        let result = linker.detect_passive_voice_obligor("been approved by Manager");
        assert!(result.is_some());
        assert_eq!(result.unwrap().ambiguous.best.value.text, "Manager");
    }

    #[test]
    fn test_passive_voice_detection_provided_by() {
        let linker = ObligationPartyLinker::new();

        let result = linker.detect_passive_voice_obligor("be provided by Service Provider");

        assert!(result.is_some());
        let participant = result.unwrap();
        // Should capture multi-word name (both words)
        assert_eq!(participant.ambiguous.best.value.text, "Service Provider");
        assert_eq!(participant.ambiguous.best.value.role, ParticipantRole::Obligor);
    }

    #[test]
    fn test_passive_voice_multi_word_party_names() {
        // Test that we capture party names with more than 2 words
        let linker = ObligationPartyLinker::new();

        // Three-word party name
        let result = linker.detect_passive_voice_obligor("be delivered by Acme Software Corporation");
        assert!(result.is_some());
        assert_eq!(result.unwrap().ambiguous.best.value.text, "Acme Software Corporation");

        // "the" article is stripped, so "The First National Bank" becomes "First National Bank"
        let result = linker.detect_passive_voice_obligor("be approved by the First National Bank");
        assert!(result.is_some());
        assert_eq!(result.unwrap().ambiguous.best.value.text, "First National Bank");

        // Without "the", captures all capitalized words
        let result = linker.detect_passive_voice_obligor("be approved by First National Bank");
        assert!(result.is_some());
        assert_eq!(result.unwrap().ambiguous.best.value.text, "First National Bank");
    }

    #[test]
    fn test_passive_voice_without_agent_implicit() {
        let linker = ObligationPartyLinker::new();

        let result = linker.detect_passive_voice_obligor("be made");

        assert!(result.is_some());
        let participant = result.unwrap();
        assert_eq!(participant.ambiguous.best.value.text, "(implicit)");
        assert!(participant.needs_review);
    }

    #[test]
    fn test_passive_voice_stops_at_conjunction() {
        // "and" should not be captured as part of party name
        // This tests that the regex correctly stops at lowercase words
        let linker = ObligationPartyLinker::new();

        let result = linker.detect_passive_voice_obligor("was approved by Smith and Jones");

        assert!(result.is_some());
        let participant = result.unwrap();
        // Should only capture "Smith", not "Smith and Jones"
        // because "and" starts with lowercase and breaks the capitalized word sequence
        assert_eq!(participant.ambiguous.best.value.text, "Smith");
    }

    #[test]
    fn test_no_passive_voice_active_voice() {
        let linker = ObligationPartyLinker::new();

        let result = linker.detect_passive_voice_obligor("deliver goods");

        assert!(result.is_none());
    }

    // ========================================================================
    // Beneficiary extraction tests
    // ========================================================================

    #[test]
    fn test_beneficiary_to_pattern() {
        let linker = ObligationPartyLinker::new();
        let obligation = ObligationPhrase {
            obligor: ObligorReference::TermRef {
                term_name: "Tenant".to_string(),
                confidence: 0.9,
            },
            obligation_type: ObligationType::Duty,
            action: "deliver goods to Landlord".to_string(),
            conditions: vec![],
        };
        let chains = vec![];

        let result = linker.extract_beneficiary(&obligation, &chains);

        assert!(result.is_some());
        let beneficiary = result.unwrap();
        assert_eq!(beneficiary.ambiguous.best.value.text, "Landlord");
    }

    #[test]
    fn test_beneficiary_direct_object() {
        let linker = ObligationPartyLinker::new();
        let obligation = ObligationPhrase {
            obligor: ObligorReference::TermRef {
                term_name: "Tenant".to_string(),
                confidence: 0.9,
            },
            obligation_type: ObligationType::Duty,
            action: "pay Landlord rent".to_string(),
            conditions: vec![],
        };
        let chains = vec![];

        let result = linker.extract_beneficiary(&obligation, &chains);

        assert!(result.is_some());
        let beneficiary = result.unwrap();
        assert_eq!(beneficiary.ambiguous.best.value.text, "Landlord");
    }

    #[test]
    fn test_no_beneficiary_simple_action() {
        let linker = ObligationPartyLinker::new();
        let obligation = ObligationPhrase {
            obligor: ObligorReference::TermRef {
                term_name: "Company".to_string(),
                confidence: 0.9,
            },
            obligation_type: ObligationType::Prohibition,
            action: "disclose information".to_string(),
            conditions: vec![],
        };
        let chains = vec![];

        let result = linker.extract_beneficiary(&obligation, &chains);

        // "information" is not capitalized, should not be detected as beneficiary
        // This tests that we don't falsely detect beneficiaries
        assert!(result.is_none() || result.as_ref().map(|r| r.ambiguous.best.value.text != "information").unwrap_or(true));
    }

    // ========================================================================
    // Query API tests
    // ========================================================================

    #[test]
    fn test_obligations_for_party() {
        let linker = ObligationPartyLinker::new();

        let obligations = vec![
            linker.link(
                make_scoped_obligation(
                    ObligorReference::TermRef {
                        term_name: "Tenant".to_string(),
                        confidence: 0.9,
                    },
                    "pay rent",
                ),
                &[],
            ),
            linker.link(
                make_scoped_obligation(
                    ObligorReference::TermRef {
                        term_name: "Landlord".to_string(),
                        confidence: 0.9,
                    },
                    "maintain premises",
                ),
                &[],
            ),
            linker.link(
                make_scoped_obligation(
                    ObligorReference::TermRef {
                        term_name: "Tenant".to_string(),
                        confidence: 0.9,
                    },
                    "provide notice",
                ),
                &[],
            ),
        ];

        let tenant_obligations = ObligationPartyLinker::obligations_for_party("Tenant", &obligations);
        assert_eq!(tenant_obligations.len(), 2);

        let landlord_obligations = ObligationPartyLinker::obligations_for_party("Landlord", &obligations);
        assert_eq!(landlord_obligations.len(), 1);

        let other_obligations = ObligationPartyLinker::obligations_for_party("Other", &obligations);
        assert!(other_obligations.is_empty());
    }

    #[test]
    fn test_obligations_for_party_case_insensitive() {
        let linker = ObligationPartyLinker::new();

        let obligations = vec![linker.link(
            make_scoped_obligation(
                ObligorReference::TermRef {
                    term_name: "TENANT".to_string(),
                    confidence: 0.9,
                },
                "pay rent",
            ),
            &[],
        )];

        let result = ObligationPartyLinker::obligations_for_party("tenant", &obligations);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_obligations_for_party_includes_beneficiaries() {
        let linker = ObligationPartyLinker::new();

        let obligations = vec![linker.link(
            make_scoped_obligation(
                ObligorReference::TermRef {
                    term_name: "Tenant".to_string(),
                    confidence: 0.9,
                },
                "pay Landlord rent",
            ),
            &[],
        )];

        // Landlord appears as beneficiary
        let landlord_obligations = ObligationPartyLinker::obligations_for_party("Landlord", &obligations);
        assert_eq!(landlord_obligations.len(), 1);
    }

    // ========================================================================
    // Configuration tests
    // ========================================================================

    #[test]
    fn test_custom_config() {
        let config = ObligationPartyLinkerConfig {
            review_threshold: 0.8,
            passive_voice_penalty: 0.3,
            noun_phrase_confidence: 0.5,
            implicit_obligor_confidence: 0.3,
        };
        let linker = ObligationPartyLinker::with_config(config.clone());

        // With higher threshold, more things should be flagged
        let obligor = ObligorReference::TermRef {
            term_name: "Tenant".to_string(),
            confidence: 0.75, // Below 0.8 threshold
        };
        let scoped = make_scoped_obligation(obligor, "pay rent");
        let linked = linker.link(scoped, &[]);

        assert!(linked.obligor.needs_review, "Should be flagged with strict threshold");
    }

    // ========================================================================
    // Integration-style tests
    // ========================================================================

    #[test]
    fn test_linked_obligation_needs_review() {
        let linker = ObligationPartyLinker::new();

        // Certain obligor
        let certain = linker.link(
            make_scoped_obligation(
                ObligorReference::TermRef {
                    term_name: "Tenant".to_string(),
                    confidence: 0.95,
                },
                "pay rent",
            ),
            &[],
        );
        assert!(!certain.needs_review());

        // Uncertain obligor
        let uncertain = linker.link(
            make_scoped_obligation(
                ObligorReference::PronounRef {
                    pronoun: "It".to_string(),
                    resolved_to: "something".to_string(),
                    is_defined_term: false,
                    confidence: 0.3,
                },
                "do something",
            ),
            &[],
        );
        assert!(uncertain.needs_review());
    }

    #[test]
    fn test_linked_obligation_review_reasons() {
        let linker = ObligationPartyLinker::new();

        let linked = linker.link(
            make_scoped_obligation(
                ObligorReference::PronounRef {
                    pronoun: "It".to_string(),
                    resolved_to: "something".to_string(),
                    is_defined_term: false,
                    confidence: 0.3,
                },
                "pay Vendor goods",
            ),
            &[],
        );

        let reasons = linked.review_reasons();
        assert!(!reasons.is_empty());
    }
}

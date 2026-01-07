//! Verification helpers for accountability analytics.
//!
//! External review tooling can use these helpers to queue nodes for review,
//! resolve beneficiary links, and record reviewer notes.

use serde::Serialize;

use crate::{ObligationNode, Scored, ScoreSource};
use crate::utils::normalize_party_name;

/// Target for a verification action.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum VerificationTarget {
    /// Entire obligation node (covers all beneficiaries/conditions).
    Node(u32),
    /// A specific beneficiary link identified by the clause that produced it.
    BeneficiaryLink {
        /// Obligation node identifier.
        node_id: u32,
        /// Clause identifier that produced the beneficiary link.
        source_clause_id: u32,
        /// Beneficiary display text for disambiguation.
        beneficiary_display_text: String,
    },
    /// Condition edge attached to a clause.
    ConditionLink {
        /// Obligation node identifier.
        node_id: u32,
        /// Clause identifier where the condition originated.
        source_clause_id: u32,
        /// Captured condition text.
        condition_text: String,
    },
    /// Obligor link needing review (from LinkedObligation).
    ObligorLink {
        /// Obligation node identifier.
        node_id: u32,
        /// Clause identifier from which the obligor was derived.
        source_clause_id: u32,
        /// Obligor display text for disambiguation.
        obligor_display_text: String,
    },
}

/// Reviewer note associated with a verification target.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VerificationNote {
    /// What part of the graph this note applies to.
    pub target: VerificationTarget,
    /// Reviewer identifier (LLM pass ID or human reviewer).
    pub verifier_id: String,
    /// Free-form note content.
    pub note: String,
}

/// Supported verification actions that mutate the accountability graph.
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationAction {
    /// Mark an entire obligation node as verified.
    VerifyNode {
        /// Target node identifier.
        node_id: u32,
        /// Reviewer identifier.
        verifier_id: String,
        /// Optional reviewer note.
        note: Option<String>,
    },
    /// Resolve or confirm a beneficiary link.
    ResolveBeneficiary {
        /// Target node identifier.
        node_id: u32,
        /// Clause identifier that produced the beneficiary link.
        source_clause_id: u32,
        /// Beneficiary display text for disambiguation.
        beneficiary_display_text: String,
        /// Reviewer identifier.
        verifier_id: String,
        /// Chain identifier that should be associated with the beneficiary.
        resolved_chain_id: Option<u32>,
        /// Optional display text override (e.g., corrected party name).
        updated_display_text: Option<String>,
        /// Optional reviewer note.
        note: Option<String>,
    },
    /// Verify a condition link.
    VerifyCondition {
        /// Target node identifier.
        node_id: u32,
        /// Clause identifier that produced the condition.
        source_clause_id: u32,
        /// Condition text to disambiguate matches.
        condition_text: String,
        /// Reviewer identifier.
        verifier_id: String,
        /// Whether to clear the `mentions_unknown_entity` flag.
        clear_unknown_flag: bool,
        /// Optional reviewer note.
        note: Option<String>,
    },
}

impl VerificationAction {
    /// Construct an action that marks a node as verified.
    pub fn verify_node(node_id: u32, verifier_id: &str) -> Self {
        Self::VerifyNode {
            node_id,
            verifier_id: verifier_id.to_string(),
            note: None,
        }
    }

    /// Construct an action that resolves a beneficiary link.
    pub fn resolve_beneficiary(
        node_id: u32,
        source_clause_id: u32,
        beneficiary_display_text: &str,
        verifier_id: &str,
        resolved_chain_id: Option<u32>,
    ) -> Self {
        Self::ResolveBeneficiary {
            node_id,
            source_clause_id,
            beneficiary_display_text: beneficiary_display_text.to_string(),
            verifier_id: verifier_id.to_string(),
            resolved_chain_id,
            updated_display_text: None,
            note: None,
        }
    }

    /// Construct an action that verifies a condition link.
    pub fn verify_condition(
        node_id: u32,
        source_clause_id: u32,
        condition_text: &str,
        verifier_id: &str,
    ) -> Self {
        Self::VerifyCondition {
            node_id,
            source_clause_id,
            condition_text: condition_text.to_string(),
            verifier_id: verifier_id.to_string(),
            clear_unknown_flag: true,
            note: None,
        }
    }

    /// Attach a reviewer note to this action.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        match &mut self {
            VerificationAction::VerifyNode { note: note_slot, .. }
            | VerificationAction::ResolveBeneficiary { note: note_slot, .. }
            | VerificationAction::VerifyCondition { note: note_slot, .. } => {
                *note_slot = Some(note.into());
            }
        }
        self
    }

    /// Override the beneficiary display text for `ResolveBeneficiary`.
    pub fn with_updated_display(mut self, text: impl Into<String>) -> Self {
        if let VerificationAction::ResolveBeneficiary {
            updated_display_text,
            ..
        } = &mut self
        {
            *updated_display_text = Some(text.into());
        }
        self
    }

    /// Keep the unknown-entity flag when verifying a condition.
    pub fn keep_condition_flag(mut self) -> Self {
        if let VerificationAction::VerifyCondition {
            clear_unknown_flag,
            ..
        } = &mut self
        {
            *clear_unknown_flag = false;
        }
        self
    }
}

/// Apply a verification action to the supplied set of nodes.
pub fn apply_verification_action(
    nodes: &mut [Scored<ObligationNode>],
    action: VerificationAction,
) -> bool {
    match action {
        VerificationAction::VerifyNode {
            node_id,
            verifier_id,
            note,
        } => nodes
            .iter_mut()
            .find(|scored| scored.value.node_id == node_id)
            .map(|node| {
                mark_score_verified(node, &verifier_id);
                push_note(
                    &mut node.value,
                    VerificationTarget::Node(node_id),
                    verifier_id,
                    note,
                );
                true
            })
            .unwrap_or(false),
        VerificationAction::ResolveBeneficiary {
            node_id,
            source_clause_id,
            beneficiary_display_text,
            verifier_id,
            resolved_chain_id,
            updated_display_text,
            note,
        } => nodes
            .iter_mut()
            .find(|scored| scored.value.node_id == node_id)
            .map(|node| {
                let mut updated = false;
                let target_name = normalize_party_name(&beneficiary_display_text);
                for link in &mut node.value.beneficiaries {
                    if link.source_clause_id == source_clause_id
                        && normalize_party_name(&link.display_text) == target_name
                    {
                        if let Some(text) = &updated_display_text {
                            link.display_text = text.clone();
                        }
                        link.chain_id = resolved_chain_id;
                        link.needs_verification = false;
                        link.has_verified_chain = resolved_chain_id.is_some();
                        // Also clear the needs_review flag (Gate 5)
                        link.needs_review = false;
                        link.review_reason = None;
                        updated = true;
                    }
                }

                if updated {
                    // Mark as verified when all beneficiaries are resolved (both flags)
                    if node.value.beneficiaries.iter().all(|b| !b.needs_verification && !b.needs_review) {
                        mark_score_verified(node, &verifier_id);
                    }
                    push_note(
                        &mut node.value,
                        VerificationTarget::BeneficiaryLink {
                            node_id,
                            source_clause_id,
                            beneficiary_display_text,
                        },
                        verifier_id,
                        note,
                    );
                }
                updated
            })
            .unwrap_or(false),
        VerificationAction::VerifyCondition {
            node_id,
            source_clause_id,
            condition_text,
            verifier_id,
            clear_unknown_flag,
            note,
        } => nodes
            .iter_mut()
            .find(|scored| scored.value.node_id == node_id)
            .map(|node| {
                let target_text = normalize_condition_text(&condition_text);
                let mut updated = false;
                for link in &mut node.value.condition_links {
                    if link.source_clause_id == source_clause_id
                        && normalize_condition_text(&link.condition.text) == target_text
                    {
                        if clear_unknown_flag {
                            link.condition.mentions_unknown_entity = false;
                        }
                        updated = true;
                    }
                }
                for clause in &mut node.value.clauses {
                    if clause.clause_id == source_clause_id {
                        for condition in &mut clause.conditions {
                            if normalize_condition_text(&condition.text) == target_text {
                                if clear_unknown_flag {
                                    condition.mentions_unknown_entity = false;
                                }
                                updated = true;
                            }
                        }
                    }
                }

                if updated {
                    push_note(
                        &mut node.value,
                        VerificationTarget::ConditionLink {
                            node_id,
                            source_clause_id,
                            condition_text,
                        },
                        verifier_id,
                        note,
                    );
                }
                updated
            })
            .unwrap_or(false),
    }
}

fn mark_score_verified(node: &mut Scored<ObligationNode>, verifier_id: &str) {
    node.confidence = 1.0;
    node.source = ScoreSource::HumanVerified {
        verifier_id: verifier_id.to_string(),
    };
}

fn push_note(
    node: &mut ObligationNode,
    target: VerificationTarget,
    verifier_id: String,
    note: Option<String>,
) {
    if let Some(note_text) = note {
        node.verification_notes.push(VerificationNote {
            target,
            verifier_id,
            note: note_text,
        });
    }
}

fn normalize_condition_text(text: &str) -> String {
    text.trim().to_lowercase()
}

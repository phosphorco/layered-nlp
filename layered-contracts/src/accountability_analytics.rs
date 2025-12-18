//! Query helpers and serialization utilities for accountability analytics.
//!
//! Consumers can build an [`ObligationGraph`] from Layer 9 outputs and run
//! party-centric queries, filter by conditions, export JSON payloads, and
//! surface verification queues for unresolved beneficiaries.

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::Serialize;
use serde_json::{self, Value};

use crate::accountability_graph::{BeneficiaryLink, ConditionLink, ObligationNode};
use crate::clause_aggregate::ClauseAggregateEntry;
use crate::contract_clause::{ClauseCondition, ClauseDuty, ClauseParty};
use crate::contract_keyword::ContractKeyword;
use crate::obligation::ObligationType;
use crate::utils::normalize_party_name;
use crate::verification::{VerificationNote, VerificationTarget};
use crate::Scored;

/// Party-centric view over a set of obligation nodes.
pub struct ObligationGraph<'a> {
    nodes: &'a [Scored<ObligationNode>],
}

impl<'a> ObligationGraph<'a> {
    /// Create a new graph wrapper.
    pub fn new(nodes: &'a [Scored<ObligationNode>]) -> Self {
        Self { nodes }
    }

    /// Return a slice of all nodes backing this graph.
    pub fn nodes(&self) -> &'a [Scored<ObligationNode>] {
        self.nodes
    }

    /// Return a party-centric view grouped by beneficiary chains.
    pub fn for_party(&self, chain_id: u32) -> PartyAnalytics<'a> {
        self.for_party_internal(Some(chain_id), None)
    }

    /// Return a party view based on display text when no chain is available.
    pub fn for_party_name(&self, display_text: &str) -> PartyAnalytics<'a> {
        self.for_party_internal(None, Some(display_text))
    }

    /// Resolve a party either by chain ID or display-name fallback.
    pub fn for_party_or_display(
        &self,
        chain_id: Option<u32>,
        display_text: &str,
    ) -> PartyAnalytics<'a> {
        if let Some(chain) = chain_id {
            let analytics = self.for_party_internal(Some(chain), Some(display_text));
            if !analytics.beneficiary_groups.is_empty() {
                return analytics;
            }
        }
        self.for_party_internal(None, Some(display_text))
    }

    /// Filter nodes by a predicate operating on normalized clause conditions.
    pub fn with_condition<F>(&self, predicate: F) -> Vec<&'a Scored<ObligationNode>>
    where
        F: Fn(&ClauseCondition) -> bool,
    {
        self.filter_nodes(|node| {
            node.value
                .condition_links
                .iter()
                .any(|link| predicate(&link.condition))
        })
    }

    /// Filter nodes by condition keyword (If, Unless, SubjectTo, etc.).
    pub fn with_condition_type(
        &self,
        keyword: ContractKeyword,
    ) -> Vec<&'a Scored<ObligationNode>> {
        self.with_condition(|condition| condition.condition_type == keyword)
    }

    /// Filter nodes that reference a specific section label within conditions.
    pub fn referencing_section(&self, section_label: &str) -> Vec<&'a Scored<ObligationNode>> {
        let needle = section_label.to_lowercase();
        self.with_condition(|condition| condition.text.to_lowercase().contains(&needle))
    }

    /// Surface unresolved beneficiaries that need verification.
    pub fn verification_queue(&self) -> Vec<VerificationQueueItem> {
        let mut queue = Vec::new();
        for node in self.nodes {
            for link in &node.value.beneficiaries {
                if link.needs_verification {
                    queue.push(VerificationQueueItem {
                        target: VerificationTarget::BeneficiaryLink {
                            node_id: node.value.node_id,
                            source_clause_id: link.source_clause_id,
                            beneficiary_display_text: link.display_text.clone(),
                        },
                        node_id: node.value.node_id,
                        clause_id: link.source_clause_id,
                        obligor: PartySummary::from(&node.value.obligor),
                        details: VerificationQueueDetails::Beneficiary {
                            beneficiary_text: link.display_text.clone(),
                            has_chain: link.chain_id.is_some(),
                        },
                    });
                }
            }
            for condition_link in node
                .value
                .condition_links
                .iter()
                .filter(|link| link.condition.mentions_unknown_entity)
            {
                queue.push(VerificationQueueItem {
                    target: VerificationTarget::ConditionLink {
                        node_id: node.value.node_id,
                        source_clause_id: condition_link.source_clause_id,
                        condition_text: condition_link.condition.text.clone(),
                    },
                    node_id: node.value.node_id,
                    clause_id: condition_link.source_clause_id,
                    obligor: PartySummary::from(&node.value.obligor),
                    details: VerificationQueueDetails::Condition {
                        condition_text: condition_link.condition.text.clone(),
                        condition_type: format_contract_keyword(
                            &condition_link.condition.condition_type,
                        ),
                        mentions_unknown_entity: true,
                    },
                });
            }
        }
        queue
    }

    /// Build a JSON-friendly payload for downstream consumers.
    pub fn payload(&self) -> AccountabilityPayload {
        AccountabilityPayload {
            nodes: self.nodes.iter().map(AccountabilityNodePayload::from).collect(),
            verification_queue: self.verification_queue(),
        }
    }

    fn for_party_internal(
        &self,
        chain_id: Option<u32>,
        fallback_display: Option<&str>,
    ) -> PartyAnalytics<'a> {
        let normalized_fallback = fallback_display.map(normalize_party_name);
        let normalized_fallback_str = normalized_fallback.as_deref();
        let mut matches: Vec<&Scored<ObligationNode>> = self
            .nodes
            .iter()
            .filter(|node| {
                if let Some(chain) = chain_id {
                    node.value.obligor.chain_id == Some(chain)
                } else if let Some(name) = normalized_fallback_str {
                    normalize_party_name(&node.value.obligor.display_text) == name
                } else {
                    false
                }
            })
            .collect();

        if matches.is_empty() && normalized_fallback_str.is_some() && chain_id.is_some() {
            let fallback_name = normalized_fallback_str.unwrap();
            matches = self
                .nodes
                .iter()
                .filter(|node| {
                    normalize_party_name(&node.value.obligor.display_text) == fallback_name
                })
                .collect();
        }

        let display_text = matches
            .first()
            .map(|node| node.value.obligor.display_text.clone())
            .or_else(|| fallback_display.map(|text| text.to_string()))
            .unwrap_or_else(|| "Unknown Party".to_string());

        let mut buckets: HashMap<BeneficiaryKey, BeneficiaryBucket<'a>> = HashMap::new();
        let mut unassigned_nodes = Vec::new();
        for node in &matches {
            if node.value.beneficiaries.is_empty() {
                unassigned_nodes.push(*node);
                continue;
            }

            for link in &node.value.beneficiaries {
                let key = BeneficiaryKey {
                    chain_id: link.chain_id,
                    normalized_name: normalize_party_name(&link.display_text),
                };
                let entry = buckets.entry(key).or_insert_with(|| BeneficiaryBucket {
                    chain_id: link.chain_id,
                    display_text: link.display_text.clone(),
                    needs_verification: false,
                    nodes: Vec::new(),
                });
                entry.needs_verification |= link.needs_verification;
                if !entry
                    .nodes
                    .iter()
                    .any(|existing| existing.value.node_id == node.value.node_id)
                {
                    entry.nodes.push(*node);
                }
            }
        }

        unassigned_nodes.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(Ordering::Equal)
        });

        let mut beneficiary_groups: Vec<BeneficiaryGroup<'a>> = buckets
            .into_iter()
            .map(|(_, mut bucket)| {
                bucket.nodes.sort_by(|a, b| {
                    b.confidence
                        .partial_cmp(&a.confidence)
                        .unwrap_or(Ordering::Equal)
                });

                BeneficiaryGroup {
                    descriptor: BeneficiaryDescriptor {
                        display_text: bucket.display_text,
                        chain_id: bucket.chain_id,
                        needs_verification: bucket.needs_verification,
                    },
                    nodes: bucket.nodes,
                }
            })
            .collect();

        beneficiary_groups.sort_by(|a, b| {
            let a_conf = a.nodes.first().map(|node| node.confidence).unwrap_or(0.0);
            let b_conf = b.nodes.first().map(|node| node.confidence).unwrap_or(0.0);
            b_conf
                .partial_cmp(&a_conf)
                .unwrap_or(Ordering::Equal)
        });

        PartyAnalytics {
            obligor_chain_id: matches
                .first()
                .and_then(|node| node.value.obligor.chain_id),
            obligor_display_text: display_text,
            beneficiary_groups,
            unassigned_nodes,
        }
    }

    fn filter_nodes<F>(&self, predicate: F) -> Vec<&'a Scored<ObligationNode>>
    where
        F: Fn(&Scored<ObligationNode>) -> bool,
    {
        let mut filtered: Vec<_> = self.nodes.iter().filter(|node| predicate(node)).collect();
        filtered.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(Ordering::Equal)
        });
        filtered
    }
}

/// Aggregated obligations for a party, grouped by beneficiary.
#[derive(Debug)]
pub struct PartyAnalytics<'a> {
    /// Chain identifier when available.
    pub obligor_chain_id: Option<u32>,
    /// Preferred display text for this party.
    pub obligor_display_text: String,
    /// Beneficiary-centric groupings sorted by confidence.
    pub beneficiary_groups: Vec<BeneficiaryGroup<'a>>,
    /// Obligations that lack beneficiary detection.
    pub unassigned_nodes: Vec<&'a Scored<ObligationNode>>,
}

/// Group of obligations linked to a beneficiary.
#[derive(Debug)]
pub struct BeneficiaryGroup<'a> {
    /// Descriptor summarizing the beneficiary.
    pub descriptor: BeneficiaryDescriptor,
    /// Obligations contributing to this beneficiary.
    pub nodes: Vec<&'a Scored<ObligationNode>>,
}

/// Normalized beneficiary metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BeneficiaryDescriptor {
    /// Display text as captured from the clause.
    pub display_text: String,
    /// Chain id when resolved.
    pub chain_id: Option<u32>,
    /// True if this beneficiary still needs verification.
    pub needs_verification: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BeneficiaryKey {
    chain_id: Option<u32>,
    normalized_name: String,
}

struct BeneficiaryBucket<'a> {
    chain_id: Option<u32>,
    display_text: String,
    needs_verification: bool,
    nodes: Vec<&'a Scored<ObligationNode>>,
}

/// Queue entry for unresolved graph artifacts.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VerificationQueueItem {
    /// Target used by verification tooling.
    pub target: VerificationTarget,
    /// Obligation node identifier.
    pub node_id: u32,
    /// Clause identifier where the target was found.
    pub clause_id: u32,
    /// Obligor metadata for reviewer context.
    pub obligor: PartySummary,
    /// Detailed context for the verification request.
    pub details: VerificationQueueDetails,
}

/// Additional context for a verification queue entry.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VerificationQueueDetails {
    /// Beneficiary resolution request.
    Beneficiary {
        /// Original beneficiary surface text.
        beneficiary_text: String,
        /// Whether this link already has a chain id (even if unverified).
        has_chain: bool,
    },
    /// Condition verification request.
    Condition {
        /// Captured condition text.
        condition_text: String,
        /// Condition keyword (If/SubjectTo/etc).
        condition_type: String,
        /// Whether the parser flagged an unknown entity.
        mentions_unknown_entity: bool,
    },
}

/// Serialized payload for feeding BI/search pipelines.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AccountabilityPayload {
    /// Node-centric snapshot.
    pub nodes: Vec<AccountabilityNodePayload>,
    /// Outstanding verification queue entries.
    pub verification_queue: Vec<VerificationQueueItem>,
}

impl AccountabilityPayload {
    /// Convert to a JSON value for downstream consumers.
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).expect("payload always serializes")
    }

    /// Convert to a pretty-printed JSON string.
    pub fn to_json_string(&self) -> String {
        serde_json::to_string_pretty(self).expect("payload always serializes")
    }
}

/// Node payload that mirrors `ObligationNode` plus provenance.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AccountabilityNodePayload {
    pub node_id: u32,
    pub aggregate_id: u32,
    pub confidence: f64,
    pub confidence_breakdown: Vec<String>,
    pub obligor: PartySummary,
    pub beneficiaries: Vec<BeneficiaryPayload>,
    pub conditions: Vec<ConditionPayload>,
    pub clauses: Vec<ClausePayload>,
    pub needs_verification: bool,
    pub notes: Vec<VerificationNote>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PartySummary {
    pub display_text: String,
    pub chain_id: Option<u32>,
    pub has_verified_chain: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BeneficiaryPayload {
    pub display_text: String,
    pub chain_id: Option<u32>,
    pub has_verified_chain: bool,
    pub needs_verification: bool,
    pub source_clause_id: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConditionPayload {
    pub source_clause_id: u32,
    pub condition_type: String,
    pub text: String,
    pub mentions_unknown_entity: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ClausePayload {
    pub clause_id: u32,
    pub obligation_type: String,
    pub action: String,
    pub conditions: Vec<ConditionPayload>,
    pub clause_confidence: f64,
}

impl From<&Scored<ObligationNode>> for AccountabilityNodePayload {
    fn from(node: &Scored<ObligationNode>) -> Self {
        let beneficiaries: Vec<_> = node
            .value
            .beneficiaries
            .iter()
            .map(BeneficiaryPayload::from)
            .collect();

        let needs_verification = beneficiaries.iter().any(|b| b.needs_verification);

        Self {
            node_id: node.value.node_id,
            aggregate_id: node.value.aggregate_id,
            confidence: node.confidence,
            confidence_breakdown: node.value.confidence_breakdown.clone(),
            obligor: PartySummary::from(&node.value.obligor),
            beneficiaries,
            conditions: node
                .value
                .condition_links
                .iter()
                .map(ConditionPayload::from)
                .collect(),
            clauses: node
                .value
                .clauses
                .iter()
                .map(ClausePayload::from)
                .collect(),
            needs_verification,
            notes: node.value.verification_notes.clone(),
        }
    }
}

impl From<&ClauseParty> for PartySummary {
    fn from(party: &ClauseParty) -> Self {
        Self {
            display_text: party.display_text.clone(),
            chain_id: party.chain_id,
            has_verified_chain: party.has_verified_chain,
        }
    }
}

impl From<&BeneficiaryLink> for BeneficiaryPayload {
    fn from(link: &BeneficiaryLink) -> Self {
        Self {
            display_text: link.display_text.clone(),
            chain_id: link.chain_id,
            has_verified_chain: link.has_verified_chain,
            needs_verification: link.needs_verification,
            source_clause_id: link.source_clause_id,
        }
    }
}

impl From<&ConditionLink> for ConditionPayload {
    fn from(link: &ConditionLink) -> Self {
        ConditionPayload::from_condition(&link.condition, link.source_clause_id)
    }
}

impl From<&ClauseAggregateEntry> for ClausePayload {
    fn from(entry: &ClauseAggregateEntry) -> Self {
        Self {
            clause_id: entry.clause_id,
            obligation_type: format_obligation_type(&entry.duty),
            action: entry.duty.action.clone(),
            conditions: entry
                .conditions
                .iter()
                .map(|condition| ConditionPayload::from_condition(condition, entry.clause_id))
                .collect(),
            clause_confidence: entry.clause_confidence,
        }
    }
}

impl ConditionPayload {
    fn from_condition(condition: &ClauseCondition, clause_id: u32) -> Self {
        Self {
            source_clause_id: clause_id,
            condition_type: format_contract_keyword(&condition.condition_type),
            text: condition.text.clone(),
            mentions_unknown_entity: condition.mentions_unknown_entity,
        }
    }
}

fn format_obligation_type(duty: &ClauseDuty) -> String {
    match duty.obligation_type {
        ObligationType::Duty => "Duty",
        ObligationType::Permission => "Permission",
        ObligationType::Prohibition => "Prohibition",
    }
    .to_string()
}

fn format_contract_keyword(keyword: &ContractKeyword) -> String {
    match keyword {
        ContractKeyword::Shall => "Shall",
        ContractKeyword::May => "May",
        ContractKeyword::ShallNot => "ShallNot",
        ContractKeyword::Means => "Means",
        ContractKeyword::Includes => "Includes",
        ContractKeyword::Hereinafter => "Hereinafter",
        ContractKeyword::If => "If",
        ContractKeyword::Unless => "Unless",
        ContractKeyword::Provided => "Provided",
        ContractKeyword::SubjectTo => "SubjectTo",
        ContractKeyword::Party => "Party",
    }
    .to_string()
}

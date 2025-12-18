//! Clause aggregation to provide party-centric obligation rollups (Layer 8).
//!
//! Consumes [`ContractClause`] outputs and groups contiguous clauses by obligor
//! so downstream systems can ask "show me every duty for Party X" without
//! walking sentence-by-sentence.

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};

use crate::contract_clause::{ClauseCondition, ClauseDuty, ClauseParty, ContractClause};
use crate::utils::normalize_party_name;
use crate::scored::Scored;

/// Aggregated view of one party's duties across nearby clauses.
#[derive(Debug, Clone, PartialEq)]
pub struct ClauseAggregate {
    /// Deterministic identifier derived from the first clause in this aggregate.
    pub aggregate_id: u32,
    /// Party that owns the grouped clauses (copied from the first clause).
    pub obligor: ClauseParty,
    /// All clause IDs represented in this aggregate (preserves ordering).
    pub clause_ids: Vec<u32>,
    /// Detailed entries for each clause in the aggregate.
    pub clauses: Vec<ClauseAggregateEntry>,
    /// Start offset of the first clause.
    pub source_start: usize,
    /// End offset of the last clause.
    pub source_end: usize,
}

impl ClauseAggregate {
    /// Returns all conditions across all clauses in this aggregate.
    ///
    /// This is a convenience method for downstream consumers who need to check
    /// "what conditions apply to this party's obligations" without iterating
    /// through each clause entry individually.
    ///
    /// Note: conditions may be duplicated if they appear in multiple clauses.
    pub fn all_conditions(&self) -> Vec<&ClauseCondition> {
        self.clauses
            .iter()
            .flat_map(|entry| entry.conditions.iter())
            .collect()
    }

    /// Returns all unique condition texts across this aggregate.
    ///
    /// Deduplicates by condition text to avoid repetition when the same
    /// condition applies to multiple clauses.
    pub fn unique_conditions(&self) -> Vec<&ClauseCondition> {
        let mut seen = std::collections::HashSet::new();
        self.clauses
            .iter()
            .flat_map(|entry| entry.conditions.iter())
            .filter(|cond| seen.insert(&cond.text))
            .collect()
    }
}

/// Individual clause entry stored within an aggregate.
#[derive(Debug, Clone, PartialEq)]
pub struct ClauseAggregateEntry {
    /// Clause identifier from Layer 7.
    pub clause_id: u32,
    /// Duty metadata.
    pub duty: ClauseDuty,
    /// Conditions attached to this clause.
    pub conditions: Vec<ClauseCondition>,
    /// Clause-level confidence inherited from Layer 7.
    pub clause_confidence: f64,
}

/// Resolver settings controlling how aggregates are formed and scored.
pub struct ClauseAggregationResolver {
    /// Maximum token gap (based on clause offsets) allowed between clauses to stay in the same aggregate.
    max_gap_tokens: usize,
    /// Maximum span size (offset difference) before we assume the aggregate crosses sections.
    max_span_without_penalty: usize,
    /// Penalty when the obligor lacks a pronoun chain id (display-text fallback).
    missing_chain_penalty: f64,
    /// Penalty when an aggregate spans more than `max_span_without_penalty`.
    cross_section_penalty: f64,
}

impl Default for ClauseAggregationResolver {
    fn default() -> Self {
        Self {
            max_gap_tokens: 30,
            max_span_without_penalty: 40,
            missing_chain_penalty: 0.05,
            cross_section_penalty: 0.10,
        }
    }
}

impl ClauseAggregationResolver {
    /// Create a resolver with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a resolver with custom settings.
    pub fn with_settings(
        max_gap_tokens: usize,
        max_span_without_penalty: usize,
        missing_chain_penalty: f64,
        cross_section_penalty: f64,
    ) -> Self {
        Self {
            max_gap_tokens,
            max_span_without_penalty,
            missing_chain_penalty,
            cross_section_penalty,
        }
    }

    fn party_key(clause: &ContractClause) -> ClausePartyKey {
        ClausePartyKey {
            chain_id: clause.obligor.chain_id,
            normalized_name: normalize_party_name(&clause.obligor.display_text),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClausePartyKey {
    chain_id: Option<u32>,
    normalized_name: String,
}

impl ClausePartyKey {
    /// Relaxed matching for party keys.
    ///
    /// Two keys match if:
    /// 1. They are exactly equal (same chain_id and name), OR
    /// 2. They have the same normalized name AND at least one side has no chain_id
    ///
    /// This allows "Company" with chain_id=Some(1) to merge with "Company" with
    /// chain_id=None, but not with "Company" with chain_id=Some(2).
    fn matches_relaxed(&self, other: &ClausePartyKey) -> bool {
        if self == other {
            return true;
        }

        if self.normalized_name != other.normalized_name {
            return false;
        }

        // Same name - check if chains are compatible
        match (self.chain_id, other.chain_id) {
            (Some(a), Some(b)) => a == b, // Both have chain_id, must match
            _ => true,                     // At least one is None, allow merge
        }
    }
}

struct ClauseAggregateBuilder {
    anchor_selection: LLSelection,
    key: ClausePartyKey,
    obligor: ClauseParty,
    clause_ids: Vec<u32>,
    entries: Vec<ClauseAggregateEntry>,
    source_start: usize,
    source_end: usize,
    last_offset: usize,
}

impl ClauseAggregateBuilder {
    fn new(
        selection: LLSelection,
        clause: &Scored<ContractClause>,
        key: ClausePartyKey,
    ) -> Self {
        let entry = ClauseAggregateEntry {
            clause_id: clause.value.clause_id,
            duty: clause.value.duty.clone(),
            conditions: clause.value.conditions.clone(),
            clause_confidence: clause.confidence,
        };

        Self {
            anchor_selection: selection,
            key,
            obligor: clause.value.obligor.clone(),
            clause_ids: vec![clause.value.clause_id],
            entries: vec![entry],
            source_start: clause.value.source_offset,
            source_end: clause.value.source_offset,
            last_offset: clause.value.source_offset,
        }
    }

    /// Check if a new clause can be merged into this aggregate.
    ///
    /// Uses relaxed matching for party keys to allow merging when one side
    /// has no chain_id (e.g., a plain noun phrase vs a pronoun-resolved reference).
    fn can_merge(&self, key: &ClausePartyKey, offset: usize, max_gap_tokens: usize) -> bool {
        self.key.matches_relaxed(key) && offset.saturating_sub(self.last_offset) <= max_gap_tokens
    }

    fn add_clause(&mut self, clause: &Scored<ContractClause>) {
        self.clause_ids.push(clause.value.clause_id);
        self.entries.push(ClauseAggregateEntry {
            clause_id: clause.value.clause_id,
            duty: clause.value.duty.clone(),
            conditions: clause.value.conditions.clone(),
            clause_confidence: clause.confidence,
        });
        self.last_offset = clause.value.source_offset;
        self.source_end = clause.value.source_offset;
    }

    fn span(&self) -> usize {
        self.source_end.saturating_sub(self.source_start)
    }

    fn build(self, aggregate_id: u32, settings: &ClauseAggregationResolver) -> (ClauseAggregate, f64) {
        let min_clause_conf = self
            .entries
            .iter()
            .map(|entry| entry.clause_confidence)
            .fold(1.0, f64::min);

        let mut confidence = min_clause_conf;
        if self.obligor.chain_id.is_none() {
            confidence -= settings.missing_chain_penalty;
        }
        if self.entries.len() > 1 && self.span() > settings.max_span_without_penalty {
            confidence -= settings.cross_section_penalty;
        }
        confidence = confidence.clamp(0.0, 1.0);

        (
            ClauseAggregate {
                aggregate_id,
                obligor: self.obligor,
                clause_ids: self.clause_ids,
                clauses: self.entries,
                source_start: self.source_start,
                source_end: self.source_end,
            },
            confidence,
        )
    }
}

impl Resolver for ClauseAggregationResolver {
    type Attr = Scored<ClauseAggregate>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut clauses: Vec<_> = selection
            .find_by(&x::attr::<Scored<ContractClause>>())
            .into_iter()
            .collect();

        if clauses.is_empty() {
            return Vec::new();
        }

        clauses.sort_by_key(|(_, clause)| clause.value.source_offset);

        let mut results = Vec::new();
        let mut next_id = 1u32;
        let mut current_builder: Option<ClauseAggregateBuilder> = None;

        for (sel, clause) in clauses {
            let key = Self::party_key(&clause.value);
            let offset = clause.value.source_offset;

            if let Some(builder) = current_builder.as_mut() {
                if builder.can_merge(&key, offset, self.max_gap_tokens) {
                    builder.add_clause(&clause);
                    continue;
                }

                let finished = current_builder.take().unwrap();
                let anchor = finished.anchor_selection.clone();
                let (aggregate, confidence) = finished.build(next_id, self);
                next_id += 1;
                results.push(
                    anchor.finish_with_attr(Scored::derived(aggregate, confidence)),
                );
            }

            current_builder = Some(ClauseAggregateBuilder::new(sel.clone(), &clause, key));
        }

        if let Some(finished) = current_builder.take() {
            let anchor = finished.anchor_selection.clone();
            let (aggregate, confidence) = finished.build(next_id, self);
            results.push(anchor.finish_with_attr(Scored::derived(aggregate, confidence)));
        }

        results
    }
}

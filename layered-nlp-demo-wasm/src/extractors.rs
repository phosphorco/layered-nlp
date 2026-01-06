//! Extraction functions for each span type.
//!
//! These functions extract spans from resolved LLLines for demo visualization.

use layered_contracts::{
    ClauseAggregate, ContractClause, ContractKeyword, DefinedTerm, ObligationNode,
    ObligationPhrase, PronounChain, PronounReference, Scored, TermReference,
};
use layered_deixis::{DeicticCategory, DeicticReference, DeicticSubcategory};
use layered_nlp::{x, LLLine};

use crate::manifest::RawSpan;

/// Extract contract keywords (shall, must, if, etc.)
pub fn extract_contract_keywords(ll_line: &LLLine) -> Vec<RawSpan> {
    ll_line
        .find(&x::attr::<ContractKeyword>())
        .into_iter()
        .map(|find| {
            let (start, end) = find.range();
            RawSpan {
                start: start as u32,
                end: end as u32,
                label: format!("{:?}", find.attr()),
                metadata: None,
                associations: vec![],
            }
        })
        .collect()
}

/// Extract defined terms ("Agreement", "Tenant", etc.)
pub fn extract_defined_terms(ll_line: &LLLine) -> Vec<RawSpan> {
    ll_line
        .find(&x::attr::<Scored<DefinedTerm>>())
        .into_iter()
        .map(|find| {
            let (start, end) = find.range();
            let scored = find.attr();
            RawSpan {
                start: start as u32,
                end: end as u32,
                label: scored.value.term_name.clone(),
                metadata: Some(serde_json::json!({
                    "confidence": scored.confidence,
                    "definition_type": format!("{:?}", scored.value.definition_type),
                })),
                associations: vec![],
            }
        })
        .collect()
}

/// Extract term references
pub fn extract_term_references(ll_line: &LLLine) -> Vec<RawSpan> {
    ll_line
        .find(&x::attr::<Scored<TermReference>>())
        .into_iter()
        .map(|find| {
            let (start, end) = find.range();
            let scored = find.attr();
            RawSpan {
                start: start as u32,
                end: end as u32,
                label: scored.value.term_name.clone(),
                metadata: Some(serde_json::json!({
                    "confidence": scored.confidence,
                    "definition_type": format!("{:?}", scored.value.definition_type),
                })),
                associations: vec![],
            }
        })
        .collect()
}

/// Extract pronoun references with resolution candidates
pub fn extract_pronoun_references(ll_line: &LLLine) -> Vec<RawSpan> {
    ll_line
        .find(&x::attr::<Scored<PronounReference>>())
        .into_iter()
        .map(|find| {
            let (start, end) = find.range();
            let scored = find.attr();
            let resolved = scored.value.candidates.first();
            RawSpan {
                start: start as u32,
                end: end as u32,
                label: scored.value.pronoun.clone(),
                metadata: Some(serde_json::json!({
                    "confidence": scored.confidence,
                    "pronoun_type": format!("{:?}", scored.value.pronoun_type),
                    "resolved_to": resolved.map(|c| c.text.clone()),
                    "resolution_confidence": resolved.map(|c| c.confidence),
                })),
                associations: vec![],
            }
        })
        .collect()
}

/// Extract obligation phrases with conditions
pub fn extract_obligation_phrases(ll_line: &LLLine) -> Vec<RawSpan> {
    // Note: Full provenance tracking via query_with_associations deferred to Gate 3
    // when associations can be properly serialized
    ll_line
        .find(&x::attr::<Scored<ObligationPhrase>>())
        .into_iter()
        .map(|find| {
            let (start, end) = find.range();
            let scored = find.attr();
            RawSpan {
                start: start as u32,
                end: end as u32,
                label: format!("{:?}", scored.value.obligation_type),
                metadata: Some(serde_json::json!({
                    "confidence": scored.confidence,
                    "obligor": format!("{:?}", scored.value.obligor),
                    "action": scored.value.action.clone(),
                    "conditions": scored.value.conditions.iter().map(|c| {
                        serde_json::json!({
                            "type": format!("{:?}", c.condition_type),
                            "preview": c.text_preview.clone(),
                        })
                    }).collect::<Vec<_>>(),
                })),
                associations: vec![],
            }
        })
        .collect()
}

/// Extract pronoun chains
pub fn extract_pronoun_chains(ll_line: &LLLine) -> Vec<RawSpan> {
    ll_line
        .find(&x::attr::<Scored<PronounChain>>())
        .into_iter()
        .map(|find| {
            let (start, end) = find.range();
            let scored = find.attr();
            RawSpan {
                start: start as u32,
                end: end as u32,
                label: scored.value.canonical_name.clone(),
                metadata: Some(serde_json::json!({
                    "confidence": scored.confidence,
                    "chain_id": scored.value.chain_id,
                    "is_defined_term": scored.value.is_defined_term,
                    "mention_count": scored.value.mentions.len(),
                    "has_verified_mention": scored.value.has_verified_mention,
                })),
                associations: vec![],
            }
        })
        .collect()
}

/// Extract contract clauses
pub fn extract_contract_clauses(ll_line: &LLLine) -> Vec<RawSpan> {
    ll_line
        .find(&x::attr::<Scored<ContractClause>>())
        .into_iter()
        .map(|find| {
            let (start, end) = find.range();
            let scored = find.attr();
            RawSpan {
                start: start as u32,
                end: end as u32,
                label: format!(
                    "{}: {:?}",
                    scored.value.obligor.display_text, scored.value.duty.obligation_type
                ),
                metadata: Some(serde_json::json!({
                    "confidence": scored.confidence,
                    "clause_id": scored.value.clause_id,
                    "obligor": scored.value.obligor.display_text.clone(),
                    "obligor_chain_id": scored.value.obligor.chain_id,
                    "obligation_type": format!("{:?}", scored.value.duty.obligation_type),
                    "action": scored.value.duty.action.clone(),
                    "conditions": scored.value.conditions.iter().map(|c| {
                        serde_json::json!({
                            "type": format!("{:?}", c.condition_type),
                            "text": c.text.clone(),
                            "mentions_unknown_entity": c.mentions_unknown_entity,
                        })
                    }).collect::<Vec<_>>(),
                })),
                associations: vec![],
            }
        })
        .collect()
}

/// Extract clause aggregates
pub fn extract_clause_aggregates(ll_line: &LLLine) -> Vec<RawSpan> {
    ll_line
        .find(&x::attr::<Scored<ClauseAggregate>>())
        .into_iter()
        .map(|find| {
            let (start, end) = find.range();
            let scored = find.attr();
            RawSpan {
                start: start as u32,
                end: end as u32,
                label: format!(
                    "{} ({})",
                    scored.value.obligor.display_text,
                    scored.value.clause_ids.len()
                ),
                metadata: Some(serde_json::json!({
                    "confidence": scored.confidence,
                    "aggregate_id": scored.value.aggregate_id,
                    "obligor": scored.value.obligor.display_text.clone(),
                    "obligor_chain_id": scored.value.obligor.chain_id,
                    "clause_count": scored.value.clause_ids.len(),
                    "clause_ids": scored.value.clause_ids.clone(),
                })),
                associations: vec![],
            }
        })
        .collect()
}

/// Extract obligation nodes (accountability graph)
pub fn extract_obligation_nodes(ll_line: &LLLine) -> Vec<RawSpan> {
    ll_line
        .find(&x::attr::<Scored<ObligationNode>>())
        .into_iter()
        .map(|find| {
            let (start, end) = find.range();
            let scored = find.attr();
            RawSpan {
                start: start as u32,
                end: end as u32,
                label: format!(
                    "{} → {} beneficiaries",
                    scored.value.obligor.display_text,
                    scored.value.beneficiaries.len()
                ),
                metadata: Some(serde_json::json!({
                    "confidence": scored.confidence,
                    "node_id": scored.value.node_id,
                    "obligor": scored.value.obligor.display_text.clone(),
                    "obligor_chain_id": scored.value.obligor.chain_id,
                    "beneficiaries": scored.value.beneficiaries.iter().map(|b| {
                        serde_json::json!({
                            "display_text": b.display_text.clone(),
                            "chain_id": b.chain_id,
                            "needs_verification": b.needs_verification,
                        })
                    }).collect::<Vec<_>>(),
                    "condition_count": scored.value.condition_links.len(),
                    "clause_count": scored.value.clauses.len(),
                    "confidence_breakdown": scored.value.confidence_breakdown.clone(),
                })),
                associations: vec![],
            }
        })
        .collect()
}

/// Extract deictic references (pronouns, place, time, discourse markers)
pub fn extract_deictic_references(ll_line: &LLLine) -> Vec<RawSpan> {
    ll_line
        .find(&x::attr::<DeicticReference>())
        .into_iter()
        .map(|find| {
            let (start, end) = find.range();
            let deictic = find.attr();
            RawSpan {
                start: start as u32,
                end: end as u32,
                label: format_deictic_label(&deictic.category, &deictic.subcategory),
                metadata: Some(serde_json::json!({
                    "category": format!("{:?}", deictic.category),
                    "subcategory": format!("{:?}", deictic.subcategory),
                    "surface_text": deictic.surface_text.clone(),
                    "confidence": deictic.confidence,
                    "source": format_deictic_source(&deictic.source),
                    "resolved_referent": deictic.resolved_referent.as_ref().map(|r| {
                        serde_json::json!({
                            "text": r.text.clone(),
                            "confidence": r.resolution_confidence,
                        })
                    }),
                })),
                associations: vec![],
            }
        })
        .collect()
}

fn format_deictic_label(category: &DeicticCategory, subcategory: &DeicticSubcategory) -> String {
    use DeicticCategory as Cat;
    use DeicticSubcategory as Sub;

    let cat = match category {
        Cat::Person => "Person",
        Cat::Place => "Place",
        Cat::Time => "Time",
        Cat::Discourse => "Discourse",
        Cat::Social => "Social",
    };

    let sub = match subcategory {
        Sub::PersonFirst => "1st",
        Sub::PersonFirstPlural => "1st plural",
        Sub::PersonSecond => "2nd",
        Sub::PersonSecondPlural => "2nd plural",
        Sub::PersonThirdSingular => "3rd singular",
        Sub::PersonThirdPlural => "3rd plural",
        Sub::PersonRelative => "relative",
        Sub::PlaceProximal => "proximal",
        Sub::PlaceDistal => "distal",
        Sub::PlaceOther => "other",
        Sub::TimePresent => "present",
        Sub::TimePast => "past",
        Sub::TimeFuture => "future",
        Sub::TimeRelative => "relative",
        Sub::TimeDefinedTerm => "defined term",
        Sub::TimeDuration => "duration",
        Sub::TimeDeadline => "deadline",
        Sub::DiscourseThisDocument => "this document",
        Sub::DiscourseSectionRef => "section ref",
        Sub::DiscourseAnaphoric => "anaphoric",
        Sub::DiscourseCataphoric => "cataphoric",
        Sub::DiscourseMarker => "marker",
        Sub::SocialFormal => "formal",
        Sub::SocialInformal => "informal",
        Sub::Other => "other",
    };

    format!("{} ({})", cat, sub)
}

fn format_deictic_source(source: &layered_deixis::DeicticSource) -> serde_json::Value {
    use layered_deixis::DeicticSource;
    match source {
        DeicticSource::WordList { pattern } => serde_json::json!({
            "type": "WordList",
            "pattern": pattern,
        }),
        DeicticSource::PronounResolver => serde_json::json!({ "type": "PronounResolver" }),
        DeicticSource::TemporalResolver => serde_json::json!({ "type": "TemporalResolver" }),
        DeicticSource::SectionReferenceResolver => {
            serde_json::json!({ "type": "SectionReferenceResolver" })
        }
        DeicticSource::POSTag => serde_json::json!({ "type": "POSTag" }),
        DeicticSource::Derived => serde_json::json!({ "type": "Derived" }),
    }
}

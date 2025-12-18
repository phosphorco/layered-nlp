use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use layered_contracts::{
    ClauseAggregate, ClauseAggregationResolver, ContractClause, ContractClauseResolver,
    ContractKeyword, ContractKeywordResolver, DefinedTerm, DefinedTermResolver,
    ObligationNode, AccountabilityGraphResolver, ObligationPhrase, ObligationPhraseResolver,
    ProhibitionResolver, PronounChain, PronounChainResolver, PronounReference, PronounResolver,
    Scored, TermReference, TermReferenceResolver,
};
use layered_nlp::{create_line_from_string, x};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub start_offset: u32,
    pub end_offset: u32,
    pub label: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub text: String,
    pub spans: Vec<Span>,
    pub version: String,
}

#[wasm_bindgen]
pub fn analyze_contract(text: &str) -> JsValue {
    let result = analyze_contract_internal(text);
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

fn analyze_contract_internal(text: &str) -> AnalysisResult {
    // Run the full 9-layer resolver stack
    let ll_line = create_line_from_string(text)
        // Layer 1-2: Keywords
        .run(&ContractKeywordResolver::new())
        .run(&ProhibitionResolver::new())
        // Layer 3: Defined Terms
        .run(&DefinedTermResolver::new())
        // Layer 4: Term References
        .run(&TermReferenceResolver::new())
        // Layer 5: Pronouns
        .run(&PronounResolver::new())
        // Layer 6: Obligation Phrases
        .run(&ObligationPhraseResolver::new())
        // Layer 7: Pronoun Chains (coreference)
        .run(&PronounChainResolver::new())
        // Layer 8: Contract Clauses
        .run(&ContractClauseResolver::new())
        // Layer 9: Clause Aggregates
        .run(&ClauseAggregationResolver::new())
        // Layer 10: Accountability Graph
        .run(&AccountabilityGraphResolver::new());

    let mut spans = Vec::new();

    // Layer 1-2: ContractKeyword spans
    for find in ll_line.find(&x::attr::<ContractKeyword>()) {
        let (start, end) = find.range();
        spans.push(Span {
            start_offset: start as u32,
            end_offset: end as u32,
            label: format!("{:?}", find.attr()),
            kind: "ContractKeyword".to_string(),
            metadata: None,
        });
    }

    // Layer 3: DefinedTerm spans
    for find in ll_line.find(&x::attr::<Scored<DefinedTerm>>()) {
        let (start, end) = find.range();
        let scored = find.attr();
        spans.push(Span {
            start_offset: start as u32,
            end_offset: end as u32,
            label: scored.value.term_name.clone(),
            kind: "DefinedTerm".to_string(),
            metadata: Some(serde_json::json!({
                "confidence": scored.confidence,
                "definition_type": format!("{:?}", scored.value.definition_type),
            })),
        });
    }

    // Layer 4: TermReference spans
    for find in ll_line.find(&x::attr::<Scored<TermReference>>()) {
        let (start, end) = find.range();
        let scored = find.attr();
        spans.push(Span {
            start_offset: start as u32,
            end_offset: end as u32,
            label: scored.value.term_name.clone(),
            kind: "TermReference".to_string(),
            metadata: Some(serde_json::json!({
                "confidence": scored.confidence,
                "definition_type": format!("{:?}", scored.value.definition_type),
            })),
        });
    }

    // Layer 5: PronounReference spans
    for find in ll_line.find(&x::attr::<Scored<PronounReference>>()) {
        let (start, end) = find.range();
        let scored = find.attr();
        let best_candidate = scored.value.candidates.first();
        spans.push(Span {
            start_offset: start as u32,
            end_offset: end as u32,
            label: scored.value.pronoun.clone(),
            kind: "PronounReference".to_string(),
            metadata: Some(serde_json::json!({
                "confidence": scored.confidence,
                "pronoun_type": format!("{:?}", scored.value.pronoun_type),
                "resolved_to": best_candidate.map(|c| &c.text),
                "resolution_confidence": best_candidate.map(|c| c.confidence),
            })),
        });
    }

    // Layer 6: ObligationPhrase spans
    for find in ll_line.find(&x::attr::<Scored<ObligationPhrase>>()) {
        let (start, end) = find.range();
        let scored = find.attr();
        spans.push(Span {
            start_offset: start as u32,
            end_offset: end as u32,
            label: format!("{:?}", scored.value.obligation_type),
            kind: "ObligationPhrase".to_string(),
            metadata: Some(serde_json::json!({
                "confidence": scored.confidence,
                "obligor": format!("{:?}", scored.value.obligor),
                "action": scored.value.action,
                "conditions": scored.value.conditions.iter()
                    .map(|c| serde_json::json!({
                        "type": format!("{:?}", c.condition_type),
                        "preview": c.text_preview,
                    }))
                    .collect::<Vec<_>>(),
            })),
        });
    }

    // Layer 7: PronounChain spans
    for find in ll_line.find(&x::attr::<Scored<PronounChain>>()) {
        let (start, end) = find.range();
        let scored = find.attr();
        spans.push(Span {
            start_offset: start as u32,
            end_offset: end as u32,
            label: scored.value.canonical_name.clone(),
            kind: "PronounChain".to_string(),
            metadata: Some(serde_json::json!({
                "confidence": scored.confidence,
                "chain_id": scored.value.chain_id,
                "is_defined_term": scored.value.is_defined_term,
                "mention_count": scored.value.mentions.len(),
                "has_verified_mention": scored.value.has_verified_mention,
            })),
        });
    }

    // Layer 8: ContractClause spans
    for find in ll_line.find(&x::attr::<Scored<ContractClause>>()) {
        let (start, end) = find.range();
        let scored = find.attr();
        spans.push(Span {
            start_offset: start as u32,
            end_offset: end as u32,
            label: format!("{}: {:?}", scored.value.obligor.display_text, scored.value.duty.obligation_type),
            kind: "ContractClause".to_string(),
            metadata: Some(serde_json::json!({
                "confidence": scored.confidence,
                "clause_id": scored.value.clause_id,
                "obligor": scored.value.obligor.display_text,
                "obligor_chain_id": scored.value.obligor.chain_id,
                "obligation_type": format!("{:?}", scored.value.duty.obligation_type),
                "action": scored.value.duty.action,
                "conditions": scored.value.conditions.iter()
                    .map(|c| serde_json::json!({
                        "type": format!("{:?}", c.condition_type),
                        "text": c.text,
                        "mentions_unknown_entity": c.mentions_unknown_entity,
                    }))
                    .collect::<Vec<_>>(),
            })),
        });
    }

    // Layer 9: ClauseAggregate spans
    for find in ll_line.find(&x::attr::<Scored<ClauseAggregate>>()) {
        let (start, end) = find.range();
        let scored = find.attr();
        spans.push(Span {
            start_offset: start as u32,
            end_offset: end as u32,
            label: format!("{} ({})", scored.value.obligor.display_text, scored.value.clause_ids.len()),
            kind: "ClauseAggregate".to_string(),
            metadata: Some(serde_json::json!({
                "confidence": scored.confidence,
                "aggregate_id": scored.value.aggregate_id,
                "obligor": scored.value.obligor.display_text,
                "obligor_chain_id": scored.value.obligor.chain_id,
                "clause_count": scored.value.clause_ids.len(),
                "clause_ids": scored.value.clause_ids,
            })),
        });
    }

    // Layer 10: ObligationNode spans (accountability graph)
    for find in ll_line.find(&x::attr::<Scored<ObligationNode>>()) {
        let (start, end) = find.range();
        let scored = find.attr();
        spans.push(Span {
            start_offset: start as u32,
            end_offset: end as u32,
            label: format!("{} → {} beneficiaries", 
                scored.value.obligor.display_text, 
                scored.value.beneficiaries.len()
            ),
            kind: "ObligationNode".to_string(),
            metadata: Some(serde_json::json!({
                "confidence": scored.confidence,
                "node_id": scored.value.node_id,
                "obligor": scored.value.obligor.display_text,
                "obligor_chain_id": scored.value.obligor.chain_id,
                "beneficiaries": scored.value.beneficiaries.iter()
                    .map(|b| serde_json::json!({
                        "display_text": b.display_text,
                        "chain_id": b.chain_id,
                        "needs_verification": b.needs_verification,
                    }))
                    .collect::<Vec<_>>(),
                "condition_count": scored.value.condition_links.len(),
                "clause_count": scored.value.clauses.len(),
                "confidence_breakdown": scored.value.confidence_breakdown,
            })),
        });
    }

    AnalysisResult {
        text: text.to_string(),
        spans,
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

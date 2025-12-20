use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use layered_contracts::{
    ClauseAggregate, ClauseAggregationResolver, ContractClause, ContractClauseResolver,
    ContractKeyword, ContractKeywordResolver, DefinedTerm, DefinedTermResolver,
    ObligationNode, AccountabilityGraphResolver, ObligationPhrase, ObligationPhraseResolver,
    ProhibitionResolver, PronounChain, PronounChainResolver, PronounReference, PronounResolver,
    Scored, TermReference, TermReferenceResolver,
};
use layered_deixis::{
    DeicticCategory, DeicticReference, DeicticSubcategory, DiscourseMarkerResolver,
    PersonPronounResolver, PlaceDeicticResolver, SimpleTemporalResolver,
};
use layered_nlp::{create_line_from_string, x};

/// Format DeicticSubcategory as human-readable label
fn format_deictic_label(category: &DeicticCategory, subcategory: &DeicticSubcategory) -> String {
    let cat = match category {
        DeicticCategory::Person => "Person",
        DeicticCategory::Place => "Place",
        DeicticCategory::Time => "Time",
        DeicticCategory::Discourse => "Discourse",
        DeicticCategory::Social => "Social",
    };

    let sub = match subcategory {
        // Person
        DeicticSubcategory::PersonFirst => "1st",
        DeicticSubcategory::PersonFirstPlural => "1st plural",
        DeicticSubcategory::PersonSecond => "2nd",
        DeicticSubcategory::PersonSecondPlural => "2nd plural",
        DeicticSubcategory::PersonThirdSingular => "3rd singular",
        DeicticSubcategory::PersonThirdPlural => "3rd plural",
        DeicticSubcategory::PersonRelative => "relative",
        // Place
        DeicticSubcategory::PlaceProximal => "proximal",
        DeicticSubcategory::PlaceDistal => "distal",
        DeicticSubcategory::PlaceOther => "other",
        // Time
        DeicticSubcategory::TimePresent => "present",
        DeicticSubcategory::TimePast => "past",
        DeicticSubcategory::TimeFuture => "future",
        DeicticSubcategory::TimeRelative => "relative",
        DeicticSubcategory::TimeDefinedTerm => "defined term",
        DeicticSubcategory::TimeDuration => "duration",
        DeicticSubcategory::TimeDeadline => "deadline",
        // Discourse
        DeicticSubcategory::DiscourseThisDocument => "this document",
        DeicticSubcategory::DiscourseSectionRef => "section ref",
        DeicticSubcategory::DiscourseAnaphoric => "anaphoric",
        DeicticSubcategory::DiscourseCataphoric => "cataphoric",
        DeicticSubcategory::DiscourseMarker => "marker",
        // Social
        DeicticSubcategory::SocialFormal => "formal",
        DeicticSubcategory::SocialInformal => "informal",
        // Other
        DeicticSubcategory::Other => "other",
    };

    format!("{} ({})", cat, sub)
}

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
    // Run the full resolver stack
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
        .run(&AccountabilityGraphResolver::new())
        // Layer 11-14: Deixis (simple word-list resolvers)
        // These fill gaps for deictic expressions not covered by contract-specific resolvers
        .run(&PersonPronounResolver::new())
        .run(&PlaceDeicticResolver::new())
        .run(&SimpleTemporalResolver::new())
        .run(&DiscourseMarkerResolver::new());

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

    // Layer 6: ObligationPhrase spans (with provenance associations)
    // Build associations lookup once, then match by pointer equality
    let obligation_associations: Vec<_> = ll_line
        .query_with_associations::<Scored<ObligationPhrase>>()
        .into_iter()
        .flat_map(|(_, _, attrs_with_assocs)| attrs_with_assocs)
        .collect();

    for find in ll_line.find(&x::attr::<Scored<ObligationPhrase>>()) {
        let (start, end) = find.range();
        let scored = find.attr();

        // Match by pointer equality - both find() and query_with_associations()
        // return references to the same stored values
        let scored_ptr: *const Scored<ObligationPhrase> = *scored;
        let provenance: Vec<serde_json::Value> = obligation_associations
            .iter()
            .filter(|(attr, _)| std::ptr::eq(*attr as *const _, scored_ptr))
            .flat_map(|(_, assocs)| assocs.iter())
            .map(|assoc| serde_json::json!({
                "label": assoc.label(),
                "glyph": assoc.glyph(),
                "target_token_range": [assoc.span.start_idx, assoc.span.end_idx],
            }))
            .collect();

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
                "provenance": provenance,
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

    // Layer 11-14: DeicticReference spans (from simple word-list resolvers)
    for find in ll_line.find(&x::attr::<DeicticReference>()) {
        let (start, end) = find.range();
        let deictic = find.attr();

        // Format source for display
        let source_display = match &deictic.source {
            layered_deixis::DeicticSource::WordList { pattern } => {
                serde_json::json!({ "type": "WordList", "pattern": pattern })
            }
            layered_deixis::DeicticSource::PronounResolver => {
                serde_json::json!({ "type": "PronounResolver" })
            }
            layered_deixis::DeicticSource::TemporalResolver => {
                serde_json::json!({ "type": "TemporalResolver" })
            }
            layered_deixis::DeicticSource::SectionReferenceResolver => {
                serde_json::json!({ "type": "SectionReferenceResolver" })
            }
            layered_deixis::DeicticSource::POSTag => {
                serde_json::json!({ "type": "POSTag" })
            }
            layered_deixis::DeicticSource::Derived => {
                serde_json::json!({ "type": "Derived" })
            }
        };

        // Format resolved referent if present
        let referent_json = deictic.resolved_referent.as_ref().map(|r| {
            serde_json::json!({
                "text": r.text,
                "confidence": r.resolution_confidence,
            })
        });

        spans.push(Span {
            start_offset: start as u32,
            end_offset: end as u32,
            label: format_deictic_label(&deictic.category, &deictic.subcategory),
            kind: "DeicticReference".to_string(),
            metadata: Some(serde_json::json!({
                "category": format!("{:?}", deictic.category),
                "subcategory": format!("{:?}", deictic.subcategory),
                "surface_text": deictic.surface_text,
                "confidence": deictic.confidence,
                "source": source_display,
                "resolved_referent": referent_json,
            })),
        });
    }

    AnalysisResult {
        text: text.to_string(),
        spans,
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that simple personal pronouns are detected as deixis
    #[test]
    fn test_deixis_simple_pronouns() {
        let result = analyze_contract_internal("I will meet you there tomorrow.");

        let deixis_spans: Vec<_> = result.spans.iter()
            .filter(|s| s.kind == "DeicticReference")
            .collect();

        // Should detect: "I" (PersonFirst), "you" (PersonSecond), "there" (PlaceDistal), "tomorrow" (TimeFuture)
        assert!(deixis_spans.len() >= 4, "Expected at least 4 deixis spans, got {}", deixis_spans.len());

        // Check for person deixis
        let person_spans: Vec<_> = deixis_spans.iter()
            .filter(|s| {
                s.metadata.as_ref()
                    .and_then(|m| m.get("category"))
                    .map(|c| c.as_str() == Some("Person"))
                    .unwrap_or(false)
            })
            .collect();
        assert!(person_spans.len() >= 2, "Expected at least 2 person deixis spans");

        // Check for place deixis ("there")
        let place_spans: Vec<_> = deixis_spans.iter()
            .filter(|s| {
                s.metadata.as_ref()
                    .and_then(|m| m.get("category"))
                    .map(|c| c.as_str() == Some("Place"))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(place_spans.len(), 1, "Expected 1 place deixis span");

        // Check for time deixis ("tomorrow")
        let time_spans: Vec<_> = deixis_spans.iter()
            .filter(|s| {
                s.metadata.as_ref()
                    .and_then(|m| m.get("category"))
                    .map(|c| c.as_str() == Some("Time"))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(time_spans.len(), 1, "Expected 1 time deixis span");
    }

    /// Test that discourse markers are detected
    #[test]
    fn test_deixis_discourse_markers() {
        let result = analyze_contract_internal(
            "However, the contract is valid. Therefore, we proceed."
        );

        let discourse_spans: Vec<_> = result.spans.iter()
            .filter(|s| {
                s.kind == "DeicticReference" &&
                s.metadata.as_ref()
                    .and_then(|m| m.get("category"))
                    .map(|c| c.as_str() == Some("Discourse"))
                    .unwrap_or(false)
            })
            .collect();

        // Should detect: "However" and "Therefore"
        assert_eq!(discourse_spans.len(), 2, "Expected 2 discourse markers");

        // Check that "However" was detected
        let however = discourse_spans.iter()
            .find(|s| s.metadata.as_ref()
                .and_then(|m| m.get("surface_text"))
                .map(|t| t.as_str() == Some("However"))
                .unwrap_or(false));
        assert!(however.is_some(), "Expected 'However' to be detected");

        // Check that "Therefore" was detected
        let therefore = discourse_spans.iter()
            .find(|s| s.metadata.as_ref()
                .and_then(|m| m.get("surface_text"))
                .map(|t| t.as_str() == Some("Therefore"))
                .unwrap_or(false));
        assert!(therefore.is_some(), "Expected 'Therefore' to be detected");
    }

    /// Test that metadata structure is complete
    #[test]
    fn test_deixis_metadata_structure() {
        let result = analyze_contract_internal("I am here now.");

        let deixis_spans: Vec<_> = result.spans.iter()
            .filter(|s| s.kind == "DeicticReference")
            .collect();

        assert!(!deixis_spans.is_empty(), "Expected deixis spans");

        for span in deixis_spans {
            let metadata = span.metadata.as_ref().expect("Metadata should be present");

            // Check all required fields exist
            assert!(metadata.get("category").is_some(), "Missing category");
            assert!(metadata.get("subcategory").is_some(), "Missing subcategory");
            assert!(metadata.get("surface_text").is_some(), "Missing surface_text");
            assert!(metadata.get("confidence").is_some(), "Missing confidence");
            assert!(metadata.get("source").is_some(), "Missing source");

            // Check confidence is a valid number
            let confidence = metadata.get("confidence").unwrap().as_f64().unwrap();
            assert!(confidence >= 0.0 && confidence <= 1.0, "Confidence should be 0-1");

            // Check source has type field
            let source = metadata.get("source").unwrap();
            assert!(source.get("type").is_some(), "Source should have type field");
        }
    }

    /// Test that no duplicate spans exist for the same token
    #[test]
    fn test_deixis_no_duplicates() {
        let result = analyze_contract_internal("I will be there.");

        let deixis_spans: Vec<_> = result.spans.iter()
            .filter(|s| s.kind == "DeicticReference")
            .collect();

        // Check for duplicates by (start_offset, end_offset) pairs
        let mut seen_ranges: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
        for span in &deixis_spans {
            let range = (span.start_offset, span.end_offset);
            assert!(
                seen_ranges.insert(range),
                "Duplicate deixis span at offsets {:?}", range
            );
        }
    }

    /// Test edge cases with neutral text (no deictic words)
    #[test]
    fn test_deixis_neutral_text() {
        let result = analyze_contract_internal("Contract terms apply.");
        // No simple deictic words in this sentence
        let deixis_count = result.spans.iter().filter(|s| s.kind == "DeicticReference").count();
        assert_eq!(deixis_count, 0, "Expected no deixis in neutral text");

        let result = analyze_contract_internal("The parties agree to binding arbitration.");
        let deixis_count = result.spans.iter().filter(|s| s.kind == "DeicticReference").count();
        assert_eq!(deixis_count, 0, "Expected no deixis in neutral text");
    }

    /// Integration test with sample contract text from the plan
    #[test]
    fn test_deixis_sample_contract() {
        let sample = r#"This Agreement ("Agreement") is entered into as of the Effective Date.

The Company shall deliver the Product within 30 days. It must comply with
all applicable regulations. We agree to the terms herein.

However, if the Company fails to deliver, the Buyer may terminate this
Agreement. Therefore, timely performance is essential.

I, the undersigned, represent that the information provided here is accurate.
You shall notify us of any changes. They must be submitted in writing."#;

        let result = analyze_contract_internal(sample);

        let deixis_spans: Vec<_> = result.spans.iter()
            .filter(|s| s.kind == "DeicticReference")
            .collect();

        // Count by category
        let person_count = deixis_spans.iter()
            .filter(|s| s.metadata.as_ref()
                .and_then(|m| m.get("category"))
                .map(|c| c.as_str() == Some("Person"))
                .unwrap_or(false))
            .count();

        let discourse_count = deixis_spans.iter()
            .filter(|s| s.metadata.as_ref()
                .and_then(|m| m.get("category"))
                .map(|c| c.as_str() == Some("Discourse"))
                .unwrap_or(false))
            .count();

        let place_count = deixis_spans.iter()
            .filter(|s| s.metadata.as_ref()
                .and_then(|m| m.get("category"))
                .map(|c| c.as_str() == Some("Place"))
                .unwrap_or(false))
            .count();

        // Expected: We, I, You, us, They (person), However, Therefore (discourse), here (place)
        assert!(person_count >= 4, "Expected at least 4 person deixis, got {}", person_count);
        assert!(discourse_count >= 2, "Expected at least 2 discourse markers, got {}", discourse_count);
        assert!(place_count >= 1, "Expected at least 1 place deixis, got {}", place_count);
    }

    /// Performance benchmark - measures analysis time for sample contract
    /// Run with: cargo test -p layered-nlp-demo-wasm -- --nocapture test_performance
    #[test]
    fn test_performance_benchmark() {
        let sample = r#"SERVICES AGREEMENT

"Company" means ABC Corporation (the "ABC"). "Contractor" means XYZ Services LLC (the "Provider").

The Contractor shall provide consulting services to the Company. The Company shall pay the Contractor within 30 days of invoice receipt. We agree to the terms herein.

However, the Company may terminate this agreement if Contractor fails to perform. It shall provide 30 days written notice. Therefore, timely performance is essential.

I, the undersigned, represent that the information provided here is accurate. You shall notify us of any changes.

Unless otherwise agreed, the Contractor shall maintain confidentiality. Subject to applicable law, the Provider shall indemnify the Company."#;

        // Warm-up run
        let _ = analyze_contract_internal(sample);

        // Timed runs
        let iterations = 100;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let _ = analyze_contract_internal(sample);
        }
        let elapsed = start.elapsed();

        let avg_micros = elapsed.as_micros() as f64 / iterations as f64;
        let avg_millis = avg_micros / 1000.0;

        println!("\n=== Performance Benchmark ===");
        println!("Text length: {} chars", sample.len());
        println!("Iterations: {}", iterations);
        println!("Total time: {:?}", elapsed);
        println!("Average: {:.2} ms ({:.0} µs)", avg_millis, avg_micros);
        println!("=============================\n");

        // Assert reasonable performance
        // Debug mode is ~10-15x slower than release, so use different thresholds
        let threshold_ms = if cfg!(debug_assertions) { 50.0 } else { 10.0 };
        assert!(
            avg_millis < threshold_ms,
            "Analysis too slow: {:.2} ms (expected < {:.0} ms)",
            avg_millis,
            threshold_ms
        );
    }
}

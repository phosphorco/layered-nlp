use layered_nlp::{create_line_from_string, LLLine};
use layered_part_of_speech::POSTagResolver;
use serde_json::json;

use crate::{
    apply_verification_action, AccountabilityGraphResolver, ClauseAggregationResolver,
    ContractClauseResolver, ContractKeyword, ContractKeywordResolver, DefinedTermResolver,
    ObligationGraph, ObligationNode, ObligationPhraseResolver, PronounChainResolver,
    PronounResolver, ProhibitionResolver, Scored, TermReferenceResolver, VerificationAction,
    VerificationQueueDetails,
};

fn base_line(input: &str) -> LLLine {
    create_line_from_string(input)
        .run(&POSTagResolver::default())
        .run(&ContractKeywordResolver::default())
        .run(&ProhibitionResolver::default())
        .run(&DefinedTermResolver::default())
        .run(&TermReferenceResolver::default())
        .run(&PronounResolver::default())
        .run(&ObligationPhraseResolver::default())
        .run(&PronounChainResolver::default())
        .run(&ContractClauseResolver::default())
        .run(&ClauseAggregationResolver::default())
        .run(&AccountabilityGraphResolver::default())
}

fn graph_nodes(input: &str) -> Vec<Scored<ObligationNode>> {
    base_line(input)
        .query::<Scored<ObligationNode>>()
        .into_iter()
        .flat_map(|(_, _, attrs)| attrs.into_iter())
        .cloned()
        .collect()
}

#[test]
#[ignore = "Non-deterministic: beneficiary_groups order depends on HashMap iteration"]
fn party_explorer_groups_by_beneficiary() {
    let input = r#"XYZ Inc (the "Buyer") exists. ABC Corp (the "Seller") shall deliver goods to the Buyer subject to Section 5. The Seller shall obtain Buyer consent if the Buyer submits a written request. The Seller shall remit fees to Regional Authority."#;
    let nodes = graph_nodes(input);
    let graph = ObligationGraph::new(&nodes);

    let seller_node = nodes
        .iter()
        .find(|node| node.value.obligor.display_text.contains("Seller"))
        .expect("seller node");

    let analytics = graph.for_party_or_display(
        seller_node.value.obligor.chain_id,
        &seller_node.value.obligor.display_text,
    );
    insta::assert_snapshot!(format!("{analytics:#?}"));
}

#[test]
fn condition_filters_reference_section() {
    let nodes = graph_nodes(
        r#"XYZ Inc (the "Buyer") exists. ABC Corp (the "Seller") shall deliver goods to the Buyer subject to Section 5. The Seller shall obtain Buyer consent if the Buyer submits a written request."#,
    );
    let graph = ObligationGraph::new(&nodes);
    let matches = graph.referencing_section("Section 5");
    let ids: Vec<_> = matches
        .into_iter()
        .map(|node| node.value.node_id)
        .collect();
    assert_eq!(ids, vec![1]);
}

#[test]
fn verification_queue_and_resolution() {
    let input = r#"The Vendor shall deliver goods to Regional Authority under Section 2."#;
    let mut nodes = graph_nodes(input);
    let graph = ObligationGraph::new(&nodes);
    let queue = graph.verification_queue();
    assert_eq!(queue.len(), 1);

    let target = &queue[0];
    let beneficiary_text = match &target.details {
        VerificationQueueDetails::Beneficiary { beneficiary_text, .. } => beneficiary_text.clone(),
        _ => panic!("expected beneficiary queue entry"),
    };
    let action = VerificationAction::resolve_beneficiary(
        target.node_id,
        target.clause_id,
        &beneficiary_text,
        "reviewer-1",
        Some(42),
    )
    .with_note("Beneficiary confirmed as regulator");

    assert!(apply_verification_action(&mut nodes, action));
    let refreshed = ObligationGraph::new(&nodes);
    assert!(refreshed.verification_queue().is_empty());
    let node = nodes.first().expect("node available");
    assert_eq!(node.confidence, 1.0);
    assert!(!node.value.beneficiaries[0].needs_verification);
    assert_eq!(node.value.verification_notes.len(), 1);
}

#[test]
fn accountability_payload_snapshot() {
    let input = r#"XYZ Inc (the "Buyer") exists. ABC Corp (the "Seller") shall deliver goods to the Buyer subject to Section 5. The Seller shall obtain Buyer consent if the Buyer submits a written request. The Seller shall remit fees to Regional Authority."#;
    let nodes = graph_nodes(input);
    let graph = ObligationGraph::new(&nodes);
    let payload = graph.payload();
    insta::assert_snapshot!(payload.to_json_string());
}

#[test]
fn resolve_one_of_multiple_beneficiaries() {
    let input =
        r#"The Vendor shall deliver goods to Regional Authority and remit reports to Municipal Agency."#;
    let mut nodes = graph_nodes(input);
    let graph = ObligationGraph::new(&nodes);
    let queue = graph.verification_queue();
    assert_eq!(queue.len(), 2);
    let first_entry = queue
        .iter()
        .find(|entry| matches!(entry.details, VerificationQueueDetails::Beneficiary { .. }))
        .expect("beneficiary entry");

    let beneficiary_text = match &first_entry.details {
        VerificationQueueDetails::Beneficiary { beneficiary_text, .. } => beneficiary_text.clone(),
        _ => unreachable!(),
    };

    let action = VerificationAction::resolve_beneficiary(
        first_entry.node_id,
        first_entry.clause_id,
        &beneficiary_text,
        "reviewer-1",
        Some(7),
    );
    assert!(apply_verification_action(&mut nodes, action));

    let node = nodes.first().expect("node available");
    assert!(
        node.value
            .beneficiaries
            .iter()
            .any(|link| normalize_text(&link.display_text) == normalize_text(&beneficiary_text)
                && !link.needs_verification
                && link.chain_id == Some(7))
    );
    assert!(
        node.value
            .beneficiaries
            .iter()
            .any(|link| link.needs_verification),
        "second beneficiary should still need verification"
    );
    assert!(node.confidence < 1.0, "node should not be fully verified");
}

#[test]
fn verify_condition_queue_entry_and_action() {
    let input = r#"The Seller shall deliver goods if Regulatory Agency approves."#;
    let mut nodes = graph_nodes(input);
    let graph = ObligationGraph::new(&nodes);
    let queue = graph.verification_queue();
    let condition_entry = queue
        .iter()
        .find(|entry| matches!(entry.details, VerificationQueueDetails::Condition { .. }))
        .expect("condition queue item");

    let condition_text = match &condition_entry.target {
        crate::VerificationTarget::ConditionLink {
            condition_text, ..
        } => condition_text.clone(),
        _ => panic!("expected condition target"),
    };

    let action = VerificationAction::verify_condition(
        condition_entry.node_id,
        condition_entry.clause_id,
        &condition_text,
        "reviewer-2",
    )
    .with_note("Condition references regulator");
    assert!(apply_verification_action(&mut nodes, action));

    let refreshed_queue = ObligationGraph::new(&nodes).verification_queue();
    assert!(
        !refreshed_queue
            .iter()
            .any(|item| matches!(item.details, VerificationQueueDetails::Condition { .. })),
        "condition queue entries should be cleared"
    );
}

#[test]
fn apply_verification_action_invalid_node_returns_false() {
    let input = r#"The Vendor shall deliver goods to Regional Authority."#;
    let mut nodes = graph_nodes(input);
    let result = apply_verification_action(
        &mut nodes,
        VerificationAction::verify_node(9999, "reviewer"),
    );
    assert!(!result);
}

#[test]
fn empty_graph_payload_serializes_cleanly() {
    let graph = ObligationGraph::new(&[]);
    let payload = graph.payload();
    assert!(payload.nodes.is_empty());
    assert!(payload.verification_queue.is_empty());
    assert_eq!(
        payload.to_value(),
        json!({
            "nodes": [],
            "verification_queue": []
        })
    );
}

#[test]
fn party_query_without_matches_returns_empty_groups() {
    let nodes = graph_nodes(
        r#"XYZ Inc (the "Buyer") exists. ABC Corp (the "Seller") shall deliver goods to the Buyer."#,
    );
    let graph = ObligationGraph::new(&nodes);
    let analytics = graph.for_party_or_display(Some(9999), "Ghost Party");
    assert!(analytics.beneficiary_groups.is_empty());
    assert!(analytics.unassigned_nodes.is_empty());
    assert_eq!(analytics.obligor_display_text, "Ghost Party");
}

#[test]
fn condition_filter_no_matches_returns_empty() {
    let nodes = graph_nodes(r#"The Seller shall deliver goods if the Buyer approves."#);
    let graph = ObligationGraph::new(&nodes);
    let matches = graph.with_condition_type(ContractKeyword::Provided);
    assert!(matches.is_empty());
}

fn normalize_text(value: &str) -> String {
    value.trim().to_lowercase()
}

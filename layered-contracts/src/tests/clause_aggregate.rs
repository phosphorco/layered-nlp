use layered_nlp::{create_line_from_string, LLLineDisplay};
use layered_part_of_speech::POSTagResolver;

use crate::{
    ClauseAggregate, ClauseAggregationResolver, ContractClause, ContractClauseResolver,
    ContractKeyword, ContractKeywordResolver, DefinedTermResolver, ObligationPhraseResolver,
    PronounChainResolver, PronounResolver, ProhibitionResolver, Scored, TermReferenceResolver,
};

fn test_clause_aggregates(input: &str) -> String {
    let ll_line = create_line_from_string(input)
        .run(&POSTagResolver::default())
        .run(&ContractKeywordResolver::default())
        .run(&ProhibitionResolver::default())
        .run(&DefinedTermResolver::default())
        .run(&TermReferenceResolver::default())
        .run(&PronounResolver::default())
        .run(&ObligationPhraseResolver::default())
        .run(&PronounChainResolver::default())
        .run(&ContractClauseResolver::default())
        .run(&ClauseAggregationResolver::default());

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<Scored<ContractClause>>();
    display.include::<Scored<ClauseAggregate>>();

    format!("{}", display)
}

// ============ Aggregation Basics ============

#[test]
fn aggregate_single_clause() {
    insta::assert_snapshot!(test_clause_aggregates(
        r#"ABC Corp (the "Company") shall deliver goods."#
    ));
}

#[test]
fn aggregate_multiple_clauses_same_party() {
    insta::assert_snapshot!(test_clause_aggregates(
        r#"ABC Corp (the "Company") shall deliver goods and shall pay any applicable fees."#
    ));
}

#[test]
fn aggregate_breaks_on_other_party() {
    insta::assert_snapshot!(test_clause_aggregates(
        r#"ABC Corp (the "Seller") shall deliver goods. XYZ Inc (the "Buyer") shall pay the price."#
    ));
}

// ============ Confidence Heuristics ============

#[test]
fn aggregate_missing_chain_penalty() {
    insta::assert_snapshot!(test_clause_aggregates(
        r#"The Vendor shall deliver goods promptly."#
    ));
}

#[test]
fn aggregate_cross_section_penalty() {
    insta::assert_snapshot!(test_clause_aggregates(
        r#"ABC Corp (the "Company") shall deliver goods promptly, and upon request of any regulator shall provide detailed compliance reports, and after termination of this Agreement shall maintain records for seven years."#
    ));
}

// ============ Pronoun Chain Tests ============

#[test]
fn aggregate_via_pronoun_chain() {
    // Verifies that clauses with pronoun obligors ("It") that resolve to the same
    // party get aggregated together via chain_id matching.
    insta::assert_snapshot!(test_clause_aggregates(
        r#"ABC Corp (the "Company") exists. It shall deliver goods. It shall pay fees."#
    ));
}

// ============ Interleaved Parties Tests ============

#[test]
fn aggregate_does_not_merge_interleaved_parties() {
    // When the same party appears, then a different party, then the same original
    // party again, they should NOT be merged into a single aggregate.
    insta::assert_snapshot!(test_clause_aggregates(
        r#"ABC Corp (the "Seller") shall deliver goods. XYZ Inc (the "Buyer") shall inspect. The Seller shall repair defects."#
    ));
}

// ============ Chain-Preferring Party Matching Tests ============

#[test]
fn aggregate_relaxed_matching_with_and_without_chain() {
    // When one clause has chain_id (from pronoun resolution) and another doesn't
    // (plain noun phrase), they should still merge if names match.
    // This tests relaxed matching where "Company" with chain_id=None merges with
    // "Company" with chain_id=Some(1).
    insta::assert_snapshot!(test_clause_aggregates(
        r#"ABC Corp (the "Company") shall deliver goods. The Company shall pay fees."#
    ));
}

// ============ Gap Boundary Tests ============

#[test]
fn aggregate_gap_at_exact_max_boundary() {
    // This test documents behavior when gap is at exactly max_gap_tokens (30).
    // With <= comparison, gap=30 should still merge.
    // Note: We can't easily control exact token offsets, but this tests
    // multi-clause aggregation behavior with the default settings.
    insta::assert_snapshot!(test_clause_aggregates(
        r#"ABC Corp (the "Company") shall deliver goods. The Company shall pay fees. The Company shall provide support."#
    ));
}

// ============ Condition Rollup Tests ============

#[test]
fn all_conditions_rollup() {
    // Verifies the all_conditions() method rolls up conditions from all clauses
    let ll_line = create_line_from_string(
        r#"If payment is late, ABC Corp (the "Company") shall deliver goods. The Company shall pay fees unless disputed."#
    )
    .run(&POSTagResolver::default())
    .run(&ContractKeywordResolver::default())
    .run(&ProhibitionResolver::default())
    .run(&DefinedTermResolver::default())
    .run(&TermReferenceResolver::default())
    .run(&PronounResolver::default())
    .run(&ObligationPhraseResolver::default())
    .run(&PronounChainResolver::default())
    .run(&ContractClauseResolver::default())
    .run(&ClauseAggregationResolver::default());

    let aggregates = ll_line.query::<Scored<ClauseAggregate>>();

    assert_eq!(aggregates.len(), 1, "expected one aggregate");
    let agg = &aggregates[0].2[0].value;

    let all_conditions = agg.all_conditions();
    assert!(
        all_conditions.len() >= 2,
        "expected at least 2 conditions (If + Unless)"
    );

    let condition_types: Vec<_> = all_conditions
        .iter()
        .map(|c| &c.condition_type)
        .collect();
    assert!(
        condition_types.contains(&&ContractKeyword::If),
        "expected If condition"
    );
    assert!(
        condition_types.contains(&&ContractKeyword::Unless),
        "expected Unless condition"
    );
}

use layered_nlp::{create_line_from_string, LLLineDisplay};
use layered_part_of_speech::POSTagResolver;

use crate::{
    ContractKeyword, ContractKeywordResolver, DefinedTerm, DefinedTermResolver,
    ObligationPhrase, ObligationPhraseResolver, ProhibitionResolver, PronounReference,
    PronounResolver, Scored, TermReference, TermReferenceResolver,
};

fn test_obligations(input: &str) -> String {
    let ll_line = create_line_from_string(input)
        .run(&POSTagResolver::default())
        .run(&ContractKeywordResolver::default())
        .run(&ProhibitionResolver::default())
        .run(&DefinedTermResolver::default())
        .run(&TermReferenceResolver::default())
        .run(&PronounResolver::default())
        .run(&ObligationPhraseResolver::default());

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<ContractKeyword>();
    display.include::<Scored<DefinedTerm>>();
    display.include::<Scored<TermReference>>();
    display.include::<Scored<PronounReference>>();
    // Use include_with_associations to show obligor source arrows
    display.include_with_associations::<Scored<ObligationPhrase>>();

    format!("{}", display)
}

// ============ Basic Obligation Type Tests ============

#[test]
fn obligation_duty_shall() {
    // Basic "shall" obligation
    insta::assert_snapshot!(test_obligations(
        r#"ABC Corp (the "Company") shall deliver goods."#
    ));
}

#[test]
fn obligation_permission_may() {
    // "may" indicates permission
    insta::assert_snapshot!(test_obligations(
        r#"ABC Corp (the "Company") may terminate this Agreement."#
    ));
}

#[test]
fn obligation_prohibition_shall_not() {
    // "shall not" indicates prohibition
    insta::assert_snapshot!(test_obligations(
        r#"ABC Corp (the "Company") shall not disclose confidential information."#
    ));
}

// ============ Obligor Source Tests ============

#[test]
fn obligation_obligor_from_term_reference() {
    // Obligor is a term reference
    insta::assert_snapshot!(test_obligations(
        r#""Contractor" means John Doe. The Contractor shall provide services."#
    ));
}

#[test]
fn obligation_obligor_from_pronoun() {
    // Obligor resolved through pronoun
    insta::assert_snapshot!(test_obligations(
        r#"ABC Corp (the "Company") exists. It shall deliver goods."#
    ));
}

#[test]
fn obligation_obligor_plain_noun() {
    // Obligor is a plain capitalized noun (not a defined term)
    insta::assert_snapshot!(test_obligations("The Vendor shall deliver products."));
}

// ============ Conditional Tests ============

#[test]
fn obligation_with_if_condition() {
    // Obligation with "if" condition
    insta::assert_snapshot!(test_obligations(
        r#"ABC Corp (the "Company") shall deliver goods if payment is received."#
    ));
}

#[test]
fn obligation_with_unless_condition() {
    // Obligation with "unless" condition
    insta::assert_snapshot!(test_obligations(
        r#"ABC Corp (the "Company") shall deliver goods unless otherwise agreed."#
    ));
}

#[test]
fn obligation_with_provided_condition() {
    // Obligation with "provided" condition
    insta::assert_snapshot!(test_obligations(
        r#"ABC Corp (the "Company") shall deliver goods provided that notice is given."#
    ));
}

// ============ Edge Cases ============

#[test]
fn condition_scoped_to_same_sentence() {
    // Condition in first sentence should NOT attach to obligation in second sentence
    insta::assert_snapshot!(test_obligations(
        r#"If approved, the fee applies. ABC Corp (the "Company") shall deliver goods."#
    ));
}

#[test]
fn subject_to_stops_action() {
    // "subject to" should stop action extraction and become a condition
    insta::assert_snapshot!(test_obligations(
        r#"ABC Corp (the "Company") shall pay the fee subject to Section 5."#
    ));
}

#[test]
fn condition_only_applies_to_nearest_modal() {
    // "If payment is late" should only attach to first "shall deliver", not second "shall refund"
    insta::assert_snapshot!(test_obligations(
        r#"If payment is late, the Company shall deliver, and the Vendor shall refund."#
    ));
}

#[test]
fn multiword_noun_obligor() {
    // "Service Provider" should be captured as a single multi-word phrase
    insta::assert_snapshot!(test_obligations(
        "The Service Provider shall deliver services on time."
    ));
}

#[test]
fn no_obligor_should_skip() {
    // No discernible obligor - should not produce obligation
    insta::assert_snapshot!(test_obligations("shall deliver goods."));
}

#[test]
fn multiple_obligations_in_sentence() {
    // Multiple modals in one sentence
    insta::assert_snapshot!(test_obligations(
        r#"ABC Corp (the "Company") shall deliver goods and the Vendor may inspect them."#
    ));
}

#[test]
fn obligation_across_sentences() {
    // Obligations in separate sentences
    insta::assert_snapshot!(test_obligations(
        r#"ABC Corp (the "Company") shall deliver goods. The Company shall ensure quality."#
    ));
}

// ============ Real Contract Examples ============

#[test]
fn contract_delivery_clause() {
    insta::assert_snapshot!(test_obligations(
        r#"ABC Corporation (the "Seller") shall deliver the Products to Buyer within thirty days of the Effective Date."#
    ));
}

#[test]
fn contract_payment_clause() {
    insta::assert_snapshot!(test_obligations(
        r#"The Buyer shall pay the Purchase Price to the Seller within fifteen days of delivery."#
    ));
}

#[test]
fn contract_confidentiality_clause() {
    insta::assert_snapshot!(test_obligations(
        r#"XYZ Inc (the "Receiving Party") shall not disclose Confidential Information to any third party unless required by law."#
    ));
}

// ============ Regression Tests ============

/// Regression test: ActionSpan must align with trimmed action text.
///
/// When `trim_trailing_conjunction` removes trailing words like "and the Vendor",
/// the ActionSpan association must be adjusted to cover only the retained words.
/// This test verifies that the span indices match the trimmed action, not the
/// raw extracted text.
#[test]
fn action_span_aligns_with_trimmed_text() {
    use layered_nlp::create_line_from_string;

    let input = r#"ABC Corp (the "Company") shall deliver goods and the Vendor may inspect them."#;

    let ll_line = create_line_from_string(input)
        .run(&POSTagResolver::default())
        .run(&ContractKeywordResolver::default())
        .run(&ProhibitionResolver::default())
        .run(&DefinedTermResolver::default())
        .run(&TermReferenceResolver::default())
        .run(&PronounResolver::default())
        .run(&ObligationPhraseResolver::default());

    // Query obligations with their associations
    let obligations = ll_line.query_with_associations::<Scored<ObligationPhrase>>();

    // Find the "deliver goods" obligation (Company's duty)
    let deliver_obligation = obligations
        .iter()
        .flat_map(|(_, _, attrs)| attrs.iter())
        .find(|(attr, _)| attr.value.action == "deliver goods")
        .expect("should find 'deliver goods' obligation");

    let (obligation, associations) = deliver_obligation;

    // Verify the action text was trimmed correctly (not "deliver goods and the Vendor")
    assert_eq!(
        obligation.value.action, "deliver goods",
        "Action should be trimmed to 'deliver goods', not include trailing conjunction"
    );

    // Find the ActionSpan association
    let action_span_assoc = associations
        .iter()
        .find(|assoc| assoc.label() == "action_span")
        .expect("should have action_span association");

    // Get the action span indices
    let action_span = action_span_assoc.span;

    // Calculate what the trimmed action covers in tokens:
    // Token indices: 0=ABC, 1=Corp, 2=(, 3=the, 4=", 5=Company, 6=", 7=), 8=shall, 9=deliver, 10=goods, 11=and, ...
    // "deliver goods" should be tokens 12-16 (depends on tokenization)
    // The key assertion: the span should NOT include "and the Vendor" tokens

    // Verify span covers exactly 2 words (deliver, goods) by checking the action_span
    // doesn't extend to include "and" or beyond
    let span_length = action_span.end_idx - action_span.start_idx + 1;

    // The span should be small (just "deliver" and "goods" plus any whitespace tokens)
    // It should NOT be large enough to include "and the Vendor"
    assert!(
        span_length <= 5, // deliver (1) + space (1) + goods (1) + possible punctuation = ~3-5 tokens
        "ActionSpan should cover only 'deliver goods', got span length {} (indices {}..{})",
        span_length,
        action_span.start_idx,
        action_span.end_idx
    );

    // Also verify the obligor span is present
    let obligor_span_assoc = associations
        .iter()
        .find(|assoc| assoc.label() == "obligor_source")
        .expect("should have obligor_source association");

    assert_eq!(
        obligor_span_assoc.glyph(),
        Some("@"),
        "obligor_source should have '@' glyph"
    );
    assert_eq!(
        action_span_assoc.glyph(),
        Some("#"),
        "action_span should have '#' glyph"
    );
}

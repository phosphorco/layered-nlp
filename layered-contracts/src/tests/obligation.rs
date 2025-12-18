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
    display.include::<Scored<ObligationPhrase>>();

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

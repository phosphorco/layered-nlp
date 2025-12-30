use layered_nlp::{create_line_from_string, LLLineDisplay};
use layered_part_of_speech::POSTagResolver;

use crate::{
    ContractKeyword, ContractKeywordResolver, DefinedTerm, DefinedTermResolver, PronounReference,
    PronounResolver, Scored, TermReference, TermReferenceResolver,
};

fn test_pronouns(input: &str) -> String {
    let ll_line = create_line_from_string(input)
        .run(&POSTagResolver::default())
        .run(&ContractKeywordResolver::default())
        .run(&DefinedTermResolver::default())
        .run(&TermReferenceResolver::default())
        .run(&PronounResolver::default());

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<ContractKeyword>();
    display.include::<Scored<DefinedTerm>>();
    display.include::<Scored<TermReference>>();
    display.include::<Scored<PronounReference>>();

    format!("{}", display)
}

// ============ Basic Pronoun Resolution Tests ============

#[test]
fn pronoun_after_defined_term() {
    // "It" should resolve to "Company"
    insta::assert_snapshot!(test_pronouns(
        r#""Company" means ABC Corp. It shall deliver goods."#
    ));
}

#[test]
fn pronoun_after_parenthetical() {
    // "It" should resolve to "Contractor"
    insta::assert_snapshot!(test_pronouns(
        r#"John Doe (the "Contractor") agrees. It shall comply."#
    ));
}

#[test]
fn pronoun_its_possessive() {
    // "its" should resolve to "Company"
    insta::assert_snapshot!(test_pronouns(
        r#"ABC Corp (the "Company") agrees to deliver its products."#
    ));
}

#[test]
fn pronoun_they_plural() {
    // "They" should resolve to "Parties"
    insta::assert_snapshot!(test_pronouns(
        r#""Parties" means the Company and Contractor. They shall cooperate."#
    ));
}

// ============ Distance and Same-Sentence Tests ============

#[test]
fn pronoun_same_sentence() {
    // "it" in same sentence should have higher confidence
    insta::assert_snapshot!(test_pronouns(
        r#"The Company shall deliver and it must comply."#
    ));
}

#[test]
fn pronoun_different_sentence() {
    // "It" in different sentence should have lower confidence
    insta::assert_snapshot!(test_pronouns(
        r#"The Company shall deliver. It must comply with regulations."#
    ));
}

#[test]
fn pronoun_multiple_sentences_away() {
    // "It" far from antecedent
    insta::assert_snapshot!(test_pronouns(
        r#"The Company exists. The terms apply. The rules govern. It shall comply."#
    ));
}

// ============ Multiple Candidates Tests ============

#[test]
fn pronoun_multiple_defined_terms() {
    // "It" with two defined terms - should prefer nearest
    insta::assert_snapshot!(test_pronouns(
        r#"ABC Corp (the "Company") and XYZ Inc (the "Vendor") agree. It shall deliver."#
    ));
}

#[test]
fn pronoun_defined_term_vs_noun() {
    // "It" should prefer defined term over plain noun
    insta::assert_snapshot!(test_pronouns(
        r#"ABC Corp (the "Company") owns equipment. It shall be maintained."#
    ));
}

// ============ Pronoun Type Tests ============

#[test]
fn pronoun_he_masculine() {
    insta::assert_snapshot!(test_pronouns(
        r#"John Doe (the "Consultant") agrees. He shall provide services."#
    ));
}

#[test]
fn pronoun_she_feminine() {
    insta::assert_snapshot!(test_pronouns(
        r#"Jane Smith (the "Advisor") agrees. She shall consult."#
    ));
}

#[test]
fn pronoun_this_relative() {
    insta::assert_snapshot!(test_pronouns(
        r#"The Agreement contains terms. This shall govern."#
    ));
}

// ============ Edge Cases ============

#[test]
fn no_pronouns() {
    // No pronouns to resolve
    insta::assert_snapshot!(test_pronouns(
        r#""Company" means ABC Corp. The Company shall deliver."#
    ));
}

#[test]
fn no_antecedents() {
    // Pronoun with no prior nouns
    insta::assert_snapshot!(test_pronouns("It shall deliver goods."));
}

#[test]
fn pronoun_before_definition() {
    // Pronoun appears before any definition - should not resolve
    insta::assert_snapshot!(test_pronouns(
        r#"It exists. "Company" means ABC Corp."#
    ));
}

// ============ Real Contract Examples ============

#[test]
fn contract_preamble_with_pronouns() {
    insta::assert_snapshot!(test_pronouns(
        r#"ABC Corporation (the "Company") and John Doe (the "Contractor") enter this Agreement. The Company shall pay the Contractor. It shall remit payment monthly."#
    ));
}

#[test]
fn obligation_with_pronoun_chain() {
    insta::assert_snapshot!(test_pronouns(
        r#"The Service Provider shall deliver services. It shall ensure quality. It must comply with standards."#
    ));
}

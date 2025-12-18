use layered_nlp::{create_line_from_string, LLLineDisplay};

use crate::{ContractKeyword, ContractKeywordResolver, DefinedTerm, DefinedTermResolver, Scored};

fn test_defined_terms(input: &str) -> String {
    let ll_line = create_line_from_string(input)
        .run(&ContractKeywordResolver::default())
        .run(&DefinedTermResolver::default());

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<ContractKeyword>();
    display.include::<Scored<DefinedTerm>>();

    format!("{}", display)
}

// ============ Pattern 1: QuotedMeans Tests ============

#[test]
fn quoted_means_simple() {
    insta::assert_snapshot!(test_defined_terms(r#""Company" means ABC Corporation"#));
}

#[test]
fn quoted_means_multiword() {
    insta::assert_snapshot!(test_defined_terms(
        r#""Effective Date" means the date of this Agreement"#
    ));
}

#[test]
fn quoted_means_alternate_keyword() {
    // "mean" (singular) should also work
    insta::assert_snapshot!(test_defined_terms(r#""Services" mean all consulting work"#));
}

// ============ Pattern 2: Parenthetical Tests ============

#[test]
fn parenthetical_with_the() {
    insta::assert_snapshot!(test_defined_terms(r#"ABC Corporation (the "Company")"#));
}

#[test]
fn parenthetical_without_the() {
    insta::assert_snapshot!(test_defined_terms(r#"John Smith ("Contractor")"#));
}

#[test]
fn parenthetical_multiword() {
    insta::assert_snapshot!(test_defined_terms(r#"Acme Inc. (the "Purchasing Party")"#));
}

// ============ Pattern 3: Hereinafter Tests ============

#[test]
fn hereinafter_simple() {
    insta::assert_snapshot!(test_defined_terms(r#"ABC Corp, hereinafter "Company""#));
}

#[test]
fn hereinafter_referred_to_as() {
    insta::assert_snapshot!(test_defined_terms(
        r#"ABC Corp, hereinafter referred to as the "Company""#
    ));
}

#[test]
fn hereinafter_with_the() {
    insta::assert_snapshot!(test_defined_terms(r#"John Doe, hereinafter the "Consultant""#));
}

// ============ Edge Case Tests ============

#[test]
fn no_quoted_term() {
    // Plain text without quotes - "means" keyword detected but no term definition
    insta::assert_snapshot!(test_defined_terms("ABC Corporation means the company"));
}

#[test]
fn unclosed_quote() {
    // Unclosed quote should not match as a defined term
    insta::assert_snapshot!(test_defined_terms(r#""Company means something"#));
}

#[test]
fn empty_quotes() {
    // Empty quotes should not create a term
    insta::assert_snapshot!(test_defined_terms(r#""" means nothing"#));
}

#[test]
fn multiple_definitions_in_sentence() {
    // Multiple terms in one line
    insta::assert_snapshot!(test_defined_terms(
        r#""Company" means ABC Corp (the "Parent") and its subsidiaries"#
    ));
}

// ============ Real Contract Examples ============

#[test]
fn real_contract_preamble() {
    insta::assert_snapshot!(test_defined_terms(
        r#"This Agreement is entered into by ABC Corporation (the "Company") and John Doe, hereinafter referred to as the "Contractor""#
    ));
}

#[test]
fn definitions_section() {
    insta::assert_snapshot!(test_defined_terms(
        r#""Confidential Information" means any information disclosed by one party"#
    ));
}

#[test]
fn term_with_obligation() {
    // Definition followed by obligation
    insta::assert_snapshot!(test_defined_terms(
        r#""Contractor" means John Doe. The Contractor shall deliver the goods."#
    ));
}

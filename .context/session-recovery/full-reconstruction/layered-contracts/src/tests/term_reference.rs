use layered_nlp::{create_line_from_string, LLLineDisplay};

use crate::{
    ContractKeyword, ContractKeywordResolver, DefinedTerm, DefinedTermResolver, Scored,
    TermReference, TermReferenceResolver,
};

fn test_term_references(input: &str) -> String {
    let ll_line = create_line_from_string(input)
        .run(&ContractKeywordResolver::default())
        .run(&DefinedTermResolver::default())
        .run(&TermReferenceResolver::default());

    let mut display = LLLineDisplay::new(&ll_line);
    display.include::<ContractKeyword>();
    display.include::<Scored<DefinedTerm>>();
    display.include::<Scored<TermReference>>();

    format!("{}", display)
}

// ============ Basic Reference Tests ============

#[test]
fn reference_after_quoted_means() {
    // "Company" means ABC. The Company shall...
    insta::assert_snapshot!(test_term_references(
        r#""Company" means ABC Corp. The Company shall deliver."#
    ));
}

#[test]
fn reference_after_parenthetical() {
    // ABC Corp (the "Company"). Company shall...
    insta::assert_snapshot!(test_term_references(
        r#"ABC Corp (the "Company"). Company shall deliver."#
    ));
}

#[test]
fn reference_after_hereinafter() {
    // ABC, hereinafter "Company". The Company...
    insta::assert_snapshot!(test_term_references(
        r#"ABC Corp, hereinafter "Company". The Company shall comply."#
    ));
}

#[test]
fn reference_lowercase() {
    // Should still match with lower confidence
    insta::assert_snapshot!(test_term_references(
        r#""Contractor" means John Doe. The contractor shall deliver."#
    ));
}

#[test]
fn reference_case_sensitive() {
    // Exact case gets higher confidence than case-insensitive
    insta::assert_snapshot!(test_term_references(
        r#""Contractor" means John. Contractor shall deliver. The contractor agrees."#
    ));
}

// ============ Multi-word Terms ============

#[test]
fn multiword_reference() {
    // "Effective Date" -> later "Effective Date" reference
    insta::assert_snapshot!(test_term_references(
        r#""Effective Date" means January 1. The Effective Date shall govern."#
    ));
}

#[test]
fn partial_multiword_no_match() {
    // "Effective" alone doesn't match "Effective Date"
    insta::assert_snapshot!(test_term_references(
        r#""Effective Date" means January 1. This Effective period begins."#
    ));
}

// ============ Edge Cases ============

#[test]
fn no_reference_in_definition() {
    // The definition span itself is not a reference
    insta::assert_snapshot!(test_term_references(r#""Contractor" means John Doe."#));
}

#[test]
fn multiple_terms_multiple_references() {
    // Two terms defined, both referenced
    insta::assert_snapshot!(test_term_references(
        r#"ABC Corp (the "Company") and John Doe (the "Contractor"). The Company shall pay the Contractor."#
    ));
}

#[test]
fn reference_with_article() {
    // "the Company" reference detection (+0.05 confidence)
    insta::assert_snapshot!(test_term_references(
        r#"ABC Corp (the "Company"). The Company shall act. Company agrees."#
    ));
}

#[test]
fn no_defined_terms() {
    // No definitions = no references
    insta::assert_snapshot!(test_term_references(
        "The Company shall deliver goods to the Contractor."
    ));
}

// ============ Real Contract Examples ============

#[test]
fn contract_with_references() {
    // Full sentence with definition then references
    insta::assert_snapshot!(test_term_references(
        r#"This Agreement is between ABC Corporation (the "Company") and John Doe (the "Consultant"). The Company shall pay the Consultant."#
    ));
}

#[test]
fn multiple_references_same_term() {
    // Same term referenced multiple times
    insta::assert_snapshot!(test_term_references(
        r#""Contractor" means John. Contractor shall deliver. Contractor shall comply. The Contractor agrees."#
    ));
}

#[test]
fn reference_in_obligation() {
    // Reference within an obligation phrase
    insta::assert_snapshot!(test_term_references(
        r#""Service Provider" means ABC Corp. The Service Provider shall deliver services."#
    ));
}

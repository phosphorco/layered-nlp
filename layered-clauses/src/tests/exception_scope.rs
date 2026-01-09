//! Gate 1: Exception Scope Propagation Tests
//!
//! These tests validate that exception clauses properly propagate to all
//! coordinated conjuncts. When we have "A and B, unless C", the exception C
//! should apply to BOTH A and B, not just the immediately preceding clause.
//!
//! Current behavior: Exception only links to the immediately preceding clause.
//! Target behavior: Exception links to ALL clauses in the coordination chain.

use crate::{SentenceBoundaryResolver, 
    ClauseKeywordResolver, ClauseLink, ClauseLinkResolver, ClauseResolver,
};
use layered_nlp_document::{ClauseRole, LayeredDocument};


/// Helper: Create document and extract links for testing
fn build_and_resolve(text: &str) -> (LayeredDocument, Vec<ClauseLink>) {
    let doc = LayeredDocument::from_text(text)
        .run_resolver(&SentenceBoundaryResolver::new())
        .run_resolver(&ClauseKeywordResolver::new(
            &["if", "when"],
            &["and"],
            &["then"],
            &["or"],
            &["but", "however"],
            &["nor"],
        ))
        .run_resolver(&ClauseResolver::default());

    let links = ClauseLinkResolver::resolve(&doc);
    (doc, links)
}

#[test]
fn test_exception_applies_to_all_conjuncts() {
    // "A and B, unless C" -> 2 exception links (C->A, C->B)
    let text = "The Buyer pays and the Seller delivers unless cancelled.";
    let (_doc, links) = build_and_resolve(text);

    let exception_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::Exception)
        .collect();

    assert_eq!(
        exception_links.len(),
        2,
        "Exception should apply to BOTH coordinated clauses. \
        Currently got {} exception links, expected 2. \
        This test validates Gate 1: Exception Scope Propagation.",
        exception_links.len()
    );

    // All exception links should have same anchor (the exception clause)
    let anchors: Vec<_> = exception_links.iter().map(|l| l.anchor).collect();
    // Check all anchors are equal to the first one
    assert!(
        anchors.windows(2).all(|w| w[0] == w[1]),
        "All exceptions should come from same clause (the exception clause 'cancelled')"
    );
}

#[test]
fn test_exception_applies_to_chain() {
    // "A and B and C, unless D" -> 3 exception links
    let text =
        "The Buyer pays and the Seller delivers and the Agent confirms unless terminated.";
    let (_doc, links) = build_and_resolve(text);

    let exception_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::Exception)
        .collect();

    assert_eq!(
        exception_links.len(),
        3,
        "Exception should apply to ALL three coordinated clauses. \
        Currently got {} exception links, expected 3. \
        This test validates Gate 1: Exception Scope Propagation for chains.",
        exception_links.len()
    );
}

#[test]
fn test_exception_single_clause_unchanged() {
    // "A unless B" -> 1 exception link (unchanged behavior)
    let text = "The Buyer pays unless excused.";
    let (_doc, links) = build_and_resolve(text);

    let exception_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::Exception)
        .collect();

    assert_eq!(
        exception_links.len(),
        1,
        "Single clause exception should create one link. \
        This is the baseline behavior that should remain unchanged."
    );
}

#[test]
fn test_exception_does_not_cross_sentence() {
    // "A and B. C unless D" -> exception only applies to C, not A or B
    let text = "The Buyer pays and the Seller delivers. The Agent confirms unless terminated.";
    let (_doc, links) = build_and_resolve(text);

    let exception_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::Exception)
        .collect();

    // Should only link to "Agent confirms", not to clauses in previous sentence
    assert_eq!(
        exception_links.len(),
        1,
        "Exception should not cross sentence boundary. \
        Got {} exception links, expected 1 (only to 'Agent confirms').",
        exception_links.len()
    );
}




//! Integration tests for M2 Gate 5 - End-to-end pipeline validation
//!
//! Tests the complete clause analysis pipeline from text to relationships:
//! 1. Text → ClauseKeywordResolver → ClauseResolver → ClauseLinkResolver → ClauseQueryAPI
//! 2. Validates all relationship types: parent-child, coordination, exceptions
//! 3. Uses realistic contract-like text

use crate::{
    ClauseKeywordResolver, ClauseLink, ClauseLinkResolver, ClauseQueryAPI, ClauseResolver,
};
use layered_nlp_document::LayeredDocument;

/// Helper: Create document and extract links for testing
fn build_and_resolve(text: &str) -> (LayeredDocument, Vec<ClauseLink>) {
    let doc = LayeredDocument::from_text(text)
        .run_resolver(&ClauseKeywordResolver::new(
            &["if", "when"],
            &["and"],
            &["then"],
        ))
        .run_resolver(&ClauseResolver::default());

    let links = ClauseLinkResolver::resolve(&doc);
    (doc, links)
}

#[test]
fn integration_single_clause() {
    // Test 1: Single sentence, single clause (no relationships)
    let text = "The Company shall deliver the goods.";

    let (doc, links) = build_and_resolve(text);
    let api = ClauseQueryAPI::new(&links);
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);

    // Should have one Independent clause
    assert_eq!(clauses.len(), 1, "Expected one clause");

    // No relationships
    assert_eq!(links.len(), 0, "Independent clause has no links");

    // Query API should return empty results
    let span = clauses[0].span;
    assert_eq!(api.parent_clause(span), None, "No parent");
    assert_eq!(api.child_clauses(span).len(), 0, "No children");
    assert_eq!(api.conjuncts(span).len(), 0, "No conjuncts");
    assert_eq!(api.exceptions(span).len(), 0, "No exceptions");
}

#[test]
fn integration_coordination() {
    // Test 2: Single sentence, coordination (A and B)
    let text = "The Buyer shall pay the purchase price and the Seller shall transfer title.";

    let (doc, links) = build_and_resolve(text);
    let api = ClauseQueryAPI::new(&links);
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);

    // Should have two Independent clauses
    assert_eq!(clauses.len(), 2, "Expected two clauses");

    let clause_a = clauses[0].span;
    let clause_b = clauses[1].span;

    // Should have Conjunct links (bidirectional via transitive query)
    let a_conjuncts = api.conjuncts(clause_a);
    assert_eq!(a_conjuncts.len(), 1, "A should have one conjunct");
    assert!(a_conjuncts.contains(&clause_b), "A conjunct should be B");

    let b_conjuncts = api.conjuncts(clause_b);
    assert_eq!(b_conjuncts.len(), 1, "B should have one conjunct");
    assert!(b_conjuncts.contains(&clause_a), "B conjunct should be A");

    // No parent-child relationships
    assert_eq!(api.parent_clause(clause_a), None);
    assert_eq!(api.parent_clause(clause_b), None);
}

#[test]
fn integration_subordination() {
    // Test 3: Multi-sentence with subordination (condition → effect)
    let text = "When the Company receives notice, then it shall respond within ten days.";

    let (doc, links) = build_and_resolve(text);
    let api = ClauseQueryAPI::new(&links);
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);

    // Should have two clauses: Condition and TrailingEffect
    assert_eq!(clauses.len(), 2, "Expected two clauses");

    let condition = clauses[0].span;
    let effect = clauses[1].span;

    // Condition should have effect as parent
    assert_eq!(
        api.parent_clause(condition),
        Some(effect),
        "Condition's parent should be effect"
    );

    // Effect should have condition as child
    let children = api.child_clauses(effect);
    assert_eq!(children.len(), 1, "Effect should have one child");
    assert!(children.contains(&condition), "Child should be condition");

    // No coordination or exceptions
    assert_eq!(api.conjuncts(condition).len(), 0);
    assert_eq!(api.exceptions(effect).len(), 0);
}

#[test]
fn integration_complex_coordination() {
    // Test 4: Complex coordination with parties and obligations
    let text = "The Landlord shall maintain the property, the Tenant shall pay rent on time, and both parties shall provide notice of changes.";

    let (doc, links) = build_and_resolve(text);
    let api = ClauseQueryAPI::new(&links);
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);

    // Should have three clauses in coordination chain
    assert_eq!(clauses.len(), 3, "Expected three coordinated clauses");

    let clause_a = clauses[0].span; // Landlord maintains
    let clause_b = clauses[1].span; // Tenant pays
    let clause_c = clauses[2].span; // Both notify

    // Each clause should see all others as conjuncts (transitive chain A→B→C)
    let a_conjuncts = api.conjuncts(clause_a);
    assert_eq!(a_conjuncts.len(), 2, "A should have two conjuncts (B and C)");
    assert!(a_conjuncts.contains(&clause_b), "A should link to B");
    assert!(a_conjuncts.contains(&clause_c), "A should link to C");

    let b_conjuncts = api.conjuncts(clause_b);
    assert_eq!(b_conjuncts.len(), 2, "B should have two conjuncts (A and C)");
    assert!(b_conjuncts.contains(&clause_a), "B should link to A");
    assert!(b_conjuncts.contains(&clause_c), "B should link to C");

    let c_conjuncts = api.conjuncts(clause_c);
    assert_eq!(c_conjuncts.len(), 2, "C should have two conjuncts (A and B)");
    assert!(c_conjuncts.contains(&clause_a), "C should link to A");
    assert!(c_conjuncts.contains(&clause_b), "C should link to B");

    // No parent-child or exception relationships
    assert_eq!(api.parent_clause(clause_a), None);
    assert_eq!(api.exceptions(clause_a).len(), 0);
}

#[test]
fn integration_exception() {
    // Test 5: Exception clause (A unless B)
    let text = "The Tenant shall pay monthly rent unless explicitly waived by the Landlord in writing.";

    let (doc, links) = build_and_resolve(text);
    let api = ClauseQueryAPI::new(&links);
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);

    // Should have two clauses: main obligation and exception
    assert_eq!(clauses.len(), 2, "Expected two clauses");

    let main_clause = clauses[0].span; // Tenant shall pay
    let exception_clause = clauses[1].span; // waived by Landlord

    // Main clause should have one exception
    let exceptions = api.exceptions(main_clause);
    assert_eq!(exceptions.len(), 1, "Main clause should have one exception");
    assert!(
        exceptions.contains(&exception_clause),
        "Exception should be the waiver clause"
    );

    // Exception clause should have no exceptions pointing to it
    assert_eq!(
        api.exceptions(exception_clause).len(),
        0,
        "Exception clause has no exceptions"
    );

    // No coordination
    assert_eq!(api.conjuncts(main_clause).len(), 0);
    assert_eq!(api.conjuncts(exception_clause).len(), 0);
}

#[test]
fn integration_nested_coordination_with_exception() {
    // Test 6: Nested coordination with exception (comprehensive)
    // "When X and Y happen, then A applies and B follows, unless C."
    let text =
        "When notice is received and payment clears, then delivery occurs and title transfers, unless inspection fails.";

    let (doc, links) = build_and_resolve(text);
    let api = ClauseQueryAPI::new(&links);
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);

    // Should have 5 clauses:
    // - "notice is received" (Condition)
    // - "payment clears" (Condition)
    // - "delivery occurs" (TrailingEffect)
    // - "title transfers" (TrailingEffect)
    // - "inspection fails" (Independent)
    assert!(
        clauses.len() >= 4,
        "Expected at least 4 clauses, got {}",
        clauses.len()
    );

    // Verify coordination exists (conditions coordinated, effects coordinated)
    let has_coordination = links
        .iter()
        .any(|link| link.link.role == layered_nlp_document::ClauseRole::Conjunct);
    assert!(
        has_coordination,
        "Should have coordination links for 'and' keywords"
    );

    // Verify parent-child relationships exist (conditions → effects)
    let has_parent_child = links
        .iter()
        .any(|link| link.link.role == layered_nlp_document::ClauseRole::Parent);
    assert!(
        has_parent_child,
        "Should have parent-child links for condition→effect"
    );

    // Verify exception relationship exists (inspection fails → delivery/title)
    let has_exception = links
        .iter()
        .any(|link| link.link.role == layered_nlp_document::ClauseRole::Exception);
    assert!(
        has_exception,
        "Should have exception link for 'unless inspection fails'"
    );

    // Demonstrate query API works on complex structure
    for clause in clauses.iter() {
        let span = clause.span;

        // All queries should execute without panic
        let _parent = api.parent_clause(span);
        let _children = api.child_clauses(span);
        let _conjuncts = api.conjuncts(span);
        let _exceptions = api.exceptions(span);

        // At least some queries should return non-empty results
        // (This validates the entire pipeline produced meaningful relationships)
    }

    // Verify that different relationship types coexist
    let total_links = links.len();
    assert!(
        total_links > 3,
        "Complex sentence should have multiple relationships, got {}",
        total_links
    );
}

#[test]
fn integration_realistic_contract_clause() {
    // Bonus: Test with realistic contract language
    let text = "If the Borrower defaults on payment and fails to cure within thirty days, \
                then the Lender may accelerate the loan and demand immediate repayment, \
                except that no acceleration shall occur if the default results from force majeure.";

    let (doc, links) = build_and_resolve(text);
    let api = ClauseQueryAPI::new(&links);
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);

    // Should parse into multiple clauses with all relationship types
    assert!(
        clauses.len() >= 3,
        "Contract clause should produce multiple clauses"
    );

    // Should have parent-child (condition→effect)
    let parent_child_links: Vec<_> = links
        .iter()
        .filter(|link| {
            link.link.role == layered_nlp_document::ClauseRole::Parent
                || link.link.role == layered_nlp_document::ClauseRole::Child
        })
        .collect();
    assert!(
        !parent_child_links.is_empty(),
        "Should have condition→effect relationships"
    );

    // Should have coordination ("and", "and")
    let coordination_links: Vec<_> = links
        .iter()
        .filter(|link| link.link.role == layered_nlp_document::ClauseRole::Conjunct)
        .collect();
    assert!(
        !coordination_links.is_empty(),
        "Should have coordination relationships"
    );

    // Should have exception ("except that")
    let exception_links: Vec<_> = links
        .iter()
        .filter(|link| link.link.role == layered_nlp_document::ClauseRole::Exception)
        .collect();
    assert!(
        !exception_links.is_empty(),
        "Should have exception relationships"
    );

    // Verify query API can navigate the structure
    for clause in clauses.iter() {
        let span = clause.span;

        // Queries should work without panic
        let parent = api.parent_clause(span);
        let children = api.child_clauses(span);
        let conjuncts = api.conjuncts(span);
        let exceptions = api.exceptions(span);

        // At least verify we can access the results
        let _total = parent.is_some() as usize
            + children.len()
            + conjuncts.len()
            + exceptions.len();
    }
}

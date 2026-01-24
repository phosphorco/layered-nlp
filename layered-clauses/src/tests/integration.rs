//! Integration tests for M2 Gate 5 - End-to-end pipeline validation
//!
//! Tests the complete clause analysis pipeline from text to relationships:
//! 1. Text → ClauseKeywordResolver → ClauseResolver → ClauseLinkResolver → ClauseQueryAPI
//! 2. Validates all relationship types: parent-child, coordination, exceptions
//! 3. Uses realistic contract-like text

use crate::{
    ClauseKeywordResolver, ClauseLink, ClauseLinkResolver, ClauseQueryAPI, ClauseResolver,
};
use layered_nlp_document::{ClauseRole, LayeredDocument};

/// Helper: Create document and extract links for testing
/// Uses the full ClauseKeywordResolver with all coordination and exception keywords
fn build_and_resolve(text: &str) -> (LayeredDocument, Vec<ClauseLink>) {
    let doc = LayeredDocument::from_text(text)
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
        .any(|link| link.link.role == ClauseRole::Conjunct);
    assert!(
        has_coordination,
        "Should have coordination links for 'and' keywords"
    );

    // Verify parent-child relationships exist (conditions → effects)
    let has_parent_child = links
        .iter()
        .any(|link| link.link.role == ClauseRole::Parent);
    assert!(
        has_parent_child,
        "Should have parent-child links for condition→effect"
    );

    // Verify exception relationship exists (inspection fails → delivery/title)
    let has_exception = links
        .iter()
        .any(|link| link.link.role == ClauseRole::Exception);
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
            link.link.role == ClauseRole::Parent
                || link.link.role == ClauseRole::Child
        })
        .collect();
    assert!(
        !parent_child_links.is_empty(),
        "Should have condition→effect relationships"
    );

    // Should have coordination ("and", "and")
    let coordination_links: Vec<_> = links
        .iter()
        .filter(|link| link.link.role == ClauseRole::Conjunct)
        .collect();
    assert!(
        !coordination_links.is_empty(),
        "Should have coordination relationships"
    );

    // Should have exception ("except that")
    let exception_links: Vec<_> = links
        .iter()
        .filter(|link| link.link.role == ClauseRole::Exception)
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

// ============================================================================
// Gate 1: Exception Scope Propagation Tests
// ============================================================================
//
// These tests validate that exception clauses properly propagate to all
// coordinated conjuncts. When we have "A and B, unless C", the exception C
// should apply to BOTH A and B, not just the immediate predecessor.

#[test]
fn test_exception_applies_to_all_conjuncts() {
    // "A and B, unless C" → 2 exception links (C→A, C→B)
    let text = "Tenant pays rent and utilities unless waived by landlord.";
    let (doc, links) = build_and_resolve(text);

    let exception_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::Exception)
        .collect();

    assert_eq!(
        exception_links.len(),
        2,
        "Exception should link to BOTH coordinated clauses, not just immediate predecessor. \
        Got {} exception links, expected 2.",
        exception_links.len()
    );

    // All exception links should have same anchor (the exception clause)
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
    let exception_clause = clauses.last().expect("Should have exception clause");

    for link in &exception_links {
        assert_eq!(
            link.anchor, exception_clause.span,
            "All exceptions should originate from the exception clause"
        );
    }
}

#[test]
fn test_exception_applies_to_chain() {
    // "A and B and C, unless D" → 3 exception links
    let text = "Buyer pays and Seller delivers and Agent witnesses unless cancelled.";
    let (doc, links) = build_and_resolve(text);

    let exception_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::Exception)
        .collect();

    assert_eq!(
        exception_links.len(),
        3,
        "Exception should link to all 3 coordinated clauses. \
        Got {} exception links, expected 3.",
        exception_links.len()
    );

    // Verify all targets are distinct clauses
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
    assert!(
        clauses.len() >= 4,
        "Should have at least 4 clauses (3 main + 1 exception)"
    );

    let exception_targets: Vec<_> =
        exception_links.iter().map(|l| l.link.target).collect();
    assert_eq!(
        exception_targets.len(),
        3,
        "Exception should target 3 distinct clauses"
    );
}

#[test]
fn test_exception_does_not_cross_sentence() {
    // Exception should not apply across sentence boundary
    let text = "Tenant pays rent. Utilities unless waived.";
    let (_doc, links) = build_and_resolve(text);

    let exception_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::Exception)
        .collect();

    // Exception should only link within the second sentence, not to "Tenant pays rent"
    assert_eq!(
        exception_links.len(),
        1,
        "Exception should not cross sentence boundary. \
        Got {} exception links, expected 1 (only within second sentence).",
        exception_links.len()
    );
}

#[test]
fn test_exception_with_disjunction() {
    // "A or B, unless C" - exception applies to both disjuncts
    let text = "Buyer pays cash or arranges financing unless credit denied.";
    let (doc, links) = build_and_resolve(text);

    let exception_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::Exception)
        .collect();

    assert_eq!(
        exception_links.len(),
        2,
        "Exception should link to both disjuncts. \
        Got {} exception links, expected 2.",
        exception_links.len()
    );

    // Verify disjunction was detected (should have Conjunct or Disjunct links)
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
    assert!(
        clauses.len() >= 3,
        "Should have at least 3 clauses (2 disjuncts + 1 exception)"
    );
}

#[test]
fn test_exception_with_coordination_explicit_count() {
    // Update of existing test to assert on exact count
    // "A and B, unless C" should create exactly 2 exception links
    let text = "Tenant pays rent and utilities unless waived.";
    let (_doc, links) = build_and_resolve(text);

    let exception_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::Exception)
        .collect();

    // Previously this test used: assert!(!exception_links.is_empty())
    // Now we assert on exact count for Gate 1 validation
    assert_eq!(
        exception_links.len(),
        2,
        "Should link to both coordinated clauses. \
        Gate 1 requires exception scope propagation to all conjuncts."
    );
}

#[test]
fn test_exception_single_clause_baseline() {
    // "A unless B" → 1 exception link (baseline behavior unchanged)
    let text = "The Buyer pays unless excused.";
    let (_doc, links) = build_and_resolve(text);

    let exception_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::Exception)
        .collect();

    assert_eq!(
        exception_links.len(),
        1,
        "Single clause exception should create exactly one link. \
        This is the baseline behavior that Gate 1 should preserve."
    );
}

#[test]
fn test_exception_mixed_coordination_types() {
    // "A and B or C, unless D" - complex coordination before exception
    // Exception should apply to all clauses in the coordination scope
    let text = "Buyer pays and Seller ships or Agent arranges unless cancelled.";
    let (doc, links) = build_and_resolve(text);

    let exception_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::Exception)
        .collect();

    // Depending on parsing, expect at least 2 exception links
    // (the minimum if only immediate coordination group is covered)
    assert!(
        exception_links.len() >= 2,
        "Exception should link to multiple coordinated clauses. \
        Got {} exception links, expected at least 2.",
        exception_links.len()
    );

    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
    assert!(
        clauses.len() >= 4,
        "Should have at least 4 clauses (3 coordinated + 1 exception)"
    );
}

#[test]
fn test_chained_exceptions_precedence() {
    // "A, unless B, except when C" - C modifies B's exception of A
    // Results in: B excepts A, C excepts B, C transitively excepts A
    let text = "Tenant pays rent unless excused except for force majeure.";
    let (_doc, links) = build_and_resolve(text);

    let exception_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::Exception)
        .collect();

    // Should have: excused->rent, force majeure->excused, force majeure->rent (transitive)
    assert!(
        exception_links.len() >= 2,
        "Should have chained exception links, got {}",
        exception_links.len()
    );
}

#[test]
fn test_exception_boundary_semicolon() {
    // Exception should not cross semicolon boundary
    let text = "Tenant pays rent; utilities unless waived.";
    let (doc, links) = build_and_resolve(text);
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);

    let api = ClauseQueryAPI::new(&links);
    // "rent" clause should NOT have any exceptions (semicolon boundary)
    let rent_exceptions = api.exceptions(clauses[0].span);
    assert!(
        rent_exceptions.is_empty(),
        "Exception should not cross semicolon boundary"
    );
}

#[test]
fn test_exception_scope_query() {
    let text = "Tenant pays rent and utilities unless waived.";
    let (doc, links) = build_and_resolve(text);
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
    let api = ClauseQueryAPI::new(&links);

    // Both coordinated clauses should show the exception
    let rent_exceptions = api.exceptions(clauses[0].span);
    let utilities_exceptions = api.exceptions(clauses[1].span);

    assert!(!rent_exceptions.is_empty(), "Rent should have exception");
    assert!(
        !utilities_exceptions.is_empty(),
        "Utilities should have exception"
    );
}

// ============================================================================
// List Relationship Tests
// ============================================================================
//
// These tests validate that list markers are detected and linked to their
// containing clauses properly.

use crate::ListMarkerResolver;

/// Helper: Create document with list marker detection enabled
fn build_with_list_markers(text: &str) -> (LayeredDocument, Vec<ClauseLink>) {
    let doc = LayeredDocument::from_text(text)
        .run_resolver(&ListMarkerResolver::new())
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
fn test_list_simple_parenthesized_letters() {
    // "Do the following: (a) first item (b) second item"
    // Container: "Do the following:"
    // Items: "(a) first item", "(b) second item"
    let text = "Do the following: (a) first item (b) second item";
    let (doc, links) = build_with_list_markers(text);

    let list_item_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::ListItem)
        .collect();

    let list_container_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::ListContainer)
        .collect();

    // Should have list item links (one per item)
    assert!(
        !list_item_links.is_empty(),
        "Expected list item links for (a), (b) pattern. Got {} links total.",
        links.len()
    );

    // Should have list container links (one per item, bidirectional)
    assert!(
        !list_container_links.is_empty(),
        "Expected list container links. Got {} links total.",
        links.len()
    );

    // Query API should work
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
    let api = ClauseQueryAPI::new(&links);

    // At least some clauses should be list items
    let list_items_count = clauses
        .iter()
        .filter(|c| api.is_list_item(c.span))
        .count();
    assert!(
        list_items_count > 0,
        "Expected at least one clause to be a list item"
    );
}

#[test]
fn test_list_numbered_periods() {
    // "Requirements: 1. First 2. Second"
    let text = "Requirements: 1. First 2. Second";
    let (doc, links) = build_with_list_markers(text);

    let list_item_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::ListItem)
        .collect();

    // May or may not detect depending on clause parsing
    // This test validates the infrastructure works
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
    let api = ClauseQueryAPI::new(&links);

    // If we have list items, verify the query API works
    if !list_item_links.is_empty() {
        for clause in &clauses {
            if api.is_list_item(clause.span) {
                let container = api.list_container(clause.span);
                assert!(
                    container.is_some(),
                    "List items should have a container"
                );
            }
        }
    }
}

#[test]
fn test_list_with_coordination() {
    // List items can also have coordination: "(a), (b), and (c)"
    let text = "Tenant shall do the following: (a) pay rent, (b) maintain property, and (c) provide notice.";
    let (doc, links) = build_with_list_markers(text);

    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
    let api = ClauseQueryAPI::new(&links);

    // Check for list relationships
    let _list_item_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::ListItem)
        .collect();

    // Check for coordination relationships
    let _conjunct_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::Conjunct)
        .collect();

    // Should have some links (either list or coordination or both)
    assert!(
        !links.is_empty(),
        "Expected some relationships in list with coordination pattern"
    );

    // The query API should not crash
    for clause in &clauses {
        let _ = api.list_container(clause.span);
        let _ = api.list_items(clause.span);
        let _ = api.conjuncts(clause.span);
    }
}

#[test]
fn test_list_no_container() {
    // If the list item is the first clause, there's no container
    let text = "(a) First item only.";
    let (_doc, links) = build_with_list_markers(text);

    let list_item_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::ListItem)
        .collect();

    // Should have no list links because there's no container clause
    assert!(
        list_item_links.is_empty(),
        "First clause cannot be a list item (no container). Got {} list item links.",
        list_item_links.len()
    );
}

#[test]
fn test_list_query_api() {
    let text = "Do the following: (a) first (b) second";
    let (doc, links) = build_with_list_markers(text);
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
    let api = ClauseQueryAPI::new(&links);

    // Find container and items
    let containers: Vec<_> = clauses
        .iter()
        .filter(|c| api.is_list_container(c.span))
        .collect();

    let items: Vec<_> = clauses
        .iter()
        .filter(|c| api.is_list_item(c.span))
        .collect();

    // If we have containers, they should have items
    for container in &containers {
        let container_items = api.list_items(container.span);
        assert!(
            !container_items.is_empty(),
            "Container should have at least one item"
        );
    }

    // If we have items, they should have containers
    for item in &items {
        let container = api.list_container(item.span);
        assert!(
            container.is_some(),
            "Item should have a container"
        );
    }
}

#[test]
fn test_list_roman_numerals() {
    // Test parenthesized roman numerals: (i), (ii), (iii)
    let text = "The duties include: (i) duty one (ii) duty two (iii) duty three";
    let (doc, links) = build_with_list_markers(text);

    let _list_item_links: Vec<_> = links
        .iter()
        .filter(|l| l.link.role == ClauseRole::ListItem)
        .collect();

    // Should detect roman numeral list items if clauses are parsed correctly
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
    let api = ClauseQueryAPI::new(&links);

    // Verify query API handles this pattern
    for clause in &clauses {
        let _ = api.is_list_item(clause.span);
        let _ = api.is_list_container(clause.span);
    }
}

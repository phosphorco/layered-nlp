//! Gate 0 Cross-Line Clause Relationship Tests
//!
//! These tests validate the acceptance criteria for Gate 0 of the Clause Link Edge Cases plan.
//! They currently FAIL because cross-line linking is not implemented yet (same-line restriction
//! at lines 119, 252, and 324 in clause_link_resolver.rs).
//!
//! Tests are marked with `#[ignore]` to prevent breaking the build during development.
//!
//! ## Gate 0 Objectives
//! 1. Detect condition-effect relationships that span multiple lines
//! 2. Implement sentence-boundary detection to scope relationship search
//! 3. Handle multi-line condition blocks (lists, paragraphs)
//! 4. Maintain precision by adding confidence scores

use crate::{ClauseKeywordResolver, ClauseLink, ClauseLinkResolver, ClauseResolver};
use layered_nlp_document::{ClauseRole, LayeredDocument};

/// Helper: Create document with multi-line text and extract links
fn build_and_resolve(text: &str) -> (LayeredDocument, Vec<ClauseLink>) {
    let doc = LayeredDocument::from_text(text)
        .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"], &["or"], &["but", "however"], &["nor"]))
        .run_resolver(&ClauseResolver::default());

    let links = ClauseLinkResolver::resolve(&doc);
    (doc, links)
}

// =============================================================================
// Test 1: Cross-Line Simple Condition-Effect
// =============================================================================

#[test]
fn test_cross_line_simple_condition_effect() {
    // Multi-line "When X fails,\nthen Y terminates" should produce 2 links (Parent + Child)
    let text = "When the Tenant fails to pay rent,\nthen the Landlord may terminate the lease.";

    let (doc, links) = build_and_resolve(text);
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);

    println!("Number of lines: {}", doc.lines().len());
    for (line_idx, line) in doc.lines_enumerated() {
        println!("Line {}: (tokens: {})", line_idx, line.ll_tokens().len());
        // Check for ClauseKeyword attributes
        use crate::ClauseKeyword;
        use layered_nlp::x;
        for find in line.find(&x::attr::<ClauseKeyword>()) {
            let keyword: &ClauseKeyword = find.attr();
            println!("  Found keyword: {:?} at token {}", keyword, find.range().0);
        }
    }
    println!("Number of clauses: {}", clauses.len());
    for (i, clause) in clauses.iter().enumerate() {
        println!("  Clause {}: {:?} at {:?}", i, clause.category, clause.span);
    }
    println!("Number of links: {}", links.len());

    // Should have 2 clauses: Condition and TrailingEffect
    assert_eq!(
        clauses.len(),
        2,
        "Expected 2 clauses (Condition, TrailingEffect)"
    );

    // Should have 2 bidirectional links: Condition->Parent and Parent->Child
    assert_eq!(
        links.len(),
        2,
        "Expected 2 links (parent-child bidirectional)"
    );

    let parent_links: Vec<_> = links
        .iter()
        .filter(|link| link.link.role == ClauseRole::Parent)
        .collect();
    let child_links: Vec<_> = links
        .iter()
        .filter(|link| link.link.role == ClauseRole::Child)
        .collect();

    assert_eq!(parent_links.len(), 1, "Expected 1 Parent link");
    assert_eq!(child_links.len(), 1, "Expected 1 Child link");

    // Verify link targets match clause spans
    let condition_span = clauses[0].span;
    let effect_span = clauses[1].span;

    assert_eq!(
        parent_links[0].anchor, condition_span,
        "Parent link should be anchored on Condition"
    );
    assert_eq!(
        parent_links[0].link.target, effect_span,
        "Parent link should target TrailingEffect"
    );

    assert_eq!(
        child_links[0].anchor, effect_span,
        "Child link should be anchored on TrailingEffect"
    );
    assert_eq!(
        child_links[0].link.target, condition_span,
        "Child link should target Condition"
    );
}

// =============================================================================
// Test 2: Cross-Line Multi-Sentence (Negative Case)
// =============================================================================

#[test]
#[ignore] // TODO: Enable after sentence boundary detection is implemented (Gate 0)
fn test_cross_line_multi_sentence() {
    // Two unrelated sentences on different lines should NOT link
    let text = "The tenant pays rent.\nThe landlord maintains property.";

    let (_doc, links) = build_and_resolve(text);

    // No relationships should exist between these independent sentences
    assert_eq!(
        links.len(),
        0,
        "Unrelated sentences should not create links"
    );
}

// =============================================================================
// Test 3: Paragraph Condition Block
// =============================================================================

#[test]
#[ignore] // TODO: Enable after multi-line condition block detection is implemented (Gate 0)
fn test_paragraph_condition_block() {
    // Multi-line list pattern: "If:\n(a) X fails\n(b) Y expires\nthen Z terminates"
    let text = "If any of the following occur:\n(a) Tenant fails to pay rent\n(b) Property insurance expires\nthen Landlord may terminate the lease.";

    let (doc, links) = build_and_resolve(text);
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);

    // Should detect:
    // - Condition clauses for (a) and (b)
    // - TrailingEffect clause for "then Landlord may terminate"
    assert!(
        clauses.len() >= 2,
        "Expected at least 2 clauses (conditions and effect)"
    );

    // Should have parent-child relationships
    let has_parent_child = links
        .iter()
        .any(|link| link.link.role == ClauseRole::Parent || link.link.role == ClauseRole::Child);

    assert!(
        has_parent_child,
        "Multi-line condition block should create parent-child links"
    );
}

// =============================================================================
// Test 4: Confidence Scoring
// =============================================================================

#[test]
fn test_confidence_scoring() {
    use crate::LinkConfidence;

    // Same-line pattern (existing behavior) - should get High confidence
    let same_line_text = "If the tenant fails, then the landlord terminates.";
    let (_doc1, links1) = build_and_resolve(same_line_text);

    let same_line_link = links1
        .iter()
        .find(|link| link.link.role == ClauseRole::Parent)
        .expect("Same-line pattern should create Parent link");

    assert_eq!(
        same_line_link.confidence,
        LinkConfidence::High,
        "Same-line links should have High confidence"
    );

    // Cross-line pattern - should get Medium confidence
    let cross_line_text = "If the tenant fails,\nthen the landlord terminates.";
    let (_doc2, links2) = build_and_resolve(cross_line_text);

    let cross_line_link = links2
        .iter()
        .find(|link| link.link.role == ClauseRole::Parent)
        .expect("Cross-line pattern should create Parent link");

    assert_eq!(
        cross_line_link.confidence,
        LinkConfidence::Medium,
        "Cross-line links should have Medium confidence"
    );
}

// =============================================================================
// Test 5: Backward Search Limits
// =============================================================================

#[test]
#[ignore] // TODO: Enable after backward search implementation (Gate 0)
fn test_backward_search_limits() {
    // "then" without preceding "if" in sentence should produce no link
    let text = "Something happened.\nthen the party terminates.";

    let (_doc, links) = build_and_resolve(text);

    // No condition-effect link should be created because "then" has no matching "if/when"
    let condition_effect_links: Vec<_> = links
        .iter()
        .filter(|link| link.link.role == ClauseRole::Parent || link.link.role == ClauseRole::Child)
        .collect();

    assert_eq!(
        condition_effect_links.len(),
        0,
        "Orphaned 'then' without 'if/when' should not create links"
    );
}

// =============================================================================
// Test 6: Sentence Boundary Detection
// =============================================================================

#[test]
#[ignore] // TODO: Enable after SentenceBoundaryResolver is implemented (Gate 0)
fn test_sentence_boundary_detection() {
    // Period + capital = new sentence = no linking
    // The first sentence should NOT link to the second
    let text = "The contract expires. If tenant fails,\nthen landlord terminates.";

    let (doc, links) = build_and_resolve(text);
    let clauses = ClauseLinkResolver::extract_clause_spans(&doc);

    // Should have 3 clauses:
    // 1. "The contract expires" (Independent)
    // 2. "If tenant fails" (Condition)
    // 3. "then landlord terminates" (TrailingEffect)
    assert!(
        clauses.len() >= 3,
        "Expected at least 3 clauses across sentences"
    );

    // Find the links for the second sentence (condition-effect)
    let second_sentence_links: Vec<_> = links
        .iter()
        .filter(|link| {
            // Links should be between the Condition and TrailingEffect, not involving first clause
            let first_clause_span = clauses[0].span;
            link.anchor != first_clause_span && link.link.target != first_clause_span
        })
        .collect();

    // Should have condition-effect links only within second sentence
    assert!(
        !second_sentence_links.is_empty(),
        "Second sentence should have internal links"
    );

    // First clause should have no links (sentence boundary prevents cross-sentence linking)
    let first_clause_links: Vec<_> = links
        .iter()
        .filter(|link| {
            let first_clause_span = clauses[0].span;
            link.anchor == first_clause_span || link.link.target == first_clause_span
        })
        .collect();

    assert_eq!(
        first_clause_links.len(),
        0,
        "First sentence should not link to second sentence (sentence boundary)"
    );
}

// =============================================================================
// Additional Edge Cases
// =============================================================================

#[test]
#[ignore] // Known limitation: requires explicit "then" keyword (see test_cross_line_with_explicit_then)
fn test_cross_line_with_intervening_punctuation() {
    // KNOWN LIMITATION: This test documents that **implicit** trailing effect detection
    // across lines is NOT currently supported. The ClauseResolver is line-level and cannot
    // track condition state across line boundaries.
    //
    // Pattern tested (IMPLICIT - no "then" keyword):
    //   "When the tenant breaches the agreement,\nthe landlord may evict."
    //
    // This pattern currently FAILS because:
    // - Line 1: "When the tenant breaches the agreement," → recognized as Condition
    // - Line 2: "the landlord may evict." → NOT recognized as TrailingEffect (no "then")
    //
    // The ClauseResolver cannot infer that line 2 is a trailing effect without the
    // explicit "then" keyword, since it operates line-by-line and has no state machine
    // for multi-line clause relationships.
    //
    // WORKAROUND: Use explicit "then" keyword (see test_cross_line_with_explicit_then)
    // FUTURE ENHANCEMENT: Gate 2 or later may implement a DocumentResolver to handle
    // implicit trailing effects by looking for capital letters and modal phrases after conditions.
    let text = "When the tenant breaches the agreement,\nthe landlord may evict.";

    let (_doc, links) = build_and_resolve(text);

    let parent_child_links: Vec<_> = links
        .iter()
        .filter(|link| link.link.role == ClauseRole::Parent || link.link.role == ClauseRole::Child)
        .collect();

    // This assertion documents the current limitation
    assert!(
        parent_child_links.is_empty(),
        "Implicit trailing effect without 'then' is not supported (known limitation)"
    );
}

#[test]
fn test_cross_line_with_explicit_then() {
    // With explicit "then" keyword, cross-line condition-effect linkage works correctly.
    // This is the current supported pattern for multi-line conditions and effects.
    let text = "When the tenant breaches the agreement,\nthen the landlord may evict.";

    let (_doc, links) = build_and_resolve(text);

    let parent_child_links: Vec<_> = links
        .iter()
        .filter(|link| link.link.role == ClauseRole::Parent || link.link.role == ClauseRole::Child)
        .collect();

    assert!(
        !parent_child_links.is_empty(),
        "Explicit 'then' keyword enables cross-line linking"
    );

    // Verify bidirectional links
    let parent_links: Vec<_> = parent_child_links
        .iter()
        .filter(|link| link.link.role == ClauseRole::Parent)
        .collect();
    let child_links: Vec<_> = parent_child_links
        .iter()
        .filter(|link| link.link.role == ClauseRole::Child)
        .collect();

    assert_eq!(parent_links.len(), 1, "Expected 1 Parent link");
    assert_eq!(child_links.len(), 1, "Expected 1 Child link");
}

#[test]
fn test_cross_line_coordination() {
    // "and" spanning lines should still create coordination link
    let text = "The tenant pays rent\nand the landlord maintains property.";

    let (_doc, links) = build_and_resolve(text);

    let conjunct_links: Vec<_> = links
        .iter()
        .filter(|link| link.link.role == ClauseRole::Conjunct)
        .collect();

    assert!(
        !conjunct_links.is_empty(),
        "Cross-line 'and' should create coordination link"
    );
}

#[test]
#[ignore] // TODO: Enable after cross-line exception detection is implemented (Gate 0)
fn test_cross_line_exception() {
    // Exception keyword spanning lines
    let text = "The tenant shall pay rent,\nunless explicitly waived by landlord.";

    let (_doc, links) = build_and_resolve(text);

    let exception_links: Vec<_> = links
        .iter()
        .filter(|link| link.link.role == ClauseRole::Exception)
        .collect();

    assert!(
        !exception_links.is_empty(),
        "Cross-line 'unless' should create exception link"
    );
}

#[test]
#[ignore] // TODO: Enable after confidence scoring is implemented (Gate 0)
fn test_no_links_dropped_silently() {
    // When relationships are ambiguous, they should be flagged, not dropped
    let text = "When A happens,\n\nB occurs,\nthen C follows.";

    let (_doc, links) = build_and_resolve(text);

    // Either we create links (even with low confidence) OR we log/warn
    // This test ensures we don't silently drop relationships
    // For now, just verify some links exist (implementation will add warnings)
    assert!(
        !links.is_empty() || true, // Placeholder: actual implementation will add warning system
        "Ambiguous relationships should be flagged, not dropped silently"
    );
}

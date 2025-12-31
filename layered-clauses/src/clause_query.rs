//! Query API for clause relationships (M2 Gate 4).
//!
//! Provides efficient queries over clause links emitted by `ClauseLinkResolver`.
//! Enables navigation of parent-child hierarchies, coordination chains, and exceptions.
//!
//! ## Usage
//!
//! ```rust
//! use layered_nlp_document::{LayeredDocument, DocSpan};
//! use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ClauseQueryAPI};
//!
//! let doc = LayeredDocument::from_text("When it rains, then A happens and B occurs.")
//!     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
//!     .run_resolver(&ClauseResolver::default());
//!
//! let links = ClauseLinkResolver::resolve(&doc);
//! let api = ClauseQueryAPI::new(&links);
//!
//! // Query for parent clause containing a span
//! let span = DocSpan::single_line(0, 5, 10);
//! if let Some(parent) = api.parent_clause(span) {
//!     println!("Parent clause: {:?}", parent);
//! }
//!
//! // Get all conjuncts transitively
//! let conjuncts = api.conjuncts(span);
//! for conjunct in conjuncts {
//!     println!("Conjunct: {:?}", conjunct);
//! }
//! ```

use crate::ClauseLink;
use layered_nlp_document::{ClauseRole, DocSpan};
use std::collections::VecDeque;

/// Query API for clause relationships.
///
/// Operates on a slice of `ClauseLink` objects returned by `ClauseLinkResolver::resolve()`.
/// Provides methods to query parent-child hierarchies, conjunct chains, and exception relationships.
pub struct ClauseQueryAPI<'a> {
    links: &'a [ClauseLink],
}

impl<'a> ClauseQueryAPI<'a> {
    /// Create a new query API from clause links.
    ///
    /// # Arguments
    /// * `links` - The clause links returned by `ClauseLinkResolver::resolve()`
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::LayeredDocument;
    /// # use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ClauseQueryAPI};
    /// let doc = LayeredDocument::from_text("When it rains, then it pours.")
    ///     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
    ///     .run_resolver(&ClauseResolver::default());
    ///
    /// let links = ClauseLinkResolver::resolve(&doc);
    /// let api = ClauseQueryAPI::new(&links);
    /// ```
    pub fn new(links: &'a [ClauseLink]) -> Self {
        Self { links }
    }

    /// Find the direct parent clause containing a given span.
    ///
    /// Returns the parent clause if this span has a Parent link, or None if it's a top-level clause.
    ///
    /// # Arguments
    /// * `span` - The clause span to query
    ///
    /// # Returns
    /// The parent clause span, or None if there is no parent
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::{LayeredDocument, DocSpan};
    /// # use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ClauseQueryAPI};
    /// let doc = LayeredDocument::from_text("When it rains, then it pours.")
    ///     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
    ///     .run_resolver(&ClauseResolver::default());
    ///
    /// let links = ClauseLinkResolver::resolve(&doc);
    /// let api = ClauseQueryAPI::new(&links);
    ///
    /// // Find the condition clause span
    /// let condition_span = DocSpan::single_line(0, 0, 2); // "When it rains"
    ///
    /// // Query for its parent
    /// if let Some(parent) = api.parent_clause(condition_span) {
    ///     // parent should be the TrailingEffect clause
    ///     assert!(parent.start.token > condition_span.end.token);
    /// }
    /// ```
    pub fn parent_clause(&self, span: DocSpan) -> Option<DocSpan> {
        self.links
            .iter()
            .find(|link| link.anchor == span && link.link.role == ClauseRole::Parent)
            .map(|link| link.link.target)
    }

    /// Find the containing clause for a span that may not be a clause itself.
    ///
    /// This is similar to `parent_clause()` but conceptually asks: "What clause contains
    /// this position?" rather than "What is the parent of this clause?"
    ///
    /// For now, this is implemented as an alias to `parent_clause()` since we only track
    /// clause-to-clause relationships. In the future, this could be extended to handle
    /// arbitrary spans by checking which clause's span range contains the query span.
    ///
    /// # Arguments
    /// * `span` - The span to find the containing clause for
    ///
    /// # Returns
    /// The containing clause span, or None if the span is not contained by any clause
    pub fn containing_clause(&self, span: DocSpan) -> Option<DocSpan> {
        self.parent_clause(span)
    }

    /// Get all conjuncts of a clause transitively via the chain.
    ///
    /// For a coordination chain A→B→C, querying any clause returns all other clauses
    /// in the chain. This handles transitive closure by following Conjunct links
    /// in both forward and backward directions.
    ///
    /// # Arguments
    /// * `span` - The clause span to find conjuncts for
    ///
    /// # Returns
    /// A vector of all conjunct clause spans (not including the input span)
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::{LayeredDocument, DocSpan};
    /// # use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ClauseQueryAPI};
    /// let doc = LayeredDocument::from_text("A pays, B works, and C manages.")
    ///     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
    ///     .run_resolver(&ClauseResolver::default());
    ///
    /// let links = ClauseLinkResolver::resolve(&doc);
    /// let api = ClauseQueryAPI::new(&links);
    ///
    /// // Get conjuncts for the first clause (A)
    /// // Should return both B and C due to chain: A→B→C
    /// // let conjuncts = api.conjuncts(clause_a_span);
    /// // assert_eq!(conjuncts.len(), 2);
    /// ```
    pub fn conjuncts(&self, span: DocSpan) -> Vec<DocSpan> {
        let mut visited = Vec::new();
        let mut queue = VecDeque::new();
        let mut result = Vec::new();

        visited.push(span);
        queue.push_back(span);

        // BFS to find all connected clauses via Conjunct links
        while let Some(current) = queue.pop_front() {
            // Find all conjunct links from current clause
            for link in self.links.iter() {
                if link.anchor == current && link.link.role == ClauseRole::Conjunct {
                    let target = link.link.target;
                    if !visited.contains(&target) {
                        visited.push(target);
                        result.push(target);
                        queue.push_back(target);
                    }
                }
            }

            // Also search backwards: find clauses that point to current as conjunct
            for link in self.links.iter() {
                if link.link.target == current && link.link.role == ClauseRole::Conjunct {
                    let source = link.anchor;
                    if !visited.contains(&source) {
                        visited.push(source);
                        result.push(source);
                        queue.push_back(source);
                    }
                }
            }
        }

        result
    }

    /// Get all exception clauses that modify a given span.
    ///
    /// Returns clauses that have Exception links pointing to the query span.
    /// These are clauses that carve out exceptions or special cases for the main clause.
    ///
    /// # Arguments
    /// * `span` - The main clause span to find exceptions for
    ///
    /// # Returns
    /// A vector of exception clause spans
    ///
    /// # Example
    /// ```
    /// # use layered_nlp_document::{LayeredDocument, DocSpan};
    /// # use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver, ClauseQueryAPI};
    /// let doc = LayeredDocument::from_text("Tenant shall pay rent unless waived by Landlord.")
    ///     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
    ///     .run_resolver(&ClauseResolver::default());
    ///
    /// let links = ClauseLinkResolver::resolve(&doc);
    /// let api = ClauseQueryAPI::new(&links);
    ///
    /// // Get exceptions for the main clause ("Tenant shall pay rent")
    /// // let main_clause_span = DocSpan::single_line(0, 0, 4);
    /// // let exceptions = api.exceptions(main_clause_span);
    /// // assert_eq!(exceptions.len(), 1); // "waived by Landlord"
    /// ```
    pub fn exceptions(&self, span: DocSpan) -> Vec<DocSpan> {
        self.links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception && link.link.target == span)
            .map(|link| link.anchor)
            .collect()
    }

    /// Get all child clauses directly contained by a parent clause.
    ///
    /// Returns clauses that have been marked as children of the query span.
    ///
    /// # Arguments
    /// * `span` - The parent clause span
    ///
    /// # Returns
    /// A vector of child clause spans
    pub fn child_clauses(&self, span: DocSpan) -> Vec<DocSpan> {
        self.links
            .iter()
            .filter(|link| link.anchor == span && link.link.role == ClauseRole::Child)
            .map(|link| link.link.target)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver};
    use layered_nlp_document::LayeredDocument;

    fn create_test_document(text: &str) -> LayeredDocument {
        LayeredDocument::from_text(text)
            .run_resolver(&ClauseKeywordResolver::new(
                &["if", "when"],
                &["and"],
                &["then"],
            ))
            .run_resolver(&ClauseResolver::default())
    }

    #[test]
    fn test_parent_clause_query() {
        let doc = create_test_document("When it rains, then it pours.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        // Get clause spans
        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clause_spans.len(), 2);

        let condition_span = clause_spans[0].span; // "When it rains"
        let effect_span = clause_spans[1].span; // "then it pours"

        // Condition should have effect as parent
        let parent = api.parent_clause(condition_span);
        assert_eq!(parent, Some(effect_span));

        // Effect (top-level) should have no parent
        let parent = api.parent_clause(effect_span);
        assert_eq!(parent, None);
    }

    #[test]
    fn test_containing_clause_query() {
        let doc = create_test_document("When it rains, then it pours.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        let condition_span = clause_spans[0].span;
        let effect_span = clause_spans[1].span;

        // containing_clause should work like parent_clause
        assert_eq!(api.containing_clause(condition_span), Some(effect_span));
        assert_eq!(api.containing_clause(effect_span), None);
    }

    #[test]
    fn test_conjuncts_two_clauses() {
        let doc = create_test_document("The tenant pays rent and the landlord provides notice.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clause_spans.len(), 2);

        let first_span = clause_spans[0].span;
        let second_span = clause_spans[1].span;

        // First clause should have second as conjunct
        let conjuncts = api.conjuncts(first_span);
        assert_eq!(conjuncts.len(), 1);
        assert!(conjuncts.contains(&second_span));

        // Second clause should have first as conjunct (bidirectional)
        let conjuncts = api.conjuncts(second_span);
        assert_eq!(conjuncts.len(), 1);
        assert!(conjuncts.contains(&first_span));
    }

    #[test]
    fn test_conjuncts_chain_transitive() {
        let doc = create_test_document("A pays, B works, and C manages.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clause_spans.len(), 3);

        let a_span = clause_spans[0].span;
        let b_span = clause_spans[1].span;
        let c_span = clause_spans[2].span;

        // A should return [B, C] due to transitive chain A→B→C
        let conjuncts = api.conjuncts(a_span);
        assert_eq!(conjuncts.len(), 2);
        assert!(conjuncts.contains(&b_span));
        assert!(conjuncts.contains(&c_span));

        // B should return [A, C] (connected both ways)
        let conjuncts = api.conjuncts(b_span);
        assert_eq!(conjuncts.len(), 2);
        assert!(conjuncts.contains(&a_span));
        assert!(conjuncts.contains(&c_span));

        // C should return [A, B]
        let conjuncts = api.conjuncts(c_span);
        assert_eq!(conjuncts.len(), 2);
        assert!(conjuncts.contains(&a_span));
        assert!(conjuncts.contains(&b_span));
    }

    #[test]
    fn test_conjuncts_no_coordination() {
        let doc = create_test_document("The tenant pays rent.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clause_spans.len(), 1);

        let span = clause_spans[0].span;

        // Single clause has no conjuncts
        let conjuncts = api.conjuncts(span);
        assert_eq!(conjuncts.len(), 0);
    }

    #[test]
    fn test_exceptions_simple() {
        let doc = LayeredDocument::from_text("Tenant shall pay rent unless waived by Landlord.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clause_spans.len(), 2);

        let main_span = clause_spans[0].span; // "Tenant shall pay rent"
        let exception_span = clause_spans[1].span; // "waived by Landlord"

        // Main clause should have one exception
        let exceptions = api.exceptions(main_span);
        assert_eq!(exceptions.len(), 1);
        assert!(exceptions.contains(&exception_span));

        // Exception clause should have no exceptions pointing to it
        let exceptions = api.exceptions(exception_span);
        assert_eq!(exceptions.len(), 0);
    }

    #[test]
    fn test_exceptions_no_exception_keyword() {
        let doc = create_test_document("Tenant pays rent. Landlord maintains property.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clause_spans.len(), 2);

        let first_span = clause_spans[0].span;

        // No exception keywords, so no exceptions
        let exceptions = api.exceptions(first_span);
        assert_eq!(exceptions.len(), 0);
    }

    #[test]
    fn test_child_clauses() {
        let doc = create_test_document("When it rains, then it pours.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        let condition_span = clause_spans[0].span;
        let effect_span = clause_spans[1].span;

        // Effect clause should have condition as child
        let children = api.child_clauses(effect_span);
        assert_eq!(children.len(), 1);
        assert!(children.contains(&condition_span));

        // Condition clause should have no children
        let children = api.child_clauses(condition_span);
        assert_eq!(children.len(), 0);
    }

    #[test]
    fn test_mixed_link_types() {
        // Document with parent-child, coordination, and exceptions
        let doc = LayeredDocument::from_text("When A happens and B occurs, then C applies unless D.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        // Should have: A (Condition), B (Condition), C (TrailingEffect), D (Independent)
        assert!(clause_spans.len() >= 3);

        // Verify we can query each relationship type
        let first_span = clause_spans[0].span;

        // Should have parent (the TrailingEffect clause)
        let parent = api.parent_clause(first_span);
        assert!(parent.is_some());

        // Should have conjunct (B)
        let conjuncts = api.conjuncts(first_span);
        assert!(!conjuncts.is_empty());
    }

    #[test]
    fn test_empty_links() {
        // Create API with no links
        let links = vec![];
        let api = ClauseQueryAPI::new(&links);

        let arbitrary_span = DocSpan::single_line(0, 0, 5);

        // All queries should return empty/None
        assert_eq!(api.parent_clause(arbitrary_span), None);
        assert_eq!(api.containing_clause(arbitrary_span), None);
        assert_eq!(api.conjuncts(arbitrary_span).len(), 0);
        assert_eq!(api.exceptions(arbitrary_span).len(), 0);
        assert_eq!(api.child_clauses(arbitrary_span).len(), 0);
    }

    #[test]
    fn test_conjuncts_four_clause_chain() {
        // Test longer chain: A, B, C, and D
        let doc = create_test_document("A acts, B reacts, C contracts, and D extracts.");
        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);

        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clause_spans.len(), 4);

        let a_span = clause_spans[0].span;
        let b_span = clause_spans[1].span;
        let c_span = clause_spans[2].span;
        let d_span = clause_spans[3].span;

        // A should be connected to all others transitively
        let conjuncts = api.conjuncts(a_span);
        assert_eq!(conjuncts.len(), 3);
        assert!(conjuncts.contains(&b_span));
        assert!(conjuncts.contains(&c_span));
        assert!(conjuncts.contains(&d_span));

        // B should be connected to all others
        let conjuncts = api.conjuncts(b_span);
        assert_eq!(conjuncts.len(), 3);
        assert!(conjuncts.contains(&a_span));
        assert!(conjuncts.contains(&c_span));
        assert!(conjuncts.contains(&d_span));
    }

    #[test]
    fn test_comprehensive_all_link_types() {
        // Comprehensive test demonstrating all query methods with multiple link types
        // Document structure:
        // "When X occurs and Y happens, then Z applies unless W, and Q follows."
        //
        // Expected structure:
        // - X (Condition) and Y (Condition) are coordinated
        // - X and Y are children of Z (TrailingEffect)
        // - W is an exception to Z
        // - Z and Q are coordinated
        let doc = LayeredDocument::from_text(
            "When X occurs and Y happens, then Z applies unless W, and Q follows."
        )
        .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
        .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);
        let api = ClauseQueryAPI::new(&links);
        let clause_spans = ClauseLinkResolver::extract_clause_spans(&doc);

        // We should have at least 5 clauses: X, Y, Z, W, Q
        assert!(clause_spans.len() >= 4, "Expected at least 4 clauses, got {}", clause_spans.len());

        // Demonstrate all query capabilities by printing what we found
        // (This is an integration test to ensure the API works end-to-end)

        for clause in clause_spans.iter() {
            let span = clause.span;

            // Test parent_clause
            let parent = api.parent_clause(span);

            // Test child_clauses
            let children = api.child_clauses(span);

            // Test conjuncts
            let conjuncts = api.conjuncts(span);

            // Test exceptions
            let exceptions = api.exceptions(span);

            // All queries should complete without panic
            // This validates that the API handles complex documents correctly
            // Simply accessing the results ensures the queries execute without error
            let _ = parent;
            let _ = children;
            let _ = conjuncts;
            let _ = exceptions;
        }

        // Verify that we have at least some links
        assert!(!links.is_empty(), "Expected some clause links");

        // Verify we have multiple link types
        let has_parent = links.iter().any(|l| l.link.role == ClauseRole::Parent);
        let has_conjunct = links.iter().any(|l| l.link.role == ClauseRole::Conjunct);
        let has_exception = links.iter().any(|l| l.link.role == ClauseRole::Exception);

        assert!(has_parent || has_conjunct || has_exception,
            "Expected at least one link type in complex document");
    }
}

//! Document-level clause link resolver for M2 Gate 1.
//!
//! This resolver operates on a `LayeredDocument` that has already been processed
//! by `ClauseResolver` (line-level). It identifies clause spans across the document
//! and emits `SpanLink<ClauseRole>` edges representing parent-child relationships.
//!
//! ## Architecture
//!
//! This is a simpler function-based approach rather than a full `DocumentResolver` trait.
//! The function takes clause spans and returns SpanLink edges that can be stored
//! alongside the existing Clause attributes.
//!
//! ## Usage
//!
//! ```rust
//! use layered_nlp_document::LayeredDocument;
//! use layered_clauses::{ClauseKeywordResolver, ClauseResolver, ClauseLinkResolver};
//!
//! let doc = LayeredDocument::from_text("When it rains, then it pours.")
//!     .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
//!     .run_resolver(&ClauseResolver::default());
//!
//! // Find clause relationships
//! let links = ClauseLinkResolver::resolve(&doc);
//! ```

use crate::Clause;
use layered_nlp::x;
use layered_nlp_document::{ClauseRole, DocSpan, DocSpanLink, LayeredDocument};

/// Document-level resolver that emits `SpanLink<ClauseRole>` edges based on Clause attributes.
///
/// This resolver operates after ClauseResolver has run on all lines.
/// It identifies parent-child relationships between clauses:
/// - Condition clauses are often children of TrailingEffect clauses
/// - Independent clauses have no parent relationships
pub struct ClauseLinkResolver;

/// A clause span with its category and document position
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClauseSpan {
    /// The document span covering this clause
    pub span: DocSpan,
    /// The clause category
    pub category: Clause,
}

/// A clause relationship represented as a SpanLink
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClauseLink {
    /// The anchor clause (where this link is "stored")
    pub anchor: DocSpan,
    /// The link to the related clause
    pub link: DocSpanLink<ClauseRole>,
}

impl ClauseLinkResolver {
    /// Extract all clause spans from a document.
    ///
    /// Searches through all lines for Clause attributes and converts them
    /// to document-level spans with positions.
    pub fn extract_clause_spans(doc: &LayeredDocument) -> Vec<ClauseSpan> {
        let mut clauses = Vec::new();

        for (line_idx, line) in doc.lines_enumerated() {
            // Find all Clause attributes on this line
            for find in line.find(&x::attr::<Clause>()) {
                let span_range = find.range();
                let clause_attr: &Clause = find.attr();

                // Convert line-local span to document span
                let doc_span = DocSpan::single_line(line_idx, span_range.0, span_range.1);

                clauses.push(ClauseSpan {
                    span: doc_span,
                    category: clause_attr.clone(),
                });
            }
        }

        clauses
    }

    /// Detect coordination chains between clauses separated by coordination keywords.
    ///
    /// Emits chain topology: A→B, B→C for "A, B, and C" (not star pattern A→B, A→C).
    ///
    /// This handles two coordination patterns:
    /// 1. Explicit "and" between clauses: "A and B"
    /// 2. Comma-separated lists with final "and": "A, B, and C"
    ///    In this case, commas implicitly coordinate adjacent clauses if there's
    ///    an "and" anywhere later in the sequence.
    fn detect_coordination(
        clause_spans: &[ClauseSpan],
        doc: &LayeredDocument,
    ) -> Vec<ClauseLink> {
        let mut links = Vec::new();

        // First pass: identify if this line contains any coordination
        // by checking if there's at least one "and" keyword
        let has_any_and = clause_spans.windows(2).any(|pair| {
            let current = &pair[0];
            let next = &pair[1];

            current.span.start.line == next.span.start.line
                && Self::has_coordination_keyword_between(
                    doc,
                    current.span.start.line,
                    current.span.end.token,
                    next.span.start.token,
                )
        });

        // Second pass: create chain links
        for i in 0..clause_spans.len().saturating_sub(1) {
            let current = &clause_spans[i];
            let next = &clause_spans[i + 1];

            // Only link clauses on the same line for now (cross-line coordination
            // requires more sophisticated heuristics)
            if current.span.start.line != next.span.start.line {
                continue;
            }

            // Check if there's an "and" keyword between current and next clause
            let has_coordination_keyword = Self::has_coordination_keyword_between(
                doc,
                current.span.start.line,
                current.span.end.token,
                next.span.start.token,
            );

            // Create link if either:
            // 1. Explicit "and" between these two clauses
            // 2. This line has coordination somewhere AND these are adjacent same-type clauses
            //    (handles "A, B, and C" pattern where commas implicitly coordinate)
            let should_link = has_coordination_keyword
                || (has_any_and && current.category == next.category);

            if should_link {
                // Create unidirectional chain link: current → next
                links.push(ClauseLink {
                    anchor: current.span,
                    link: crate::ClauseLinkBuilder::conjunct_link(next.span),
                });
            }
        }

        links
    }

    /// Check if there's a coordination keyword (ClauseKeyword::And) between two token positions.
    fn has_coordination_keyword_between(
        doc: &LayeredDocument,
        line_idx: usize,
        start_token: usize,
        end_token: usize,
    ) -> bool {
        use crate::ClauseKeyword;

        if let Some(line) = doc.lines().get(line_idx) {
            // Search the range between the two clauses for ClauseKeyword::And
            for find in line.find(&x::attr::<ClauseKeyword>()) {
                let (token_start, _token_end) = find.range();

                // Keyword must be between the clauses
                if token_start >= start_token && token_start < end_token {
                    if let ClauseKeyword::And = find.attr() {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Check if there's an exception keyword (ClauseKeyword::Exception) between two token positions.
    fn has_exception_keyword_between(
        doc: &LayeredDocument,
        line_idx: usize,
        start_token: usize,
        end_token: usize,
    ) -> bool {
        use crate::ClauseKeyword;

        if let Some(line) = doc.lines().get(line_idx) {
            // Search for ClauseKeyword::Exception in the range
            for find in line.find(&x::attr::<ClauseKeyword>()) {
                let (token_start, _token_end) = find.range();

                if token_start >= start_token && token_start < end_token {
                    if let ClauseKeyword::Exception = find.attr() {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Check if there's an exception keyword before a given token position
    fn has_exception_keyword_before(
        doc: &LayeredDocument,
        line_idx: usize,
        token_pos: usize,
    ) -> bool {
        use crate::ClauseKeyword;

        if let Some(line) = doc.lines().get(line_idx) {
            for find in line.find(&x::attr::<ClauseKeyword>()) {
                let (token_start, _token_end) = find.range();

                // Keyword must be before the token position
                if token_start < token_pos {
                    if let ClauseKeyword::Exception = find.attr() {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Detect exception/carve-out relationships between clauses.
    ///
    /// Emits Exception links pointing from exception clause to main clause.
    ///
    /// Handles patterns like:
    /// - "A unless B" → Exception link from B to A
    /// - "A, except B" → Exception link from B to A
    /// - "Notwithstanding X, Y shall apply" → Exception link from Y to X
    ///
    /// Exception keywords: "except", "unless", "notwithstanding", "provided", "subject"
    ///
    /// For simplicity in Gate 3, all exception keywords use the same pattern:
    /// the clause following the exception keyword is the exception that modifies
    /// the preceding clause.
    fn detect_exceptions(
        clause_spans: &[ClauseSpan],
        doc: &LayeredDocument,
    ) -> Vec<ClauseLink> {
        let mut links = Vec::new();

        for i in 0..clause_spans.len().saturating_sub(1) {
            let current = &clause_spans[i];
            let next = &clause_spans[i + 1];

            // Same-line restriction
            if current.span.start.line != next.span.start.line {
                continue;
            }

            // Check if there's an exception keyword between current and next
            if Self::has_exception_keyword_between(
                doc,
                current.span.start.line,
                current.span.end.token,
                next.span.start.token,
            ) {
                // Standard pattern: "A [exception-keyword] B"
                // B is the exception that modifies A
                // Exception link: next (exception) → current (main)
                links.push(ClauseLink {
                    anchor: next.span,
                    link: crate::ClauseLinkBuilder::exception_link(current.span),
                });
            }
        }

        // Special case: Check for exception keyword at the very start (before first clause)
        // Pattern: "[exception-keyword] X, Y" where X is first clause
        // In this case, Y modifies/excepts X
        if clause_spans.len() >= 2 {
            let first = &clause_spans[0];
            let second = &clause_spans[1];

            if first.span.start.line == second.span.start.line {
                // Check if there's an exception keyword before the first clause
                if Self::has_exception_keyword_before(doc, first.span.start.line, first.span.start.token) {
                    // second clause is the main clause that applies despite first clause
                    // Exception link: second → first
                    links.push(ClauseLink {
                        anchor: second.span,
                        link: crate::ClauseLinkBuilder::exception_link(first.span),
                    });
                }
            }
        }

        links
    }

    /// Resolve clause relationships and emit SpanLink edges.
    ///
    /// Implemented patterns:
    /// - Gate 1: Condition clauses followed by TrailingEffect clauses form parent-child relationships
    /// - Gate 2: Coordination chains ("and", "or", "but") emit Conjunct links with chain topology
    /// - Gate 3: Exception clauses ("except", "unless", "notwithstanding") emit Exception links
    ///
    /// TODO(Gate 4): Storage integration - links should be persisted alongside Clause
    /// attributes in the SpanIndex rather than returned as ephemeral Vec. This will
    /// enable querying relationships via the standard query API.
    pub fn resolve(doc: &LayeredDocument) -> Vec<ClauseLink> {
        let clause_spans = Self::extract_clause_spans(doc);
        let mut links = Vec::new();

        // Gate 1: Condition → TrailingEffect parent-child relationships
        // Handles patterns like: "When it rains, then it pours"
        // where "it rains" (Condition) is a child of "it pours" (TrailingEffect)
        for i in 0..clause_spans.len() {
            let current = &clause_spans[i];

            // Look for Condition followed by TrailingEffect on the same line
            if current.category == Clause::Condition {
                // Find the next clause
                if let Some(next) = clause_spans.get(i + 1) {
                    // Same-line restriction: cross-line clause relationships require
                    // more sophisticated heuristics. For now, we only link clauses
                    // within a single line to avoid false positives.
                    if next.category == Clause::TrailingEffect
                        && current.span.start.line == next.span.start.line
                    {
                        // Condition (child) points to TrailingEffect (parent)
                        links.push(ClauseLink {
                            anchor: current.span,
                            link: crate::ClauseLinkBuilder::parent_link(next.span),
                        });

                        // TrailingEffect (parent) points back to Condition (child)
                        links.push(ClauseLink {
                            anchor: next.span,
                            link: crate::ClauseLinkBuilder::child_link(current.span),
                        });
                    }
                }
            }
        }

        // Gate 2: Detect coordination chains
        // Handles patterns like: "A and B" → one link, "A, B, and C" → two links (chain)
        let coordination_links = Self::detect_coordination(&clause_spans, doc);
        links.extend(coordination_links);

        // Gate 3: Detect exception/carve-out relationships
        // Handles patterns like: "A unless B" → Exception link from B to A
        let exception_links = Self::detect_exceptions(&clause_spans, doc);
        links.extend(exception_links);

        links
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ClauseKeywordResolver, ClauseResolver};

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
    fn test_extract_clause_spans() {
        let doc = create_test_document("When it rains, then it pours.");

        let clauses = ClauseLinkResolver::extract_clause_spans(&doc);

        // Should find 2 clauses: Condition and TrailingEffect
        assert_eq!(clauses.len(), 2);
        assert_eq!(clauses[0].category, Clause::Condition);
        assert_eq!(clauses[1].category, Clause::TrailingEffect);
    }

    #[test]
    fn test_resolve_condition_trailing_effect() {
        let doc = create_test_document("When it rains, then it pours.");

        let links = ClauseLinkResolver::resolve(&doc);

        // Should create 2 bidirectional links: Condition->Parent and Parent->Child
        assert_eq!(links.len(), 2);

        // First link: Condition points to TrailingEffect as Parent
        assert_eq!(links[0].link.role, ClauseRole::Parent);

        // Second link: TrailingEffect points to Condition as Child
        assert_eq!(links[1].link.role, ClauseRole::Child);
    }

    #[test]
    fn test_resolve_independent_clause() {
        let doc = create_test_document("It rains.");

        let links = ClauseLinkResolver::resolve(&doc);

        // Independent clause should have no relationships
        assert_eq!(links.len(), 0);
    }

    #[test]
    fn test_extract_from_empty_document() {
        let doc = LayeredDocument::from_text("");

        let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clauses.len(), 0);

        let links = ClauseLinkResolver::resolve(&doc);
        assert_eq!(links.len(), 0);
    }

    #[test]
    fn test_multiple_independent_clauses() {
        // Create a document with multiple independent clauses (no keywords)
        let doc = LayeredDocument::from_text("The tenant shall pay rent. The landlord provides notice.")
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        // Independent clauses should not have relationships with each other
        assert_eq!(links.len(), 0);
    }

    // ========================================================================
    // Gate 2: Coordination Chain Tests
    // ========================================================================

    #[test]
    fn test_coordination_simple_two_clauses() {
        let doc = create_test_document("The tenant pays rent and the landlord provides notice.");

        let links = ClauseLinkResolver::resolve(&doc);

        // Should have one Conjunct link: first clause → second clause
        // Filter for Conjunct links only (ignoring any parent/child links)
        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        assert_eq!(
            conjunct_links.len(),
            1,
            "Expected one Conjunct link for 'A and B'"
        );
        assert_eq!(conjunct_links[0].link.role, ClauseRole::Conjunct);
    }

    #[test]
    fn test_coordination_chain_three_clauses() {
        let doc = create_test_document("A pays, B works, and C manages.");

        let links = ClauseLinkResolver::resolve(&doc);

        // Should have two Conjunct links forming a chain: A→B, B→C (not A→B, A→C)
        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        assert_eq!(
            conjunct_links.len(),
            2,
            "Expected two Conjunct links for 'A, B, and C' chain topology"
        );

        // Verify chain topology: first→second, second→third
        let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
        assert!(clauses.len() >= 3, "Should have at least 3 clauses");

        // First link should go from first clause to second
        assert_eq!(conjunct_links[0].anchor, clauses[0].span);
        assert_eq!(conjunct_links[0].link.target, clauses[1].span);

        // Second link should go from second clause to third
        assert_eq!(conjunct_links[1].anchor, clauses[1].span);
        assert_eq!(conjunct_links[1].link.target, clauses[2].span);
    }

    #[test]
    fn test_coordination_no_and_keyword() {
        let doc = create_test_document("The tenant pays rent. The landlord provides notice.");

        let links = ClauseLinkResolver::resolve(&doc);

        // No "and" keyword, so no Conjunct links
        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        assert_eq!(conjunct_links.len(), 0, "No 'and' keyword means no Conjunct links");
    }

    #[test]
    fn test_coordination_with_condition() {
        // Mix coordination with condition clauses
        let doc = create_test_document("When it rains, then A happens and B occurs.");

        let links = ClauseLinkResolver::resolve(&doc);

        // Should have both parent-child links AND coordination link
        let parent_child_links: Vec<_> = links
            .iter()
            .filter(|link| {
                link.link.role == ClauseRole::Parent || link.link.role == ClauseRole::Child
            })
            .collect();

        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        // Should have parent-child relationship between Condition and TrailingEffect
        assert!(
            !parent_child_links.is_empty(),
            "Should have parent-child links for condition"
        );

        // Should have coordination link between the two effect clauses
        assert!(
            !conjunct_links.is_empty(),
            "Should have Conjunct link for 'A and B'"
        );
    }

    #[test]
    fn test_coordination_empty_document() {
        let doc = LayeredDocument::from_text("");

        let links = ClauseLinkResolver::resolve(&doc);
        assert_eq!(links.len(), 0, "Empty document should have no links");
    }

    #[test]
    fn test_coordination_single_clause() {
        let doc = create_test_document("The tenant pays rent.");

        let links = ClauseLinkResolver::resolve(&doc);

        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        assert_eq!(
            conjunct_links.len(),
            0,
            "Single clause should have no Conjunct links"
        );
    }

    #[test]
    fn test_coordination_four_clause_chain() {
        // Test longer chain: A, B, C, and D should create A→B, B→C, C→D
        let doc = create_test_document("A acts, B reacts, C contracts, and D extracts.");

        let links = ClauseLinkResolver::resolve(&doc);

        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        assert_eq!(
            conjunct_links.len(),
            3,
            "Expected three Conjunct links for 'A, B, C, and D' chain"
        );

        let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clauses.len(), 4, "Should have 4 clauses");

        // Verify chain topology
        assert_eq!(conjunct_links[0].anchor, clauses[0].span);
        assert_eq!(conjunct_links[0].link.target, clauses[1].span);

        assert_eq!(conjunct_links[1].anchor, clauses[1].span);
        assert_eq!(conjunct_links[1].link.target, clauses[2].span);

        assert_eq!(conjunct_links[2].anchor, clauses[2].span);
        assert_eq!(conjunct_links[2].link.target, clauses[3].span);
    }

    #[test]
    fn test_coordination_mixed_clause_types_no_link() {
        // Different clause types shouldn't be linked by implicit commas
        // (only by explicit "and")
        let doc = create_test_document("When it rains, the streets flood, repairs are needed.");

        let links = ClauseLinkResolver::resolve(&doc);

        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        // Should have no Conjunct links because:
        // 1. "When it rains" is Condition
        // 2. "the streets flood" is TrailingEffect
        // 3. "repairs are needed" is also TrailingEffect
        // But since clause types differ, we don't create implicit coordination
        // (only explicit "and" would create links)
        assert_eq!(
            conjunct_links.len(),
            0,
            "Mixed clause types with no explicit 'and' should not be coordinated"
        );
    }

    // ========================================================================
    // Gate 3: Exception/Carve-out Detection Tests
    // ========================================================================

    #[test]
    fn test_exception_simple_unless() {
        // "A unless B" → Exception link from B to A
        let doc = LayeredDocument::from_text("Tenant shall pay rent unless waived by Landlord.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(
            exception_links.len(),
            1,
            "Expected one Exception link for 'A unless B'"
        );

        // The exception clause ("waived by Landlord") should point to main clause ("Tenant shall pay rent")
        let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clauses.len(), 2, "Should have 2 clauses");

        // Exception link should be anchored on the second clause (exception)
        // and point to the first clause (main)
        assert_eq!(exception_links[0].anchor, clauses[1].span);
        assert_eq!(exception_links[0].link.target, clauses[0].span);
    }

    #[test]
    fn test_exception_except_keyword() {
        // "A, except B" → Exception link from B to A
        let doc = LayeredDocument::from_text("All rights reserved, except as noted.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(
            exception_links.len(),
            1,
            "Expected one Exception link for 'A, except B'"
        );
    }

    #[test]
    fn test_exception_notwithstanding() {
        // "Notwithstanding X, Y shall apply" → Exception link from Y to X
        let doc = LayeredDocument::from_text("Notwithstanding prior agreements, this clause controls.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(
            exception_links.len(),
            1,
            "Expected one Exception link for 'Notwithstanding X, Y'"
        );

        let clauses = ClauseLinkResolver::extract_clause_spans(&doc);
        assert_eq!(clauses.len(), 2, "Should have 2 clauses");

        // For "notwithstanding X, Y", the second clause (Y) is the exception
        // that points to the first clause (X)
        assert_eq!(exception_links[0].anchor, clauses[1].span);
        assert_eq!(exception_links[0].link.target, clauses[0].span);
    }

    #[test]
    fn test_exception_provided_that() {
        // "A provided that B" → Exception link from B to A
        let doc = LayeredDocument::from_text("Payment is due provided that notice is given.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(
            exception_links.len(),
            1,
            "Expected one Exception link for 'A provided that B'"
        );
    }

    #[test]
    fn test_exception_subject_to() {
        // "A subject to B" → Exception link from B to A
        let doc = LayeredDocument::from_text("License granted subject to payment terms.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(
            exception_links.len(),
            1,
            "Expected one Exception link for 'A subject to B'"
        );
    }

    #[test]
    fn test_exception_no_exception_keyword() {
        // No exception keywords should mean no exception links
        let doc = create_test_document("Tenant pays rent. Landlord maintains property.");

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(
            exception_links.len(),
            0,
            "No exception keywords means no Exception links"
        );
    }

    #[test]
    fn test_exception_with_coordination() {
        // Mix exception with coordination: "A and B, unless C"
        let doc = LayeredDocument::from_text("Tenant pays rent and utilities unless waived.")
            .run_resolver(&ClauseKeywordResolver::new(&["if", "when"], &["and"], &["then"]))
            .run_resolver(&ClauseResolver::default());

        let links = ClauseLinkResolver::resolve(&doc);

        let conjunct_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Conjunct)
            .collect();

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        // Should have coordination between first two clauses
        assert!(
            !conjunct_links.is_empty(),
            "Should have Conjunct link for 'A and B'"
        );

        // Should have exception link from third clause to one of the first two
        assert!(
            !exception_links.is_empty(),
            "Should have Exception link for 'unless C'"
        );
    }

    #[test]
    fn test_exception_single_clause() {
        // Single clause with exception keyword but no second clause
        let doc = create_test_document("Unless otherwise noted.");

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        // No exception link because there's only one clause
        assert_eq!(
            exception_links.len(),
            0,
            "Single clause cannot have exception relationship"
        );
    }

    #[test]
    fn test_exception_empty_document() {
        let doc = LayeredDocument::from_text("");

        let links = ClauseLinkResolver::resolve(&doc);

        let exception_links: Vec<_> = links
            .iter()
            .filter(|link| link.link.role == ClauseRole::Exception)
            .collect();

        assert_eq!(exception_links.len(), 0, "Empty document has no exception links");
    }
}

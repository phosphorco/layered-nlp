//! Clause relationship link builders using M0 foundation types.
//!
//! Provides helpers for creating `SpanLink<ClauseRole>` edges between clause spans.

use layered_nlp_document::{ClauseRole, DocSpan, DocSpanLink, SpanLink};

/// Helper for creating clause hierarchy links.
///
/// Links are unidirectional - call both `parent_link` and `child_link`
/// to enable traversal in both directions.
pub struct ClauseLinkBuilder;

impl ClauseLinkBuilder {
    /// Create a parent link (stored on child clause, points to parent).
    ///
    /// # Arguments
    /// * `parent_span` - The parent clause span this child belongs to
    ///
    /// # Example
    /// ```
    /// # use layered_clauses::ClauseLinkBuilder;
    /// # use layered_nlp_document::DocSpan;
    /// let parent = DocSpan::single_line(0, 0, 5);
    /// let link = ClauseLinkBuilder::parent_link(parent);
    /// ```
    pub fn parent_link(parent_span: DocSpan) -> DocSpanLink<ClauseRole> {
        SpanLink::new(ClauseRole::Parent, parent_span)
    }

    /// Create a child link (stored on parent clause, points to child).
    ///
    /// # Arguments
    /// * `child_span` - The child clause span contained by this parent
    pub fn child_link(child_span: DocSpan) -> DocSpanLink<ClauseRole> {
        SpanLink::new(ClauseRole::Child, child_span)
    }

    /// Create a conjunct link for coordinated clauses.
    ///
    /// Chain topology: A→B, B→C for "A, B, and C"
    ///
    /// # Arguments
    /// * `next_conjunct` - The next conjunct in the coordination chain
    pub fn conjunct_link(next_conjunct: DocSpan) -> DocSpanLink<ClauseRole> {
        SpanLink::new(ClauseRole::Conjunct, next_conjunct)
    }

    /// Create an exception link (stored on exception clause, points to main).
    ///
    /// # Arguments
    /// * `main_clause` - The main clause this exception modifies
    pub fn exception_link(main_clause: DocSpan) -> DocSpanLink<ClauseRole> {
        SpanLink::new(ClauseRole::Exception, main_clause)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parent_link_construction() {
        let parent_span = DocSpan::single_line(0, 0, 5);
        let link = ClauseLinkBuilder::parent_link(parent_span);

        assert_eq!(link.role, ClauseRole::Parent);
        assert_eq!(link.target, parent_span);
    }

    #[test]
    fn test_child_link_construction() {
        let child_span = DocSpan::single_line(0, 10, 15);
        let link = ClauseLinkBuilder::child_link(child_span);

        assert_eq!(link.role, ClauseRole::Child);
        assert_eq!(link.target, child_span);
    }

    #[test]
    fn test_conjunct_link_construction() {
        let next_span = DocSpan::single_line(0, 20, 25);
        let link = ClauseLinkBuilder::conjunct_link(next_span);

        assert_eq!(link.role, ClauseRole::Conjunct);
        assert_eq!(link.target, next_span);
    }

    #[test]
    fn test_exception_link_construction() {
        let main_span = DocSpan::single_line(0, 0, 30);
        let link = ClauseLinkBuilder::exception_link(main_span);

        assert_eq!(link.role, ClauseRole::Exception);
        assert_eq!(link.target, main_span);
    }
}

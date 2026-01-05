//! Clause relationship link builders using M0 foundation types.
//!
//! Provides helpers for creating `SpanLink<ClauseRole>` edges between clause spans.

use crate::clause_link_resolver::LinkConfidence;
use crate::clause_link_resolver::CoordinationType;
use layered_nlp_document::{ClauseRole, DocSpan, DocSpanLink, SpanLink};

/// Helper for creating clause hierarchy links.
///
/// Links are unidirectional - call both `parent_link` and `child_link`
/// to enable traversal in both directions.
pub struct ClauseLinkBuilder {
    confidence: LinkConfidence,
}

impl ClauseLinkBuilder {
    /// Create a new builder with default high confidence.
    pub fn new() -> Self {
        Self {
            confidence: LinkConfidence::High,
        }
    }

    /// Set the confidence level for the link.
    pub fn with_confidence(mut self, confidence: LinkConfidence) -> Self {
        self.confidence = confidence;
        self
    }

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

    /// Create a conjunct link with a specific coordination type.
    ///
    /// # Arguments
    /// * `next_conjunct` - The next conjunct in the coordination chain
    /// * `coordination_type` - The type of coordination ("and", "or", "but", "nor")
    pub fn conjunct_link_typed(
        next_conjunct: DocSpan,
        _coordination_type: CoordinationType,
    ) -> DocSpanLink<ClauseRole> {
        // Note: coordination_type is not stored in the link itself,
        // but in the ClauseLink wrapper that holds this DocSpanLink
        SpanLink::new(ClauseRole::Conjunct, next_conjunct)
    }


    /// Create an exception link (stored on exception clause, points to main).
    ///
    /// # Arguments
    /// * `main_clause` - The main clause this exception modifies
    pub fn exception_link(main_clause: DocSpan) -> DocSpanLink<ClauseRole> {
        SpanLink::new(ClauseRole::Exception, main_clause)
    }

    /// Create a list item link (stored on list item clause, points to container).
    ///
    /// # Arguments
    /// * `container_clause` - The container clause that introduces this list
    pub fn list_item_link(container_clause: DocSpan) -> DocSpanLink<ClauseRole> {
        SpanLink::new(ClauseRole::ListItem, container_clause)
    }

    /// Create a list container link (stored on container clause, points to item).
    ///
    /// # Arguments
    /// * `item_clause` - A list item clause in this container's list
    pub fn list_container_link(item_clause: DocSpan) -> DocSpanLink<ClauseRole> {
        SpanLink::new(ClauseRole::ListContainer, item_clause)
    }

    /// Create a cross-reference link (stored on clause, points to section reference span).
    ///
    /// Used when a clause references another section (e.g., "subject to Section 3.2").
    /// The anchor is the clause span, the target is the section reference span within it.
    ///
    /// # Arguments
    /// * `section_ref_span` - The span containing the section reference (e.g., "Section 3.2")
    pub fn cross_reference_link(section_ref_span: DocSpan) -> DocSpanLink<ClauseRole> {
        SpanLink::new(ClauseRole::CrossReference, section_ref_span)
    }

    /// Create a relative clause link to its head noun
    pub fn relative_link(head_noun_span: DocSpan) -> DocSpanLink<ClauseRole> {
        SpanLink::new(ClauseRole::Relative, head_noun_span)
    }
}

impl Default for ClauseLinkBuilder {
    fn default() -> Self {
        Self::new()
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

    #[test]
    fn test_list_item_link_construction() {
        let container_span = DocSpan::single_line(0, 0, 10);
        let link = ClauseLinkBuilder::list_item_link(container_span);

        assert_eq!(link.role, ClauseRole::ListItem);
        assert_eq!(link.target, container_span);
    }

    #[test]
    fn test_list_container_link_construction() {
        let item_span = DocSpan::single_line(0, 15, 25);
        let link = ClauseLinkBuilder::list_container_link(item_span);

        assert_eq!(link.role, ClauseRole::ListContainer);
        assert_eq!(link.target, item_span);
    }

    #[test]
    fn test_cross_reference_link_construction() {
        let section_ref_span = DocSpan::single_line(0, 10, 20);
        let link = ClauseLinkBuilder::cross_reference_link(section_ref_span);

        assert_eq!(link.role, ClauseRole::CrossReference);
        assert_eq!(link.target, section_ref_span);
    }
}

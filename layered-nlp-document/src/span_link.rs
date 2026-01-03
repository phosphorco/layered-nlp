//! Generic binary relation types for span-to-span links.
//!
//! SpanLink represents a typed edge from an anchor span to a target span,
//! enabling relationship graphs like clause hierarchy, attachment, and semantic roles.

use crate::DocSpan;

/// Generic binary relation from anchor span to target span.
///
/// The anchor is the span where this attribute is stored.
/// The target is what the relation points to.
///
/// # Type Parameters
/// - `R`: Role enum specific to the relation family
/// - `S`: Span type (defaults to DocSpan for document-level)
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpanLink<R, S = DocSpan> {
    /// Semantic role of target with respect to anchor
    pub role: R,
    /// The target span
    pub target: S,
}

impl<R, S> SpanLink<R, S> {
    pub fn new(role: R, target: S) -> Self {
        Self { role, target }
    }
}

/// Document-level span link (default)
pub type DocSpanLink<R> = SpanLink<R, DocSpan>;

// ============================================================================
// Role Enums - Templates for downstream milestones
// ============================================================================

/// M2: Clause hierarchy relations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ClauseRole {
    /// Parent clause contains this one
    Parent,
    /// Child clause contained by this one
    Child,
    /// Coordinated clause at same level
    Conjunct,
    /// Exception clause modifying parent
    Exception,
    /// List item belongs to a parent list container
    ListItem,
    /// Container clause that introduces a list
    ListContainer,
    /// Cross-reference to another section (e.g., "subject to Section 3.2")
    CrossReference,
    /// Self-referential link for standalone clauses with obligations
    /// Used when a clause has an obligation_type but no relationships to other clauses
    Self_,
}

/// M6: PP/Relative clause attachment relations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AttachmentRole {
    /// The head this modifier attaches to
    Head,
}

/// M8: Semantic role relations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SemanticRole {
    /// Actor performing action
    Agent,
    /// Entity affected by action
    Theme,
    /// Entity benefiting from action
    Beneficiary,
    /// Conditional clause
    Condition,
    /// Exception to the rule
    Exception,
    /// Location of action
    Location,
    /// Time of action
    Time,
}

/// M4: Conflict relations (for multi-span conflicts)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ConflictRole {
    /// Left side of conflict
    Left,
    /// Right side of conflict
    Right,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DocPosition;

    fn sample_span() -> DocSpan {
        DocSpan::new(
            DocPosition { line: 0, token: 0 },
            DocPosition { line: 0, token: 5 },
        )
    }

    fn another_span() -> DocSpan {
        DocSpan::new(
            DocPosition { line: 1, token: 0 },
            DocPosition { line: 1, token: 3 },
        )
    }

    #[test]
    fn test_span_link_construction() {
        let link: SpanLink<ClauseRole> = SpanLink::new(ClauseRole::Parent, sample_span());
        assert_eq!(link.role, ClauseRole::Parent);
        assert_eq!(link.target, sample_span());
    }

    #[test]
    fn test_span_link_with_different_roles() {
        let clause_link: SpanLink<ClauseRole> = SpanLink::new(ClauseRole::Child, sample_span());
        let semantic_link: SpanLink<SemanticRole> = SpanLink::new(SemanticRole::Agent, sample_span());
        let attachment_link: SpanLink<AttachmentRole> = SpanLink::new(AttachmentRole::Head, sample_span());
        let conflict_link: SpanLink<ConflictRole> = SpanLink::new(ConflictRole::Left, sample_span());

        assert_eq!(clause_link.role, ClauseRole::Child);
        assert_eq!(semantic_link.role, SemanticRole::Agent);
        assert_eq!(attachment_link.role, AttachmentRole::Head);
        assert_eq!(conflict_link.role, ConflictRole::Left);
    }

    #[test]
    fn test_span_link_equality() {
        let link1: SpanLink<ClauseRole> = SpanLink::new(ClauseRole::Parent, sample_span());
        let link2: SpanLink<ClauseRole> = SpanLink::new(ClauseRole::Parent, sample_span());
        let link3: SpanLink<ClauseRole> = SpanLink::new(ClauseRole::Child, sample_span());
        let link4: SpanLink<ClauseRole> = SpanLink::new(ClauseRole::Parent, another_span());

        assert_eq!(link1, link2);
        assert_ne!(link1, link3); // different role
        assert_ne!(link1, link4); // different target
    }

    #[test]
    fn test_span_link_clone() {
        let link: SpanLink<SemanticRole> = SpanLink::new(SemanticRole::Theme, sample_span());
        let cloned = link.clone();
        assert_eq!(link, cloned);
    }

    #[test]
    fn test_type_alias() {
        // DocSpanLink<R> should be equivalent to SpanLink<R, DocSpan>
        let link1: DocSpanLink<ClauseRole> = SpanLink::new(ClauseRole::Conjunct, sample_span());
        let link2: SpanLink<ClauseRole, DocSpan> = SpanLink::new(ClauseRole::Conjunct, sample_span());
        assert_eq!(link1, link2);
    }
}

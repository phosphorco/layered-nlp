//! Association types for linking spans to their provenance.
//!
//! This module provides the core types for representing typed relationships
//! between spans in a document, enabling provenance tracking and relationship
//! graph traversal.

use std::any::TypeId;
use std::fmt::Debug;
use std::sync::Arc;

/// A reference to a token range within an LLLine.
///
/// Both indices are inclusive and refer to token positions (not character positions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpanRef {
    /// Inclusive start token index
    pub start_idx: usize,
    /// Inclusive end token index
    pub end_idx: usize,
}

impl SpanRef {
    /// Create a new span reference.
    pub fn new(start_idx: usize, end_idx: usize) -> Self {
        Self { start_idx, end_idx }
    }
}

/// A typed semantic label for an association between spans.
///
/// Implement this trait to define custom association types that describe
/// the relationship between a source span and its referenced target span.
///
/// # Example
///
/// ```
/// use layered_nlp::Association;
///
/// #[derive(Debug, Clone)]
/// pub struct ObligorSource;
///
/// impl Association for ObligorSource {
///     fn label(&self) -> &'static str { "obligor_source" }
///     fn glyph(&self) -> Option<&'static str> { Some("@") }
/// }
/// ```
pub trait Association: Debug + Send + Sync + 'static {
    /// Returns the semantic label for this association type.
    ///
    /// This label is used in display output and for programmatic identification.
    fn label(&self) -> &'static str;

    /// Returns an optional single-character glyph for display.
    ///
    /// When present, this glyph is rendered before the label in arrow displays.
    fn glyph(&self) -> Option<&'static str> {
        None
    }
}

/// Object-safe wrapper trait for type-erased association storage.
///
/// This trait is automatically implemented for all types implementing [`Association`].
pub trait AssociationAny: Debug + Send + Sync {
    /// Returns the semantic label.
    fn label(&self) -> &'static str;

    /// Returns the optional display glyph.
    fn glyph(&self) -> Option<&'static str>;

    /// Returns the concrete TypeId of the association.
    fn association_type_id(&self) -> TypeId;
}

impl<A: Association> AssociationAny for A {
    fn label(&self) -> &'static str {
        Association::label(self)
    }

    fn glyph(&self) -> Option<&'static str> {
        Association::glyph(self)
    }

    fn association_type_id(&self) -> TypeId {
        TypeId::of::<A>()
    }
}

/// An association linking to a target span with a typed semantic label.
///
/// This struct combines a [`SpanRef`] (the target of the association) with
/// a type-erased [`Association`] that describes the relationship.
#[derive(Clone)]
pub struct AssociatedSpan {
    /// The target span being referenced
    pub span: SpanRef,
    /// Type-erased association describing the relationship
    association: Arc<dyn AssociationAny>,
}

impl Debug for AssociatedSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssociatedSpan")
            .field("span", &self.span)
            .field("label", &self.label())
            .field("glyph", &self.glyph())
            .finish()
    }
}

impl AssociatedSpan {
    /// Create a new associated span with the given association type and target span.
    pub fn new<A: Association>(association: A, span: SpanRef) -> Self {
        Self {
            span,
            association: Arc::new(association),
        }
    }

    /// Returns the semantic label of this association.
    pub fn label(&self) -> &'static str {
        self.association.label()
    }

    /// Returns the optional display glyph of this association.
    pub fn glyph(&self) -> Option<&'static str> {
        self.association.glyph()
    }

    /// Returns the TypeId of the concrete association type.
    pub fn association_type_id(&self) -> TypeId {
        self.association.association_type_id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[derive(Debug, Clone)]
    struct TestAssociation;

    impl Association for TestAssociation {
        fn label(&self) -> &'static str {
            "test_label"
        }

        fn glyph(&self) -> Option<&'static str> {
            Some("@")
        }
    }

    #[derive(Debug, Clone)]
    struct AnotherAssociation;

    impl Association for AnotherAssociation {
        fn label(&self) -> &'static str {
            "another"
        }
        // No glyph - uses default None
    }

    #[test]
    fn span_ref_equality_and_hashing() {
        let span1 = SpanRef::new(0, 5);
        let span2 = SpanRef::new(0, 5);
        let span3 = SpanRef::new(1, 5);

        assert_eq!(span1, span2);
        assert_ne!(span1, span3);

        // Test hashing works
        let mut set = HashSet::new();
        set.insert(span1);
        assert!(set.contains(&span2));
        assert!(!set.contains(&span3));
    }

    #[test]
    fn association_returns_correct_label_and_glyph() {
        let assoc = TestAssociation;
        assert_eq!(Association::label(&assoc), "test_label");
        assert_eq!(Association::glyph(&assoc), Some("@"));

        let another = AnotherAssociation;
        assert_eq!(Association::label(&another), "another");
        assert_eq!(Association::glyph(&another), None);
    }

    #[test]
    fn associated_span_creation_and_accessors() {
        let span = SpanRef::new(2, 4);
        let assoc_span = AssociatedSpan::new(TestAssociation, span);

        assert_eq!(assoc_span.span, span);
        assert_eq!(assoc_span.label(), "test_label");
        assert_eq!(assoc_span.glyph(), Some("@"));
    }

    #[test]
    fn associated_span_is_clone() {
        let span = SpanRef::new(1, 3);
        let assoc_span = AssociatedSpan::new(TestAssociation, span);
        let cloned = assoc_span.clone();

        assert_eq!(cloned.span, assoc_span.span);
        assert_eq!(cloned.label(), assoc_span.label());
    }

    #[test]
    fn association_type_id_distinguishes_types() {
        let span = SpanRef::new(0, 1);
        let assoc1 = AssociatedSpan::new(TestAssociation, span);
        let assoc2 = AssociatedSpan::new(AnotherAssociation, span);

        assert_ne!(
            assoc1.association_type_id(),
            assoc2.association_type_id(),
            "Different association types should have different TypeIds"
        );

        // Same type should have same TypeId
        let assoc3 = AssociatedSpan::new(TestAssociation, SpanRef::new(5, 10));
        assert_eq!(assoc1.association_type_id(), assoc3.association_type_id());
    }

    #[test]
    fn associated_span_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AssociatedSpan>();
    }

    #[test]
    fn associated_span_debug_output() {
        let span = SpanRef::new(0, 2);
        let assoc_span = AssociatedSpan::new(TestAssociation, span);
        let debug_str = format!("{:?}", assoc_span);

        assert!(debug_str.contains("AssociatedSpan"));
        assert!(debug_str.contains("test_label"));
    }
}

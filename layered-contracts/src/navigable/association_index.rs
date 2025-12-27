//! Lazy bidirectional association index for reverse lookups (FR-009 Gate 4).
//!
//! The `AssociationIndex` enables efficient reverse traversal of associations,
//! answering the question "which spans link TO this span?" in O(k) time where
//! k is the number of incoming edges.
//!
//! ## Design
//!
//! Associations are stored on the **source** span, pointing to **target** spans:
//!
//! ```text
//! [Obligation span] --ObligorSource--> [DefinedTerm span]
//!      ↑ source (associations stored here)     ↑ target
//! ```
//!
//! Forward navigation (`linked::<A>()`) is O(k) where k = outgoing associations for that span.
//! Reverse navigation (`linked_from::<A>()`) requires this index to find all sources.
//!
//! ## Lazy Initialization
//!
//! The index is built on first reverse query and cached in `ContractDocument`.
//! Building is O(n) where n = total associations across all spans.

use std::collections::HashMap;

use crate::document::ContractDocument;
use crate::semantic_span::SpanId;

/// An entry in the association index representing one edge.
#[derive(Debug, Clone)]
pub struct AssociationEdge {
    /// The label of the association type (e.g., "obligor_source")
    pub label: &'static str,
    /// The other end of the edge (source for incoming, target for outgoing)
    pub other: SpanId,
}

/// Bidirectional index for efficient association traversal.
///
/// Built lazily on first reverse query. Once built, both forward and reverse
/// lookups are O(k) where k is the number of edges for that span.
#[derive(Debug, Default)]
pub struct AssociationIndex {
    /// Outgoing edges: source SpanId -> [(label, target SpanId)]
    /// Note: This duplicates data already in SemanticSpan::associations,
    /// but provides SpanId-based lookup instead of DocSpan-based.
    outgoing: HashMap<SpanId, Vec<AssociationEdge>>,
    
    /// Incoming edges: target SpanId -> [(label, source SpanId)]
    /// This is the key data structure enabling reverse queries.
    incoming: HashMap<SpanId, Vec<AssociationEdge>>,
}

impl AssociationIndex {
    /// Build a new association index from a ContractDocument.
    ///
    /// Scans all spans in the document and builds both outgoing and incoming maps.
    /// Time complexity: O(n) where n = total associations across all spans.
    pub fn build(doc: &ContractDocument) -> Self {
        let mut outgoing: HashMap<SpanId, Vec<AssociationEdge>> = HashMap::new();
        let mut incoming: HashMap<SpanId, Vec<AssociationEdge>> = HashMap::new();
        
        let span_index = doc.doc_spans();
        
        // Iterate all spans with their IDs
        for (source_id, semantic_span) in span_index.iter_with_ids() {
            for assoc in semantic_span.associations() {
                let label = assoc.label();
                
                // Try to resolve the target DocSpan to a SpanId
                if let Some(target_id) = span_index.id_for_span(&assoc.span) {
                    // Add outgoing edge: source -> target
                    outgoing
                        .entry(source_id)
                        .or_default()
                        .push(AssociationEdge {
                            label,
                            other: target_id,
                        });
                    
                    // Add incoming edge: target <- source
                    incoming
                        .entry(target_id)
                        .or_default()
                        .push(AssociationEdge {
                            label,
                            other: source_id,
                        });
                }
                // If target doesn't resolve to a SpanId, skip it
                // (same behavior as forward navigation)
            }
        }
        
        Self { outgoing, incoming }
    }
    
    /// Get outgoing edges for a span (sources linking FROM this span).
    ///
    /// Returns an empty slice if the span has no outgoing associations.
    pub fn outgoing(&self, id: SpanId) -> &[AssociationEdge] {
        self.outgoing.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }
    
    /// Get incoming edges for a span (sources linking TO this span).
    ///
    /// Returns an empty slice if no spans link to this span.
    pub fn incoming(&self, id: SpanId) -> &[AssociationEdge] {
        self.incoming.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }
    
    /// Get outgoing edges filtered by label.
    pub fn outgoing_by_label<'a>(&'a self, id: SpanId, label: &'a str) -> impl Iterator<Item = SpanId> + 'a {
        self.outgoing(id)
            .iter()
            .filter(move |edge| edge.label == label)
            .map(|edge| edge.other)
    }
    
    /// Get incoming edges filtered by label.
    pub fn incoming_by_label<'a>(&'a self, id: SpanId, label: &'a str) -> impl Iterator<Item = SpanId> + 'a {
        self.incoming(id)
            .iter()
            .filter(move |edge| edge.label == label)
            .map(|edge| edge.other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::DocSpan;
    use crate::semantic_span::{DocAssociatedSpan, SemanticSpan};
    use layered_nlp::Association;

    #[derive(Debug, Clone, Default)]
    struct TestAssoc;
    impl Association for TestAssoc {
        fn label(&self) -> &'static str {
            "test_assoc"
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct TestAttr {
        name: String,
    }

    #[test]
    fn test_build_empty_document() {
        let doc = ContractDocument::from_text("Line one");
        let index = AssociationIndex::build(&doc);
        
        assert!(index.outgoing.is_empty());
        assert!(index.incoming.is_empty());
    }

    #[test]
    fn test_build_with_associations() {
        let mut doc = ContractDocument::from_text("Line one\nLine two\nLine three");
        
        // Span 0: target
        let target_span = DocSpan::single_line(0, 0, 2);
        doc.add_span(target_span, TestAttr { name: "Target".into() });
        
        // Span 1: source with association to target
        let source_span = DocSpan::single_line(1, 0, 2);
        let source = SemanticSpan::with_associations(
            source_span,
            TestAttr { name: "Source".into() },
            vec![DocAssociatedSpan::new(TestAssoc, target_span)],
        );
        doc.add_semantic_span(source);
        
        let index = AssociationIndex::build(&doc);
        
        // Check outgoing from source
        let outgoing = index.outgoing(SpanId(1));
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].label, "test_assoc");
        assert_eq!(outgoing[0].other, SpanId(0));
        
        // Check incoming to target
        let incoming = index.incoming(SpanId(0));
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].label, "test_assoc");
        assert_eq!(incoming[0].other, SpanId(1));
    }

    #[test]
    fn test_multiple_incoming_edges() {
        let mut doc = ContractDocument::from_text("Line0\nLine1\nLine2\nLine3");
        
        // Target span
        let target = DocSpan::single_line(0, 0, 2);
        doc.add_span(target, TestAttr { name: "Target".into() });
        
        // Two sources pointing to same target
        for line in 1..=2 {
            let source_span = DocSpan::single_line(line, 0, 2);
            let source = SemanticSpan::with_associations(
                source_span,
                TestAttr { name: format!("Source{}", line) },
                vec![DocAssociatedSpan::new(TestAssoc, target)],
            );
            doc.add_semantic_span(source);
        }
        
        let index = AssociationIndex::build(&doc);
        
        let incoming = index.incoming(SpanId(0));
        assert_eq!(incoming.len(), 2);
        
        let sources: Vec<_> = incoming.iter().map(|e| e.other).collect();
        assert!(sources.contains(&SpanId(1)));
        assert!(sources.contains(&SpanId(2)));
    }

    #[test]
    fn test_filter_by_label() {
        let mut doc = ContractDocument::from_text("Line0\nLine1\nLine2");
        
        #[derive(Debug, Clone, Default)]
        struct OtherAssoc;
        impl Association for OtherAssoc {
            fn label(&self) -> &'static str {
                "other_assoc"
            }
        }
        
        let target = DocSpan::single_line(0, 0, 2);
        doc.add_span(target, TestAttr { name: "Target".into() });
        
        // Source with two different association types to same target
        let source_span = DocSpan::single_line(1, 0, 2);
        let source = SemanticSpan::with_associations(
            source_span,
            TestAttr { name: "Source".into() },
            vec![
                DocAssociatedSpan::new(TestAssoc, target),
                DocAssociatedSpan::new(OtherAssoc, target),
            ],
        );
        doc.add_semantic_span(source);
        
        let index = AssociationIndex::build(&doc);
        
        // Filter by label
        let test_assoc_incoming: Vec<_> = index.incoming_by_label(SpanId(0), "test_assoc").collect();
        assert_eq!(test_assoc_incoming.len(), 1);
        
        let other_assoc_incoming: Vec<_> = index.incoming_by_label(SpanId(0), "other_assoc").collect();
        assert_eq!(other_assoc_incoming.len(), 1);
        
        let missing_incoming: Vec<_> = index.incoming_by_label(SpanId(0), "missing").collect();
        assert!(missing_incoming.is_empty());
    }
}

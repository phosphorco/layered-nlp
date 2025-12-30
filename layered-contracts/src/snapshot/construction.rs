//! Snapshot construction from ContractDocument.
//!
//! This module provides `SnapshotBuilder` which traverses a `ContractDocument`
//! and builds a canonical `Snapshot` with stable IDs.

use std::collections::BTreeMap;

use crate::{ContractDocument, Scored, ScoreSource};

use super::types::{
    InputSource, Snapshot, SnapshotDocPos, SnapshotDocSpan, SnapshotKind,
    SnapshotSpanId, SpanData,
};

/// Builder for constructing snapshots from documents.
///
/// The builder extracts spans from a `ContractDocument`, generates stable IDs,
/// and produces a canonical `Snapshot`.
///
/// # ID Generation
///
/// IDs are generated deterministically:
/// 1. Spans are grouped by type
/// 2. Within each type, spans are sorted by position
/// 3. IDs are assigned as `{prefix}-{index}` where index is 0-based
///
/// This ensures the same document produces the same IDs across runs.
pub struct SnapshotBuilder<'a> {
    doc: &'a ContractDocument,
    type_extractors: Vec<Box<dyn SpanExtractor + 'a>>,
}

impl<'a> SnapshotBuilder<'a> {
    /// Create a new builder from a document.
    pub fn new(doc: &'a ContractDocument) -> Self {
        Self {
            doc,
            type_extractors: Vec::new(),
        }
    }

    /// Add an extractor for a specific type that implements SnapshotKind.
    ///
    /// This registers a type to be extracted from line-level attributes.
    /// Values are stored using their Debug representation.
    pub fn with_line_type<T: 'static + std::fmt::Debug + SnapshotKind>(mut self) -> Self {
        self.type_extractors.push(Box::new(LineTypeExtractor::<T> {
            _marker: std::marker::PhantomData,
        }));
        self
    }

    /// Add an extractor for Scored<T> line-level attributes.
    /// Values are stored using their Debug representation.
    pub fn with_scored_line_type<T: 'static + std::fmt::Debug + SnapshotKind>(mut self) -> Self {
        self.type_extractors.push(Box::new(ScoredLineTypeExtractor::<T> {
            _marker: std::marker::PhantomData,
        }));
        self
    }

    /// Add an extractor for document-level spans of a specific type.
    /// Values are stored using their Debug representation.
    pub fn with_doc_type<T: 'static + std::fmt::Debug + SnapshotKind>(mut self) -> Self {
        self.type_extractors.push(Box::new(DocTypeExtractor::<T> {
            _marker: std::marker::PhantomData,
        }));
        self
    }

    /// Add the standard set of extractors for common contract types.
    pub fn with_standard_types(self) -> Self {
        self.with_scored_line_type::<crate::defined_term::DefinedTerm>()
            .with_scored_line_type::<crate::term_reference::TermReference>()
            .with_scored_line_type::<crate::obligation::ObligationPhrase>()
            .with_line_type::<crate::section_header::SectionHeader>()
            .with_line_type::<crate::temporal::TemporalExpression>()
            .with_line_type::<crate::contract_keyword::ContractKeyword>()
    }

    /// Build the snapshot.
    pub fn build(self) -> Snapshot {
        let mut all_spans: Vec<RawSpanData> = Vec::new();

        // Extract spans from all registered extractors
        for extractor in &self.type_extractors {
            let extracted = extractor.extract(self.doc);
            all_spans.extend(extracted);
        }

        // Group by type name
        let mut by_type: BTreeMap<String, Vec<RawSpanData>> = BTreeMap::new();
        for span in all_spans {
            by_type.entry(span.type_name.clone()).or_default().push(span);
        }

        // Sort each group by position and assign IDs
        let mut spans: BTreeMap<String, Vec<SpanData>> = BTreeMap::new();

        for (type_name, mut type_spans) in by_type {
            // Sort by position (line, token)
            type_spans.sort_by(|a, b| {
                let a_key = (a.start_line, a.start_token, a.end_line, a.end_token);
                let b_key = (b.start_line, b.start_token, b.end_line, b.end_token);
                a_key.cmp(&b_key)
            });

            // Assign IDs and build SpanData
            let mut span_data_list = Vec::new();
            for (index, raw) in type_spans.into_iter().enumerate() {
                let id = SnapshotSpanId::new(&raw.prefix, index);

                span_data_list.push(SpanData {
                    id,
                    position: SnapshotDocSpan::new(
                        SnapshotDocPos::new(raw.start_line as u32, raw.start_token as u32),
                        SnapshotDocPos::new(raw.end_line as u32, raw.end_token as u32),
                    ),
                    type_name: raw.type_name,
                    value: raw.value,
                    confidence: raw.confidence,
                    source: raw.source,
                    associations: vec![], // Will be filled in second pass
                });
            }

            spans.insert(type_name, span_data_list);
        }

        // Build input source from document's original text
        // This preserves exact line content including whitespace
        let input = InputSource::Inline(
            self.doc
                .original_text()
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(|s| s.to_string())
                .collect(),
        );

        Snapshot {
            version: 1,
            input,
            spans,
            auxiliary: BTreeMap::new(),
        }
    }
}

/// Internal raw span data before ID assignment.
struct RawSpanData {
    prefix: String,
    type_name: String,
    start_line: usize,
    start_token: usize,
    end_line: usize,
    end_token: usize,
    value: ron::Value,
    confidence: Option<f64>,
    source: Option<String>,
}

/// Trait for extracting spans of different types.
trait SpanExtractor {
    fn extract(&self, doc: &ContractDocument) -> Vec<RawSpanData>;
}

/// Extractor for plain line-level attributes.
struct LineTypeExtractor<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: 'static + std::fmt::Debug + SnapshotKind> SpanExtractor for LineTypeExtractor<T> {
    fn extract(&self, doc: &ContractDocument) -> Vec<RawSpanData> {
        let mut results = Vec::new();

        for (line_idx, line) in doc.lines().iter().enumerate() {
            for (range, _text, attrs) in line.query::<T>() {
                for attr in attrs {
                    let value = ron::Value::String(format!("{:?}", attr));

                    results.push(RawSpanData {
                        prefix: T::SNAPSHOT_PREFIX.to_string(),
                        type_name: T::SNAPSHOT_TYPE_NAME.to_string(),
                        start_line: line_idx,
                        start_token: range.0,
                        end_line: line_idx,
                        end_token: range.1,
                        value,
                        confidence: None,
                        source: None,
                    });
                }
            }
        }

        results
    }
}

/// Extractor for Scored<T> line-level attributes.
struct ScoredLineTypeExtractor<T> {
    _marker: std::marker::PhantomData<T>,
}

impl<T: 'static + std::fmt::Debug + SnapshotKind> SpanExtractor for ScoredLineTypeExtractor<T> {
    fn extract(&self, doc: &ContractDocument) -> Vec<RawSpanData> {
        let mut results = Vec::new();

        for (line_idx, line) in doc.lines().iter().enumerate() {
            for (range, _text, attrs) in line.query::<Scored<T>>() {
                for scored in attrs {
                    let value = ron::Value::String(format!("{:?}", scored.value));

                    // TODO(Gate6): Redact non-deterministic fields like pass_id in
                    // Snapshot::apply_redactions(). For now, we use a best-effort 
                    // deterministic representation that omits known volatile fields.
                    let source_desc = match &scored.source {
                        ScoreSource::RuleBased { rule_name } => {
                            Some(format!("RuleBased({})", rule_name))
                        }
                        ScoreSource::LLMPass { model, .. } => {
                            // Omit pass_id as it may be non-deterministic
                            Some(format!("LLMPass(model={})", model))
                        }
                        ScoreSource::HumanVerified { .. } => {
                            // Omit verifier_id for determinism
                            Some("HumanVerified".to_string())
                        }
                        ScoreSource::Derived => Some("Derived".to_string()),
                    };

                    results.push(RawSpanData {
                        prefix: T::SNAPSHOT_PREFIX.to_string(),
                        type_name: T::SNAPSHOT_TYPE_NAME.to_string(),
                        start_line: line_idx,
                        start_token: range.0,
                        end_line: line_idx,
                        end_token: range.1,
                        value,
                        confidence: Some(scored.confidence),
                        source: source_desc,
                    });
                }
            }
        }

        results
    }
}

/// Extractor for document-level SemanticSpan attributes.
///
/// Note: Currently a stub since LayeredDocument doesn't yet support document-level
/// span querying. This will be implemented when SemanticSpan support is added.
#[allow(dead_code)]
struct DocTypeExtractor<T> {
    _marker: std::marker::PhantomData<T>,
}

#[allow(dead_code)]
impl<T: 'static + std::fmt::Debug + SnapshotKind> SpanExtractor for DocTypeExtractor<T> {
    fn extract(&self, _doc: &ContractDocument) -> Vec<RawSpanData> {
        // Document-level span extraction requires SemanticSpan support,
        // which will be added in a future gate.
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::section_header::SectionHeaderResolver;

    #[test]
    fn test_builder_empty_document() {
        let doc = ContractDocument::from_text("");
        let snapshot = SnapshotBuilder::new(&doc).build();
        
        assert_eq!(snapshot.version, 1);
        assert_eq!(snapshot.span_count(), 0);
    }

    #[test]
    fn test_builder_with_section_headers() {
        let doc = ContractDocument::from_text("Section 1.1 Definitions\nSection 1.2 Obligations")
            .run_resolver(&SectionHeaderResolver::new());
        
        let snapshot = SnapshotBuilder::new(&doc)
            .with_line_type::<crate::section_header::SectionHeader>()
            .build();
        
        assert_eq!(snapshot.span_count(), 2);
        
        let sections = snapshot.spans_of_type("SectionHeader");
        assert_eq!(sections.len(), 2);
        
        // Check IDs are sequential
        assert_eq!(sections[0].id.0, "sh-0");
        assert_eq!(sections[1].id.0, "sh-1");
        
        // Check ordering (first section should be first)
        assert!(sections[0].position.start.line == 0);
        assert!(sections[1].position.start.line == 1);
    }

    #[test]
    fn test_id_stability() {
        let text = "Section 1.1 Definitions\nSection 1.2 Obligations";
        
        // Build twice, should get same IDs
        let doc1 = ContractDocument::from_text(text)
            .run_resolver(&SectionHeaderResolver::new());
        let snap1 = SnapshotBuilder::new(&doc1)
            .with_line_type::<crate::section_header::SectionHeader>()
            .build();
        
        let doc2 = ContractDocument::from_text(text)
            .run_resolver(&SectionHeaderResolver::new());
        let snap2 = SnapshotBuilder::new(&doc2)
            .with_line_type::<crate::section_header::SectionHeader>()
            .build();
        
        // IDs should be identical
        let sections1 = snap1.spans_of_type("SectionHeader");
        let sections2 = snap2.spans_of_type("SectionHeader");
        
        for (s1, s2) in sections1.iter().zip(sections2.iter()) {
            assert_eq!(s1.id, s2.id);
        }
    }

    #[test]
    fn test_id_uniqueness() {
        let doc = ContractDocument::from_text(
            "Section 1.1 A\nSection 1.2 B\nSection 2.1 C"
        )
        .run_resolver(&SectionHeaderResolver::new());
        
        let snapshot = SnapshotBuilder::new(&doc)
            .with_line_type::<crate::section_header::SectionHeader>()
            .build();
        
        let sections = snapshot.spans_of_type("SectionHeader");
        let ids: Vec<_> = sections.iter().map(|s| &s.id).collect();
        
        // Check all IDs are unique
        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique_ids.len(), ids.len());
    }

    #[test]
    fn test_ron_roundtrip() {
        let doc = ContractDocument::from_text("Section 1.1 Test")
            .run_resolver(&SectionHeaderResolver::new());
        
        let snapshot = SnapshotBuilder::new(&doc)
            .with_line_type::<crate::section_header::SectionHeader>()
            .build();
        
        let ron_str = snapshot.to_ron_string().expect("serialization failed");
        let parsed = Snapshot::from_ron_string(&ron_str).expect("deserialization failed");
        
        // Re-serialize and compare
        let ron_str2 = parsed.to_ron_string().expect("re-serialization failed");
        assert_eq!(ron_str, ron_str2);
    }
}

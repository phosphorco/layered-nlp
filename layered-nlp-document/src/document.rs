//! Document-level abstractions for multi-line processing.
//!
//! The core `layered-nlp` library operates on single lines (`LLLine`).
//! This module provides `LayeredDocument` which wraps multiple lines
//! and enables cross-line operations like section structure detection.

use layered_nlp::{LLLine, Resolver};
use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Position within a multi-line document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DocPosition {
    /// Line index (0-based)
    pub line: usize,
    /// Token index within that line
    pub token: usize,
}

impl DocPosition {
    pub fn new(line: usize, token: usize) -> Self {
        Self { line, token }
    }

    /// Create a position at the end of a line.
    ///
    /// The token index points to the last token in the line.
    pub fn end_of_line(line: usize, last_token_idx: usize) -> Self {
        Self {
            line,
            token: last_token_idx,
        }
    }
}

/// A span within a document that can cross line boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DocSpan {
    pub start: DocPosition,
    pub end: DocPosition,
}

impl DocSpan {
    pub fn new(start: DocPosition, end: DocPosition) -> Self {
        Self { start, end }
    }

    /// Create a span within a single line.
    pub fn single_line(line: usize, start_token: usize, end_token: usize) -> Self {
        Self {
            start: DocPosition::new(line, start_token),
            end: DocPosition::new(line, end_token),
        }
    }

    /// Returns true if this span is within a single line.
    pub fn is_single_line(&self) -> bool {
        self.start.line == self.end.line
    }

    /// Convert to (start_idx, end_idx) tuple if single-line.
    /// Returns None if span crosses line boundaries.
    pub fn to_lrange(&self) -> Option<(usize, usize)> {
        if self.is_single_line() {
            Some((self.start.token, self.end.token))
        } else {
            None
        }
    }

    /// Returns the number of lines this span covers.
    pub fn line_count(&self) -> usize {
        self.end.line - self.start.line + 1
    }

    /// Check if this span contains the given position.
    pub fn contains(&self, pos: &DocPosition) -> bool {
        // A position is contained if:
        // 1. It's on a line between start.line and end.line (inclusive)
        // 2. If on start line, token >= start.token
        // 3. If on end line, token <= end.token
        if pos.line < self.start.line || pos.line > self.end.line {
            return false;
        }
        if pos.line == self.start.line && pos.token < self.start.token {
            return false;
        }
        if pos.line == self.end.line && pos.token > self.end.token {
            return false;
        }
        true
    }

    /// Check if this span overlaps with another span.
    pub fn overlaps(&self, other: &DocSpan) -> bool {
        // Two spans overlap if neither ends before the other starts
        // No overlap if: self ends before other starts OR other ends before self starts
        let self_ends_before = self.end.line < other.start.line
            || (self.end.line == other.start.line && self.end.token < other.start.token);
        let other_ends_before = other.end.line < self.start.line
            || (other.end.line == self.start.line && other.end.token < self.start.token);

        !self_ends_before && !other_ends_before
    }
}

/// Document-level attribute storage using TypeId-indexed vectors.
/// Mirrors the line-level attribute pattern but for document-wide semantics.
#[derive(Default)]
struct DocAttrStore {
    /// TypeId -> Vec<Box<dyn Any + Send + Sync>>
    attrs: HashMap<TypeId, Vec<Box<dyn Any + Send + Sync>>>,
}

impl DocAttrStore {
    fn new() -> Self {
        Self {
            attrs: HashMap::new(),
        }
    }

    fn add<T: 'static + Send + Sync>(&mut self, attr: T) {
        let type_id = TypeId::of::<T>();
        self.attrs
            .entry(type_id)
            .or_insert_with(Vec::new)
            .push(Box::new(attr));
    }

    fn query<T: 'static>(&self) -> Vec<&T> {
        let type_id = TypeId::of::<T>();
        self.attrs
            .get(&type_id)
            .map(|vec| {
                vec.iter()
                    .filter_map(|boxed| boxed.downcast_ref::<T>())
                    .collect()
            })
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    fn query_mut<T: 'static>(&mut self) -> Vec<&mut T> {
        let type_id = TypeId::of::<T>();
        self.attrs
            .get_mut(&type_id)
            .map(|vec| {
                vec.iter_mut()
                    .filter_map(|boxed| boxed.downcast_mut::<T>())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Trait for document-level resolvers that analyze entire documents.
/// Unlike line-level `Resolver` which processes individual lines,
/// DocumentResolver operates across the full document structure.
pub trait DocumentResolver {
    /// The attribute type produced by this resolver
    type Attr: 'static + Send + Sync;

    /// Run the resolver on the document, producing document-level attributes.
    fn resolve(&self, doc: &LayeredDocument) -> Vec<Self::Attr>;
}

/// A document composed of multiple lines with cross-line structure.
///
/// This is the generic document type. Domain-specific crates can create
/// type aliases (e.g., `type ContractDocument = LayeredDocument`).
pub struct LayeredDocument {
    /// All lines of the document (non-empty lines only)
    lines: Vec<LLLine>,
    /// Maps internal line index to original source line number (1-based for display)
    line_to_source: Vec<usize>,
    /// Original text, preserved for display
    original_text: String,
    /// Document-level attributes indexed by type
    doc_attrs: DocAttrStore,
}

impl std::fmt::Debug for LayeredDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayeredDocument")
            .field("line_count", &self.lines.len())
            .field("original_text_len", &self.original_text.len())
            .finish()
    }
}

impl LayeredDocument {
    /// Create a document from raw text.
    ///
    /// The text is split by newlines, and each line is tokenized
    /// using `layered_nlp::create_line_from_string`. Empty lines are
    /// filtered out, but original line numbers are preserved via
    /// `source_line_number()`.
    pub fn from_text(text: &str) -> Self {
        let mut lines = Vec::new();
        let mut line_to_source = Vec::new();

        for (source_idx, line_text) in text.lines().enumerate() {
            if !line_text.trim().is_empty() {
                lines.push(layered_nlp::create_line_from_string(line_text));
                line_to_source.push(source_idx + 1); // 1-based for display
            }
        }

        Self {
            lines,
            line_to_source,
            original_text: text.to_string(),
            doc_attrs: DocAttrStore::new(),
        }
    }

    /// Get the number of lines in the document.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Get a reference to a specific line.
    pub fn get_line(&self, index: usize) -> Option<&LLLine> {
        self.lines.get(index)
    }

    /// Get a mutable reference to a specific line.
    pub fn get_line_mut(&mut self, index: usize) -> Option<&mut LLLine> {
        self.lines.get_mut(index)
    }

    /// Iterate over all lines with their indices.
    pub fn lines_enumerated(&self) -> impl Iterator<Item = (usize, &LLLine)> {
        self.lines.iter().enumerate()
    }

    /// Run a resolver on all lines in the document.
    ///
    /// This is the bridge between per-line `Resolver` trait and document-level processing.
    pub fn run_resolver<R: Resolver>(self, resolver: &R) -> Self {
        let lines = self
            .lines
            .into_iter()
            .map(|line| line.run(resolver))
            .collect();
        Self {
            lines,
            line_to_source: self.line_to_source,
            original_text: self.original_text,
            doc_attrs: self.doc_attrs,
        }
    }

    /// Get the original text.
    pub fn original_text(&self) -> &str {
        &self.original_text
    }

    /// Take ownership of the lines (consuming the document).
    pub fn into_lines(self) -> Vec<LLLine> {
        self.lines
    }

    /// Get a reference to all lines.
    pub fn lines(&self) -> &[LLLine] {
        &self.lines
    }

    /// Convert internal line index to source line number (1-based).
    ///
    /// Use this when displaying line numbers to users, as it accounts
    /// for filtered empty lines.
    pub fn source_line_number(&self, internal_index: usize) -> Option<usize> {
        self.line_to_source.get(internal_index).copied()
    }

    /// Get the internal index to source line mapping.
    pub fn line_mapping(&self) -> &[usize] {
        &self.line_to_source
    }

    /// Add a document-level attribute.
    pub fn add_doc_attr<T: 'static + Send + Sync>(&mut self, attr: T) {
        self.doc_attrs.add(attr);
    }

    /// Query all document-level attributes of type T.
    pub fn query_doc<T: 'static>(&self) -> Vec<&T> {
        self.doc_attrs.query()
    }

    /// Add multiple document-level attributes.
    pub fn add_doc_attrs<T: 'static + Send + Sync>(&mut self, attrs: impl IntoIterator<Item = T>) {
        for attr in attrs {
            self.doc_attrs.add(attr);
        }
    }

    /// Run a document-level resolver and store its results.
    /// Returns self for chaining.
    pub fn run_document_resolver<R: DocumentResolver>(mut self, resolver: &R) -> Self {
        let attrs = resolver.resolve(&self);
        for attr in attrs {
            self.doc_attrs.add(attr);
        }
        self
    }
}

/// Error types for document processing.
#[derive(Debug, Clone)]
pub enum ProcessError {
    /// Section header parsing failed
    MalformedSectionHeader {
        line: usize,
        text: String,
        reason: String,
    },
    /// Section numbering is inconsistent
    InconsistentNumbering {
        expected: String,
        found: String,
        line: usize,
    },
    /// Reference to non-existent section
    DanglingReference {
        reference: String,
        location: DocSpan,
    },
    /// Generic processing error
    Other(String),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::MalformedSectionHeader { line, text, reason } => {
                write!(f, "Line {}: malformed header '{}': {}", line, text, reason)
            }
            ProcessError::InconsistentNumbering { expected, found, line } => {
                write!(
                    f,
                    "Line {}: expected section '{}', found '{}'",
                    line, expected, found
                )
            }
            ProcessError::DanglingReference { reference, location } => {
                write!(
                    f,
                    "Dangling reference '{}' at line {}",
                    reference, location.start.line
                )
            }
            ProcessError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for ProcessError {}

/// Result wrapper that collects errors without halting processing.
#[derive(Debug)]
pub struct ProcessResult<T> {
    pub value: T,
    pub errors: Vec<ProcessError>,
    pub warnings: Vec<String>,
}

impl<T> ProcessResult<T> {
    pub fn ok(value: T) -> Self {
        Self {
            value,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn with_errors(value: T, errors: Vec<ProcessError>) -> Self {
        Self {
            value,
            errors,
            warnings: Vec::new(),
        }
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn add_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    pub fn add_error(&mut self, error: ProcessError) {
        self.errors.push(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doc_span_single_line() {
        let span = DocSpan::single_line(5, 0, 10);
        assert!(span.is_single_line());
        assert_eq!(span.to_lrange(), Some((0, 10)));
        assert_eq!(span.line_count(), 1);
    }

    #[test]
    fn test_doc_span_multi_line() {
        let span = DocSpan::new(
            DocPosition::new(2, 5),
            DocPosition::new(4, 10),
        );
        assert!(!span.is_single_line());
        assert_eq!(span.to_lrange(), None);
        assert_eq!(span.line_count(), 3);
    }

    #[test]
    fn test_document_from_text() {
        let text = "Section 1. Introduction\nThis is the intro.\n\nSection 2. Terms";
        let doc = LayeredDocument::from_text(text);
        // Empty lines are filtered out
        assert_eq!(doc.line_count(), 3);
    }

    #[test]
    fn test_source_line_numbers() {
        // Source text with blank lines at positions 3-4 (0-indexed: 2-3)
        let text = "Line 1\nLine 2\n\n\nLine 5\nLine 6";
        let doc = LayeredDocument::from_text(text);

        // Should have 4 non-empty lines
        assert_eq!(doc.line_count(), 4);

        // Source line numbers should map correctly (1-based)
        assert_eq!(doc.source_line_number(0), Some(1)); // "Line 1"
        assert_eq!(doc.source_line_number(1), Some(2)); // "Line 2"
        assert_eq!(doc.source_line_number(2), Some(5)); // "Line 5" (after 2 blank lines)
        assert_eq!(doc.source_line_number(3), Some(6)); // "Line 6"
        assert_eq!(doc.source_line_number(4), None);    // Out of bounds
    }

    #[test]
    fn test_process_result() {
        let mut result = ProcessResult::ok(42);
        assert!(!result.has_errors());

        result.add_error(ProcessError::Other("test error".into()));
        assert!(result.has_errors());
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_docspan_contains() {
        let span = DocSpan::new(
            DocPosition { line: 1, token: 5 },
            DocPosition { line: 3, token: 10 },
        );

        // Inside
        assert!(span.contains(&DocPosition { line: 2, token: 0 }));
        assert!(span.contains(&DocPosition { line: 1, token: 5 })); // start boundary
        assert!(span.contains(&DocPosition { line: 3, token: 10 })); // end boundary

        // Outside
        assert!(!span.contains(&DocPosition { line: 0, token: 5 })); // before
        assert!(!span.contains(&DocPosition { line: 4, token: 0 })); // after
        assert!(!span.contains(&DocPosition { line: 1, token: 4 })); // same line, before start token
        assert!(!span.contains(&DocPosition { line: 3, token: 11 })); // same line, after end token
    }

    #[test]
    fn test_docspan_overlaps() {
        let span1 = DocSpan::new(
            DocPosition { line: 1, token: 0 },
            DocPosition { line: 1, token: 10 },
        );
        let span2 = DocSpan::new(
            DocPosition { line: 1, token: 5 },
            DocPosition { line: 1, token: 15 },
        );
        let span3 = DocSpan::new(
            DocPosition { line: 1, token: 11 },
            DocPosition { line: 1, token: 20 },
        );

        assert!(span1.overlaps(&span2)); // overlap
        assert!(span2.overlaps(&span1)); // symmetric
        assert!(!span1.overlaps(&span3)); // no overlap (adjacent but not overlapping)
        assert!(span2.overlaps(&span3)); // overlap
    }
}

#[cfg(test)]
mod doc_attr_tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestAttr {
        value: String,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct OtherAttr {
        count: i32,
    }

    #[test]
    fn test_add_and_query_single_attr() {
        let mut doc = LayeredDocument::from_text("test");
        doc.add_doc_attr(TestAttr { value: "hello".to_string() });

        let results = doc.query_doc::<TestAttr>();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].value, "hello");
    }

    #[test]
    fn test_add_multiple_same_type() {
        let mut doc = LayeredDocument::from_text("test");
        doc.add_doc_attr(TestAttr { value: "first".to_string() });
        doc.add_doc_attr(TestAttr { value: "second".to_string() });

        let results = doc.query_doc::<TestAttr>();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_query_different_types() {
        let mut doc = LayeredDocument::from_text("test");
        doc.add_doc_attr(TestAttr { value: "test".to_string() });
        doc.add_doc_attr(OtherAttr { count: 42 });

        let test_attrs = doc.query_doc::<TestAttr>();
        let other_attrs = doc.query_doc::<OtherAttr>();

        assert_eq!(test_attrs.len(), 1);
        assert_eq!(other_attrs.len(), 1);
        assert_eq!(other_attrs[0].count, 42);
    }

    #[test]
    fn test_query_empty_returns_empty_vec() {
        let doc = LayeredDocument::from_text("test");
        let results = doc.query_doc::<TestAttr>();
        assert!(results.is_empty());
    }

    #[test]
    fn test_add_doc_attrs_batch() {
        let mut doc = LayeredDocument::from_text("test");
        doc.add_doc_attrs(vec![
            TestAttr { value: "a".to_string() },
            TestAttr { value: "b".to_string() },
            TestAttr { value: "c".to_string() },
        ]);

        let results = doc.query_doc::<TestAttr>();
        assert_eq!(results.len(), 3);
    }
}

#[cfg(test)]
mod document_resolver_tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct WordCount {
        count: usize,
    }

    struct WordCountResolver;

    impl DocumentResolver for WordCountResolver {
        type Attr = WordCount;

        fn resolve(&self, doc: &LayeredDocument) -> Vec<Self::Attr> {
            let count = doc.lines().iter().map(|l| l.ll_tokens().len()).sum();
            vec![WordCount { count }]
        }
    }

    #[test]
    fn test_run_document_resolver() {
        let doc = LayeredDocument::from_text("hello world\nfoo bar baz")
            .run_document_resolver(&WordCountResolver);

        let results = doc.query_doc::<WordCount>();
        assert_eq!(results.len(), 1);
        assert!(results[0].count > 0);
    }

    #[test]
    fn test_chain_document_resolvers() {
        #[derive(Debug, Clone, PartialEq)]
        struct LineCount { count: usize }

        struct LineCountResolver;
        impl DocumentResolver for LineCountResolver {
            type Attr = LineCount;
            fn resolve(&self, doc: &LayeredDocument) -> Vec<Self::Attr> {
                vec![LineCount { count: doc.line_count() }]
            }
        }

        let doc = LayeredDocument::from_text("line1\nline2\nline3")
            .run_document_resolver(&WordCountResolver)
            .run_document_resolver(&LineCountResolver);

        let word_counts = doc.query_doc::<WordCount>();
        let line_counts = doc.query_doc::<LineCount>();

        assert_eq!(word_counts.len(), 1);
        assert_eq!(line_counts.len(), 1);
        assert_eq!(line_counts[0].count, 3);
    }
}

//! Document-level abstractions for multi-line processing.
//!
//! The core `layered-nlp` library operates on single lines (`LLLine`).
//! This module provides `LayeredDocument` which wraps multiple lines
//! and enables cross-line operations like section structure detection.

use layered_nlp::{LLLine, Resolver};

/// Position within a multi-line document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

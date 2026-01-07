//! Core types for parsed `.nlp` fixture files.

use serde::{Deserialize, Serialize};
use std::ops::Range;

/// A parsed `.nlp` fixture document (Gate 1b: multi-paragraph support).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NlpFixture {
    /// Optional title from `# Title` header
    pub title: Option<String>,
    /// Paragraphs separated by `---`
    pub paragraphs: Vec<Paragraph>,
    /// Document-wide entity registry (from «ID:text» markers)
    pub entities: Vec<EntityDef>,
    /// Assertions defined for spans/entities
    pub assertions: Vec<Assertion>,
}

/// A single paragraph within the document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paragraph {
    /// Paragraph index (0-based)
    pub index: usize,
    /// The normalized text (markers removed)
    pub text: String,
    /// Span markers in this paragraph
    pub spans: Vec<SpanMarker>,
}

/// An entity definition from «ID:text» syntax.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDef {
    /// The entity ID (e.g., "T" from «T:The Tenant»)
    pub id: String,
    /// The defined text
    pub text: String,
    /// Which paragraph this entity was defined in
    pub paragraph_idx: usize,
    /// Character range in the paragraph's normalized text
    pub char_range: Range<usize>,
}

/// A span marker extracted from «ID:text» syntax.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanMarker {
    /// The marker ID - can be numeric ("1") or named ("T")
    pub id: MarkerId,
    /// The marked text content (without guillemets)
    pub text: String,
    /// Character range in the paragraph's normalized text
    pub char_range: Range<usize>,
}

/// Marker ID can be numeric or named.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MarkerId {
    /// Numeric ID from «1:text»
    Numeric(usize),
    /// Named ID from «T:text» (creates entity)
    Named(String),
}

/// An assertion about a span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assertion {
    /// What this assertion references
    pub target: RefTarget,
    /// The span type name (e.g., "Obligation", "DefinedTerm")
    pub span_type: String,
    /// The assertion body (key=value pairs, etc.)
    pub body: AssertionBody,
    /// Source line number for error reporting
    pub source_line: usize,
}

/// Reference target for an assertion (Gate 1b: extended).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RefTarget {
    /// Reference to a numbered span: [1], [2], etc.
    Span(usize),
    /// Reference to a named entity: §T, §Landlord
    Entity(String),
    /// Reference by text content with optional occurrence index: ["text"], ["text"@2]
    TextRef {
        text: String,
        /// 0-indexed occurrence (default 0)
        occurrence: usize,
    },
}

/// Parsed assertion body containing field assertions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssertionBody {
    /// Key-value field assertions (e.g., modal=shall)
    pub field_checks: Vec<FieldCheck>,
}

/// A single field check within an assertion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldCheck {
    /// Field name (e.g., "modal")
    pub field: String,
    /// Expected value (e.g., "shall")
    pub expected: String,
    /// Comparison operator
    pub operator: CompareOp,
}

/// Comparison operators for field checks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CompareOp {
    /// Exact equality: field=value
    Equals,
    /// Greater than or equal: field>=value
    Gte,
    /// Less than or equal: field<=value
    Lte,
    /// Contains: field~=value (for string matching)
    Contains,
}

impl Default for CompareOp {
    fn default() -> Self {
        CompareOp::Equals
    }
}

impl NlpFixture {
    /// Create an empty fixture (for testing/building).
    pub fn empty() -> Self {
        Self {
            title: None,
            paragraphs: Vec::new(),
            entities: Vec::new(),
            assertions: Vec::new(),
        }
    }

    /// Get normalized text (combines all paragraphs for single-paragraph compatibility).
    pub fn normalized_text(&self) -> String {
        self.paragraphs
            .iter()
            .map(|p| p.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Get all spans across all paragraphs (for backwards compatibility).
    pub fn spans(&self) -> Vec<&SpanMarker> {
        self.paragraphs.iter().flat_map(|p| &p.spans).collect()
    }

    /// Find entity by ID.
    pub fn entity_by_id(&self, id: &str) -> Option<&EntityDef> {
        self.entities.iter().find(|e| e.id == id)
    }

    /// Find span by numeric ID in any paragraph.
    pub fn span_by_numeric_id(&self, id: usize) -> Option<(&Paragraph, &SpanMarker)> {
        for para in &self.paragraphs {
            for span in &para.spans {
                if span.id == MarkerId::Numeric(id) {
                    return Some((para, span));
                }
            }
        }
        None
    }

    /// Get assertions for a specific span ID (backwards compatibility).
    pub fn assertions_for_span(&self, id: usize) -> Vec<&Assertion> {
        self.assertions
            .iter()
            .filter(|a| a.target == RefTarget::Span(id))
            .collect()
    }
}

impl std::fmt::Display for MarkerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarkerId::Numeric(n) => write!(f, "{}", n),
            MarkerId::Named(s) => write!(f, "{}", s),
        }
    }
}

impl std::fmt::Display for RefTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefTarget::Span(n) => write!(f, "[{}]", n),
            RefTarget::Entity(id) => write!(f, "§{}", id),
            RefTarget::TextRef { text, occurrence } => {
                if *occurrence == 0 {
                    write!(f, "[\"{}\"]", text)
                } else {
                    write!(f, "[\"{}\"@{}]", text, occurrence)
                }
            }
        }
    }
}

impl std::fmt::Display for CompareOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompareOp::Equals => write!(f, "="),
            CompareOp::Gte => write!(f, ">="),
            CompareOp::Lte => write!(f, "<="),
            CompareOp::Contains => write!(f, "~="),
        }
    }
}

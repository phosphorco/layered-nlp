//! Core types for the snapshot system.
//!
//! These types form the canonical storage format (RON-serializable).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Stable identifier for a span within a snapshot.
///
/// IDs follow the pattern `{prefix}-{index}` where:
/// - `prefix` is determined by the attribute type (e.g., "dt" for DefinedTerm)
/// - `index` is a zero-based counter within that type
///
/// # Examples
///
/// - `dt-0`, `dt-1` — DefinedTerm spans
/// - `tr-0` — TermReference span
/// - `ob-0` — ObligationPhrase span
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct SnapshotSpanId(pub String);

impl SnapshotSpanId {
    /// Create a new span ID from prefix and index.
    pub fn new(prefix: &str, index: usize) -> Self {
        Self(format!("{}-{}", prefix, index))
    }

    /// Get the prefix part of the ID (e.g., "dt" from "dt-0").
    pub fn prefix(&self) -> Option<&str> {
        self.0.split('-').next()
    }

    /// Get the index part of the ID (e.g., 0 from "dt-0").
    pub fn index(&self) -> Option<usize> {
        self.0.split('-').last()?.parse().ok()
    }
}

impl std::fmt::Display for SnapshotSpanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for SnapshotSpanId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Position within a multi-line document (for snapshot serialization).
///
/// Uses `u32` for compact serialization. Mirrors `DocPosition` from the core library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct SnapshotDocPos {
    /// Line index (0-based)
    pub line: u32,
    /// Token index within that line
    pub token: u32,
}

impl SnapshotDocPos {
    pub fn new(line: u32, token: u32) -> Self {
        Self { line, token }
    }
}

impl From<crate::DocPosition> for SnapshotDocPos {
    fn from(pos: crate::DocPosition) -> Self {
        Self {
            line: pos.line as u32,
            token: pos.token as u32,
        }
    }
}

/// A span within a document (for snapshot serialization).
///
/// Mirrors `DocSpan` from the core library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotDocSpan {
    pub start: SnapshotDocPos,
    pub end: SnapshotDocPos,
}

impl SnapshotDocSpan {
    pub fn new(start: SnapshotDocPos, end: SnapshotDocPos) -> Self {
        Self { start, end }
    }
}

impl From<crate::DocSpan> for SnapshotDocSpan {
    fn from(span: crate::DocSpan) -> Self {
        Self {
            start: span.start.into(),
            end: span.end.into(),
        }
    }
}

/// Data about an association (typed edge) between spans.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssociationData {
    /// The semantic label of this association (e.g., "obligor_source")
    pub label: String,
    /// The target span ID this association points to
    pub target: SnapshotSpanId,
    /// Optional display glyph (e.g., "@", "#")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glyph: Option<String>,
}

/// Data for a single span in the snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanData {
    /// Stable identifier for this span
    pub id: SnapshotSpanId,
    /// The document span position
    pub position: SnapshotDocSpan,
    /// The type name (e.g., "DefinedTerm", "ObligationPhrase")
    pub type_name: String,
    /// The serialized value (stored as RON Value for flexibility)
    pub value: ron::Value,
    /// Confidence score (0.0-1.0), if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// Score source description, if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Associations linking this span to other spans
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub associations: Vec<AssociationData>,
}

/// Source of input text for the snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputSource {
    /// Inline text stored directly in the snapshot
    Inline(Vec<String>),
    /// Reference to an external file (relative path)
    FileRef(String),
}

/// The canonical snapshot storage format.
///
/// This is the single source of truth for test snapshots. All human-readable
/// views are derived from this structure.
///
/// # Determinism
///
/// The snapshot format is designed for deterministic output:
/// - `spans` uses `BTreeMap` for consistent ordering by type name
/// - Spans within each type are sorted by position
/// - IDs are generated deterministically based on type and order
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Schema version for forward compatibility
    pub version: u32,
    /// The input text (inline or file reference)
    pub input: InputSource,
    /// Spans grouped by type name, sorted by position within each group
    pub spans: BTreeMap<String, Vec<SpanData>>,
    /// Auxiliary metadata (extensible)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub auxiliary: BTreeMap<String, ron::Value>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            version: 1,
            input: InputSource::Inline(Vec::new()),
            spans: BTreeMap::new(),
            auxiliary: BTreeMap::new(),
        }
    }
}

impl Snapshot {
    /// Create a new empty snapshot.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a snapshot from a ContractDocument using standard extractors.
    ///
    /// This is the primary construction path for most use cases.
    /// For more control over which types to extract, use `SnapshotBuilder`.
    pub fn from_document(doc: &crate::ContractDocument) -> Self {
        crate::snapshot::SnapshotBuilder::new(doc)
            .with_standard_types()
            .build()
    }

    /// Create a snapshot with inline input text.
    pub fn with_inline_input(lines: Vec<String>) -> Self {
        Self {
            input: InputSource::Inline(lines),
            ..Default::default()
        }
    }

    /// Serialize to a RON string.
    pub fn to_ron_string(&self) -> Result<String, ron::Error> {
        let config = ron::ser::PrettyConfig::new()
            .depth_limit(10)
            .separate_tuple_members(true)
            .enumerate_arrays(false);
        ron::ser::to_string_pretty(self, config)
    }

    /// Deserialize from a RON string.
    pub fn from_ron_string(s: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(s)
    }

    /// Get all spans of a given type.
    pub fn spans_of_type(&self, type_name: &str) -> &[SpanData] {
        self.spans.get(type_name).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get the total number of spans.
    pub fn span_count(&self) -> usize {
        self.spans.values().map(|v| v.len()).sum()
    }

    /// Check if a span ID exists in the snapshot.
    pub fn contains_id(&self, id: &SnapshotSpanId) -> bool {
        self.spans.values().any(|spans| spans.iter().any(|s| &s.id == id))
    }

    /// Find a span by its ID.
    pub fn find_by_id(&self, id: &SnapshotSpanId) -> Option<&SpanData> {
        self.spans.values().flat_map(|spans| spans.iter()).find(|s| &s.id == id)
    }

    /// Apply basic redaction for snapshot stability.
    ///
    /// This redacts non-deterministic fields like LLM pass IDs that could
    /// cause snapshot churn. More comprehensive redaction is available
    /// via `apply_redactions()` in Gate 6.
    pub fn redact(mut self) -> Self {
        for spans in self.spans.values_mut() {
            for span in spans.iter_mut() {
                // Redact source field to remove any potentially non-deterministic content
                if let Some(source) = &mut span.source {
                    // Already simplified in construction, but ensure no volatile data
                    if source.starts_with("LLMPass(") {
                        *source = "LLMPass([REDACTED])".to_string();
                    }
                }
            }
        }
        self
    }
}

/// Trait for types that can be stored in snapshots.
///
/// Implement this trait to provide type-specific prefixes for stable ID generation.
///
/// # Example
///
/// ```ignore
/// impl SnapshotKind for DefinedTerm {
///     const SNAPSHOT_PREFIX: &'static str = "dt";
///     const SNAPSHOT_TYPE_NAME: &'static str = "DefinedTerm";
/// }
/// ```
pub trait SnapshotKind {
    /// The prefix used for span IDs (e.g., "dt", "tr", "ob").
    const SNAPSHOT_PREFIX: &'static str;
    /// The type name as it appears in the snapshot (e.g., "DefinedTerm").
    const SNAPSHOT_TYPE_NAME: &'static str;
}

// Implement SnapshotKind for core types

impl SnapshotKind for crate::defined_term::DefinedTerm {
    const SNAPSHOT_PREFIX: &'static str = "dt";
    const SNAPSHOT_TYPE_NAME: &'static str = "DefinedTerm";
}

impl SnapshotKind for crate::term_reference::TermReference {
    const SNAPSHOT_PREFIX: &'static str = "tr";
    const SNAPSHOT_TYPE_NAME: &'static str = "TermReference";
}

impl SnapshotKind for crate::obligation::ObligationPhrase {
    const SNAPSHOT_PREFIX: &'static str = "ob";
    const SNAPSHOT_TYPE_NAME: &'static str = "ObligationPhrase";
}

impl SnapshotKind for crate::section_header::SectionHeader {
    const SNAPSHOT_PREFIX: &'static str = "sh";
    const SNAPSHOT_TYPE_NAME: &'static str = "SectionHeader";
}

impl SnapshotKind for crate::temporal::TemporalExpression {
    const SNAPSHOT_PREFIX: &'static str = "te";
    const SNAPSHOT_TYPE_NAME: &'static str = "TemporalExpression";
}

impl SnapshotKind for crate::contract_keyword::ContractKeyword {
    const SNAPSHOT_PREFIX: &'static str = "kw";
    const SNAPSHOT_TYPE_NAME: &'static str = "ContractKeyword";
}

impl SnapshotKind for crate::pronoun::PronounReference {
    const SNAPSHOT_PREFIX: &'static str = "pr";
    const SNAPSHOT_TYPE_NAME: &'static str = "PronounReference";
}

// Note: SnapshotKind impls for Coordination, BridgingReference, and document_types
// will be added when those modules are implemented.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_id_creation() {
        let id = SnapshotSpanId::new("dt", 0);
        assert_eq!(id.0, "dt-0");
        assert_eq!(id.prefix(), Some("dt"));
        assert_eq!(id.index(), Some(0));
    }

    #[test]
    fn test_span_id_display() {
        let id = SnapshotSpanId::new("ob", 42);
        assert_eq!(format!("{}", id), "ob-42");
    }

    #[test]
    fn test_snapshot_doc_pos_ordering() {
        let p1 = SnapshotDocPos::new(1, 5);
        let p2 = SnapshotDocPos::new(1, 10);
        let p3 = SnapshotDocPos::new(2, 0);

        assert!(p1 < p2);
        assert!(p2 < p3);
        assert!(p1 < p3);
    }

    #[test]
    fn test_snapshot_default() {
        let snap = Snapshot::new();
        assert_eq!(snap.version, 1);
        assert!(snap.spans.is_empty());
        assert_eq!(snap.span_count(), 0);
    }

    #[test]
    fn test_snapshot_ron_roundtrip() {
        let mut snap = Snapshot::with_inline_input(vec![
            "Line one".to_string(),
            "Line two".to_string(),
        ]);
        
        snap.spans.insert("DefinedTerm".to_string(), vec![
            SpanData {
                id: SnapshotSpanId::new("dt", 0),
                position: SnapshotDocSpan::new(
                    SnapshotDocPos::new(0, 0),
                    SnapshotDocPos::new(0, 3),
                ),
                type_name: "DefinedTerm".to_string(),
                value: ron::Value::String("Company".to_string()),
                confidence: Some(0.95),
                source: Some("QuotedMeans".to_string()),
                associations: vec![],
            },
        ]);

        let ron_str = snap.to_ron_string().expect("serialization failed");
        let parsed = Snapshot::from_ron_string(&ron_str).expect("deserialization failed");
        
        assert_eq!(snap, parsed);
    }

    #[test]
    fn test_snapshot_find_by_id() {
        let mut snap = Snapshot::new();
        snap.spans.insert("DefinedTerm".to_string(), vec![
            SpanData {
                id: SnapshotSpanId::new("dt", 0),
                position: SnapshotDocSpan::new(
                    SnapshotDocPos::new(0, 0),
                    SnapshotDocPos::new(0, 3),
                ),
                type_name: "DefinedTerm".to_string(),
                value: ron::Value::String("Test".to_string()),
                confidence: None,
                source: None,
                associations: vec![],
            },
        ]);

        let id = SnapshotSpanId::new("dt", 0);
        assert!(snap.contains_id(&id));
        assert!(snap.find_by_id(&id).is_some());

        let missing_id = SnapshotSpanId::new("dt", 99);
        assert!(!snap.contains_id(&missing_id));
        assert!(snap.find_by_id(&missing_id).is_none());
    }

    #[test]
    fn test_snapshot_kind_prefixes() {
        use crate::defined_term::DefinedTerm;
        use crate::term_reference::TermReference;
        use crate::section_header::SectionHeader;
        
        assert_eq!(DefinedTerm::SNAPSHOT_PREFIX, "dt");
        assert_eq!(TermReference::SNAPSHOT_PREFIX, "tr");
        assert_eq!(SectionHeader::SNAPSHOT_PREFIX, "sh");
    }
}

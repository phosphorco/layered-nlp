//! Semantic summary rendering for snapshots.
//!
//! This module provides `SnapshotRenderer` which produces human-readable
//! semantic summaries from `Snapshot` data, grouped by category.

use std::fmt::Write;

use super::types::{Snapshot, SpanData};

/// Semantic categories for grouping spans in rendered output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SemanticCategory {
    /// Defined terms ("Company" means...)
    Definitions,
    /// References to terms, sections, pronouns
    References,
    /// Obligation phrases (shall, must, may)
    Obligations,
    /// Document structure (sections, headers)
    Structure,
    /// Temporal expressions (within 30 days)
    Temporal,
    /// Other/uncategorized
    Other,
}

impl SemanticCategory {
    /// Display name for the category.
    pub fn display_name(&self) -> &'static str {
        match self {
            SemanticCategory::Definitions => "DEFINITIONS",
            SemanticCategory::References => "REFERENCES",
            SemanticCategory::Obligations => "OBLIGATIONS",
            SemanticCategory::Structure => "STRUCTURE",
            SemanticCategory::Temporal => "TEMPORAL",
            SemanticCategory::Other => "OTHER",
        }
    }

    /// Emoji glyph for the category.
    pub fn glyph(&self) -> &'static str {
        match self {
            SemanticCategory::Definitions => "📖",
            SemanticCategory::References => "📎",
            SemanticCategory::Obligations => "⚖️",
            SemanticCategory::Structure => "🏗️",
            SemanticCategory::Temporal => "⏱️",
            SemanticCategory::Other => "📝",
        }
    }

    /// All categories in display order.
    pub fn all_in_order() -> &'static [SemanticCategory] {
        &[
            SemanticCategory::Definitions,
            SemanticCategory::References,
            SemanticCategory::Obligations,
            SemanticCategory::Structure,
            SemanticCategory::Temporal,
            SemanticCategory::Other,
        ]
    }
}

/// Classify a type name into a semantic category.
pub fn classify_type_name(type_name: &str) -> SemanticCategory {
    match type_name {
        "DefinedTerm" => SemanticCategory::Definitions,
        
        "TermReference" | "PronounReference" | "BridgingReference" | "Coordination" => {
            SemanticCategory::References
        }
        
        "ObligationPhrase" | "ContractKeyword" => SemanticCategory::Obligations,
        
        "SectionHeader" | "RecitalSection" | "AppendixBoundary" | "FootnoteBlock" | "PrecedenceRule" => {
            SemanticCategory::Structure
        }
        
        "TemporalExpression" => SemanticCategory::Temporal,
        
        _ => SemanticCategory::Other,
    }
}

/// Configuration for snapshot rendering.
#[derive(Debug, Clone)]
pub struct SnapshotRenderer {
    /// Maximum spans to show per type before eliding
    pub max_spans_per_type: usize,
    /// Whether to show confidence scores
    pub show_confidence: bool,
    /// Threshold below which confidence is displayed
    pub confidence_threshold: f64,
}

impl Default for SnapshotRenderer {
    fn default() -> Self {
        Self {
            max_spans_per_type: 20,
            show_confidence: true,
            confidence_threshold: 0.8,
        }
    }
}

impl SnapshotRenderer {
    /// Create a new renderer with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a renderer with custom max spans per type.
    pub fn with_max_spans(mut self, max: usize) -> Self {
        self.max_spans_per_type = max;
        self
    }

    /// Create a renderer with custom confidence threshold.
    pub fn with_confidence_threshold(mut self, threshold: f64) -> Self {
        self.confidence_threshold = threshold;
        self
    }

    /// Enable or disable confidence display.
    pub fn with_show_confidence(mut self, show: bool) -> Self {
        self.show_confidence = show;
        self
    }

    /// Render a semantic summary of the snapshot.
    ///
    /// The output groups spans by semantic category (Definitions, References, etc.)
    /// with each span showing its ID, a brief value summary, and optional confidence.
    pub fn render_semantic(&self, snapshot: &Snapshot) -> String {
        let mut output = String::new();

        // Group spans by category
        let mut by_category: std::collections::BTreeMap<SemanticCategory, Vec<(&str, &SpanData)>> =
            std::collections::BTreeMap::new();

        for (type_name, spans) in &snapshot.spans {
            let category = classify_type_name(type_name);
            for span in spans {
                by_category.entry(category).or_default().push((type_name, span));
            }
        }

        // Render each category in order
        for category in SemanticCategory::all_in_order() {
            if let Some(spans) = by_category.get(category) {
                if spans.is_empty() {
                    continue;
                }

                // Category header
                writeln!(
                    output,
                    "{} {} ({})",
                    category.glyph(),
                    category.display_name(),
                    spans.len()
                )
                .unwrap();

                // Sort spans by position for consistent output
                let mut sorted_spans = spans.clone();
                sorted_spans.sort_by(|a, b| {
                    let a_pos = (
                        a.1.position.start.line,
                        a.1.position.start.token,
                        a.1.position.end.line,
                        a.1.position.end.token,
                    );
                    let b_pos = (
                        b.1.position.start.line,
                        b.1.position.start.token,
                        b.1.position.end.line,
                        b.1.position.end.token,
                    );
                    a_pos.cmp(&b_pos)
                });

                // Render spans with per-type elision
                // We limit how many spans of each *type* are shown, not per category
                let mut shown_per_type: std::collections::HashMap<&str, usize> =
                    std::collections::HashMap::new();
                let mut shown_count = 0;
                let total = sorted_spans.len();
                
                for (type_name, span) in &sorted_spans {
                    let count = shown_per_type.entry(*type_name).or_insert(0);
                    if *count < self.max_spans_per_type {
                        *count += 1;
                        self.render_span(&mut output, type_name, span);
                        shown_count += 1;
                    }
                }

                if total > shown_count {
                    let elided = total - shown_count;
                    writeln!(
                        output,
                        "  ... {} more {} elided; see .ron snapshot",
                        elided,
                        if elided == 1 { "span" } else { "spans" }
                    )
                    .unwrap();
                }

                writeln!(output).unwrap();
            }
        }

        output.trim_end().to_string()
    }

    /// Render a single span.
    fn render_span(&self, output: &mut String, type_name: &str, span: &SpanData) {
        // Extract a brief value summary from the Debug string
        let value_summary = self.summarize_value(&span.value);

        // Format: [id] value_summary (type) conf=X.XX →[targets]
        write!(output, "  [{}] {}", span.id, value_summary).unwrap();

        // Add type name if it's not obvious from the value
        if !value_summary.contains(type_name) {
            write!(output, " ({})", type_name).unwrap();
        }

        // Add confidence if below threshold and enabled
        if self.show_confidence {
            if let Some(conf) = span.confidence {
                if conf < self.confidence_threshold {
                    write!(output, " conf={:.2}", conf).unwrap();
                }
            }
        }

        // Add association summary if any
        if !span.associations.is_empty() {
            let targets: Vec<_> = span
                .associations
                .iter()
                .map(|a| format!("→[{}]", a.target))
                .collect();
            write!(output, " {}", targets.join(" ")).unwrap();
        }

        writeln!(output).unwrap();
    }

    /// Extract a brief summary from a RON value.
    ///
    /// Uses char-based truncation to handle Unicode safely.
    fn summarize_value(&self, value: &ron::Value) -> String {
        const MAX_LEN: usize = 60;
        const TRUNC_LEN: usize = 57;

        match value {
            ron::Value::String(s) => {
                if s.chars().count() > MAX_LEN {
                    let mut out = String::new();
                    for (i, ch) in s.chars().enumerate() {
                        if i >= TRUNC_LEN {
                            break;
                        }
                        out.push(ch);
                    }
                    out.push_str("...");
                    out
                } else {
                    s.clone()
                }
            }
            _ => format!("{:?}", value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_type_name() {
        assert_eq!(classify_type_name("DefinedTerm"), SemanticCategory::Definitions);
        assert_eq!(classify_type_name("TermReference"), SemanticCategory::References);
        assert_eq!(classify_type_name("ObligationPhrase"), SemanticCategory::Obligations);
        assert_eq!(classify_type_name("SectionHeader"), SemanticCategory::Structure);
        assert_eq!(classify_type_name("TemporalExpression"), SemanticCategory::Temporal);
        assert_eq!(classify_type_name("UnknownType"), SemanticCategory::Other);
    }

    #[test]
    fn test_category_display_info() {
        let cat = SemanticCategory::Definitions;
        assert_eq!(cat.display_name(), "DEFINITIONS");
        assert_eq!(cat.glyph(), "📖");
    }

    #[test]
    fn test_renderer_default() {
        let renderer = SnapshotRenderer::default();
        assert_eq!(renderer.max_spans_per_type, 20);
        assert!(renderer.show_confidence);
        assert!((renderer.confidence_threshold - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_render_empty_snapshot() {
        let snapshot = Snapshot::new();
        let renderer = SnapshotRenderer::new();
        let output = renderer.render_semantic(&snapshot);
        assert!(output.is_empty());
    }
}

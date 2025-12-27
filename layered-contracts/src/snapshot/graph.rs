//! Association graph rendering (FR-010 Gate 4).
//!
//! This module provides `render_graph()` which produces a tree-style
//! visualization of span associations, showing relationships between spans.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write;

use super::types::{Snapshot, SpanData, SnapshotSpanId};
use super::semantic::{classify_type_name, SemanticCategory};

/// Configuration for graph rendering.
#[derive(Debug, Clone)]
pub struct GraphRenderer {
    /// Maximum spans to show per category before eliding
    pub max_spans_per_category: usize,
    /// Whether to show reverse associations (incoming references)
    pub show_reverse_associations: bool,
}

impl Default for GraphRenderer {
    fn default() -> Self {
        Self {
            max_spans_per_category: 50,
            show_reverse_associations: true,
        }
    }
}

impl GraphRenderer {
    /// Create a new renderer with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum spans per category.
    pub fn with_max_spans(mut self, max: usize) -> Self {
        self.max_spans_per_category = max;
        self
    }

    /// Enable or disable reverse association display.
    pub fn with_reverse_associations(mut self, show: bool) -> Self {
        self.show_reverse_associations = show;
        self
    }

    /// Render the association graph for a snapshot.
    pub fn render_graph(&self, snapshot: &Snapshot) -> String {
        let mut output = String::new();

        // Build lookup maps
        let span_map = self.build_span_map(snapshot);
        let reverse_index = self.build_reverse_index(snapshot);

        // Collect spans that have associations (either outgoing or incoming)
        let relevant_spans = self.collect_relevant_spans(snapshot, &reverse_index);

        if relevant_spans.is_empty() {
            return String::new();
        }

        // Group spans by semantic category
        let by_category = self.group_by_category(&relevant_spans, &span_map);

        // Render each category
        for category in SemanticCategory::all_in_order() {
            if let Some(span_ids) = by_category.get(category) {
                if span_ids.is_empty() {
                    continue;
                }

                // Category header
                writeln!(output, "{} {} ASSOCIATIONS", category.glyph(), category.display_name())
                    .unwrap();

                let mut shown = 0;
                for span_id in span_ids {
                    if shown >= self.max_spans_per_category {
                        writeln!(
                            output,
                            "  ... {} more spans elided",
                            span_ids.len() - shown
                        )
                        .unwrap();
                        break;
                    }

                    if let Some(span) = span_map.get(span_id) {
                        self.render_span_with_associations(
                            &mut output,
                            span,
                            &span_map,
                            &reverse_index,
                        );
                        shown += 1;
                    }
                }

                writeln!(output).unwrap();
            }
        }

        output.trim_end().to_string()
    }

    /// Build a map from span ID to span data.
    fn build_span_map<'a>(&self, snapshot: &'a Snapshot) -> HashMap<&'a SnapshotSpanId, &'a SpanData> {
        let mut map = HashMap::new();
        for spans in snapshot.spans.values() {
            for span in spans {
                map.insert(&span.id, span);
            }
        }
        map
    }

    /// Build a reverse index: target ID -> list of (source span, association label).
    ///
    /// Results are sorted by (source_id, label) for deterministic output.
    fn build_reverse_index<'a>(
        &self,
        snapshot: &'a Snapshot,
    ) -> HashMap<&'a SnapshotSpanId, Vec<(&'a SpanData, &'a str)>> {
        let mut index: HashMap<&SnapshotSpanId, Vec<(&SpanData, &str)>> = HashMap::new();

        for spans in snapshot.spans.values() {
            for span in spans {
                for assoc in &span.associations {
                    index
                        .entry(&assoc.target)
                        .or_default()
                        .push((span, &assoc.label));
                }
            }
        }

        // Sort each entry for determinism
        for sources in index.values_mut() {
            sources.sort_by(|a, b| {
                let key_a = (&a.0.id.0, a.1);
                let key_b = (&b.0.id.0, b.1);
                key_a.cmp(&key_b)
            });
        }

        index
    }

    /// Collect span IDs that have associations (outgoing or incoming).
    fn collect_relevant_spans<'a>(
        &self,
        snapshot: &'a Snapshot,
        reverse_index: &HashMap<&'a SnapshotSpanId, Vec<(&'a SpanData, &'a str)>>,
    ) -> Vec<&'a SnapshotSpanId> {
        let mut relevant: HashSet<&SnapshotSpanId> = HashSet::new();

        for spans in snapshot.spans.values() {
            for span in spans {
                // Has outgoing associations
                if !span.associations.is_empty() {
                    relevant.insert(&span.id);
                }

                // Has incoming associations
                if self.show_reverse_associations && reverse_index.contains_key(&span.id) {
                    relevant.insert(&span.id);
                }
            }
        }

        // Sort for determinism
        let mut sorted: Vec<_> = relevant.into_iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        sorted
    }

    /// Group span IDs by semantic category.
    fn group_by_category<'a>(
        &self,
        span_ids: &[&'a SnapshotSpanId],
        span_map: &HashMap<&'a SnapshotSpanId, &'a SpanData>,
    ) -> BTreeMap<SemanticCategory, Vec<&'a SnapshotSpanId>> {
        let mut by_category: BTreeMap<SemanticCategory, Vec<&SnapshotSpanId>> = BTreeMap::new();

        for span_id in span_ids {
            if let Some(span) = span_map.get(span_id) {
                let category = classify_type_name(&span.type_name);
                by_category.entry(category).or_default().push(*span_id);
            }
        }

        // Sort each category's spans by position for determinism
        for spans in by_category.values_mut() {
            spans.sort_by(|a, b| {
                let span_a = span_map.get(a);
                let span_b = span_map.get(b);
                match (span_a, span_b) {
                    (Some(a), Some(b)) => {
                        let key_a = (a.position.start.line, a.position.start.token);
                        let key_b = (b.position.start.line, b.position.start.token);
                        key_a.cmp(&key_b)
                    }
                    _ => a.0.cmp(&b.0),
                }
            });
        }

        by_category
    }

    /// Render a single span with its associations.
    fn render_span_with_associations(
        &self,
        output: &mut String,
        span: &SpanData,
        span_map: &HashMap<&SnapshotSpanId, &SpanData>,
        reverse_index: &HashMap<&SnapshotSpanId, Vec<(&SpanData, &str)>>,
    ) {
        // Span header: [id] TypeName(value_summary)
        let value_summary = self.summarize_value(&span.value);
        writeln!(output, "  [{}] {}({})", span.id, span.type_name, value_summary).unwrap();

        // Outgoing associations (sorted by (label, target_id) for determinism)
        let mut sorted_assocs: Vec<_> = span.associations.iter().collect();
        sorted_assocs.sort_by(|a, b| {
            let key_a = (&a.label, &a.target.0);
            let key_b = (&b.label, &b.target.0);
            key_a.cmp(&key_b)
        });

        for assoc in sorted_assocs {
            let glyph = assoc.glyph.as_deref().unwrap_or("");
            let target_summary = span_map
                .get(&assoc.target)
                .map(|s| format!("{}({})", s.type_name, self.summarize_value(&s.value)))
                .unwrap_or_else(|| "[missing]".to_string());

            writeln!(
                output,
                "    ├─{}{}→[{}] {}",
                glyph, assoc.label, assoc.target, target_summary
            )
            .unwrap();
        }

        // Incoming associations (reverse references)
        if self.show_reverse_associations {
            if let Some(sources) = reverse_index.get(&span.id) {
                for (source, label) in sources {
                    writeln!(output, "    ↑ {} from [{}]", label, source.id).unwrap();
                }
            }
        }
    }

    /// Summarize a RON value for display.
    ///
    /// Uses char-based truncation for Unicode safety. Both string values and
    /// Debug output from other types are truncated to MAX_LEN.
    fn summarize_value(&self, value: &ron::Value) -> String {
        const MAX_LEN: usize = 30;
        const TRUNC_LEN: usize = 27;

        let raw = match value {
            ron::Value::String(s) => s.clone(),
            other => format!("{:?}", other),
        };
        
        if raw.chars().count() > MAX_LEN {
            let truncated: String = raw.chars().take(TRUNC_LEN).collect();
            format!("{}...", truncated)
        } else {
            raw
        }
    }
}

impl Snapshot {
    /// Render an association graph view of the snapshot.
    ///
    /// This shows span relationships in a tree-style format, with outgoing
    /// associations displayed as `├─label→[target]` and incoming references
    /// shown as `↑ label from [source]`.
    pub fn render_graph(&self) -> String {
        GraphRenderer::new().render_graph(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{AssociationData, SnapshotDocPos, SnapshotDocSpan, SpanData};

    fn make_span(
        prefix: &str,
        idx: usize,
        line: u32,
        type_name: &str,
        value: &str,
    ) -> SpanData {
        SpanData {
            id: SnapshotSpanId::new(prefix, idx),
            position: SnapshotDocSpan::new(
                SnapshotDocPos::new(line, 0),
                SnapshotDocPos::new(line, 5),
            ),
            type_name: type_name.to_string(),
            value: ron::Value::String(value.to_string()),
            confidence: None,
            source: None,
            associations: vec![],
        }
    }

    #[test]
    fn test_graph_empty_snapshot() {
        let snapshot = Snapshot::new();
        let output = snapshot.render_graph();
        assert!(output.is_empty());
    }

    #[test]
    fn test_graph_no_associations() {
        let mut snapshot = Snapshot::new();
        snapshot.spans.insert(
            "DefinedTerm".to_string(),
            vec![make_span("dt", 0, 0, "DefinedTerm", "Company")],
        );

        let output = snapshot.render_graph();
        assert!(output.is_empty(), "No associations = no graph");
    }

    #[test]
    fn test_graph_simple_chain() {
        let mut snapshot = Snapshot::new();

        // DefinedTerm
        snapshot.spans.insert(
            "DefinedTerm".to_string(),
            vec![make_span("dt", 0, 0, "DefinedTerm", "Company")],
        );

        // TermReference with association to DefinedTerm
        let mut tr_span = make_span("tr", 0, 1, "TermReference", "Company");
        tr_span.associations.push(AssociationData {
            label: "resolves_to".to_string(),
            target: SnapshotSpanId::new("dt", 0),
            glyph: None,
        });
        snapshot.spans.insert("TermReference".to_string(), vec![tr_span]);

        let output = snapshot.render_graph();

        // Should show the TermReference with its outgoing association
        assert!(output.contains("[tr-0]"), "Should show TermReference: {}", output);
        assert!(output.contains("resolves_to→[dt-0]"), "Should show association: {}", output);

        // Should show DefinedTerm with incoming reference
        assert!(output.contains("[dt-0]"), "Should show DefinedTerm: {}", output);
        assert!(output.contains("↑ resolves_to from [tr-0]"), "Should show reverse ref: {}", output);
    }

    #[test]
    fn test_graph_multiple_outgoing() {
        let mut snapshot = Snapshot::new();

        // Two defined terms
        snapshot.spans.insert(
            "DefinedTerm".to_string(),
            vec![
                make_span("dt", 0, 0, "DefinedTerm", "Company"),
                make_span("dt", 1, 1, "DefinedTerm", "Products"),
            ],
        );

        // Obligation with multiple associations
        let mut ob_span = make_span("ob", 0, 2, "ObligationPhrase", "shall deliver");
        ob_span.associations.push(AssociationData {
            label: "obligor_source".to_string(),
            target: SnapshotSpanId::new("dt", 0),
            glyph: Some("@".to_string()),
        });
        ob_span.associations.push(AssociationData {
            label: "object".to_string(),
            target: SnapshotSpanId::new("dt", 1),
            glyph: None,
        });
        snapshot.spans.insert("ObligationPhrase".to_string(), vec![ob_span]);

        let output = snapshot.render_graph();

        // Should show both associations
        assert!(output.contains("@obligor_source→[dt-0]"), "Should show obligor: {}", output);
        assert!(output.contains("object→[dt-1]"), "Should show object: {}", output);
    }

    #[test]
    fn test_graph_determinism() {
        let mut snapshot = Snapshot::new();

        snapshot.spans.insert(
            "DefinedTerm".to_string(),
            vec![make_span("dt", 0, 0, "DefinedTerm", "Company")],
        );

        let mut tr_span = make_span("tr", 0, 1, "TermReference", "Company");
        tr_span.associations.push(AssociationData {
            label: "resolves_to".to_string(),
            target: SnapshotSpanId::new("dt", 0),
            glyph: None,
        });
        snapshot.spans.insert("TermReference".to_string(), vec![tr_span]);

        let output1 = snapshot.render_graph();
        let output2 = snapshot.render_graph();
        let output3 = snapshot.render_graph();

        assert_eq!(output1, output2);
        assert_eq!(output2, output3);
    }

    #[test]
    fn test_graph_category_grouping() {
        let mut snapshot = Snapshot::new();

        // DefinedTerm (Definitions category)
        snapshot.spans.insert(
            "DefinedTerm".to_string(),
            vec![make_span("dt", 0, 0, "DefinedTerm", "Company")],
        );

        // TermReference (References category) with association
        let mut tr_span = make_span("tr", 0, 1, "TermReference", "Company");
        tr_span.associations.push(AssociationData {
            label: "resolves_to".to_string(),
            target: SnapshotSpanId::new("dt", 0),
            glyph: None,
        });
        snapshot.spans.insert("TermReference".to_string(), vec![tr_span]);

        let output = snapshot.render_graph();

        // Should have category headers
        assert!(output.contains("DEFINITIONS ASSOCIATIONS"), "Should have Definitions: {}", output);
        assert!(output.contains("REFERENCES ASSOCIATIONS"), "Should have References: {}", output);
    }

    #[test]
    fn test_graph_without_reverse_associations() {
        let mut snapshot = Snapshot::new();

        snapshot.spans.insert(
            "DefinedTerm".to_string(),
            vec![make_span("dt", 0, 0, "DefinedTerm", "Company")],
        );

        let mut tr_span = make_span("tr", 0, 1, "TermReference", "Company");
        tr_span.associations.push(AssociationData {
            label: "resolves_to".to_string(),
            target: SnapshotSpanId::new("dt", 0),
            glyph: None,
        });
        snapshot.spans.insert("TermReference".to_string(), vec![tr_span]);

        let renderer = GraphRenderer::new().with_reverse_associations(false);
        let output = renderer.render_graph(&snapshot);

        // Should NOT show incoming references
        assert!(!output.contains("↑"), "Should not have reverse refs: {}", output);
        
        // Should still show outgoing
        assert!(output.contains("resolves_to→[dt-0]"), "Should show outgoing: {}", output);
    }
}

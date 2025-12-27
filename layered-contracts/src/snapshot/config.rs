//! Snapshot configuration and combined rendering (FR-010 Gate 5).
//!
//! This module provides `SnapshotConfig` for configuring snapshot rendering
//! and `render_all()` for producing combined views.

use std::collections::HashSet;

use super::display::DocDisplay;
use super::graph::GraphRenderer;
use super::semantic::SnapshotRenderer;
use super::types::Snapshot;

/// Rendering mode flags for combined output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    /// Semantic summary grouped by category
    Semantic,
    /// Annotated text with span overlays
    Annotated,
    /// Association graph view
    Graph,
}

/// Configuration for snapshot rendering.
///
/// Controls which sections appear in combined output, type filtering,
/// and verbosity settings.
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Types to include (empty = include all)
    pub included_types: HashSet<String>,
    /// Rendering modes to include
    pub render_modes: Vec<RenderMode>,
    /// Verbose mode (show all confidence scores, etc.)
    pub verbose: bool,
    /// Maximum spans per type/category before eliding
    pub max_spans_per_group: usize,
    /// Show line numbers in annotated view
    pub show_line_numbers: bool,
    /// Show reverse associations in graph view
    pub show_reverse_associations: bool,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            included_types: HashSet::new(),
            render_modes: vec![RenderMode::Semantic, RenderMode::Annotated, RenderMode::Graph],
            verbose: false,
            max_spans_per_group: 20,
            show_line_numbers: true,
            show_reverse_associations: true,
        }
    }
}

impl SnapshotConfig {
    /// Create a new config with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a minimal config showing only the semantic summary.
    pub fn minimal() -> Self {
        Self {
            render_modes: vec![RenderMode::Semantic],
            max_spans_per_group: 50,
            ..Default::default()
        }
    }

    /// Create a verbose config with all details.
    pub fn verbose() -> Self {
        Self {
            render_modes: vec![RenderMode::Semantic, RenderMode::Annotated, RenderMode::Graph],
            verbose: true,
            max_spans_per_group: 100,
            ..Default::default()
        }
    }

    /// Replace the included types set (only these types will be shown).
    ///
    /// Passing an empty slice resets to showing all types.
    pub fn with_types(mut self, types: &[&str]) -> Self {
        self.included_types = types.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Set render modes.
    pub fn with_modes(mut self, modes: Vec<RenderMode>) -> Self {
        self.render_modes = modes;
        self
    }

    /// Enable or disable verbose mode.
    ///
    /// In verbose mode, confidence scores are always shown (threshold set to 1.0),
    /// meaning all confidences display regardless of their value.
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Set maximum spans per group.
    pub fn with_max_spans(mut self, max: usize) -> Self {
        self.max_spans_per_group = max;
        self
    }

    /// Enable or disable line numbers in annotated view.
    pub fn with_line_numbers(mut self, show: bool) -> Self {
        self.show_line_numbers = show;
        self
    }

    /// Enable or disable reverse associations in graph view.
    pub fn with_reverse_associations(mut self, show: bool) -> Self {
        self.show_reverse_associations = show;
        self
    }
}

impl Snapshot {
    /// Render a combined view of the snapshot.
    ///
    /// This produces a multi-section output containing semantic summary,
    /// annotated text, and association graph as configured.
    pub fn render_all(&self, config: &SnapshotConfig) -> String {
        let mut output = String::new();
        let mut first = true;

        for mode in &config.render_modes {
            if !first {
                output.push_str("\n\n");
            }
            first = false;

            match mode {
                RenderMode::Semantic => {
                    output.push_str("═══ SEMANTIC SUMMARY ═══\n\n");
                    let renderer = SnapshotRenderer::new()
                        .with_max_spans(config.max_spans_per_group)
                        .with_confidence_threshold(if config.verbose { 1.0 } else { 0.8 });
                    
                    let semantic = if config.included_types.is_empty() {
                        renderer.render_semantic(self)
                    } else {
                        renderer.render_semantic(&self.filter_types(&config.included_types))
                    };
                    
                    if semantic.is_empty() {
                        output.push_str("(no spans)");
                    } else {
                        output.push_str(&semantic);
                    }
                }

                RenderMode::Annotated => {
                    output.push_str("═══ ANNOTATED TEXT ═══\n\n");
                    let mut display = DocDisplay::new(self);
                    
                    if config.verbose {
                        display = display.verbose();
                    }
                    if !config.show_line_numbers {
                        display = display.without_line_numbers();
                    }
                    
                    // Apply type filtering (sorted for determinism across runs)
                    if !config.included_types.is_empty() {
                        let mut types: Vec<_> = config.included_types.iter().collect();
                        types.sort();
                        for type_name in types {
                            display = display.include(type_name);
                        }
                    }
                    
                    let annotated = format!("{}", display);
                    if annotated.trim().is_empty() {
                        output.push_str("(no text)");
                    } else {
                        output.push_str(&annotated);
                    }
                }

                RenderMode::Graph => {
                    output.push_str("═══ ASSOCIATION GRAPH ═══\n\n");
                    let renderer = GraphRenderer::new()
                        .with_max_spans(config.max_spans_per_group)
                        .with_reverse_associations(config.show_reverse_associations);
                    
                    let graph = if config.included_types.is_empty() {
                        renderer.render_graph(self)
                    } else {
                        renderer.render_graph(&self.filter_types(&config.included_types))
                    };
                    
                    if graph.is_empty() {
                        output.push_str("(no associations)");
                    } else {
                        output.push_str(&graph);
                    }
                }
            }
        }

        output
    }

    /// Create a filtered snapshot containing only the specified types.
    fn filter_types(&self, included: &HashSet<String>) -> Snapshot {
        let mut filtered = self.clone();
        filtered.spans.retain(|type_name, _| included.contains(type_name));
        filtered
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{SnapshotDocPos, SnapshotDocSpan, SnapshotSpanId, SpanData};

    fn make_snapshot_with_spans() -> Snapshot {
        let mut snapshot = Snapshot::with_inline_input(vec![
            "Section 1.1 Definitions".to_string(),
            "\"Company\" means ABC Corp.".to_string(),
        ]);

        snapshot.spans.insert(
            "SectionHeader".to_string(),
            vec![SpanData {
                id: SnapshotSpanId::new("sh", 0),
                position: SnapshotDocSpan::new(
                    SnapshotDocPos::new(0, 0),
                    SnapshotDocPos::new(0, 3),
                ),
                type_name: "SectionHeader".to_string(),
                value: ron::Value::String("1.1".to_string()),
                confidence: None,
                source: None,
                associations: vec![],
            }],
        );

        snapshot.spans.insert(
            "DefinedTerm".to_string(),
            vec![SpanData {
                id: SnapshotSpanId::new("dt", 0),
                position: SnapshotDocSpan::new(
                    SnapshotDocPos::new(1, 0),
                    SnapshotDocPos::new(1, 2),
                ),
                type_name: "DefinedTerm".to_string(),
                value: ron::Value::String("Company".to_string()),
                confidence: Some(0.95),
                source: Some("RuleBased(quoted_means)".to_string()),
                associations: vec![],
            }],
        );

        snapshot
    }

    #[test]
    fn test_config_default() {
        let config = SnapshotConfig::default();
        assert!(config.included_types.is_empty());
        assert_eq!(config.render_modes.len(), 3);
        assert!(!config.verbose);
    }

    #[test]
    fn test_config_minimal() {
        let config = SnapshotConfig::minimal();
        assert_eq!(config.render_modes.len(), 1);
        assert_eq!(config.render_modes[0], RenderMode::Semantic);
    }

    #[test]
    fn test_config_verbose() {
        let config = SnapshotConfig::verbose();
        assert!(config.verbose);
        assert_eq!(config.render_modes.len(), 3);
    }

    #[test]
    fn test_render_all_default() {
        let snapshot = make_snapshot_with_spans();
        let config = SnapshotConfig::default();
        let output = snapshot.render_all(&config);

        assert!(output.contains("═══ SEMANTIC SUMMARY ═══"), "Should have semantic header");
        assert!(output.contains("═══ ANNOTATED TEXT ═══"), "Should have annotated header");
        assert!(output.contains("═══ ASSOCIATION GRAPH ═══"), "Should have graph header");
    }

    #[test]
    fn test_render_all_minimal() {
        let snapshot = make_snapshot_with_spans();
        let config = SnapshotConfig::minimal();
        let output = snapshot.render_all(&config);

        assert!(output.contains("═══ SEMANTIC SUMMARY ═══"));
        assert!(!output.contains("═══ ANNOTATED TEXT ═══"));
        assert!(!output.contains("═══ ASSOCIATION GRAPH ═══"));
    }

    #[test]
    fn test_render_all_type_filtering() {
        let snapshot = make_snapshot_with_spans();
        let config = SnapshotConfig::default().with_types(&["SectionHeader"]);
        let output = snapshot.render_all(&config);

        assert!(output.contains("[sh-0]"), "Should include SectionHeader");
        // DefinedTerm might still appear in semantic if we don't filter at snapshot level
        // The semantic renderer uses the filtered snapshot
    }

    #[test]
    fn test_render_all_empty_snapshot() {
        let snapshot = Snapshot::new();
        let config = SnapshotConfig::default();
        let output = snapshot.render_all(&config);

        assert!(output.contains("(no spans)"));
        assert!(output.contains("(no text)"));
        assert!(output.contains("(no associations)"));
    }

    #[test]
    fn test_filter_types() {
        let snapshot = make_snapshot_with_spans();
        let mut included = HashSet::new();
        included.insert("SectionHeader".to_string());

        let filtered = snapshot.filter_types(&included);

        assert!(filtered.spans.contains_key("SectionHeader"));
        assert!(!filtered.spans.contains_key("DefinedTerm"));
    }
}

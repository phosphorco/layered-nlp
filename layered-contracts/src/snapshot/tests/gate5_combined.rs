//! Gate 5 tests: Combined View and Macro Enhancement.
//!
//! Test scenarios from FR-010:
//! 1. Combined output — `_view` contains semantic, annotated, and graph sections
//! 2. Config filtering — Only specified types appear when filtered
//! 3. Minimal mode — Only semantic summary in output
//! 4. Verbose mode — Full confidence and source details
//! 5. Section navigation — Headers make sections easy to find
//! 6. Default behavior — No config produces reasonable default output

use crate::snapshot::{
    AssociationData, RenderMode, Snapshot, SnapshotConfig, SnapshotDocPos, SnapshotDocSpan,
    SnapshotSpanId, SpanData,
};

fn make_span(
    prefix: &str,
    idx: usize,
    line: u32,
    type_name: &str,
    value: &str,
    confidence: Option<f64>,
) -> SpanData {
    SpanData {
        id: SnapshotSpanId::new(prefix, idx),
        position: SnapshotDocSpan::new(SnapshotDocPos::new(line, 0), SnapshotDocPos::new(line, 5)),
        type_name: type_name.to_string(),
        value: ron::Value::String(value.to_string()),
        confidence,
        source: None,
        associations: vec![],
    }
}

fn make_test_snapshot() -> Snapshot {
    let mut snapshot = Snapshot::with_inline_input(vec![
        "Section 1.1 Definitions".to_string(),
        "\"Company\" means ABC Corp.".to_string(),
        "The Company shall deliver.".to_string(),
    ]);

    snapshot.spans.insert(
        "SectionHeader".to_string(),
        vec![make_span("sh", 0, 0, "SectionHeader", "1.1", None)],
    );

    snapshot.spans.insert(
        "DefinedTerm".to_string(),
        vec![make_span("dt", 0, 1, "DefinedTerm", "Company", Some(0.95))],
    );

    let mut tr_span = make_span("tr", 0, 2, "TermReference", "Company", Some(0.85));
    tr_span.associations.push(AssociationData {
        label: "resolves_to".to_string(),
        target: SnapshotSpanId::new("dt", 0),
        glyph: None,
    });
    snapshot.spans.insert("TermReference".to_string(), vec![tr_span]);

    snapshot
}

#[test]
fn test_combined_output_has_all_sections() {
    let snapshot = make_test_snapshot();
    let config = SnapshotConfig::default();
    let output = snapshot.render_all(&config);

    assert!(
        output.contains("═══ SEMANTIC SUMMARY ═══"),
        "Should have semantic header: {}",
        output
    );
    assert!(
        output.contains("═══ ANNOTATED TEXT ═══"),
        "Should have annotated header: {}",
        output
    );
    assert!(
        output.contains("═══ ASSOCIATION GRAPH ═══"),
        "Should have graph header: {}",
        output
    );
}

#[test]
fn test_combined_semantic_content() {
    let snapshot = make_test_snapshot();
    let config = SnapshotConfig::default();
    let output = snapshot.render_all(&config);

    // Should contain semantic information
    assert!(output.contains("DEFINITIONS"), "Should have category: {}", output);
    assert!(output.contains("STRUCTURE"), "Should have structure: {}", output);
    assert!(output.contains("[sh-0]"), "Should have section ID: {}", output);
    assert!(output.contains("[dt-0]"), "Should have term ID: {}", output);
}

#[test]
fn test_combined_annotated_content() {
    let snapshot = make_test_snapshot();
    let config = SnapshotConfig::default();
    let output = snapshot.render_all(&config);

    // Should contain the original text
    assert!(output.contains("Section 1.1 Definitions"), "Should have line 1: {}", output);
    assert!(
        output.contains("\"Company\" means ABC Corp."),
        "Should have line 2: {}",
        output
    );
}

#[test]
fn test_combined_graph_content() {
    let snapshot = make_test_snapshot();
    let config = SnapshotConfig::default();
    let output = snapshot.render_all(&config);

    // Should contain graph information (TermReference has an association)
    assert!(
        output.contains("resolves_to→[dt-0]"),
        "Should have association: {}",
        output
    );
}

#[test]
fn test_config_type_filtering() {
    let snapshot = make_test_snapshot();
    let config = SnapshotConfig::default().with_types(&["SectionHeader"]);
    let output = snapshot.render_all(&config);

    // Should include SectionHeader
    assert!(output.contains("[sh-0]"), "Should include SectionHeader: {}", output);

    // Semantic and graph views use filtered snapshot - should NOT contain filtered types
    assert!(
        !output.contains("DEFINITIONS"),
        "Should not have Definitions category: {}",
        output
    );
    assert!(
        !output.contains("[dt-0]"),
        "Should not include DefinedTerm ID when filtering to SectionHeader: {}",
        output
    );
    assert!(
        !output.contains("resolves_to→[dt-0]"),
        "Should not include associations to filtered types: {}",
        output
    );
}

#[test]
fn test_config_minimal_mode() {
    let snapshot = make_test_snapshot();
    let config = SnapshotConfig::minimal();
    let output = snapshot.render_all(&config);

    // Should have semantic only
    assert!(output.contains("═══ SEMANTIC SUMMARY ═══"));
    assert!(!output.contains("═══ ANNOTATED TEXT ═══"));
    assert!(!output.contains("═══ ASSOCIATION GRAPH ═══"));
}

#[test]
fn test_config_verbose_mode() {
    let snapshot = make_test_snapshot();
    let config = SnapshotConfig::verbose();
    let output = snapshot.render_all(&config);

    // Verbose mode should show confidence even when high
    // The DefinedTerm has 0.95 confidence which is normally hidden
    // In verbose mode, threshold is 1.0 so all confidences shown
    assert!(output.contains("0.95") || output.contains("0.85"), "Should show confidence: {}", output);
}

#[test]
fn test_section_headers_navigation() {
    let snapshot = make_test_snapshot();
    let config = SnapshotConfig::default();
    let output = snapshot.render_all(&config);

    // Headers should be clearly separable
    let semantic_pos = output.find("═══ SEMANTIC SUMMARY ═══");
    let annotated_pos = output.find("═══ ANNOTATED TEXT ═══");
    let graph_pos = output.find("═══ ASSOCIATION GRAPH ═══");

    assert!(semantic_pos.is_some());
    assert!(annotated_pos.is_some());
    assert!(graph_pos.is_some());

    // Order should be: semantic < annotated < graph
    assert!(semantic_pos < annotated_pos);
    assert!(annotated_pos < graph_pos);
}

#[test]
fn test_default_config_reasonable() {
    let config = SnapshotConfig::default();

    assert!(config.included_types.is_empty(), "Default includes all types");
    assert_eq!(config.render_modes.len(), 3, "Default has all modes");
    assert!(!config.verbose, "Default is not verbose");
    assert!(config.show_line_numbers, "Default shows line numbers");
    assert!(config.show_reverse_associations, "Default shows reverse associations");
}

#[test]
fn test_empty_snapshot_combined() {
    let snapshot = Snapshot::new();
    let config = SnapshotConfig::default();
    let output = snapshot.render_all(&config);

    assert!(output.contains("(no spans)"), "Should indicate no spans");
    assert!(output.contains("(no text)"), "Should indicate no text");
    assert!(output.contains("(no associations)"), "Should indicate no associations");
}

#[test]
fn test_config_custom_modes() {
    let snapshot = make_test_snapshot();
    let config = SnapshotConfig::default().with_modes(vec![RenderMode::Semantic, RenderMode::Graph]);
    let output = snapshot.render_all(&config);

    assert!(output.contains("═══ SEMANTIC SUMMARY ═══"));
    assert!(!output.contains("═══ ANNOTATED TEXT ═══"));
    assert!(output.contains("═══ ASSOCIATION GRAPH ═══"));
}

#[test]
fn test_config_builder_chaining() {
    let config = SnapshotConfig::new()
        .with_types(&["SectionHeader", "DefinedTerm"])
        .with_verbose(true)
        .with_max_spans(50)
        .with_line_numbers(false);

    assert_eq!(config.included_types.len(), 2);
    assert!(config.verbose);
    assert_eq!(config.max_spans_per_group, 50);
    assert!(!config.show_line_numbers);
}

#[test]
fn test_combined_determinism() {
    let snapshot = make_test_snapshot();
    let config = SnapshotConfig::default();

    let output1 = snapshot.render_all(&config);
    let output2 = snapshot.render_all(&config);
    let output3 = snapshot.render_all(&config);

    assert_eq!(output1, output2);
    assert_eq!(output2, output3);
}

#[test]
fn test_config_max_spans() {
    let mut snapshot = Snapshot::with_inline_input(vec!["Test".to_string()]);

    // Add many spans
    let mut spans = Vec::new();
    for i in 0..30 {
        spans.push(make_span("sh", i, 0, "SectionHeader", &format!("Section {}", i), None));
    }
    snapshot.spans.insert("SectionHeader".to_string(), spans);

    let config = SnapshotConfig::default().with_max_spans(10);
    let output = snapshot.render_all(&config);

    // Should show elision message
    assert!(
        output.contains("elided") || output.contains("more"),
        "Should show elision for many spans: {}",
        output
    );
}

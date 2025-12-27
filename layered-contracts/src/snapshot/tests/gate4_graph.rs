//! Gate 4 tests: Association Graph View.
//!
//! Test scenarios from FR-010:
//! 1. Simple chain — `TermReference → DefinedTerm` shows correctly
//! 2. Multiple outgoing — Span with 3 associations shows all
//! 3. Reverse references — Target span shows incoming `↑` indicator
//! 4. No associations — Span with no associations omitted from graph
//! 5. Circular references — Handles without infinite loop
//! 6. Ordering — Consistent ordering across runs

use crate::snapshot::{
    AssociationData, GraphRenderer, Snapshot, SnapshotDocPos, SnapshotDocSpan, SnapshotSpanId,
    SpanData,
};

fn make_span(prefix: &str, idx: usize, line: u32, type_name: &str, value: &str) -> SpanData {
    SpanData {
        id: SnapshotSpanId::new(prefix, idx),
        position: SnapshotDocSpan::new(SnapshotDocPos::new(line, 0), SnapshotDocPos::new(line, 5)),
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
fn test_graph_spans_without_associations() {
    let mut snapshot = Snapshot::new();
    snapshot
        .spans
        .insert("DefinedTerm".to_string(), vec![make_span("dt", 0, 0, "DefinedTerm", "Company")]);
    snapshot.spans.insert(
        "SectionHeader".to_string(),
        vec![make_span("sh", 0, 1, "SectionHeader", "1.1")],
    );

    let output = snapshot.render_graph();
    assert!(output.is_empty(), "No associations = no graph output");
}

#[test]
fn test_graph_simple_chain() {
    let mut snapshot = Snapshot::new();

    // DefinedTerm
    snapshot
        .spans
        .insert("DefinedTerm".to_string(), vec![make_span("dt", 0, 0, "DefinedTerm", "Company")]);

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
    assert!(
        output.contains("↑ resolves_to from [tr-0]"),
        "Should show reverse ref: {}",
        output
    );
}

#[test]
fn test_graph_multiple_outgoing_associations() {
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

    // Should show both associations from obligation
    assert!(output.contains("@obligor_source→[dt-0]"), "Should show obligor: {}", output);
    assert!(output.contains("object→[dt-1]"), "Should show object: {}", output);
}

#[test]
fn test_graph_reverse_references() {
    let mut snapshot = Snapshot::new();

    // One defined term referenced by multiple spans
    snapshot
        .spans
        .insert("DefinedTerm".to_string(), vec![make_span("dt", 0, 0, "DefinedTerm", "Company")]);

    // Two term references pointing to the same term
    let mut tr0 = make_span("tr", 0, 1, "TermReference", "Company");
    tr0.associations.push(AssociationData {
        label: "resolves_to".to_string(),
        target: SnapshotSpanId::new("dt", 0),
        glyph: None,
    });

    let mut tr1 = make_span("tr", 1, 2, "TermReference", "the Company");
    tr1.associations.push(AssociationData {
        label: "resolves_to".to_string(),
        target: SnapshotSpanId::new("dt", 0),
        glyph: None,
    });

    snapshot.spans.insert("TermReference".to_string(), vec![tr0, tr1]);

    let output = snapshot.render_graph();

    // DefinedTerm should show two incoming references
    assert!(
        output.contains("↑ resolves_to from [tr-0]"),
        "Should show ref from tr-0: {}",
        output
    );
    assert!(
        output.contains("↑ resolves_to from [tr-1]"),
        "Should show ref from tr-1: {}",
        output
    );
}

#[test]
fn test_graph_circular_references() {
    let mut snapshot = Snapshot::new();

    // Two spans that reference each other
    let mut span_a = make_span("dt", 0, 0, "DefinedTerm", "A");
    span_a.associations.push(AssociationData {
        label: "see_also".to_string(),
        target: SnapshotSpanId::new("dt", 1),
        glyph: None,
    });

    let mut span_b = make_span("dt", 1, 1, "DefinedTerm", "B");
    span_b.associations.push(AssociationData {
        label: "see_also".to_string(),
        target: SnapshotSpanId::new("dt", 0),
        glyph: None,
    });

    snapshot.spans.insert("DefinedTerm".to_string(), vec![span_a, span_b]);

    // Should not panic or infinite loop
    let output = snapshot.render_graph();

    // Both spans should appear
    assert!(output.contains("[dt-0]"), "Should show dt-0: {}", output);
    assert!(output.contains("[dt-1]"), "Should show dt-1: {}", output);

    // Both forward associations should appear
    assert!(output.contains("see_also→[dt-1]"), "Should show A→B: {}", output);
    assert!(output.contains("see_also→[dt-0]"), "Should show B→A: {}", output);
}

#[test]
fn test_graph_ordering_determinism() {
    let mut snapshot = Snapshot::new();

    // Multiple spans and associations
    snapshot.spans.insert(
        "DefinedTerm".to_string(),
        vec![
            make_span("dt", 0, 0, "DefinedTerm", "Alpha"),
            make_span("dt", 1, 1, "DefinedTerm", "Beta"),
            make_span("dt", 2, 2, "DefinedTerm", "Gamma"),
        ],
    );

    let mut tr_span = make_span("tr", 0, 3, "TermReference", "Alpha");
    tr_span.associations.push(AssociationData {
        label: "resolves_to".to_string(),
        target: SnapshotSpanId::new("dt", 0),
        glyph: None,
    });
    snapshot.spans.insert("TermReference".to_string(), vec![tr_span]);

    // Render multiple times
    let output1 = snapshot.render_graph();
    let output2 = snapshot.render_graph();
    let output3 = snapshot.render_graph();

    assert_eq!(output1, output2, "Should be deterministic");
    assert_eq!(output2, output3, "Should be deterministic");
}

#[test]
fn test_graph_category_headers() {
    let mut snapshot = Snapshot::new();

    // Spans from different categories
    snapshot
        .spans
        .insert("DefinedTerm".to_string(), vec![make_span("dt", 0, 0, "DefinedTerm", "Company")]);

    let mut tr_span = make_span("tr", 0, 1, "TermReference", "Company");
    tr_span.associations.push(AssociationData {
        label: "resolves_to".to_string(),
        target: SnapshotSpanId::new("dt", 0),
        glyph: None,
    });
    snapshot.spans.insert("TermReference".to_string(), vec![tr_span]);

    let output = snapshot.render_graph();

    // Should have category headers
    assert!(
        output.contains("DEFINITIONS ASSOCIATIONS"),
        "Should have Definitions header: {}",
        output
    );
    assert!(
        output.contains("REFERENCES ASSOCIATIONS"),
        "Should have References header: {}",
        output
    );
}

#[test]
fn test_graph_elision() {
    let mut snapshot = Snapshot::new();

    // Create many defined terms
    let mut terms = Vec::new();
    for i in 0..60 {
        terms.push(make_span("dt", i, i as u32, "DefinedTerm", &format!("Term{}", i)));
    }
    snapshot.spans.insert("DefinedTerm".to_string(), terms);

    // Add one reference that points to dt-0
    let mut tr_span = make_span("tr", 0, 100, "TermReference", "Term0");
    tr_span.associations.push(AssociationData {
        label: "resolves_to".to_string(),
        target: SnapshotSpanId::new("dt", 0),
        glyph: None,
    });
    snapshot.spans.insert("TermReference".to_string(), vec![tr_span]);

    // With default max 50 per category, definitions should be elided
    // But wait - we only have 1 DefinedTerm with associations (dt-0 via incoming)
    // Actually, all 60 terms have no outgoing associations, and only dt-0 has incoming
    // So only dt-0 should appear in the graph, not all 60

    let output = snapshot.render_graph();

    // Only dt-0 should appear (it has an incoming reference)
    assert!(output.contains("[dt-0]"), "Should show dt-0: {}", output);
    // dt-59 has no associations, should not appear
    assert!(!output.contains("[dt-59]"), "Should not show dt-59 (no associations): {}", output);
}

#[test]
fn test_graph_glyph_display() {
    let mut snapshot = Snapshot::new();

    snapshot
        .spans
        .insert("DefinedTerm".to_string(), vec![make_span("dt", 0, 0, "DefinedTerm", "Company")]);

    let mut ob_span = make_span("ob", 0, 1, "ObligationPhrase", "shall");
    ob_span.associations.push(AssociationData {
        label: "obligor".to_string(),
        target: SnapshotSpanId::new("dt", 0),
        glyph: Some("@".to_string()),
    });
    snapshot.spans.insert("ObligationPhrase".to_string(), vec![ob_span]);

    let output = snapshot.render_graph();

    // Should show the glyph before the label
    assert!(output.contains("@obligor→"), "Should show glyph: {}", output);
}

#[test]
fn test_graph_render_convenience() {
    let mut snapshot = Snapshot::new();

    snapshot
        .spans
        .insert("DefinedTerm".to_string(), vec![make_span("dt", 0, 0, "DefinedTerm", "Test")]);

    let mut tr_span = make_span("tr", 0, 1, "TermReference", "Test");
    tr_span.associations.push(AssociationData {
        label: "resolves_to".to_string(),
        target: SnapshotSpanId::new("dt", 0),
        glyph: None,
    });
    snapshot.spans.insert("TermReference".to_string(), vec![tr_span]);

    // Convenience method should work
    let output = snapshot.render_graph();
    assert!(!output.is_empty());
    assert!(output.contains("[tr-0]"));
}

#[test]
fn test_graph_elision_message() {
    let mut snapshot = Snapshot::new();

    // Create many spans that ALL have associations (so they're all relevant)
    let mut spans = Vec::new();
    for i in 0..60 {
        let mut s = make_span("dt", i, i as u32, "DefinedTerm", &format!("Term{}", i));
        // Give each one an outgoing association to the next one
        s.associations.push(AssociationData {
            label: "next".to_string(),
            target: SnapshotSpanId::new("dt", (i + 1) % 60),
            glyph: None,
        });
        spans.push(s);
    }
    snapshot.spans.insert("DefinedTerm".to_string(), spans);

    // With max 10 per category, should show elision message
    let renderer = GraphRenderer::new().with_max_spans(10);
    let output = renderer.render_graph(&snapshot);

    assert!(
        output.contains("... 50 more spans elided"),
        "Should show elision message: {}",
        output
    );
}

#[test]
fn test_graph_self_reference() {
    let mut snapshot = Snapshot::new();

    let mut span = make_span("dt", 0, 0, "DefinedTerm", "Self");
    span.associations.push(AssociationData {
        label: "self_ref".to_string(),
        target: SnapshotSpanId::new("dt", 0),
        glyph: None,
    });
    snapshot.spans.insert("DefinedTerm".to_string(), vec![span]);

    let output = snapshot.render_graph();

    // Should show both outgoing and incoming for self-reference
    assert!(output.contains("self_ref→[dt-0]"), "Should show outgoing: {}", output);
    assert!(
        output.contains("↑ self_ref from [dt-0]"),
        "Should show incoming: {}",
        output
    );
}

#[test]
fn test_graph_renderer_configuration() {
    let mut snapshot = Snapshot::new();

    snapshot
        .spans
        .insert("DefinedTerm".to_string(), vec![make_span("dt", 0, 0, "DefinedTerm", "Company")]);

    let mut tr_span = make_span("tr", 0, 1, "TermReference", "Company");
    tr_span.associations.push(AssociationData {
        label: "resolves_to".to_string(),
        target: SnapshotSpanId::new("dt", 0),
        glyph: None,
    });
    snapshot.spans.insert("TermReference".to_string(), vec![tr_span]);

    // Without reverse associations
    let renderer = GraphRenderer::new().with_reverse_associations(false);
    let output = renderer.render_graph(&snapshot);

    assert!(!output.contains("↑"), "Should not have reverse refs: {}", output);
    assert!(output.contains("resolves_to→[dt-0]"), "Should have outgoing: {}", output);
}

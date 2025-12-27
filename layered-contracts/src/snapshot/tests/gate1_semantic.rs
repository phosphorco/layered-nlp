//! Gate 1 tests: Semantic summary rendering.
//!
//! Test scenarios from FR-010:
//! 1. Category grouping — Spans appear under correct category headings
//! 2. Ordering — Categories in consistent order, spans sorted by position
//! 3. Elision — Large snapshots (50+ spans of one type) show elision message
//! 4. Confidence display — Low confidence shown, high confidence hidden
//! 5. Association display — References show target IDs (deferred until Gate 4)
//! 6. Empty categories — Categories with no spans are omitted
//! 7. Determinism — Same snapshot produces identical render output

use crate::document::ContractDocument;
use crate::snapshot::{
    classify_type_name, SemanticCategory, Snapshot, SnapshotBuilder, SnapshotRenderer,
};
use crate::contract_keyword::ContractKeywordResolver;
use crate::defined_term::DefinedTermResolver;
use crate::section_header::SectionHeaderResolver;

#[test]
fn test_render_semantic_category_grouping() {
    let text = r#"
Section 1.1 Definitions
"Company" means ABC Corp.
The Company shall deliver goods.
    "#;

    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new())
        .run_resolver(&ContractKeywordResolver::default())
        .run_resolver(&DefinedTermResolver::new());

    let snapshot = SnapshotBuilder::new(&doc).with_standard_types().build();
    let renderer = SnapshotRenderer::new();
    let output = renderer.render_semantic(&snapshot);

    // Check category headers are present
    assert!(output.contains("📖 DEFINITIONS"), "Should have Definitions section");
    assert!(output.contains("⚖️ OBLIGATIONS"), "Should have Obligations section");
    assert!(output.contains("🏗️ STRUCTURE"), "Should have Structure section");
}

#[test]
fn test_render_semantic_category_ordering() {
    let text = r#"
Section 1.1 Definitions
"Company" means ABC Corp.
The Company shall deliver goods within 30 days.
    "#;

    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new())
        .run_resolver(&ContractKeywordResolver::default())
        .run_resolver(&DefinedTermResolver::new());

    let snapshot = SnapshotBuilder::new(&doc).with_standard_types().build();
    let renderer = SnapshotRenderer::new();
    let output = renderer.render_semantic(&snapshot);

    // Categories should appear in defined order
    let def_pos = output.find("DEFINITIONS").expect("DEFINITIONS should be present");
    let obl_pos = output.find("OBLIGATIONS").expect("OBLIGATIONS should be present");
    let str_pos = output.find("STRUCTURE").expect("STRUCTURE should be present");

    // Order: Definitions < References < Obligations < Structure
    assert!(def_pos < obl_pos, "DEFINITIONS should come before OBLIGATIONS");
    assert!(obl_pos < str_pos, "OBLIGATIONS should come before STRUCTURE");
}

#[test]
fn test_render_semantic_elision() {
    // Create a snapshot with many spans manually to test elision
    let mut snapshot = Snapshot::new();
    
    // Add 30 SectionHeader spans
    let mut section_spans = Vec::new();
    for i in 0..30 {
        section_spans.push(crate::snapshot::SpanData {
            id: crate::snapshot::SnapshotSpanId::new("sh", i),
            position: crate::snapshot::SnapshotDocSpan::new(
                crate::snapshot::SnapshotDocPos::new(i as u32, 0),
                crate::snapshot::SnapshotDocPos::new(i as u32, 5),
            ),
            type_name: "SectionHeader".to_string(),
            value: ron::Value::String(format!("Section {}", i)),
            confidence: None,
            source: None,
            associations: vec![],
        });
    }
    snapshot.spans.insert("SectionHeader".to_string(), section_spans);

    // Render with max 10 spans per type
    let renderer = SnapshotRenderer::new().with_max_spans(10);
    let output = renderer.render_semantic(&snapshot);

    // Should show elision message
    assert!(
        output.contains("20 more spans elided"),
        "Should show elision message: {}",
        output
    );
    assert!(output.contains(".ron snapshot"), "Should reference RON snapshot");
}

#[test]
fn test_render_semantic_confidence_display() {
    let text = r#""Company" means ABC Corp."#;

    let doc = ContractDocument::from_text(text)
        .run_resolver(&ContractKeywordResolver::default())
        .run_resolver(&DefinedTermResolver::new());

    let snapshot = SnapshotBuilder::new(&doc)
        .with_scored_line_type::<crate::defined_term::DefinedTerm>()
        .build();

    // Render with threshold 1.0 so everything is considered "low confidence"
    let renderer = SnapshotRenderer::new().with_confidence_threshold(1.0);
    let output = renderer.render_semantic(&snapshot);

    // Should show confidence for the defined term (which has conf < 1.0)
    assert!(output.contains("conf="), "Should show confidence: {}", output);
}

#[test]
fn test_render_semantic_high_confidence_hidden() {
    let text = r#""Company" means ABC Corp."#;

    let doc = ContractDocument::from_text(text)
        .run_resolver(&ContractKeywordResolver::default())
        .run_resolver(&DefinedTermResolver::new());

    let snapshot = SnapshotBuilder::new(&doc)
        .with_scored_line_type::<crate::defined_term::DefinedTerm>()
        .build();

    // Render with threshold 0.5 so 0.95 confidence is "high" and hidden
    let renderer = SnapshotRenderer::new().with_confidence_threshold(0.5);
    let output = renderer.render_semantic(&snapshot);

    // Should NOT show confidence for the defined term (which has conf = 0.95)
    assert!(!output.contains("conf="), "Should hide high confidence: {}", output);
}

#[test]
fn test_render_semantic_empty_categories_omitted() {
    // Create a document with only section headers
    let text = "Section 1.1 Test";
    
    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new());

    let snapshot = SnapshotBuilder::new(&doc)
        .with_line_type::<crate::section_header::SectionHeader>()
        .build();

    let renderer = SnapshotRenderer::new();
    let output = renderer.render_semantic(&snapshot);

    // Should have Structure category
    assert!(output.contains("STRUCTURE"));

    // Should NOT have empty categories
    assert!(!output.contains("DEFINITIONS"));
    assert!(!output.contains("REFERENCES"));
    assert!(!output.contains("OBLIGATIONS"));
    assert!(!output.contains("TEMPORAL"));
}

#[test]
fn test_render_semantic_determinism() {
    let text = r#"
Section 1.1 Definitions
"Company" means ABC Corp.
The Company shall deliver goods.
    "#;

    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new())
        .run_resolver(&ContractKeywordResolver::default())
        .run_resolver(&DefinedTermResolver::new());

    let snapshot = SnapshotBuilder::new(&doc).with_standard_types().build();
    let renderer = SnapshotRenderer::new();

    // Render multiple times
    let output1 = renderer.render_semantic(&snapshot);
    let output2 = renderer.render_semantic(&snapshot);
    let output3 = renderer.render_semantic(&snapshot);

    // All should be identical
    assert_eq!(output1, output2);
    assert_eq!(output2, output3);
}

#[test]
fn test_render_semantic_empty_snapshot() {
    let snapshot = Snapshot::new();
    let renderer = SnapshotRenderer::new();
    let output = renderer.render_semantic(&snapshot);

    assert!(output.is_empty(), "Empty snapshot should produce empty output");
}

#[test]
fn test_render_semantic_span_ids_shown() {
    let text = "Section 1.1 Test";
    
    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new());

    let snapshot = SnapshotBuilder::new(&doc)
        .with_line_type::<crate::section_header::SectionHeader>()
        .build();

    let renderer = SnapshotRenderer::new();
    let output = renderer.render_semantic(&snapshot);

    // Should show span IDs
    assert!(output.contains("[sh-0]"), "Should show span ID: {}", output);
}

#[test]
fn test_render_semantic_unicode_value() {
    // Test that Unicode text doesn't cause panics in summarize_value
    let mut snapshot = Snapshot::new();
    
    // Add a span with Unicode content
    snapshot.spans.insert("DefinedTerm".to_string(), vec![
        crate::snapshot::SpanData {
            id: crate::snapshot::SnapshotSpanId::new("dt", 0),
            position: crate::snapshot::SnapshotDocSpan::new(
                crate::snapshot::SnapshotDocPos::new(0, 0),
                crate::snapshot::SnapshotDocPos::new(0, 5),
            ),
            type_name: "DefinedTerm".to_string(),
            // Long string with multi-byte Unicode chars that would panic with byte slicing
            // This string has 70+ characters to exceed MAX_LEN of 60
            value: ron::Value::String(
                "日本語テストテキストこれは非常に長い文字列で、60文字を超えています。切り詰めが正しく行われる必要があります。追加テキスト。".to_string()
            ),
            confidence: None,
            source: None,
            associations: vec![],
        },
    ]);

    let renderer = SnapshotRenderer::new();
    // This should NOT panic
    let output = renderer.render_semantic(&snapshot);
    
    // Should contain truncation indicator
    assert!(output.contains("..."), "Long Unicode string should be truncated");
    // Output should be valid UTF-8 (implicit from being a String)
    assert!(output.contains("[dt-0]"));
}

#[test]
fn test_render_semantic_multi_type_elision_in_category() {
    // Test that per-type elision doesn't hide entire types
    let mut snapshot = Snapshot::new();
    
    // Add 15 SectionHeaders and 15 RecitalSections (both in Structure category)
    let mut sections = Vec::new();
    let mut recitals = Vec::new();
    
    for i in 0..15 {
        sections.push(crate::snapshot::SpanData {
            id: crate::snapshot::SnapshotSpanId::new("sh", i),
            position: crate::snapshot::SnapshotDocSpan::new(
                crate::snapshot::SnapshotDocPos::new(i as u32 * 2, 0),
                crate::snapshot::SnapshotDocPos::new(i as u32 * 2, 5),
            ),
            type_name: "SectionHeader".to_string(),
            value: ron::Value::String(format!("Section {}", i)),
            confidence: None,
            source: None,
            associations: vec![],
        });
        
        recitals.push(crate::snapshot::SpanData {
            id: crate::snapshot::SnapshotSpanId::new("rc", i),
            position: crate::snapshot::SnapshotDocSpan::new(
                crate::snapshot::SnapshotDocPos::new(i as u32 * 2 + 1, 0),
                crate::snapshot::SnapshotDocPos::new(i as u32 * 2 + 1, 5),
            ),
            type_name: "RecitalSection".to_string(),
            value: ron::Value::String(format!("Recital {}", i)),
            confidence: None,
            source: None,
            associations: vec![],
        });
    }
    
    snapshot.spans.insert("SectionHeader".to_string(), sections);
    snapshot.spans.insert("RecitalSection".to_string(), recitals);

    // With max 10 per type, we should see 10 of each, not 10 total
    let renderer = SnapshotRenderer::new().with_max_spans(10);
    let output = renderer.render_semantic(&snapshot);

    // Should see spans from BOTH types
    assert!(output.contains("[sh-0]"), "Should show first SectionHeader");
    assert!(output.contains("[rc-0]"), "Should show first RecitalSection");
    
    // Should have elision (30 total, 20 shown)
    assert!(output.contains("elided"), "Should show elision message");
}

#[test]
fn test_renderer_show_confidence_toggle() {
    let r = SnapshotRenderer::new().with_show_confidence(false);
    assert!(!r.show_confidence);
    
    let r2 = SnapshotRenderer::new().with_show_confidence(true);
    assert!(r2.show_confidence);
}

#[test]
fn test_classify_all_known_types() {
    // Definitions
    assert_eq!(classify_type_name("DefinedTerm"), SemanticCategory::Definitions);

    // References
    assert_eq!(classify_type_name("TermReference"), SemanticCategory::References);
    assert_eq!(classify_type_name("PronounReference"), SemanticCategory::References);
    assert_eq!(classify_type_name("BridgingReference"), SemanticCategory::References);
    assert_eq!(classify_type_name("Coordination"), SemanticCategory::References);

    // Obligations
    assert_eq!(classify_type_name("ObligationPhrase"), SemanticCategory::Obligations);
    assert_eq!(classify_type_name("ContractKeyword"), SemanticCategory::Obligations);

    // Structure
    assert_eq!(classify_type_name("SectionHeader"), SemanticCategory::Structure);
    assert_eq!(classify_type_name("RecitalSection"), SemanticCategory::Structure);
    assert_eq!(classify_type_name("AppendixBoundary"), SemanticCategory::Structure);
    assert_eq!(classify_type_name("FootnoteBlock"), SemanticCategory::Structure);
    assert_eq!(classify_type_name("PrecedenceRule"), SemanticCategory::Structure);

    // Temporal
    assert_eq!(classify_type_name("TemporalExpression"), SemanticCategory::Temporal);

    // Other
    assert_eq!(classify_type_name("SomeUnknownType"), SemanticCategory::Other);
}

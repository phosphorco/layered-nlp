//! Gate 3 tests: Visual ASCII Overlay — DocDisplay.
//!
//! Test scenarios from FR-010:
//! 1. Single-line span — Span within one line renders with underline markers
//! 2. Multi-line span — Span across 3 lines shows `╭`, `│`, `╰` correctly
//! 3. Overlapping spans — Two spans at same position stack vertically
//! 4. Nested spans — Inner span renders inside outer span visually
//! 5. Association arrows — `@obligor→[dt-0]` appears below obligation span
//! 6. Label assignment — Spans referenced by associations get `[A]`, `[B]` labels
//! 7. Type filtering — Only included types appear in output
//! 8. Line numbers — Line numbers appear in left margin
//! 9. Empty lines — Blank lines in input preserved
//! 10. Long lines — Lines > 80 chars handled gracefully

use crate::snapshot::{
    AssociationData, Snapshot, SnapshotDocPos, SnapshotDocSpan, SnapshotSpanId, SpanData,
};
use crate::snapshot::display::DocDisplay;

fn make_span(
    prefix: &str,
    idx: usize,
    start_line: u32,
    start_token: u32,
    end_line: u32,
    end_token: u32,
    type_name: &str,
    value: &str,
) -> SpanData {
    SpanData {
        id: SnapshotSpanId::new(prefix, idx),
        position: SnapshotDocSpan::new(
            SnapshotDocPos::new(start_line, start_token),
            SnapshotDocPos::new(end_line, end_token),
        ),
        type_name: type_name.to_string(),
        value: ron::Value::String(value.to_string()),
        confidence: None,
        source: None,
        associations: vec![],
    }
}

#[test]
fn test_doc_display_single_line_span() {
    let mut snapshot = Snapshot::with_inline_input(vec![
        "Section 1.1 Definitions".to_string(),
    ]);
    
    snapshot.spans.insert("SectionHeader".to_string(), vec![
        make_span("sh", 0, 0, 0, 0, 3, "SectionHeader", "1.1"),
    ]);
    
    let output = format!("{}", DocDisplay::new(&snapshot));
    
    assert!(output.contains("Section 1.1 Definitions"), "Should contain original text");
    assert!(output.contains("[sh-0]"), "Should contain span ID");
    assert!(output.contains("╰"), "Should have underline start marker");
}

#[test]
fn test_doc_display_multi_line_span() {
    let mut snapshot = Snapshot::with_inline_input(vec![
        "WHEREAS the parties".to_string(),
        "have agreed to the".to_string(),
        "following terms.".to_string(),
    ]);
    
    snapshot.spans.insert("RecitalSection".to_string(), vec![
        make_span("rc", 0, 0, 0, 2, 2, "RecitalSection", "Recital"),
    ]);
    
    let output = format!("{}", DocDisplay::new(&snapshot));
    
    // Should contain all lines
    assert!(output.contains("WHEREAS the parties"), "Line 1");
    assert!(output.contains("have agreed to the"), "Line 2");
    assert!(output.contains("following terms."), "Line 3");
    
    // Should have multi-line span markers: ╭ at start, │ in middle, ╰ at end
    assert!(output.contains("╭"), "Should have start marker ╭: {}", output);
    assert!(output.contains("│"), "Should have continuation marker │: {}", output);
    assert!(output.contains("╰"), "Should have end marker ╰: {}", output);
    assert!(output.contains("[rc-0]"), "Should have span ID: {}", output);
}

#[test]
fn test_doc_display_overlapping_spans() {
    let mut snapshot = Snapshot::with_inline_input(vec![
        "The Company shall deliver".to_string(),
    ]);
    
    // Two spans that overlap at "Company"
    snapshot.spans.insert("TermReference".to_string(), vec![
        make_span("tr", 0, 0, 1, 0, 1, "TermReference", "Company"),
    ]);
    snapshot.spans.insert("ObligationPhrase".to_string(), vec![
        make_span("ob", 0, 0, 1, 0, 4, "ObligationPhrase", "Company shall deliver"),
    ]);
    
    let output = format!("{}", DocDisplay::new(&snapshot));
    
    // Both spans should appear
    assert!(output.contains("[tr-0]"), "Should show TermReference");
    assert!(output.contains("[ob-0]"), "Should show ObligationPhrase");
}

#[test]
fn test_doc_display_nested_spans() {
    let mut snapshot = Snapshot::with_inline_input(vec![
        "The Company shall deliver Products".to_string(),
    ]);
    
    // Outer span covers "Company shall deliver Products"
    // Inner span covers just "Company"
    snapshot.spans.insert("ObligationPhrase".to_string(), vec![
        make_span("ob", 0, 0, 1, 0, 4, "ObligationPhrase", "Company shall deliver Products"),
    ]);
    snapshot.spans.insert("TermReference".to_string(), vec![
        make_span("tr", 0, 0, 1, 0, 1, "TermReference", "Company"),
    ]);
    
    let output = format!("{}", DocDisplay::new(&snapshot));
    
    // Both should be present, outer first in rendering order
    assert!(output.contains("[ob-0]"));
    assert!(output.contains("[tr-0]"));
}

#[test]
fn test_doc_display_association_arrows() {
    let mut snapshot = Snapshot::with_inline_input(vec![
        "The Company shall deliver".to_string(),
    ]);
    
    // A term reference that's targeted
    snapshot.spans.insert("DefinedTerm".to_string(), vec![
        make_span("dt", 0, 0, 1, 0, 1, "DefinedTerm", "Company"),
    ]);
    
    // An obligation with an association to the term
    let mut ob_span = make_span("ob", 0, 0, 1, 0, 3, "ObligationPhrase", "shall deliver");
    ob_span.associations.push(AssociationData {
        label: "obligor_source".to_string(),
        target: SnapshotSpanId::new("dt", 0),
        glyph: Some("@".to_string()),
    });
    snapshot.spans.insert("ObligationPhrase".to_string(), vec![ob_span]);
    
    // Display with associations for ObligationPhrase
    let display = DocDisplay::new(&snapshot).include_with_associations("ObligationPhrase");
    let output = format!("{}", display);
    
    // Should show the association arrow
    assert!(output.contains("obligor_source") || output.contains("→"), 
        "Should show association: {}", output);
}

#[test]
fn test_doc_display_label_assignment() {
    let mut snapshot = Snapshot::with_inline_input(vec![
        "\"Company\" means ABC Corp.".to_string(),
        "The Company shall deliver.".to_string(),
    ]);
    
    // Defined term
    snapshot.spans.insert("DefinedTerm".to_string(), vec![
        make_span("dt", 0, 0, 0, 0, 1, "DefinedTerm", "Company"),
    ]);
    
    // Term reference with association to the defined term
    let mut tr_span = make_span("tr", 0, 1, 1, 1, 1, "TermReference", "Company");
    tr_span.associations.push(AssociationData {
        label: "resolves_to".to_string(),
        target: SnapshotSpanId::new("dt", 0),
        glyph: None,
    });
    snapshot.spans.insert("TermReference".to_string(), vec![tr_span]);
    
    // Include TermReference with associations
    let display = DocDisplay::new(&snapshot)
        .include("DefinedTerm")
        .include_with_associations("TermReference");
    let output = format!("{}", display);
    
    // The targeted span (dt-0) should get a label like [A]
    assert!(output.contains("[A]") || output.contains("[dt-0]"), 
        "Should have label for target span: {}", output);
}

#[test]
fn test_doc_display_type_filtering() {
    let mut snapshot = Snapshot::with_inline_input(vec![
        "Section 1.1 shall deliver".to_string(),
    ]);
    
    snapshot.spans.insert("SectionHeader".to_string(), vec![
        make_span("sh", 0, 0, 0, 0, 2, "SectionHeader", "1.1"),
    ]);
    snapshot.spans.insert("ContractKeyword".to_string(), vec![
        make_span("kw", 0, 0, 2, 0, 2, "ContractKeyword", "shall"),
    ]);
    
    // Only include SectionHeader
    let display = DocDisplay::new(&snapshot).include("SectionHeader");
    let output = format!("{}", display);
    
    assert!(output.contains("[sh-0]"), "Should include SectionHeader");
    assert!(!output.contains("[kw-0]"), "Should exclude ContractKeyword");
}

#[test]
fn test_doc_display_line_numbers() {
    let snapshot = Snapshot::with_inline_input(vec![
        "Line one".to_string(),
        "Line two".to_string(),
        "Line three".to_string(),
    ]);
    
    let output = format!("{}", DocDisplay::new(&snapshot));
    
    assert!(output.contains("1│") || output.contains("1|"), "Should have line 1");
    assert!(output.contains("2│") || output.contains("2|"), "Should have line 2");
    assert!(output.contains("3│") || output.contains("3|"), "Should have line 3");
}

#[test]
fn test_doc_display_without_line_numbers() {
    let snapshot = Snapshot::with_inline_input(vec![
        "Test line".to_string(),
    ]);
    
    let display = DocDisplay::new(&snapshot).without_line_numbers();
    let output = format!("{}", display);
    
    assert!(!output.contains("│"), "Should not have line number separator");
    assert!(output.contains("Test line"), "Should still have content");
}

#[test]
fn test_doc_display_long_lines() {
    let long_line = "A".repeat(120);
    let snapshot = Snapshot::with_inline_input(vec![long_line.clone()]);
    
    let output = format!("{}", DocDisplay::new(&snapshot));
    
    // Should contain the full line without truncation
    assert!(output.contains(&long_line), "Should handle long lines");
}

#[test]
fn test_doc_display_empty_snapshot() {
    let snapshot = Snapshot::new();
    let output = format!("{}", DocDisplay::new(&snapshot));
    
    assert!(output.is_empty() || output.trim().is_empty(), "Empty snapshot = empty output");
}

#[test]
fn test_doc_display_confidence_display() {
    let mut snapshot = Snapshot::with_inline_input(vec![
        "Test content".to_string(),
    ]);
    
    let mut span = make_span("dt", 0, 0, 0, 0, 1, "DefinedTerm", "Test");
    span.confidence = Some(0.65); // Low confidence
    snapshot.spans.insert("DefinedTerm".to_string(), vec![span]);
    
    let output = format!("{}", DocDisplay::new(&snapshot));
    
    // Low confidence should be shown by default
    assert!(output.contains("0.65") || output.contains("(0.65)"), 
        "Should show low confidence: {}", output);
}

#[test]
fn test_doc_display_verbose_mode() {
    let mut snapshot = Snapshot::with_inline_input(vec![
        "Test content".to_string(),
    ]);
    
    let mut span = make_span("dt", 0, 0, 0, 0, 1, "DefinedTerm", "Test");
    span.confidence = Some(0.95); // High confidence
    snapshot.spans.insert("DefinedTerm".to_string(), vec![span]);
    
    // Normal mode - high confidence hidden
    let _output_normal = format!("{}", DocDisplay::new(&snapshot));
    
    // Verbose mode - always show confidence
    let output_verbose = format!("{}", DocDisplay::new(&snapshot).verbose());
    
    // Verbose should show the confidence even when high
    assert!(output_verbose.contains("0.95"), "Verbose should show all confidence");
}

#[test]
fn test_doc_display_determinism() {
    let mut snapshot = Snapshot::with_inline_input(vec![
        "Section 1.1 Test".to_string(),
    ]);
    
    snapshot.spans.insert("SectionHeader".to_string(), vec![
        make_span("sh", 0, 0, 0, 0, 2, "SectionHeader", "1.1"),
    ]);
    
    let output1 = format!("{}", DocDisplay::new(&snapshot));
    let output2 = format!("{}", DocDisplay::new(&snapshot));
    let output3 = format!("{}", DocDisplay::new(&snapshot));
    
    assert_eq!(output1, output2, "Should be deterministic");
    assert_eq!(output2, output3, "Should be deterministic");
}

#[test]
fn test_render_annotated_convenience() {
    let mut snapshot = Snapshot::with_inline_input(vec![
        "Test line".to_string(),
    ]);
    
    snapshot.spans.insert("TestType".to_string(), vec![
        make_span("tt", 0, 0, 0, 0, 1, "TestType", "Test"),
    ]);
    
    let output = snapshot.render_annotated();
    
    assert!(output.contains("Test line"));
    assert!(output.contains("[tt-0]"));
}

#[test]
fn test_index_to_label() {
    use super::super::display::index_to_label;
    
    assert_eq!(index_to_label(0), "[A]");
    assert_eq!(index_to_label(1), "[B]");
    assert_eq!(index_to_label(25), "[Z]");
    assert_eq!(index_to_label(26), "[AA]");
    assert_eq!(index_to_label(27), "[AB]");
    assert_eq!(index_to_label(51), "[AZ]");
    assert_eq!(index_to_label(52), "[BA]");
}

#[test]
fn test_doc_display_label_only_for_included_targets() {
    let mut snapshot = Snapshot::with_inline_input(vec![
        "\"Company\" means ABC Corp.".to_string(),
        "The Company shall deliver.".to_string(),
    ]);
    
    // Defined term (will NOT be included)
    snapshot.spans.insert("DefinedTerm".to_string(), vec![
        make_span("dt", 0, 0, 0, 0, 1, "DefinedTerm", "Company"),
    ]);
    
    // Term reference with association to the defined term
    let mut tr_span = make_span("tr", 0, 1, 1, 1, 1, "TermReference", "Company");
    tr_span.associations.push(AssociationData {
        label: "resolves_to".to_string(),
        target: SnapshotSpanId::new("dt", 0),
        glyph: None,
    });
    snapshot.spans.insert("TermReference".to_string(), vec![tr_span]);
    
    // Only include TermReference with associations, NOT DefinedTerm
    let display = DocDisplay::new(&snapshot)
        .include_with_associations("TermReference");
    let output = format!("{}", display);
    
    // The association arrow should show the raw ID [dt-0], not a label like [A]
    // because DefinedTerm is not included in the display
    assert!(output.contains("→dt-0") || output.contains("→[dt-0]"), 
        "Should show raw target ID when target type not included: {}", output);
    assert!(!output.contains("[A]"), 
        "Should NOT have [A] label when target is not included: {}", output);
}

#[test]
fn test_doc_display_unicode_value_summary() {
    let mut snapshot = Snapshot::with_inline_input(vec![
        "日本語テスト".to_string(),
    ]);
    
    // Value with long Unicode string (> 30 chars)
    // "これは非常に長い日本語テキストで、三十文字を超えている長いテキストです。" = 35 chars
    let span = SpanData {
        id: SnapshotSpanId::new("dt", 0),
        position: SnapshotDocSpan::new(
            SnapshotDocPos::new(0, 0),
            SnapshotDocPos::new(0, 1),
        ),
        type_name: "DefinedTerm".to_string(),
        value: ron::Value::String(
            "これは非常に長い日本語テキストで、三十文字を超えている長いテキストです。".to_string()
        ),
        confidence: None,
        source: None,
        associations: vec![],
    };
    snapshot.spans.insert("DefinedTerm".to_string(), vec![span]);
    
    // Should not panic on Unicode
    let output = format!("{}", DocDisplay::new(&snapshot));
    assert!(output.contains("[dt-0]"));
    assert!(output.contains("..."), "Long text should be truncated: {}", output);
}

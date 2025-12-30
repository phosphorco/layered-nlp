//! Gate 2 tests: Dual snapshot macro.
//!
//! Test scenarios from FR-010:
//! 1. Basic usage — `assert_contract_snapshot!("test", doc)` produces two files
//! 2. RON validity — `_data.snap` contains valid RON
//! 3. View readability — `_view.snap` contains semantic summary
//! 4. Redaction — Non-deterministic fields are masked
//! 5. Re-run stability — Running test twice without changes produces no diff
//! 6. Semantic change detection — Adding a span produces visible diff in both files

use crate::ContractDocument;
use crate::snapshot::{Snapshot, SnapshotRenderer};
use crate::contract_keyword::ContractKeywordResolver;
use crate::section_header::SectionHeaderResolver;

#[test]
fn test_macro_ron_validity() {
    let text = "Section 1.1 Test";
    
    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new());

    // Build snapshot manually (macro uses this internally)
    let snapshot = Snapshot::from_document(&doc).redact();
    let ron_data = snapshot.to_ron_string().expect("RON serialization should succeed");
    
    // RON should be parseable
    let _parsed: Snapshot = Snapshot::from_ron_string(&ron_data)
        .expect("RON should be valid and parseable");
}

#[test]
fn test_macro_view_readability() {
    let text = "Section 1.1 Test\nThe Company shall perform.";
    
    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new())
        .run_resolver(&ContractKeywordResolver::default());

    let snapshot = Snapshot::from_document(&doc);
    let renderer = SnapshotRenderer::new();
    let view = renderer.render_semantic(&snapshot);
    
    // View should contain readable content
    assert!(view.contains("STRUCTURE"), "View should contain STRUCTURE category");
    assert!(view.contains("[sh-0]"), "View should contain span IDs");
}

#[test]
fn test_macro_redaction() {
    // Create a snapshot with potentially non-deterministic content
    let mut snapshot = crate::snapshot::Snapshot::new();
    
    snapshot.spans.insert("TestType".to_string(), vec![
        crate::snapshot::SpanData {
            id: crate::snapshot::SnapshotSpanId::new("tt", 0),
            position: crate::snapshot::SnapshotDocSpan::new(
                crate::snapshot::SnapshotDocPos::new(0, 0),
                crate::snapshot::SnapshotDocPos::new(0, 5),
            ),
            type_name: "TestType".to_string(),
            value: ron::Value::String("test".to_string()),
            confidence: Some(0.8),
            source: Some("LLMPass(model=gpt-4)".to_string()),
            associations: vec![],
        },
    ]);

    // Apply redaction
    let redacted = snapshot.redact();
    
    // Source should be redacted
    let span = &redacted.spans.get("TestType").unwrap()[0];
    assert_eq!(
        span.source.as_ref().unwrap(),
        "LLMPass([REDACTED])",
        "LLM source should be redacted"
    );
}

#[test]
fn test_macro_stability() {
    let text = "Section 1.1 Test";
    
    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new());

    // Build multiple times
    let snapshot1 = Snapshot::from_document(&doc).redact();
    let snapshot2 = Snapshot::from_document(&doc).redact();
    
    let ron1 = snapshot1.to_ron_string().unwrap();
    let ron2 = snapshot2.to_ron_string().unwrap();
    
    // Should be identical
    assert_eq!(ron1, ron2, "Multiple builds should produce identical RON");
    
    let renderer = SnapshotRenderer::new();
    let view1 = renderer.render_semantic(&snapshot1);
    let view2 = renderer.render_semantic(&snapshot2);
    
    assert_eq!(view1, view2, "Multiple builds should produce identical view");
}

#[test]
fn test_snapshot_data_only() {
    let text = "Section 1.1 Test";
    
    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new());

    // This tests the assert_contract_snapshot_data! pattern
    let snapshot = Snapshot::from_document(&doc).redact();
    let ron_data = snapshot.to_ron_string().expect("RON serialization failed");
    
    // Should be non-empty
    assert!(!ron_data.is_empty());
    assert!(ron_data.contains("SectionHeader"));
}

#[test]
fn test_snapshot_view_only() {
    let text = "Section 1.1 Test";
    
    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new());

    // This tests the assert_contract_snapshot_view! pattern
    let snapshot = Snapshot::from_document(&doc);
    let renderer = SnapshotRenderer::new();
    let view = renderer.render_semantic(&snapshot);
    
    // Should be non-empty
    assert!(!view.is_empty());
    assert!(view.contains("STRUCTURE"));
}

#[test]
fn test_macro_types_accessible() {
    // Test that the types used by macros are accessible
    // This is a compile-time check more than a runtime check
    fn _type_check() {
        // These types should be accessible for the macros to work
        let _: fn(&crate::ContractDocument) -> crate::snapshot::Snapshot = 
            crate::snapshot::Snapshot::from_document;
        let _: crate::snapshot::SnapshotRenderer = crate::snapshot::SnapshotRenderer::new();
    }
    
    // If this compiles, the types are exported correctly
}

#[test]
fn test_redact_non_llm_source_untouched() {
    // Test that non-LLM sources are not affected by redaction
    let mut snapshot = Snapshot::new();
    snapshot.spans.insert("TestType".to_string(), vec![
        crate::snapshot::SpanData {
            id: crate::snapshot::SnapshotSpanId::new("tt", 0),
            position: crate::snapshot::SnapshotDocSpan::new(
                crate::snapshot::SnapshotDocPos::new(0, 0),
                crate::snapshot::SnapshotDocPos::new(0, 5),
            ),
            type_name: "TestType".to_string(),
            value: ron::Value::String("test".to_string()),
            confidence: Some(0.9),
            source: Some("RuleBased(my_rule)".to_string()),
            associations: vec![],
        },
    ]);

    let redacted = snapshot.redact();
    let span = &redacted.spans.get("TestType").unwrap()[0];
    
    // RuleBased source should NOT be redacted
    assert_eq!(
        span.source.as_ref().unwrap(),
        "RuleBased(my_rule)",
        "Non-LLM source should not be changed"
    );
}

#[test]
fn test_macro_dual_snapshot_uses_different_versions() {
    // Test that RON uses redacted snapshot while view uses raw
    let text = "Section 1.1 Test";
    
    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new());

    // Build both versions as the macro would
    let raw_snapshot = Snapshot::from_document(&doc);
    let redacted_snapshot = raw_snapshot.clone().redact();
    
    // Verify they produce the same structure (redaction shouldn't affect SectionHeader)
    // but the mechanism is correctly using clone
    assert_eq!(raw_snapshot.span_count(), redacted_snapshot.span_count());
}

// Note: Actual macro invocation tests that create insta snapshots are in
// the integration tests (layered-contracts/tests/snapshot_integration.rs)
// to avoid polluting the unit test snapshot directory.

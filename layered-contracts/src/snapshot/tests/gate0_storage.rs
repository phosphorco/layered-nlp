//! Gate 0 tests: Core storage types and construction.
//!
//! Test scenarios from FR-010:
//! 1. ID stability — Same document produces same IDs across multiple builds
//! 2. ID uniqueness — No duplicate IDs within a snapshot
//! 3. Type grouping — Spans grouped correctly by type name
//! 4. Position sorting — Spans within each type sorted by position
//! 5. Association conversion — DEFERRED to Gate 4 (graph rendering)
//!    Note: associations field exists but is not populated yet
//! 6. RON round-trip — serialize -> deserialize -> serialize produces identical output
//! 7. Empty document — Empty input produces valid empty snapshot

use crate::ContractDocument;
use crate::snapshot::{Snapshot, SnapshotBuilder, SnapshotSpanId};
use crate::section_header::SectionHeaderResolver;
use crate::defined_term::DefinedTermResolver;
use crate::contract_keyword::ContractKeywordResolver;

#[test]
fn test_snapshot_id_stability() {
    let text = r#"
Section 1.1 Definitions
"Company" means ABC Corp.
Section 1.2 Obligations
The Company shall perform services.
    "#;

    // Build twice with same input
    let doc1 = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new())
        .run_resolver(&ContractKeywordResolver::default())
        .run_resolver(&DefinedTermResolver::new());
    
    let snap1 = SnapshotBuilder::new(&doc1)
        .with_standard_types()
        .build();

    let doc2 = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new())
        .run_resolver(&ContractKeywordResolver::default())
        .run_resolver(&DefinedTermResolver::new());
    
    let snap2 = SnapshotBuilder::new(&doc2)
        .with_standard_types()
        .build();

    // All IDs should match
    for type_name in snap1.spans.keys() {
        let spans1 = snap1.spans_of_type(type_name);
        let spans2 = snap2.spans_of_type(type_name);
        
        assert_eq!(spans1.len(), spans2.len(), "Type {} has different span counts", type_name);
        
        for (s1, s2) in spans1.iter().zip(spans2.iter()) {
            assert_eq!(s1.id, s2.id, "IDs differ for type {}", type_name);
        }
    }
}

#[test]
fn test_snapshot_id_uniqueness() {
    let text = r#"
Section 1.1 First Section
Section 1.2 Second Section
Section 2.1 Third Section
The Company shall perform.
The Company may terminate.
    "#;

    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new())
        .run_resolver(&ContractKeywordResolver::default());

    let snapshot = SnapshotBuilder::new(&doc)
        .with_standard_types()
        .build();

    // Collect all IDs
    let all_ids: Vec<&SnapshotSpanId> = snapshot
        .spans
        .values()
        .flat_map(|spans| spans.iter().map(|s| &s.id))
        .collect();

    // Check uniqueness
    let unique_count = {
        let mut set = std::collections::HashSet::new();
        all_ids.iter().filter(|id| set.insert(id.0.clone())).count()
    };

    assert_eq!(
        unique_count,
        all_ids.len(),
        "Found duplicate IDs in snapshot"
    );
}

#[test]
fn test_snapshot_type_grouping() {
    let text = r#"
Section 1.1 Definitions
"Company" means ABC Corp.
The Company shall perform services within 30 days.
    "#;

    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new())
        .run_resolver(&ContractKeywordResolver::default())
        .run_resolver(&DefinedTermResolver::new());

    let snapshot = SnapshotBuilder::new(&doc)
        .with_standard_types()
        .build();

    // Check type groups exist
    assert!(snapshot.spans.contains_key("SectionHeader"), "Missing SectionHeader group");
    assert!(snapshot.spans.contains_key("DefinedTerm"), "Missing DefinedTerm group");
    assert!(snapshot.spans.contains_key("ContractKeyword"), "Missing ContractKeyword group");

    // Verify spans are in correct groups
    for span in snapshot.spans_of_type("SectionHeader") {
        assert_eq!(span.type_name, "SectionHeader");
        assert!(span.id.prefix() == Some("sh"));
    }

    for span in snapshot.spans_of_type("DefinedTerm") {
        assert_eq!(span.type_name, "DefinedTerm");
        assert!(span.id.prefix() == Some("dt"));
    }
}

#[test]
fn test_snapshot_position_sorting() {
    let text = r#"
Line zero content
Section 1.1 First
Section 1.2 Second
Section 2.1 Third
    "#;

    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new());

    let snapshot = SnapshotBuilder::new(&doc)
        .with_line_type::<crate::section_header::SectionHeader>()
        .build();

    let sections = snapshot.spans_of_type("SectionHeader");
    
    // Verify sorted by position
    for i in 1..sections.len() {
        let prev = &sections[i - 1];
        let curr = &sections[i];
        
        let prev_key = (prev.position.start.line, prev.position.start.token);
        let curr_key = (curr.position.start.line, curr.position.start.token);
        
        assert!(
            prev_key <= curr_key,
            "Spans not sorted: {:?} should come before {:?}",
            prev.id,
            curr.id
        );
    }
}

#[test]
fn test_snapshot_ron_roundtrip() {
    let text = r#"
Section 1.1 Definitions
"Company" means ABC Corp.
    "#;

    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new())
        .run_resolver(&DefinedTermResolver::new());

    let snapshot = SnapshotBuilder::new(&doc)
        .with_standard_types()
        .build();

    // Serialize
    let ron_str1 = snapshot.to_ron_string().expect("serialization failed");
    
    // Deserialize
    let parsed = Snapshot::from_ron_string(&ron_str1).expect("deserialization failed");
    
    // Re-serialize
    let ron_str2 = parsed.to_ron_string().expect("re-serialization failed");
    
    // Should be identical
    assert_eq!(ron_str1, ron_str2, "RON round-trip produced different output");
}

#[test]
fn test_snapshot_empty_document() {
    let doc = ContractDocument::from_text("");
    
    let snapshot = SnapshotBuilder::new(&doc)
        .with_standard_types()
        .build();

    assert_eq!(snapshot.version, 1);
    assert_eq!(snapshot.span_count(), 0);
    assert!(snapshot.spans.is_empty());
    
    // Should still serialize/deserialize
    let ron_str = snapshot.to_ron_string().expect("serialization failed");
    let parsed = Snapshot::from_ron_string(&ron_str).expect("deserialization failed");
    assert_eq!(parsed.span_count(), 0);
}

#[test]
fn test_snapshot_confidence_preserved() {
    let text = r#""Company" means ABC Corp."#;

    let doc = ContractDocument::from_text(text)
        .run_resolver(&ContractKeywordResolver::default())
        .run_resolver(&DefinedTermResolver::new());

    let snapshot = SnapshotBuilder::new(&doc)
        .with_scored_line_type::<crate::defined_term::DefinedTerm>()
        .build();

    let terms = snapshot.spans_of_type("DefinedTerm");
    assert!(!terms.is_empty(), "Should have at least one defined term");
    
    // Check that confidence is captured
    let term = &terms[0];
    assert!(term.confidence.is_some(), "Confidence should be present for Scored types");
    assert!(term.source.is_some(), "Source should be present for Scored types");
}

#[test]
fn test_snapshot_input_preserved() {
    let text = "Line one\nLine two\nLine three";
    
    let doc = ContractDocument::from_text(text);
    let snapshot = SnapshotBuilder::new(&doc).build();

    match &snapshot.input {
        crate::snapshot::InputSource::Inline(lines) => {
            assert_eq!(
                lines,
                &vec![
                    "Line one".to_string(),
                    "Line two".to_string(),
                    "Line three".to_string(),
                ],
                "Input lines should exactly match original text"
            );
        }
        _ => panic!("Expected inline input"),
    }
}

#[test]
fn test_snapshot_find_by_id() {
    let text = "Section 1.1 Test\nSection 1.2 Another";
    
    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new());

    let snapshot = SnapshotBuilder::new(&doc)
        .with_line_type::<crate::section_header::SectionHeader>()
        .build();

    // Find existing IDs
    let id0 = SnapshotSpanId::new("sh", 0);
    let id1 = SnapshotSpanId::new("sh", 1);
    
    assert!(snapshot.find_by_id(&id0).is_some());
    assert!(snapshot.find_by_id(&id1).is_some());
    
    // Missing ID
    let missing = SnapshotSpanId::new("sh", 99);
    assert!(snapshot.find_by_id(&missing).is_none());
}

#[test]
fn test_snapshot_multiple_types() {
    let text = r#"
Section 1.1 Definitions
"Company" means ABC Corp.
The Company shall deliver goods within 30 days.
    "#;

    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new())
        .run_resolver(&ContractKeywordResolver::default())
        .run_resolver(&DefinedTermResolver::new());

    let snapshot = SnapshotBuilder::new(&doc)
        .with_standard_types()
        .build();

    // Should have multiple type groups
    let type_count = snapshot.spans.len();
    assert!(type_count >= 2, "Should have at least 2 type groups, got {}", type_count);

    // Each type should have correct prefixes
    for (type_name, spans) in &snapshot.spans {
        for span in spans {
            assert_eq!(&span.type_name, type_name);
        }
    }
}

#[test]
fn test_snapshot_associations_deferred() {
    // Association conversion is deferred to Gate 4 (graph rendering).
    // For now, the associations field exists but is always empty.
    let text = r#"
Section 1.1 Definitions
"Company" means ABC Corp.
The Company shall deliver goods.
    "#;

    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new())
        .run_resolver(&ContractKeywordResolver::default())
        .run_resolver(&DefinedTermResolver::new());

    let snapshot = SnapshotBuilder::new(&doc)
        .with_standard_types()
        .build();

    // All spans should have empty associations for now
    for spans in snapshot.spans.values() {
        for span in spans {
            assert!(
                span.associations.is_empty(),
                "Associations not yet implemented; span {} should have empty associations",
                span.id
            );
        }
    }
}

#[test]
fn test_snapshot_from_document_convenience() {
    // Test the Snapshot::from_document() convenience method
    let text = "Section 1.1 Test\nThe Company shall perform.";
    
    let doc = ContractDocument::from_text(text)
        .run_resolver(&SectionHeaderResolver::new())
        .run_resolver(&ContractKeywordResolver::default());

    let snapshot = crate::snapshot::Snapshot::from_document(&doc);

    // Should have extracted some spans with standard types
    assert!(snapshot.span_count() > 0, "Should extract some spans with standard types");
}

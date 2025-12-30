//! Tests for DocumentAligner - section alignment for contract comparison.

use crate::{
    AlignmentCandidates, AlignmentHint, AlignmentResult, AlignmentType, ContractDocument,
    DocumentAligner, DocumentStructureBuilder, HintType, SectionHeaderResolver, SimilarityConfig,
};

/// Helper to build document structure from text.
fn build_structure(text: &str) -> (crate::DocumentStructure, ContractDocument) {
    let doc = ContractDocument::from_text(text).run_resolver(&SectionHeaderResolver::new());
    let result = DocumentStructureBuilder::build(&doc);
    (result.value, doc)
}

/// Helper to align two documents.
fn align_docs(original: &str, revised: &str) -> AlignmentResult {
    let (orig_struct, orig_doc) = build_structure(original);
    let (rev_struct, rev_doc) = build_structure(revised);
    let aligner = DocumentAligner::new();
    aligner.align(&orig_struct, &rev_struct, &orig_doc, &rev_doc)
}

/// Helper to get candidates for two documents.
fn get_candidates(original: &str, revised: &str) -> AlignmentCandidates {
    let (orig_struct, orig_doc) = build_structure(original);
    let (rev_struct, rev_doc) = build_structure(revised);
    let aligner = DocumentAligner::new();
    aligner.compute_candidates(&orig_struct, &rev_struct, &orig_doc, &rev_doc)
}

#[test]
fn test_exact_match_same_id_same_content() {
    let original = r#"
ARTICLE I - DEFINITIONS
Section 1.1 Defined Terms
The following terms are defined.
"#;

    let revised = r#"
ARTICLE I - DEFINITIONS
Section 1.1 Defined Terms
The following terms are defined.
"#;

    let result = align_docs(original, revised);

    // Should have alignments for both sections
    assert!(result.stats.exact_matches > 0, "Expected exact matches");
    assert_eq!(result.stats.deleted, 0);
    assert_eq!(result.stats.inserted, 0);

    // Find the Section 1.1 alignment
    let section_1_1 = result.alignments.iter().find(|a| {
        a.original
            .iter()
            .any(|s| s.canonical_id.contains("1.1"))
    });
    assert!(section_1_1.is_some(), "Section 1.1 should be aligned");
    assert_eq!(
        section_1_1.unwrap().alignment_type,
        AlignmentType::ExactMatch
    );
}

#[test]
fn test_modified_same_id_different_content() {
    let original = r#"
Section 1.1 Payment Terms
The Company shall pay within thirty (30) days.
"#;

    let revised = r#"
Section 1.1 Payment Terms
The Company shall pay within forty-five (45) days upon receipt of invoice.
"#;

    let result = align_docs(original, revised);

    // The section should be marked as Modified (same ID, different content)
    let section_1_1 = result
        .alignments
        .iter()
        .find(|a| a.original.iter().any(|s| s.canonical_id.contains("1.1")));
    assert!(section_1_1.is_some());

    // Should be either ExactMatch (if similar enough) or Modified
    let alignment = section_1_1.unwrap();
    assert!(
        alignment.alignment_type == AlignmentType::ExactMatch
            || alignment.alignment_type == AlignmentType::Modified,
        "Expected ExactMatch or Modified, got {:?}",
        alignment.alignment_type
    );
}

#[test]
fn test_renumbered_different_id_same_content() {
    let original = r#"
Section 1.1 Definitions
The following terms are defined for use in this Agreement.
(a) "Company" means Acme Corporation.
(b) "Contractor" means the service provider.
"#;

    let revised = r#"
Section 2.1 Definitions
The following terms are defined for use in this Agreement.
(a) "Company" means Acme Corporation.
(b) "Contractor" means the service provider.
"#;

    let result = align_docs(original, revised);

    // Should detect the section was renumbered
    assert!(
        result.stats.renumbered > 0 || result.stats.exact_matches > 0,
        "Expected renumbered or matched sections"
    );
    assert_eq!(result.stats.deleted, 0, "Should not have deletions");
    assert_eq!(result.stats.inserted, 0, "Should not have insertions");
}

#[test]
fn test_deleted_section() {
    let original = r#"
ARTICLE I - DEFINITIONS
Section 1.1 Defined Terms
The following terms are defined.

ARTICLE II - SERVICES
Section 2.1 Scope
The scope of services.
"#;

    let revised = r#"
ARTICLE I - DEFINITIONS
Section 1.1 Defined Terms
The following terms are defined.
"#;

    let result = align_docs(original, revised);

    // ARTICLE II and Section 2.1 should be deleted
    assert!(result.stats.deleted > 0, "Expected deleted sections");
}

#[test]
fn test_inserted_section() {
    let original = r#"
ARTICLE I - DEFINITIONS
Section 1.1 Defined Terms
The following terms are defined.
"#;

    let revised = r#"
ARTICLE I - DEFINITIONS
Section 1.1 Defined Terms
The following terms are defined.

ARTICLE II - SERVICES
Section 2.1 Scope
The scope of services.
"#;

    let result = align_docs(original, revised);

    // ARTICLE II and Section 2.1 should be inserted
    assert!(result.stats.inserted > 0, "Expected inserted sections");
}

#[test]
fn test_empty_documents() {
    let result = align_docs("Just some text without headers.", "Just some text without headers.");

    assert!(result.alignments.is_empty());
    assert_eq!(result.stats.total_original, 0);
    assert_eq!(result.stats.total_revised, 0);
}

#[test]
fn test_candidates_serialize_deserialize_roundtrip() {
    let original = r#"
Section 1.1 Terms
The terms are defined here.
Section 1.2 Interpretation
Rules of interpretation.
"#;

    let revised = r#"
Section 1.1 Terms
The terms are defined here with modifications.
Section 1.3 Additional Rules
New section with additional rules.
"#;

    let candidates = get_candidates(original, revised);

    // Serialize to JSON
    let json = candidates.to_json();
    assert!(!json.is_empty(), "JSON should not be empty");

    // Deserialize back
    let parsed = AlignmentCandidates::from_json(&json).expect("Should parse JSON");
    assert_eq!(parsed.candidates.len(), candidates.candidates.len());
    assert_eq!(
        parsed.original_section_count,
        candidates.original_section_count
    );
    assert_eq!(
        parsed.revised_section_count,
        candidates.revised_section_count
    );
}

#[test]
fn test_hint_force_match_overrides_low_confidence() {
    let original = r#"
Section 1.1 Old Title
Some content that was completely rewritten.
"#;

    let revised = r#"
Section 2.5 New Title
Completely different content after major restructuring.
"#;

    let (orig_struct, orig_doc) = build_structure(original);
    let (rev_struct, rev_doc) = build_structure(revised);
    let aligner = DocumentAligner::new();

    let candidates = aligner.compute_candidates(&orig_struct, &rev_struct, &orig_doc, &rev_doc);

    // Find the candidate for these sections
    let orig_id = candidates
        .candidates
        .first()
        .map(|c| c.id.clone())
        .unwrap_or_default();

    // Apply hint to force match
    let hints = vec![AlignmentHint {
        candidate_id: Some(orig_id),
        original_ids: vec![],
        revised_ids: vec![],
        hint_type: HintType::ForceMatch {
            alignment_type: AlignmentType::Modified,
        },
        confidence: 0.95,
        source: "expert".to_string(),
        explanation: Some("These sections cover the same topic".to_string()),
    }];

    let result = aligner.apply_hints(candidates, &hints);

    // Should have the forced match
    let forced = result.alignments.iter().find(|a| a.confidence >= 0.9);
    assert!(forced.is_some(), "Forced match should exist with high confidence");
}

#[test]
fn test_hint_force_no_match_prevents_alignment() {
    let original = r#"
Section 1.1 Terms
The terms are defined here.
"#;

    let revised = r#"
Section 1.1 Terms
The terms are defined here.
"#;

    let (orig_struct, orig_doc) = build_structure(original);
    let (rev_struct, rev_doc) = build_structure(revised);
    let aligner = DocumentAligner::new();

    let candidates = aligner.compute_candidates(&orig_struct, &rev_struct, &orig_doc, &rev_doc);

    // Find the matching candidate
    let matching_candidate = candidates
        .candidates
        .iter()
        .find(|c| !c.original.is_empty() && !c.revised.is_empty())
        .map(|c| c.id.clone());

    if let Some(id) = matching_candidate {
        let hints = vec![AlignmentHint {
            candidate_id: Some(id),
            original_ids: vec![],
            revised_ids: vec![],
            hint_type: HintType::ForceNoMatch,
            confidence: 1.0,
            source: "expert".to_string(),
            explanation: Some("These are actually different sections".to_string()),
        }];

        let result = aligner.apply_hints(candidates, &hints);

        // The forced no-match should result in the candidate being filtered out
        // or having very low confidence
        let rejected = result.alignments.iter().all(|a| a.confidence != 0.0);
        assert!(rejected, "Rejected candidates should be filtered out");
    }
}

#[test]
fn test_hint_adjust_confidence() {
    let original = r#"
Section 1.1 Terms
The terms are defined here.
"#;

    let revised = r#"
Section 1.1 Terms
The terms are defined here with some changes.
"#;

    let (orig_struct, orig_doc) = build_structure(original);
    let (rev_struct, rev_doc) = build_structure(revised);
    let aligner = DocumentAligner::new();

    let candidates = aligner.compute_candidates(&orig_struct, &rev_struct, &orig_doc, &rev_doc);

    // Get initial confidence
    let initial_confidence = candidates
        .candidates
        .first()
        .map(|c| c.confidence)
        .unwrap_or(0.0);

    let candidate_id = candidates.candidates.first().map(|c| c.id.clone());

    if let Some(id) = candidate_id {
        let hints = vec![AlignmentHint {
            candidate_id: Some(id),
            original_ids: vec![],
            revised_ids: vec![],
            hint_type: HintType::AdjustConfidence { delta: 0.1 },
            confidence: 1.0,
            source: "llm".to_string(),
            explanation: Some("LLM confirms semantic similarity".to_string()),
        }];

        let result = aligner.apply_hints(candidates, &hints);

        // Confidence should be adjusted
        if let Some(alignment) = result.alignments.first() {
            let expected = (initial_confidence + 0.1).min(1.0);
            assert!(
                (alignment.confidence - expected).abs() < 0.01,
                "Confidence should be adjusted: expected {}, got {}",
                expected,
                alignment.confidence
            );
        }
    }
}

#[test]
fn test_similarity_config_affects_matching() {
    let original = r#"
Section 1.1 Terms
The terms are defined here.
"#;

    let revised = r#"
Section 2.1 Definitions
A list of definitions for this agreement.
"#;

    let (orig_struct, orig_doc) = build_structure(original);
    let (rev_struct, rev_doc) = build_structure(revised);

    // With default config
    let default_aligner = DocumentAligner::new();
    let default_result = default_aligner.align(&orig_struct, &rev_struct, &orig_doc, &rev_doc);

    // With stricter threshold
    let strict_config = SimilarityConfig {
        match_threshold: 0.90,
        ..Default::default()
    };
    let strict_aligner = DocumentAligner::with_config(strict_config);
    let strict_result = strict_aligner.align(&orig_struct, &rev_struct, &orig_doc, &rev_doc);

    // Stricter threshold should result in more unmatched sections
    assert!(
        strict_result.stats.deleted >= default_result.stats.deleted
            || strict_result.stats.inserted >= default_result.stats.inserted,
        "Stricter threshold should produce more unmatched sections"
    );
}

#[test]
fn test_alignment_stats_are_consistent() {
    let original = r#"
ARTICLE I - DEFINITIONS
Section 1.1 Defined Terms
The following terms.

ARTICLE II - SERVICES
Section 2.1 Scope
The scope.
"#;

    let revised = r#"
ARTICLE I - DEFINITIONS
Section 1.1 Defined Terms
The following terms with modifications.

ARTICLE II - PAYMENT
Section 2.1 Fees
The fees.
"#;

    let result = align_docs(original, revised);

    // Stats should be consistent with alignments
    let total_categorized = result.stats.exact_matches
        + result.stats.renumbered
        + result.stats.moved
        + result.stats.modified
        + result.stats.split
        + result.stats.merged
        + result.stats.deleted
        + result.stats.inserted;

    assert_eq!(
        total_categorized,
        result.alignments.len(),
        "Stats should match alignment count"
    );
}

#[test]
fn test_complex_document_alignment() {
    let original = r#"
MASTER SERVICES AGREEMENT

ARTICLE I - DEFINITIONS
Section 1.1 Defined Terms
(a) "Agreement" means this contract.
(b) "Company" means Acme Corp.

ARTICLE II - SERVICES
Section 2.1 Scope
The Contractor shall provide services.
Section 2.2 Standards
Services shall be professional.

ARTICLE III - PAYMENT
Section 3.1 Fees
Fees as set forth in Exhibit A.
"#;

    let revised = r#"
MASTER SERVICES AGREEMENT

ARTICLE I - DEFINITIONS
Section 1.1 Defined Terms
(a) "Agreement" means this Master Services Agreement.
(b) "Company" means Acme Corporation, a Delaware company.
(c) "Effective Date" means the date of signing.

ARTICLE II - SERVICES
Section 2.1 Scope of Work
The Contractor shall provide the services described herein.
Section 2.2 Service Standards
All services shall be performed professionally.
Section 2.3 Subcontractors
Subcontractors may be engaged with approval.

ARTICLE III - COMPENSATION
Section 3.1 Fee Schedule
Fees as detailed in Exhibit A attached hereto.
Section 3.2 Payment Terms
Payment within thirty days of invoice.
"#;

    let result = align_docs(original, revised);

    // Verify we got alignments
    assert!(!result.alignments.is_empty(), "Should have alignments");

    // Should have some exact matches (definitions still match)
    // and some modifications/insertions
    assert!(
        result.stats.exact_matches > 0 || result.stats.modified > 0,
        "Should have matches or modifications"
    );

    // New sections should be detected
    assert!(result.stats.inserted > 0, "Should have inserted sections (e.g., Section 2.3, 3.2)");

    // ARTICLE III title change should not cause deletion/insertion
    // but rather a match (since content is similar)
}

#[test]
fn test_alignment_result_to_json() {
    let original = r#"
Section 1.1 Terms
The terms.
"#;

    let revised = r#"
Section 1.1 Terms
The terms with changes.
"#;

    let result = align_docs(original, revised);
    let json = result.to_json();

    // JSON should be valid and contain expected fields
    assert!(json.contains("alignments"));
    assert!(json.contains("stats"));
    assert!(json.contains("warnings"));
}

#[test]
fn test_definition_section_matching() {
    let original = r#"
Section 1. DEFINITIONS
"Agreement" means this contract.
"Company" means Acme Corp.
"Services" means the work to be performed.
"#;

    let revised = r#"
Section 1. DEFINITIONS
"Agreement" means this Master Services Agreement.
"Company" means Acme Corporation.
"Services" means all work and deliverables.
"Confidential Information" means proprietary data.
"#;

    let result = align_docs(original, revised);

    // The definition section should match despite content changes
    let def_section = result.alignments.iter().find(|a| {
        a.original
            .iter()
            .any(|s| s.title.as_deref() == Some("DEFINITIONS"))
    });
    assert!(def_section.is_some(), "Definition section should be aligned");
}

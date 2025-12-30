//! Document-level section reference linker.
//!
//! This module provides `SectionReferenceLinker` which resolves section references
//! (detected by `SectionReferenceResolver`) to actual sections in the document
//! structure (built by `DocumentStructureBuilder`).
//!
//! Key responsibilities:
//! - Filter out false positives (references that overlap with headers)
//! - Resolve references to actual sections
//! - Handle ambiguities (e.g., "(i)" could be Roman 1 or Alpha 'i')
//! - Detect broken references (to non-existent sections)

use std::collections::HashMap;

use layered_nlp::x;

use crate::{ContractDocument, DocSpan, ProcessError, ProcessResult};
use crate::document_structure::{DocumentProcessor, DocumentStructure, SectionNode};
use crate::section_header::{SectionHeader, SectionIdentifier};
use crate::section_reference::{ReferenceType, SectionReference};

/// A section reference that has been linked to the document structure.
#[derive(Debug, Clone)]
pub struct LinkedReference {
    /// The original reference
    pub reference: SectionReference,
    /// Document position where this reference occurs
    pub location: DocSpan,
    /// Line number (internal document index)
    pub line: usize,
    /// Resolution result
    pub resolution: ReferenceResolution,
}

/// Result of attempting to resolve a reference.
#[derive(Debug, Clone)]
pub enum ReferenceResolution {
    /// Successfully resolved to a section.
    Resolved {
        /// Canonical identifier of the resolved section
        canonical: String,
        /// The section's title (if any)
        section_title: Option<String>,
        /// Start line of the resolved section
        section_line: usize,
        /// Updated confidence after resolution
        confidence: f64,
    },
    /// Couldn't find a matching section.
    Unresolved {
        /// Reason for failure
        reason: String,
        /// Confidence (typically lowered)
        confidence: f64,
    },
    /// Filtered out because it overlaps with a section header.
    FilteredAsHeader,
    /// Ambiguous - multiple possible resolutions.
    Ambiguous {
        /// Candidate canonical identifiers
        candidates: Vec<String>,
        /// Confidence (lowered due to ambiguity)
        confidence: f64,
    },
}

impl ReferenceResolution {
    /// Get the confidence score for this resolution.
    pub fn confidence(&self) -> f64 {
        match self {
            ReferenceResolution::Resolved { confidence, .. } => *confidence,
            ReferenceResolution::Unresolved { confidence, .. } => *confidence,
            ReferenceResolution::FilteredAsHeader => 0.0,
            ReferenceResolution::Ambiguous { confidence, .. } => *confidence,
        }
    }

    /// Returns true if this reference was successfully resolved.
    pub fn is_resolved(&self) -> bool {
        matches!(self, ReferenceResolution::Resolved { .. })
    }
}

/// Collection of linked references from a document.
#[derive(Debug, Clone)]
pub struct LinkedReferences {
    /// Successfully resolved references
    pub resolved: Vec<LinkedReference>,
    /// References that couldn't be resolved
    pub unresolved: Vec<LinkedReference>,
    /// References filtered out as headers
    pub filtered: Vec<LinkedReference>,
    /// Ambiguous references
    pub ambiguous: Vec<LinkedReference>,
}

impl LinkedReferences {
    /// Create an empty collection.
    pub fn empty() -> Self {
        Self {
            resolved: Vec::new(),
            unresolved: Vec::new(),
            filtered: Vec::new(),
            ambiguous: Vec::new(),
        }
    }

    /// Total number of references processed.
    pub fn total(&self) -> usize {
        self.resolved.len() + self.unresolved.len() + self.filtered.len() + self.ambiguous.len()
    }

    /// All references (resolved + unresolved + ambiguous), excluding filtered headers.
    pub fn all_references(&self) -> impl Iterator<Item = &LinkedReference> {
        self.resolved
            .iter()
            .chain(self.unresolved.iter())
            .chain(self.ambiguous.iter())
    }

    /// Get references by the section they point to (canonical ID).
    pub fn references_to(&self, canonical: &str) -> Vec<&LinkedReference> {
        self.resolved
            .iter()
            .filter(|r| {
                matches!(&r.resolution, ReferenceResolution::Resolved { canonical: c, .. } if c == canonical)
            })
            .collect()
    }
}

/// Links section references to actual sections in the document structure.
pub struct SectionReferenceLinker;

impl SectionReferenceLinker {
    /// Link all section references in a document to its structure.
    ///
    /// Prerequisites:
    /// - The document must have been processed with `SectionHeaderResolver`
    /// - The document must have been processed with `SectionReferenceResolver`
    /// - A `DocumentStructure` must have been built from the document
    pub fn link(
        doc: &ContractDocument,
        structure: &DocumentStructure,
    ) -> ProcessResult<LinkedReferences> {
        let mut result = LinkedReferences::empty();
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        // Build lookup maps and cache flattened structure
        let section_map = Self::build_section_map(structure);
        let flattened_sections = structure.flatten();

        // Process each line
        for (line_idx, line) in doc.lines_enumerated() {
            // Get headers on this line (for overlap detection)
            let line_headers: Vec<_> = line
                .find(&x::attr::<SectionHeader>())
                .into_iter()
                .map(|found| found.range())
                .collect();

            // Process each reference on this line
            for found in line.find(&x::attr::<SectionReference>()) {
                let reference = (*found.attr()).clone();
                let (ref_start, ref_end) = found.range();

                let location = DocSpan::single_line(line_idx, ref_start, ref_end);

                // Check for overlap with headers
                if Self::overlaps_with_header(ref_start, ref_end, &line_headers) {
                    result.filtered.push(LinkedReference {
                        reference,
                        location,
                        line: line_idx,
                        resolution: ReferenceResolution::FilteredAsHeader,
                    });
                    continue;
                }

                // Attempt resolution
                let resolution = Self::resolve_reference(
                    &reference,
                    &section_map,
                    &flattened_sections,
                    line_idx,
                );

                // Categorize result
                let linked = LinkedReference {
                    reference,
                    location,
                    line: line_idx,
                    resolution,
                };

                match &linked.resolution {
                    ReferenceResolution::Resolved { .. } => result.resolved.push(linked),
                    ReferenceResolution::Unresolved { reason, .. } => {
                        warnings.push(format!(
                            "Line {}: unresolved reference '{}': {}",
                            doc.source_line_number(line_idx).unwrap_or(line_idx),
                            linked.reference.reference_text,
                            reason
                        ));
                        result.unresolved.push(linked);
                    }
                    ReferenceResolution::FilteredAsHeader => {
                        // Already handled above, shouldn't reach here
                        result.filtered.push(linked);
                    }
                    ReferenceResolution::Ambiguous { candidates, .. } => {
                        warnings.push(format!(
                            "Line {}: ambiguous reference '{}': could be {:?}",
                            doc.source_line_number(line_idx).unwrap_or(line_idx),
                            linked.reference.reference_text,
                            candidates
                        ));
                        result.ambiguous.push(linked);
                    }
                }
            }
        }

        // Report dangling references as errors
        for unresolved in &result.unresolved {
            if unresolved.reference.target.is_some() {
                errors.push(ProcessError::DanglingReference {
                    reference: unresolved.reference.reference_text.clone(),
                    location: unresolved.location,
                });
            }
        }

        ProcessResult {
            value: result,
            errors,
            warnings,
        }
    }

    /// Build a map from canonical identifiers to section nodes.
    fn build_section_map(structure: &DocumentStructure) -> HashMap<String, SectionInfo> {
        let mut map = HashMap::new();
        for node in structure.flatten() {
            let canonical = node.header.identifier.canonical();
            map.insert(
                canonical,
                SectionInfo {
                    title: node.header.title.clone(),
                    line: node.start_line,
                },
            );
        }
        map
    }

    /// Check if a reference span overlaps with a header that starts at the beginning of the line.
    ///
    /// We only consider headers that start at token 0 (beginning of line) to avoid
    /// filtering references like "Section 1.1" in "comply with Section 1.1".
    fn overlaps_with_header(
        ref_start: usize,
        ref_end: usize,
        headers: &[(usize, usize)],
    ) -> bool {
        for (header_start, header_end) in headers {
            // Only consider headers that start at the beginning of the line
            // (token index 0 or 1 to account for potential leading whitespace)
            if *header_start > 1 {
                continue;
            }
            // Overlap if ranges intersect
            if ref_start <= *header_end && ref_end >= *header_start {
                return true;
            }
        }
        false
    }

    /// Resolve a single reference to a section.
    fn resolve_reference(
        reference: &SectionReference,
        section_map: &HashMap<String, SectionInfo>,
        flattened_sections: &[&SectionNode],
        current_line: usize,
    ) -> ReferenceResolution {
        let base_confidence = reference.confidence;

        // Handle relative references (this Section, hereof, etc.)
        if matches!(reference.reference_type, ReferenceType::Relative(_)) {
            if reference.target.is_none() {
                // "this Section", "herein", etc. - resolve to containing section
                return Self::resolve_relative_reference(
                    flattened_sections,
                    current_line,
                    base_confidence,
                );
            }
        }

        // Try to resolve the target identifier
        let Some(target) = &reference.target else {
            return ReferenceResolution::Unresolved {
                reason: "No target identifier".to_string(),
                confidence: base_confidence * 0.5,
            };
        };

        let canonical = target.canonical();

        // Direct lookup
        if let Some(section_info) = section_map.get(&canonical) {
            return ReferenceResolution::Resolved {
                canonical,
                section_title: section_info.title.clone(),
                section_line: section_info.line,
                confidence: (base_confidence * 1.1).min(1.0), // Boost for successful resolution
            };
        }

        // Handle potential ambiguity for "(i)" - could be Roman 1 or Alpha 'i'
        if let SectionIdentifier::Roman { value: 1, uppercase: false } = target {
            // Check if there's an Alpha 'i' instead
            let alpha_canonical = "i"; // Alpha canonical is just the lowercase letter
            if section_map.contains_key(alpha_canonical) && !section_map.contains_key(&canonical) {
                return ReferenceResolution::Resolved {
                    canonical: alpha_canonical.to_string(),
                    section_title: section_map.get(alpha_canonical).and_then(|s| s.title.clone()),
                    section_line: section_map.get(alpha_canonical).map(|s| s.line).unwrap_or(0),
                    confidence: base_confidence * 0.85, // Slight penalty for reinterpretation
                };
            }
            // Both exist - ambiguous
            if section_map.contains_key(alpha_canonical) && section_map.contains_key(&canonical) {
                return ReferenceResolution::Ambiguous {
                    candidates: vec![canonical, alpha_canonical.to_string()],
                    confidence: base_confidence * 0.6,
                };
            }
        }

        // Try fuzzy matching for Named sections
        if let SectionIdentifier::Named { sub_identifier, .. } = target {
            // Try without the kind prefix (e.g., "Section 3.1" might match "3.1")
            if let Some(sub) = sub_identifier {
                let sub_canonical = sub.canonical();
                if let Some(section_info) = section_map.get(&sub_canonical) {
                    return ReferenceResolution::Resolved {
                        canonical: sub_canonical,
                        section_title: section_info.title.clone(),
                        section_line: section_info.line,
                        confidence: base_confidence * 0.9, // Slight penalty for indirect match
                    };
                }
            }
        }

        // Not found
        ReferenceResolution::Unresolved {
            reason: format!("Section '{}' not found in document", canonical),
            confidence: base_confidence * 0.3,
        }
    }

    /// Resolve a relative reference like "this Section" to the containing section.
    fn resolve_relative_reference(
        flattened_sections: &[&SectionNode],
        current_line: usize,
        base_confidence: f64,
    ) -> ReferenceResolution {
        // Find the section containing this line
        let containing = Self::find_containing_section(flattened_sections, current_line);

        match containing {
            Some(section) => ReferenceResolution::Resolved {
                canonical: section.header.identifier.canonical(),
                section_title: section.header.title.clone(),
                section_line: section.start_line,
                confidence: (base_confidence * 0.95).min(1.0),
            },
            None => ReferenceResolution::Unresolved {
                reason: "Not within any section".to_string(),
                confidence: base_confidence * 0.4,
            },
        }
    }

    /// Find the innermost section containing the given line.
    fn find_containing_section<'a>(
        flattened_sections: &[&'a SectionNode],
        line: usize,
    ) -> Option<&'a SectionNode> {
        let mut best: Option<&SectionNode> = None;

        for &section in flattened_sections {
            let end = section.end_line.unwrap_or(usize::MAX);
            if section.start_line <= line && line < end {
                // This section contains the line
                // Prefer deeper (more specific) sections
                if best.is_none() || section.depth() > best.unwrap().depth() {
                    best = Some(section);
                }
            }
        }

        best
    }
}

/// Internal section info for lookups.
#[derive(Debug, Clone)]
struct SectionInfo {
    title: Option<String>,
    line: usize,
}

/// Note: If you already have a `DocumentStructure`, prefer using
/// `SectionReferenceLinker::link(doc, &structure)` directly to avoid
/// rebuilding the structure. This impl is provided for convenience when
/// you don't need to reuse the structure for other purposes.
impl DocumentProcessor for SectionReferenceLinker {
    type Output = LinkedReferences;

    fn process(&self, doc: &ContractDocument) -> ProcessResult<Self::Output> {
        // Build structure first (consider using link() directly if you already have one)
        let structure_result = crate::document_structure::DocumentStructureBuilder::build(doc);

        // Then link references
        Self::link(doc, &structure_result.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::section_header::SectionHeaderResolver;
    use crate::section_reference::SectionReferenceResolver;

    fn link_references(text: &str) -> ProcessResult<LinkedReferences> {
        let doc = ContractDocument::from_text(text)
            .run_resolver(&SectionHeaderResolver::new())
            .run_resolver(&SectionReferenceResolver::new());
        let structure = crate::document_structure::DocumentStructureBuilder::build(&doc).value;
        SectionReferenceLinker::link(&doc, &structure)
    }

    #[test]
    fn test_direct_reference_resolved() {
        let text = r#"
ARTICLE I - DEFINITIONS
Section 1.1 Terms
The following terms apply.

ARTICLE II - SERVICES
The Company shall comply with Section 1.1.
"#;

        let result = link_references(text);
        assert!(
            result.value.resolved.iter().any(|r| r.reference.reference_text.contains("Section 1.1")),
            "Expected Section 1.1 to be resolved. Resolved: {:?}, Unresolved: {:?}",
            result.value.resolved,
            result.value.unresolved
        );
    }

    #[test]
    fn test_broken_reference_detected() {
        let text = r#"
ARTICLE I - DEFINITIONS
See Section 99.99 for details.
"#;

        let result = link_references(text);
        assert!(
            result.value.unresolved.iter().any(|r| r.reference.reference_text.contains("99.99")),
            "Expected Section 99.99 to be unresolved. Resolved: {:?}, Unresolved: {:?}, Filtered: {:?}",
            result.value.resolved.iter().map(|r| &r.reference.reference_text).collect::<Vec<_>>(),
            result.value.unresolved.iter().map(|r| &r.reference.reference_text).collect::<Vec<_>>(),
            result.value.filtered.iter().map(|r| &r.reference.reference_text).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_header_filtered() {
        let text = r#"
Section 3.1 - Payment Terms
The payment terms are as follows.
"#;

        let result = link_references(text);
        // "Section 3.1" should be filtered as a header
        assert!(
            result.value.filtered.iter().any(|r| r.reference.reference_text.contains("3.1")),
            "Expected Section 3.1 to be filtered as header. Filtered: {:?}, Resolved: {:?}",
            result.value.filtered,
            result.value.resolved
        );
    }

    #[test]
    fn test_relative_reference_this_section() {
        let text = r#"
ARTICLE I - DEFINITIONS
Section 1.1 Terms
As used in this Section, terms have special meanings.
"#;

        let result = link_references(text);
        // "this Section" should resolve to Section 1.1
        let this_section_refs: Vec<_> = result
            .value
            .resolved
            .iter()
            .filter(|r| r.reference.reference_text.to_lowercase().contains("this section"))
            .collect();

        assert!(
            !this_section_refs.is_empty(),
            "Expected 'this Section' to be resolved. All resolved: {:?}",
            result.value.resolved
        );
    }

    #[test]
    fn test_article_reference() {
        let text = r#"
ARTICLE I - DEFINITIONS
Definitions here.

ARTICLE II - SERVICES
See Article I for definitions.
"#;

        let result = link_references(text);
        assert!(
            result.value.resolved.iter().any(|r| r.reference.reference_text.contains("Article I")),
            "Expected Article I to be resolved"
        );
    }

    #[test]
    fn test_cross_section_reference() {
        let text = r#"
ARTICLE I - DEFINITIONS
Section 1.1 Terms
Terms defined here.

Section 1.2 Interpretation
Rules of interpretation.

ARTICLE II - SERVICES
Section 2.1 Scope
Subject to Section 1.2, the services include...
"#;

        let result = link_references(text);
        // Section 2.1 references Section 1.2
        let ref_1_2: Vec<_> = result
            .value
            .resolved
            .iter()
            .filter(|r| r.reference.reference_text.contains("Section 1.2"))
            .collect();

        assert!(
            !ref_1_2.is_empty(),
            "Expected Section 1.2 reference to be resolved"
        );
    }

    #[test]
    fn test_references_to_lookup() {
        let text = r#"
ARTICLE I - DEFINITIONS
Section 1.1 Terms
Terms here.

ARTICLE II - SERVICES
See Section 1.1 for terms.
Also refer to Section 1.1 for definitions.
"#;

        let result = link_references(text);
        let refs_to_1_1 = result.value.references_to("SECTION:1.1");
        assert!(
            refs_to_1_1.len() >= 2,
            "Expected at least 2 references to Section 1.1, found {}",
            refs_to_1_1.len()
        );
    }

    #[test]
    fn test_hereof_standalone() {
        let text = r#"
Section 1 Introduction
As defined herein, terms apply.
"#;

        let result = link_references(text);
        // "herein" is a standalone relative reference - should resolve to Section 1
        let herein_refs: Vec<_> = result
            .value
            .all_references()
            .filter(|r| r.reference.reference_text.to_lowercase() == "herein")
            .collect();

        // It should be in resolved (pointing to containing section) or
        // have some resolution attempt
        assert!(
            !herein_refs.is_empty(),
            "Expected 'herein' reference to be processed"
        );
    }

    #[test]
    fn test_snapshot() {
        let text = r#"
ARTICLE I - DEFINITIONS

Section 1.1 Defined Terms
The following terms are defined herein.

Section 1.2 Interpretation
As used in this Section, references matter.

ARTICLE II - SERVICES

Section 2.1 Scope
Subject to Section 1.1 and Article I hereof, services apply.

Section 2.2 Standards
See Section 99.99 for non-existent content.
"#;

        let result = link_references(text);
        insta::assert_debug_snapshot!((
            result.value.resolved.len(),
            result.value.unresolved.len(),
            result.value.filtered.len(),
            result.value.ambiguous.len(),
            &result.warnings,
        ));
    }
}

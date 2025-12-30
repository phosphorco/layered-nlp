//! Document structure building from per-line section headers.
//!
//! This module provides `DocumentStructureBuilder` which takes a `ContractDocument`
//! with `SectionHeader` attributes on individual lines and builds a hierarchical
//! `DocumentStructure` representing the document's outline.

use layered_nlp::x;

use crate::document::{ContractDocument, DocPosition, DocSpan, ProcessResult};
use crate::section_header::SectionHeader;

/// Hierarchical representation of document structure.
#[derive(Debug, Clone)]
pub struct DocumentStructure {
    /// Root-level sections (articles, top-level sections)
    pub sections: Vec<SectionNode>,
}

impl DocumentStructure {
    /// Create an empty structure.
    pub fn empty() -> Self {
        Self { sections: Vec::new() }
    }

    /// Get all sections flattened (depth-first traversal).
    pub fn flatten(&self) -> Vec<&SectionNode> {
        let mut result = Vec::new();
        for section in &self.sections {
            Self::flatten_node(section, &mut result);
        }
        result
    }

    fn flatten_node<'a>(node: &'a SectionNode, result: &mut Vec<&'a SectionNode>) {
        result.push(node);
        for child in &node.children {
            Self::flatten_node(child, result);
        }
    }

    /// Find a section by its canonical identifier.
    pub fn find_by_canonical(&self, canonical: &str) -> Option<&SectionNode> {
        self.flatten()
            .into_iter()
            .find(|node| node.header.identifier.canonical() == canonical)
    }

    /// Get total number of sections (including nested).
    pub fn total_sections(&self) -> usize {
        self.flatten().len()
    }
}

/// A node in the document structure tree.
#[derive(Debug, Clone)]
pub struct SectionNode {
    /// The section header information
    pub header: SectionHeader,
    /// Line number where this section starts
    pub start_line: usize,
    /// Line number where this section ends (exclusive of next section)
    pub end_line: Option<usize>,
    /// Document span covering the entire section
    pub content_span: DocSpan,
    /// Child sections (subsections)
    pub children: Vec<SectionNode>,
}

impl SectionNode {
    /// Get the section depth based on the identifier.
    pub fn depth(&self) -> u8 {
        self.header.identifier.depth()
    }
}

/// Builds document structure from per-line section headers.
pub struct DocumentStructureBuilder;

impl DocumentStructureBuilder {
    /// Process a document and build its hierarchical structure.
    ///
    /// Prerequisites:
    /// - The document must have been processed with `SectionHeaderResolver`
    ///   to add `SectionHeader` attributes to lines containing headers.
    pub fn build(doc: &ContractDocument) -> ProcessResult<DocumentStructure> {
        let errors = Vec::new();
        let mut warnings = Vec::new();

        // Collect all section headers with their line numbers.
        // Only include headers that start at the beginning of a line (token 0 or 1)
        // to avoid false positives like "comply with Section 3.1" being treated as a header.
        let mut headers: Vec<(usize, SectionHeader)> = Vec::new();

        for (line_idx, line) in doc.lines_enumerated() {
            for found in line.find(&x::attr::<SectionHeader>()) {
                let (start, _end) = found.range();
                // Only consider headers that start at the beginning of the line
                if start > 1 {
                    continue;
                }
                // found.attr() returns &&SectionHeader, so we need (*found.attr()).clone()
                headers.push((line_idx, (*found.attr()).clone()));
            }
        }

        if headers.is_empty() {
            warnings.push("No section headers found in document".to_string());
            return ProcessResult {
                value: DocumentStructure::empty(),
                errors,
                warnings,
            };
        }

        // Sort by line number (should already be sorted, but be safe)
        headers.sort_by_key(|(line, _)| *line);

        // Build the tree structure
        let mut root_sections: Vec<SectionNode> = Vec::new();
        let mut stack: Vec<(SectionNode, u8)> = Vec::new(); // (node, depth)

        let total_lines = doc.line_count();

        for (i, (line_idx, header)) in headers.iter().enumerate() {
            let depth = header.identifier.depth();

            // Determine end line (start of next section or end of document)
            let end_line = headers
                .get(i + 1)
                .map(|(next_line, _)| *next_line)
                .or(Some(total_lines));

            // Get the actual last token index for the end position
            let end_line_idx = end_line.unwrap_or(total_lines).saturating_sub(1);
            let last_token_idx = doc
                .get_line(end_line_idx)
                .map(|line| line.ll_tokens().len().saturating_sub(1))
                .unwrap_or(0);

            let node = SectionNode {
                header: header.clone(),
                start_line: *line_idx,
                end_line,
                content_span: DocSpan::new(
                    DocPosition::new(*line_idx, 0),
                    DocPosition::end_of_line(end_line_idx, last_token_idx),
                ),
                children: Vec::new(),
            };

            // Pop from stack until we find a parent (lower depth)
            while let Some((_, parent_depth)) = stack.last() {
                if *parent_depth >= depth {
                    // This item on stack is not a parent, pop it
                    let (finished_node, _finished_depth) = stack.pop().unwrap();

                    // Add to parent or root
                    if let Some((parent, _)) = stack.last_mut() {
                        parent.children.push(finished_node);
                    } else {
                        root_sections.push(finished_node);
                    }
                } else {
                    break;
                }
            }

            // Validate depth consistency
            if let Some((_, parent_depth)) = stack.last() {
                if depth <= *parent_depth {
                    warnings.push(format!(
                        "Line {}: section '{}' (depth {}) appears after deeper section (depth {})",
                        line_idx, header.raw_text, depth, parent_depth
                    ));
                }
            }

            // Push current node onto stack
            stack.push((node, depth));
        }

        // Drain remaining stack
        while let Some((finished_node, _)) = stack.pop() {
            if let Some((parent, _)) = stack.last_mut() {
                parent.children.push(finished_node);
            } else {
                root_sections.push(finished_node);
            }
        }

        ProcessResult {
            value: DocumentStructure {
                sections: root_sections,
            },
            errors,
            warnings,
        }
    }
}

/// Trait for components that process entire documents.
///
/// Unlike `Resolver` which operates on single lines, `DocumentProcessor`
/// operates on the complete `ContractDocument`.
pub trait DocumentProcessor {
    /// The output type produced by this processor.
    type Output;

    /// Process the document and produce output.
    fn process(&self, doc: &ContractDocument) -> ProcessResult<Self::Output>;
}

impl DocumentProcessor for DocumentStructureBuilder {
    type Output = DocumentStructure;

    fn process(&self, doc: &ContractDocument) -> ProcessResult<DocumentStructure> {
        Self::build(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::section_header::SectionHeaderResolver;

    fn build_structure(text: &str) -> ProcessResult<DocumentStructure> {
        let doc = ContractDocument::from_text(text).run_resolver(&SectionHeaderResolver::new());
        DocumentStructureBuilder::build(&doc)
    }

    #[test]
    fn test_flat_structure() {
        let text = r#"
ARTICLE I - DEFINITIONS
This article defines terms.

ARTICLE II - SERVICES
This article describes services.

ARTICLE III - PAYMENT
This article covers payment.
"#;

        let result = build_structure(text);
        assert!(!result.has_errors());

        let structure = result.value;
        assert_eq!(structure.sections.len(), 3);
        assert_eq!(structure.total_sections(), 3);

        assert_eq!(structure.sections[0].header.raw_text, "ARTICLE I");
        assert_eq!(structure.sections[1].header.raw_text, "ARTICLE II");
        assert_eq!(structure.sections[2].header.raw_text, "ARTICLE III");
    }

    #[test]
    fn test_nested_structure() {
        let text = r#"
ARTICLE I - DEFINITIONS
Introductory text.

Section 1.1 Defined Terms
The following terms are defined.

Section 1.2 Interpretation
Rules of interpretation.

ARTICLE II - SERVICES
Services section.

Section 2.1 Scope
The scope of services.
"#;

        let result = build_structure(text);
        assert!(!result.has_errors());

        let structure = result.value;

        // Should have 2 root articles
        assert_eq!(structure.sections.len(), 2);

        // ARTICLE I should have 2 children
        assert_eq!(structure.sections[0].children.len(), 2);
        assert_eq!(
            structure.sections[0].children[0].header.raw_text,
            "Section 1.1"
        );

        // ARTICLE II should have 1 child
        assert_eq!(structure.sections[1].children.len(), 1);

        // Total should be 5
        assert_eq!(structure.total_sections(), 5);
    }

    #[test]
    fn test_deeply_nested() {
        let text = r#"
Section 1 Top Level
Section 1.1 First Level
Section 1.1.1 Second Level
Section 1.2 Back to First
Section 2 Another Top
"#;

        let result = build_structure(text);
        let structure = result.value;

        // Should have 2 root sections
        assert_eq!(structure.sections.len(), 2);

        // Section 1 should have children
        let section1 = &structure.sections[0];
        assert_eq!(section1.children.len(), 2); // 1.1 and 1.2

        // Section 1.1 should have 1.1.1 as child
        let section_1_1 = &section1.children[0];
        assert_eq!(section_1_1.children.len(), 1);
        assert!(section_1_1.children[0]
            .header
            .raw_text
            .contains("1.1.1"));
    }

    #[test]
    fn test_find_by_canonical() {
        let text = r#"
ARTICLE I - DEFINITIONS
Section 1.1 Terms
ARTICLE II - SERVICES
"#;

        let result = build_structure(text);
        let structure = result.value;

        // Find ARTICLE I
        let article_i = structure.find_by_canonical("ARTICLE:R1");
        assert!(article_i.is_some());
        assert!(article_i.unwrap().header.raw_text.contains("ARTICLE I"));

        // Find Section 1.1
        let section_1_1 = structure.find_by_canonical("SECTION:1.1");
        assert!(section_1_1.is_some());
    }

    #[test]
    fn test_empty_document() {
        let result = build_structure("Just some text without any section headers.");
        assert!(result.warnings.iter().any(|w| w.contains("No section headers")));
        assert_eq!(result.value.total_sections(), 0);
    }

    #[test]
    fn test_line_boundaries() {
        let text = r#"
ARTICLE I - FIRST
Content of first article.
More content.

ARTICLE II - SECOND
Content of second.
"#;

        let result = build_structure(text);
        let structure = result.value;

        // ARTICLE I should start at its line and end where ARTICLE II starts
        let article_i = &structure.sections[0];
        let article_ii = &structure.sections[1];

        assert!(article_i.start_line < article_ii.start_line);
        assert_eq!(article_i.end_line, Some(article_ii.start_line));
    }

    #[test]
    fn test_snapshot_structure() {
        let text = r#"
ARTICLE I - DEFINITIONS
Section 1.1 Terms
(a) First term
(b) Second term
Section 1.2 Interpretation
ARTICLE II - SERVICES
"#;

        let result = build_structure(text);
        insta::assert_debug_snapshot!(result.value);
    }

    #[test]
    fn test_realistic_contract_integration() {
        // A realistic contract excerpt with multiple levels of nesting
        let contract = r#"
MASTER SERVICES AGREEMENT

ARTICLE I - DEFINITIONS

Section 1.1 Defined Terms
The following terms shall have the meanings set forth below:
(a) "Agreement" means this Master Services Agreement.
(b) "Company" means Acme Corporation, a Delaware corporation.
(c) "Contractor" means the party providing services hereunder.
(d) "Confidential Information" means any non-public information.

Section 1.2 Rules of Interpretation
(a) References to "include" shall not be limiting.
(b) Headings are for convenience only.

ARTICLE II - SERVICES

Section 2.1 Scope of Services
The Contractor shall provide the services described in Exhibit A.

Section 2.2 Service Standards
The Contractor shall perform all services in a professional manner.

Section 2.3 Subcontractors
(a) The Contractor may engage subcontractors with prior approval.
(b) The Contractor remains responsible for subcontractor performance.

ARTICLE III - PAYMENT

Section 3.1 Fees
The Company shall pay the Contractor the fees set forth in Exhibit B.

Section 3.2 Payment Terms
(a) Invoices shall be submitted monthly.
(b) Payment shall be due within thirty (30) days of receipt.
(c) Late payments shall bear interest at 1.5% per month.

ARTICLE IV - TERM AND TERMINATION

Section 4.1 Term
This Agreement shall commence on the Effective Date and continue for one year.

Section 4.2 Termination for Convenience
Either party may terminate this Agreement upon thirty (30) days written notice.

Section 4.3 Termination for Cause
Either party may terminate immediately upon material breach.

ARTICLE V - CONFIDENTIALITY

Section 5.1 Obligations
Each party shall protect the Confidential Information of the other party.

Section 5.2 Exceptions
Confidential Information shall not include publicly available data.
"#;

        let result = build_structure(contract);

        // Should have no errors
        assert!(!result.has_errors(), "Unexpected errors: {:?}", result.errors);

        let structure = result.value;

        // Should have 5 top-level articles
        assert_eq!(structure.sections.len(), 5, "Expected 5 articles");

        // Verify article identifiers
        let article_names: Vec<&str> = structure
            .sections
            .iter()
            .map(|s| s.header.raw_text.as_str())
            .collect();
        assert_eq!(
            article_names,
            vec!["ARTICLE I", "ARTICLE II", "ARTICLE III", "ARTICLE IV", "ARTICLE V"]
        );

        // ARTICLE I should have 2 sections (1.1, 1.2)
        assert_eq!(structure.sections[0].children.len(), 2);

        // Section 1.1 should have 4 subsections (a, b, c, d)
        let section_1_1 = &structure.sections[0].children[0];
        assert_eq!(section_1_1.children.len(), 4);

        // Section 1.2 should have 2 subsections (a, b)
        let section_1_2 = &structure.sections[0].children[1];
        assert_eq!(section_1_2.children.len(), 2);

        // ARTICLE II should have 3 sections (2.1, 2.2, 2.3)
        assert_eq!(structure.sections[1].children.len(), 3);

        // Section 2.3 should have 2 subsections (a, b)
        let section_2_3 = &structure.sections[1].children[2];
        assert_eq!(section_2_3.children.len(), 2);

        // ARTICLE III should have 2 sections (3.1, 3.2)
        assert_eq!(structure.sections[2].children.len(), 2);

        // Section 3.2 should have 3 subsections (a, b, c)
        let section_3_2 = &structure.sections[2].children[1];
        assert_eq!(section_3_2.children.len(), 3);

        // ARTICLE IV should have 3 sections (4.1, 4.2, 4.3)
        assert_eq!(structure.sections[3].children.len(), 3);

        // ARTICLE V should have 2 sections (5.1, 5.2)
        assert_eq!(structure.sections[4].children.len(), 2);

        // Test total_sections() - should count all nested sections
        let total = structure.total_sections();
        // 5 articles + 2+3+2+3+2 sections + 4+2+2+3 subsections = 5 + 12 + 11 = 28
        assert_eq!(total, 28, "Total sections mismatch");

        // Test find_by_canonical()
        let article_i = structure.find_by_canonical("ARTICLE:R1");
        assert!(article_i.is_some());
        assert!(article_i.unwrap().header.raw_text.contains("ARTICLE I"));

        let section_3_1 = structure.find_by_canonical("SECTION:3.1");
        assert!(section_3_1.is_some());
        assert!(section_3_1.unwrap().header.title.as_deref() == Some("Fees"));

        // Test flatten()
        let flattened = structure.flatten();
        assert_eq!(flattened.len(), 28);

        // First item should be ARTICLE I
        assert!(flattened[0].header.raw_text.contains("ARTICLE I"));

        // Second item should be Section 1.1 (depth-first)
        assert!(flattened[1].header.raw_text.contains("1.1"));

        // Verify depth calculation
        assert_eq!(structure.sections[0].depth(), 2); // ARTICLE I = Named + Roman = 2
        assert_eq!(section_1_1.depth(), 3); // Section 1.1 = Named + Numeric(2 parts) = 1 + 2 = 3
    }
}

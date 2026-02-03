//! Section reference detection for contract text.
//!
//! This module provides `SectionReferenceResolver` which detects references to
//! sections like "Section 3.1", "Article IV", "Exhibit A", "this Section", etc.
//!
//! References are detected per-line but resolved to actual sections in a
//! separate document-level pass (`SectionReferenceLinker`).

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver, TextTag};

use crate::section_header::{SectionIdentifier, SectionKind};
use crate::utils::parse_roman;

/// A reference to a section within the document.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionReference {
    /// The referenced section identifier (if detectable)
    pub target: Option<SectionIdentifier>,
    /// The full reference text (e.g., "Section 3.1 above")
    pub reference_text: String,
    /// Type of reference
    pub reference_type: ReferenceType,
    /// Optional purpose of the reference (condition, definition, override)
    pub purpose: Option<ReferencePurpose>,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
}

/// The type of section reference.
#[derive(Debug, Clone, PartialEq)]
pub enum ReferenceType {
    /// Direct reference: "Section 3.1"
    Direct,
    /// Range reference: "Sections 3.1 through 3.5"
    Range {
        start: SectionIdentifier,
        end: SectionIdentifier,
    },
    /// List reference: "Sections 3.1, 3.2, and 3.4"
    List(Vec<SectionIdentifier>),
    /// Relative reference: "this Section", "the foregoing", "above"
    Relative(RelativeReference),
    /// External document reference: "Section 5 of the Master Agreement"
    External { document: String },
}

/// Types of relative section references.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativeReference {
    /// "this Section", "this Article"
    This,
    /// "the foregoing Section", "the foregoing provisions"
    Foregoing,
    /// "Section 3.1 above"
    Above,
    /// "Section 3.1 below"
    Below,
    /// "Section 3.1 hereof"
    Hereof,
    /// "as defined herein", "contained herein"
    Herein,
}

/// The purpose/context of the reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferencePurpose {
    /// "subject to Section 3.1" - a condition on an obligation
    Condition,
    /// "as defined in Section 1" - points to a definition
    Definition,
    /// "notwithstanding Section 4" - an override/exception
    Override,
    /// "in accordance with Section 2" - conformity requirement
    Conformity,
    /// "except as provided in Section 5" - an exception
    Exception,
    /// "pursuant to Section 6" - authority/basis
    Authority,
}

/// Resolver for detecting section references in contract text.
#[derive(Debug, Clone, Default)]
pub struct SectionReferenceResolver {
    /// Confidence for "Section X" / "Article Y" patterns
    direct_reference_confidence: f64,
    /// Confidence for relative references ("this Section")
    relative_reference_confidence: f64,
}

impl SectionReferenceResolver {
    pub fn new() -> Self {
        Self {
            direct_reference_confidence: 0.9,
            relative_reference_confidence: 0.7,
        }
    }

    /// Detect relative suffix (above, below, hereof, herein)
    fn detect_relative_suffix(text: &str) -> Option<RelativeReference> {
        let lower = text.to_lowercase();
        if lower.starts_with("above") {
            return Some(RelativeReference::Above);
        }
        if lower.starts_with("below") {
            return Some(RelativeReference::Below);
        }
        if lower.starts_with("hereof") {
            return Some(RelativeReference::Hereof);
        }
        if lower.starts_with("herein") {
            return Some(RelativeReference::Herein);
        }
        None
    }

    /// Check if text is a section keyword
    fn section_keyword(text: &str) -> Option<SectionKind> {
        match text.to_uppercase().as_str() {
            "SECTION" | "SEC" => Some(SectionKind::Section),
            "ARTICLE" => Some(SectionKind::Article),
            "PARAGRAPH" | "PARA" => Some(SectionKind::Paragraph),
            "CLAUSE" => Some(SectionKind::Clause),
            "EXHIBIT" => Some(SectionKind::Exhibit),
            "SCHEDULE" => Some(SectionKind::Schedule),
            "ANNEX" => Some(SectionKind::Annex),
            "APPENDIX" => Some(SectionKind::Appendix),
            _ => None,
        }
    }
}

impl Resolver for SectionReferenceResolver {
    type Attr = SectionReference;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut assignments = Vec::new();

        // Pattern 1: "this/such Section/Article" (relative reference)
        for (sel, (_, first_text)) in
            selection.find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            let lower = first_text.to_lowercase();
            if lower != "this" && lower != "such" {
                continue;
            }

            // Look for whitespace + keyword
            let mut current = sel.clone();
            if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                current = ws_sel;
            } else {
                continue;
            }

            if let Some((kw_sel, (_, kw_text))) =
                current.match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
            {
                if Self::section_keyword(kw_text).is_some() {
                    let reference_text = format!("{} {}", first_text, kw_text);
                    assignments.push(kw_sel.finish_with_attr(SectionReference {
                        target: None,
                        reference_text,
                        reference_type: ReferenceType::Relative(RelativeReference::This),
                        purpose: None,
                        confidence: self.relative_reference_confidence,
                    }));
                }
            }
        }

        // Pattern 2: "Section/Article + identifier" (direct reference)
        for (sel, (_, keyword_text)) in
            selection.find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            let kind = match Self::section_keyword(keyword_text) {
                Some(k) => k,
                None => continue,
            };

            // Look for whitespace + identifier
            let mut current = sel.clone();
            if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                current = ws_sel;
            } else {
                continue;
            }

            // Try Roman numeral first (for "Article IV")
            if let Some((id_sel, (_, id_text))) =
                current.match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
            {
                if let Some((value, uppercase)) = parse_roman(id_text) {
                    let raw_text = format!("{} {}", keyword_text, id_text);

                    // Check for relative suffix
                    let mut final_sel = id_sel.clone();
                    let mut ref_type = ReferenceType::Direct;

                    if let Some((suffix_sel, _)) =
                        final_sel.match_first_forwards(&x::whitespace())
                    {
                        if let Some((word_sel, (_, suffix_text))) = suffix_sel
                            .match_first_forwards(
                                &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                            )
                        {
                            if let Some(rel) = Self::detect_relative_suffix(suffix_text) {
                                ref_type = ReferenceType::Relative(rel);
                                final_sel = word_sel;
                            }
                        }
                    }

                    // Check it's not a header (look for punctuation like - or :)
                    let mut is_header = false;
                    if let Some((ws_sel, _)) = final_sel.match_first_forwards(&x::whitespace()) {
                        if let Some((_, (_, punc_text))) = ws_sel
                            .match_first_forwards(&x::all((x::attr_eq(&TextTag::PUNC), x::token_text())))
                        {
                            is_header = punc_text == "-" || punc_text == ":";
                        }
                    }

                    if !is_header {
                        let identifier = SectionIdentifier::Named {
                            kind,
                            sub_identifier: Some(Box::new(SectionIdentifier::Roman {
                                value,
                                uppercase,
                            })),
                        };

                        assignments.push(final_sel.finish_with_attr(SectionReference {
                            target: Some(identifier),
                            reference_text: raw_text,
                            reference_type: ref_type,
                            purpose: None,
                            confidence: self.direct_reference_confidence,
                        }));
                    }
                    continue;
                }
            }

            // Try numeric identifier ("Section 3" or "Section 3.1")
            if let Some((id_sel, (_, id_text))) =
                current.match_first_forwards(&x::all((x::attr_eq(&TextTag::NATN), x::token_text())))
            {
                let mut parts: Vec<u32> = vec![id_text.parse().unwrap_or(0)];
                let mut full_text = format!("{} {}", keyword_text, id_text);
                let mut final_sel = id_sel.clone();

                // Try to extend with ".X" parts
                loop {
                    if let Some((dot_sel, _)) = final_sel.match_first_forwards(&x::attr_eq(&'.')) {
                        if let Some((num_sel, (_, num_text))) = dot_sel.match_first_forwards(
                            &x::all((x::attr_eq(&TextTag::NATN), x::token_text())),
                        ) {
                            if let Ok(num) = num_text.parse::<u32>() {
                                parts.push(num);
                                full_text.push('.');
                                full_text.push_str(num_text);
                                final_sel = num_sel;
                                continue;
                            }
                        }
                    }
                    break;
                }

                // Check for relative suffix
                let mut ref_type = ReferenceType::Direct;
                if let Some((suffix_sel, _)) = final_sel.match_first_forwards(&x::whitespace()) {
                    if let Some((word_sel, (_, suffix_text))) = suffix_sel
                        .match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
                    {
                        if let Some(rel) = Self::detect_relative_suffix(suffix_text) {
                            ref_type = ReferenceType::Relative(rel);
                            final_sel = word_sel;
                        }
                    }
                }

                // Check it's not a header (look for punctuation like - or :)
                let mut is_header = false;
                if let Some((ws_sel, _)) = final_sel.match_first_forwards(&x::whitespace()) {
                    if let Some((_, (_, punc_text))) = ws_sel
                        .match_first_forwards(&x::all((x::attr_eq(&TextTag::PUNC), x::token_text())))
                    {
                        is_header = punc_text == "-" || punc_text == ":";
                    }
                }

                if !is_header {
                    let identifier = SectionIdentifier::Named {
                        kind,
                        sub_identifier: Some(Box::new(SectionIdentifier::Numeric { parts })),
                    };

                    assignments.push(final_sel.finish_with_attr(SectionReference {
                        target: Some(identifier),
                        reference_text: full_text,
                        reference_type: ref_type,
                        purpose: None,
                        confidence: self.direct_reference_confidence,
                    }));
                }
                continue;
            }

            // Try single letter identifier ("Exhibit A")
            if let Some((id_sel, (_, id_text))) =
                current.match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
            {
                // Single uppercase letter
                if id_text.len() == 1 && id_text.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) {
                    let letter = id_text.chars().next().unwrap();
                    let raw_text = format!("{} {}", keyword_text, id_text);

                    let identifier = SectionIdentifier::Named {
                        kind,
                        sub_identifier: Some(Box::new(SectionIdentifier::Alpha {
                            letter,
                            parenthesized: false,
                            uppercase: true,
                        })),
                    };

                    assignments.push(id_sel.finish_with_attr(SectionReference {
                        target: Some(identifier),
                        reference_text: raw_text,
                        reference_type: ReferenceType::Direct,
                        purpose: None,
                        confidence: self.direct_reference_confidence,
                    }));
                }
            }
        }

        // Pattern 3: standalone "herein" / "hereof"
        for (sel, (_, text)) in
            selection.find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            let lower = text.to_lowercase();
            if lower == "herein" || lower == "hereof" {
                let rel = if lower == "herein" {
                    RelativeReference::Herein
                } else {
                    RelativeReference::Hereof
                };

                assignments.push(sel.finish_with_attr(SectionReference {
                    target: None,
                    reference_text: text.to_string(),
                    reference_type: ReferenceType::Relative(rel),
                    purpose: None,
                    confidence: self.relative_reference_confidence * 0.8,
                }));
            }
        }

        assignments
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layered_nlp::{create_line_from_string, LLLineDisplay};

    fn detect_references(text: &str) -> Vec<SectionReference> {
        let line = create_line_from_string(text).run(&SectionReferenceResolver::new());
        line.find(&x::attr::<SectionReference>())
            .into_iter()
            .map(|found| (*found.attr()).clone())
            .collect()
    }

    #[test]
    fn test_direct_section_reference() {
        let refs = detect_references("The Company shall comply with Section 3.1");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].reference_text, "Section 3.1");
        assert!(matches!(refs[0].reference_type, ReferenceType::Direct));
        assert!(refs[0].target.is_some());
    }

    #[test]
    fn test_article_reference() {
        let refs = detect_references("See Article IV for details");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].reference_text, "Article IV");
        assert!(matches!(refs[0].reference_type, ReferenceType::Direct));
    }

    #[test]
    fn test_this_section_reference() {
        let refs = detect_references("Except as provided in this Section");
        assert!(refs
            .iter()
            .any(|r| matches!(r.reference_type, ReferenceType::Relative(RelativeReference::This))));
    }

    #[test]
    fn test_section_above_reference() {
        let refs = detect_references("As described in Section 2.1 above");
        assert!(refs.iter().any(|r| matches!(
            r.reference_type,
            ReferenceType::Relative(RelativeReference::Above)
        )));
    }

    #[test]
    fn test_nested_numeric_reference() {
        let refs = detect_references("Pursuant to Section 1.2.3 hereof");
        assert!(refs.iter().any(|r| r.reference_text.contains("1.2.3")));
        assert!(refs.iter().any(|r| matches!(
            r.reference_type,
            ReferenceType::Relative(RelativeReference::Hereof)
        )));
    }

    #[test]
    fn test_exhibit_reference() {
        let refs = detect_references("The services described in Exhibit A");
        assert!(refs.iter().any(|r| r.reference_text == "Exhibit A"));
    }

    #[test]
    fn test_herein_standalone() {
        let refs = detect_references("As defined herein");
        assert!(refs.iter().any(|r| matches!(
            r.reference_type,
            ReferenceType::Relative(RelativeReference::Herein)
        )));
    }

    #[test]
    fn test_header_reference_detected_line_level() {
        // Line-level detection should still emit a reference candidate for headers.
        // Document-level filtering is handled by SectionReferenceLinker.
        let refs = detect_references("Section 3.1 - Definitions");
        assert!(
            refs.iter().any(|r| r.reference_text.contains("Section 3.1")),
            "Expected line-level reference detection for header-like text"
        );
    }

    #[test]
    fn test_multiple_references() {
        let refs = detect_references("See Section 1.1 and Article II for details");
        assert!(refs.len() >= 2);
    }

    #[test]
    fn test_display_snapshot() {
        let line = create_line_from_string("The Company shall comply with Section 3.1 hereof")
            .run(&SectionReferenceResolver::new());
        let mut display = LLLineDisplay::new(&line);
        display.include::<SectionReference>();
        insta::assert_snapshot!(display.to_string());
    }
}

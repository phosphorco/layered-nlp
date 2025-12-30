//! Section header detection resolver.
//!
//! This resolver identifies section headers in contract text using patterns:
//! - `Section 3.1` - Numeric section references
//! - `ARTICLE I` / `Article 1` - Article headers with Roman or Arabic numerals
//! - `(a)` / `(i)` - Alphabetic/Roman list items
//!
//! Note: This is a per-line resolver. It detects headers on individual lines.
//! The `DocumentStructureBuilder` is responsible for building the hierarchical
//! structure from detected headers.

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver, TextTag};

use crate::utils::parse_roman;

/// Represents a detected section header.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionHeader {
    /// The section identifier
    pub identifier: SectionIdentifier,
    /// Optional title following the identifier (e.g., "Payment Terms")
    pub title: Option<String>,
    /// Raw text of the identifier for display (e.g., "ARTICLE I", "Section 3.1")
    pub raw_text: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
}

/// A normalized section identifier.
#[derive(Debug, Clone, PartialEq)]
pub enum SectionIdentifier {
    /// Numeric: "1", "1.1", "1.1.1" -> parts = \[1\], \[1, 1\], \[1, 1, 1\]
    Numeric { parts: Vec<u32> },
    /// Roman numeral: "I", "II", "IV" -> value is the numeric equivalent
    Roman { value: u32, uppercase: bool },
    /// Alphabetic: "A", "a", "(a)" -> letter and whether parenthesized
    Alpha {
        letter: char,
        parenthesized: bool,
        uppercase: bool,
    },
    /// Named section: "ARTICLE", "Section", "Exhibit"
    Named {
        kind: SectionKind,
        /// The identifier after the keyword (numeric, roman, or alpha)
        sub_identifier: Option<Box<SectionIdentifier>>,
    },
}

impl SectionIdentifier {
    /// Get a canonical string representation for matching/comparison.
    pub fn canonical(&self) -> String {
        match self {
            SectionIdentifier::Numeric { parts } => {
                parts
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(".")
            }
            SectionIdentifier::Roman { value, .. } => format!("R{}", value),
            SectionIdentifier::Alpha { letter, .. } => letter.to_ascii_lowercase().to_string(),
            SectionIdentifier::Named { kind, sub_identifier } => {
                let kind_str = format!("{:?}", kind).to_uppercase();
                match sub_identifier {
                    Some(sub) => format!("{}:{}", kind_str, sub.canonical()),
                    None => kind_str,
                }
            }
        }
    }

    /// Estimate the nesting depth of this identifier.
    pub fn depth(&self) -> u8 {
        match self {
            SectionIdentifier::Numeric { parts } => parts.len() as u8,
            SectionIdentifier::Roman { .. } => 1,
            SectionIdentifier::Alpha { parenthesized, .. } => {
                // (a), (b) etc. are typically at the deepest level, nested under numbered sections
                // Numbered sections like 1.1.1 have depth 4 (Named + 3 parts), so (a) needs depth 5
                if *parenthesized { 5 } else { 4 }
            }
            SectionIdentifier::Named { sub_identifier, .. } => {
                1 + sub_identifier.as_ref().map(|s| s.depth()).unwrap_or(0)
            }
        }
    }
}

/// The kind of named section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Article,
    Section,
    Subsection,
    Paragraph,
    Clause,
    Exhibit,
    Schedule,
    Annex,
    Appendix,
    Recital,
    /// Special: A "Definitions" or "Defined Terms" section.
    /// Useful for the diff engine to identify term-by-term alignment zones.
    Definition,
}

impl SectionKind {
    /// Try to parse a section kind from text.
    fn from_text(text: &str) -> Option<Self> {
        match text.to_uppercase().as_str() {
            "ARTICLE" => Some(SectionKind::Article),
            "SECTION" | "SEC" | "SEC." => Some(SectionKind::Section),
            "SUBSECTION" | "SUBSEC" | "SUBSEC." => Some(SectionKind::Subsection),
            "PARAGRAPH" | "PARA" | "PARA." => Some(SectionKind::Paragraph),
            "CLAUSE" => Some(SectionKind::Clause),
            "EXHIBIT" => Some(SectionKind::Exhibit),
            "SCHEDULE" => Some(SectionKind::Schedule),
            "ANNEX" => Some(SectionKind::Annex),
            "APPENDIX" => Some(SectionKind::Appendix),
            "RECITAL" => Some(SectionKind::Recital),
            "DEFINITIONS" | "DEFINED" => Some(SectionKind::Definition),
            _ => None,
        }
    }
}

/// Resolver for detecting section headers in contract text.
#[derive(Debug, Clone)]
pub struct SectionHeaderResolver {
    /// Confidence for "ARTICLE I" / "Section 1" patterns
    named_section_confidence: f64,
    /// Confidence for standalone numeric "1." / "1.1"
    numeric_confidence: f64,
    /// Confidence for "(a)" / "(i)" patterns
    list_item_confidence: f64,
}

impl Default for SectionHeaderResolver {
    fn default() -> Self {
        Self {
            named_section_confidence: 0.95,
            numeric_confidence: 0.80,
            list_item_confidence: 0.70,
        }
    }
}

impl SectionHeaderResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Try to parse a numeric section identifier like "1" or "1.1.1"
    fn parse_numeric(text: &str) -> Option<Vec<u32>> {
        let parts: Result<Vec<u32>, _> = text
            .split('.')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<u32>())
            .collect();

        parts.ok().filter(|p| !p.is_empty())
    }

    /// Try to extract title text following the identifier.
    fn extract_title(&self, selection: &LLSelection) -> Option<String> {
        let mut current = selection.clone();
        let mut title_parts = Vec::new();

        // Skip optional whitespace
        if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
            current = ws_sel;
        }

        // Skip optional separator (dash, colon, period)
        if let Some((sep_sel, _)) = current.match_first_forwards(&x::any_of((
            x::attr_eq(&'-'),
            x::attr_eq(&':'),
            x::attr_eq(&'.'),
        ))) {
            current = sep_sel;
            // Skip whitespace after separator
            if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                current = ws_sel;
            }
        }

        // Collect title words (stop at end of line or certain punctuation)
        loop {
            if let Some((word_sel, (_, text))) = current
                .match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
            {
                title_parts.push(text.to_string());
                current = word_sel;

                // Check for whitespace between words
                if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                    current = ws_sel;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if title_parts.is_empty() {
            None
        } else {
            Some(title_parts.join(" "))
        }
    }
}

impl Resolver for SectionHeaderResolver {
    type Attr = SectionHeader;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut assignments = Vec::new();

        // Pattern 1: Named sections like "ARTICLE I" or "Section 1.1"
        for (sel, (_, keyword_text)) in
            selection.find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            if let Some(kind) = SectionKind::from_text(keyword_text) {
                // Look for identifier after the keyword
                let mut current = sel.clone();

                // Skip whitespace
                if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                    current = ws_sel;
                }

                // Try to match Roman numeral
                if let Some((id_sel, (_, id_text))) = current
                    .match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
                {
                    if let Some((value, uppercase)) = parse_roman(id_text) {
                        let title = self.extract_title(&id_sel);
                        let raw_text = format!("{} {}", keyword_text, id_text);

                        assignments.push(id_sel.finish_with_attr(SectionHeader {
                            identifier: SectionIdentifier::Named {
                                kind,
                                sub_identifier: Some(Box::new(SectionIdentifier::Roman {
                                    value,
                                    uppercase,
                                })),
                            },
                            title,
                            raw_text,
                            confidence: self.named_section_confidence,
                        }));
                        continue;
                    }
                }

                // Try to match numeric identifier
                if let Some((id_sel, (_, id_text))) = current
                    .match_first_forwards(&x::all((x::attr_eq(&TextTag::NATN), x::token_text())))
                {
                    // Check for dotted notation (1.1, 1.1.1)
                    let mut full_text = id_text.to_string();
                    let mut final_sel = id_sel.clone();

                    loop {
                        // Try to match dot
                        if let Some((dot_sel, _)) =
                            final_sel.match_first_forwards(&x::attr_eq(&'.'))
                        {
                            // Try to match number after dot
                            if let Some((num_sel, (_, num_text))) = dot_sel.match_first_forwards(
                                &x::all((x::attr_eq(&TextTag::NATN), x::token_text())),
                            ) {
                                full_text.push('.');
                                full_text.push_str(num_text);
                                final_sel = num_sel;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    if let Some(parts) = Self::parse_numeric(&full_text) {
                        let title = self.extract_title(&final_sel);
                        let raw_text = format!("{} {}", keyword_text, full_text);

                        assignments.push(final_sel.finish_with_attr(SectionHeader {
                            identifier: SectionIdentifier::Named {
                                kind,
                                sub_identifier: Some(Box::new(SectionIdentifier::Numeric { parts })),
                            },
                            title,
                            raw_text,
                            confidence: self.named_section_confidence,
                        }));
                    }
                }
            }
        }

        // Pattern 2: Standalone numeric at start of line like "1." or "1.1"
        // (Only if no named section was found on this line)
        if assignments.is_empty() {
            if let Some((first_sel, (_, first_text))) = selection.match_first_forwards(&x::all((
                x::attr_eq(&TextTag::NATN),
                x::token_text(),
            ))) {
                let mut full_text = first_text.to_string();
                let mut final_sel = first_sel.clone();

                // Check for dotted notation
                loop {
                    if let Some((dot_sel, _)) = final_sel.match_first_forwards(&x::attr_eq(&'.')) {
                        if let Some((num_sel, (_, num_text))) = dot_sel.match_first_forwards(
                            &x::all((x::attr_eq(&TextTag::NATN), x::token_text())),
                        ) {
                            full_text.push('.');
                            full_text.push_str(num_text);
                            final_sel = num_sel;
                        } else {
                            // Ends with dot (like "1.")
                            final_sel = dot_sel;
                            break;
                        }
                    } else {
                        break;
                    }
                }

                if let Some(parts) = Self::parse_numeric(&full_text) {
                    let title = self.extract_title(&final_sel);

                    assignments.push(final_sel.finish_with_attr(SectionHeader {
                        identifier: SectionIdentifier::Numeric { parts },
                        title,
                        raw_text: full_text,
                        confidence: self.numeric_confidence,
                    }));
                }
            }
        }

        // Pattern 3: Parenthesized list items like "(a)" or "(i)"
        for (sel, _) in selection.find_by(&x::attr_eq(&'(')) {
            if let Some((inner_sel, (_, inner_text))) =
                sel.match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
            {
                // Check for closing paren
                if let Some((close_sel, _)) = inner_sel.match_first_forwards(&x::attr_eq(&')')) {
                    // Single letter?
                    if inner_text.len() == 1 {
                        let letter = inner_text.chars().next().unwrap();
                        if letter.is_ascii_alphabetic() {
                            let title = self.extract_title(&close_sel);

                            assignments.push(close_sel.finish_with_attr(SectionHeader {
                                identifier: SectionIdentifier::Alpha {
                                    letter,
                                    parenthesized: true,
                                    uppercase: letter.is_uppercase(),
                                },
                                title,
                                raw_text: format!("({})", inner_text),
                                confidence: self.list_item_confidence,
                            }));
                            continue;
                        }
                    }

                    // Roman numeral in parens like "(i)", "(ii)"?
                    if let Some((value, _)) = parse_roman(inner_text) {
                        let title = self.extract_title(&close_sel);

                        assignments.push(close_sel.finish_with_attr(SectionHeader {
                            identifier: SectionIdentifier::Roman {
                                value,
                                uppercase: inner_text.chars().next().unwrap().is_uppercase(),
                            },
                            title,
                            raw_text: format!("({})", inner_text),
                            confidence: self.list_item_confidence,
                        }));
                    }
                }
            }
        }

        assignments
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::parse_roman;
    use layered_nlp::{create_line_from_string, LLLineDisplay};

    fn detect_headers(text: &str) -> Vec<SectionHeader> {
        let line = create_line_from_string(text).run(&SectionHeaderResolver::new());
        line.find(&x::attr::<SectionHeader>())
            .into_iter()
            .map(|f| (*f.attr()).clone())
            .collect()
    }

    #[test]
    fn test_article_roman() {
        let headers = detect_headers("ARTICLE I - DEFINITIONS");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].raw_text, "ARTICLE I");
        assert_eq!(headers[0].title, Some("DEFINITIONS".to_string()));
        assert!(matches!(
            &headers[0].identifier,
            SectionIdentifier::Named {
                kind: SectionKind::Article,
                sub_identifier: Some(sub)
            } if matches!(**sub, SectionIdentifier::Roman { value: 1, uppercase: true })
        ));
    }

    #[test]
    fn test_section_numeric() {
        let headers = detect_headers("Section 3.1 Payment Terms");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].raw_text, "Section 3.1");
        assert_eq!(headers[0].title, Some("Payment Terms".to_string()));
    }

    #[test]
    fn test_section_nested_numeric() {
        let headers = detect_headers("Section 1.2.3 Detailed Requirements");
        assert_eq!(headers.len(), 1);
        assert!(matches!(
            &headers[0].identifier,
            SectionIdentifier::Named {
                sub_identifier: Some(sub),
                ..
            } if matches!(**sub, SectionIdentifier::Numeric { ref parts } if parts == &[1, 2, 3])
        ));
    }

    #[test]
    fn test_parenthesized_alpha() {
        let headers = detect_headers("(a) First item in list");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].raw_text, "(a)");
        assert!(matches!(
            headers[0].identifier,
            SectionIdentifier::Alpha {
                letter: 'a',
                parenthesized: true,
                uppercase: false
            }
        ));
    }

    #[test]
    fn test_parenthesized_roman() {
        let headers = detect_headers("(ii) Second sub-item");
        assert_eq!(headers.len(), 1);
        assert!(matches!(
            headers[0].identifier,
            SectionIdentifier::Roman {
                value: 2,
                uppercase: false
            }
        ));
    }

    #[test]
    fn test_canonical_identifiers() {
        let numeric = SectionIdentifier::Numeric { parts: vec![1, 2, 3] };
        assert_eq!(numeric.canonical(), "1.2.3");

        let roman = SectionIdentifier::Roman { value: 4, uppercase: true };
        assert_eq!(roman.canonical(), "R4");

        let alpha = SectionIdentifier::Alpha {
            letter: 'B',
            parenthesized: true,
            uppercase: true,
        };
        assert_eq!(alpha.canonical(), "b");

        let named = SectionIdentifier::Named {
            kind: SectionKind::Article,
            sub_identifier: Some(Box::new(SectionIdentifier::Roman {
                value: 1,
                uppercase: true,
            })),
        };
        assert_eq!(named.canonical(), "ARTICLE:R1");
    }

    #[test]
    fn test_depth() {
        assert_eq!(SectionIdentifier::Numeric { parts: vec![1] }.depth(), 1);
        assert_eq!(SectionIdentifier::Numeric { parts: vec![1, 2] }.depth(), 2);
        assert_eq!(SectionIdentifier::Numeric { parts: vec![1, 2, 3] }.depth(), 3);

        let named = SectionIdentifier::Named {
            kind: SectionKind::Section,
            sub_identifier: Some(Box::new(SectionIdentifier::Numeric { parts: vec![1, 2] })),
        };
        assert_eq!(named.depth(), 3); // 1 (named) + 2 (numeric parts)
    }

    #[test]
    fn test_display_snapshot() {
        let line = create_line_from_string("ARTICLE II - SERVICES").run(&SectionHeaderResolver::new());
        let mut display = LLLineDisplay::new(&line);
        display.include::<SectionHeader>();
        insta::assert_snapshot!(display.to_string(), @r###"
        ARTICLE     II     -     SERVICES
        ╰────────────╯SectionHeader { identifier: Named { kind: Article, sub_identifier: Some(Roman { value: 2, uppercase: true }) }, title: Some("SERVICES"), raw_text: "ARTICLE II", confidence: 0.95 }
        "###);
    }

    #[test]
    fn test_extended_roman_numerals() {
        // Test parsing beyond XX (20)
        assert_eq!(parse_roman("XXI"), Some((21, true)));
        assert_eq!(parse_roman("XXV"), Some((25, true)));
        assert_eq!(parse_roman("XXX"), Some((30, true)));
        assert_eq!(parse_roman("XL"), Some((40, true)));
        assert_eq!(parse_roman("L"), Some((50, true)));
        assert_eq!(parse_roman("LX"), Some((60, true)));
        assert_eq!(parse_roman("XC"), Some((90, true)));
        assert_eq!(parse_roman("C"), Some((100, true)));

        // Lowercase
        assert_eq!(parse_roman("xxi"), Some((21, false)));
        assert_eq!(parse_roman("xlii"), Some((42, false)));

        // Complex numerals
        assert_eq!(parse_roman("MCMXCIV"), Some((1994, true)));
        assert_eq!(parse_roman("MMXXV"), Some((2025, true)));

        // Invalid inputs
        assert_eq!(parse_roman(""), None);
        assert_eq!(parse_roman("ABC"), None);
        assert_eq!(parse_roman("IIII"), Some((4, true))); // Non-standard but parseable
    }

    #[test]
    fn test_article_extended_roman() {
        // Test detection of articles with higher Roman numerals
        let headers = detect_headers("ARTICLE XXV - MISCELLANEOUS");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].raw_text, "ARTICLE XXV");
        assert!(matches!(
            headers[0].identifier,
            SectionIdentifier::Named {
                kind: SectionKind::Article,
                sub_identifier: Some(ref id)
            } if matches!(**id, SectionIdentifier::Roman { value: 25, uppercase: true })
        ));
    }
}

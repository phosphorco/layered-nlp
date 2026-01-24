//! List marker detection for legal document structures.
//!
//! Detects common list marker patterns:
//! - Parenthesized letters: (a), (b), (c)
//! - Parenthesized roman numerals: (i), (ii), (iii)
//! - Parenthesized digits: (1), (2), (3)
//! - Numbered periods: 1., 2., 3.

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver, TextTag};

/// Represents different list marker styles found in legal and structured documents.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ListMarker {
    /// Parenthesized letter marker like (a), (b), (c)
    ParenthesizedLetter { letter: char },
    /// Parenthesized roman numeral like (i), (ii), (iii)
    ParenthesizedRoman { numeral: String },
    /// Parenthesized digit like (1), (2), (3)
    ParenthesizedDigit { digit: u32 },
    /// Numbered period marker like 1., 2., 3.
    NumberedPeriod { number: u32 },
}

/// Resolver that detects list markers in text.
///
/// Recognizes multi-token patterns:
/// - `(a)` = PUNC('(') + WORD('a') + PUNC(')')
/// - `(1)` = PUNC('(') + NATN('1') + PUNC(')')
/// - `1.` = NATN('1') + PUNC('.')
#[derive(Debug, Default)]
pub struct ListMarkerResolver;

impl ListMarkerResolver {
    pub fn new() -> Self {
        Self
    }

    /// Check if a string is a valid roman numeral (lowercase only).
    /// Valid characters: i, v, x (for numerals up to about 39)
    fn is_roman_numeral(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }

        // Must contain only valid roman numeral characters
        let valid_chars = ['i', 'v', 'x', 'l', 'c', 'd', 'm'];
        if !s.chars().all(|c| valid_chars.contains(&c)) {
            return false;
        }

        // Simple validation: common roman numerals used in legal lists
        // Typically: i, ii, iii, iv, v, vi, vii, viii, ix, x, xi, xii, etc.
        let common_numerals = [
            "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi", "xii", "xiii",
            "xiv", "xv", "xvi", "xvii", "xviii", "xix", "xx", "xxi", "xxii", "xxiii", "xxiv",
            "xxv", "xxvi", "xxvii", "xxviii", "xxix", "xxx",
        ];
        common_numerals.contains(&s)
    }

    /// Check if a string is a single lowercase letter (a-z)
    fn is_single_letter(s: &str) -> bool {
        let chars: Vec<char> = s.chars().collect();
        chars.len() == 1 && chars[0].is_ascii_lowercase()
    }
}

impl Resolver for ListMarkerResolver {
    type Attr = ListMarker;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut assignments = Vec::new();

        // Pattern 1: Numbered period markers - digit + '.'
        // Detected first to maintain document order in results
        // Find all natural number tokens
        let natn_matches = selection.find_by(&x::attr_eq(&TextTag::NATN));

        for (num_sel, _) in natn_matches {
            // Try to match a period after the number
            if let Some((extended_sel, _)) = num_sel.match_first_forwards(&x::token_has_any(&['.']))
            {
                // Get the number text from the original selection
                let num_text_matches = num_sel.find_by(&x::token_text());
                if let Some((_, num_text)) = num_text_matches.first() {
                    if let Ok(number) = num_text.parse::<u32>() {
                        assignments.push(extended_sel.finish_with_attr(ListMarker::NumberedPeriod {
                            number,
                        }));
                    }
                }
            }
        }

        // Pattern 2: Parenthesized markers - '(' + content + ')'
        // Find all open parentheses
        let open_paren_matches = selection.find_by(&x::token_has_any(&['(']));

        for (paren_sel, _) in open_paren_matches {
            // Try to match: content token + closing paren
            // Using seq to match the next two tokens
            let content_and_close = x::seq((x::token_text(), x::token_has_any(&[')'])));

            if let Some((extended_sel, (content_text, _))) =
                paren_sel.match_first_forwards(&content_and_close)
            {
                let content_lower = content_text.to_lowercase();

                // Classify the content
                // Check roman numerals BEFORE single letters since valid roman numerals
                // like 'i', 'v', 'x' should be classified as roman, not letter
                let marker = if Self::is_roman_numeral(&content_lower) {
                    Some(ListMarker::ParenthesizedRoman {
                        numeral: content_lower,
                    })
                } else if Self::is_single_letter(&content_lower) {
                    Some(ListMarker::ParenthesizedLetter {
                        letter: content_lower.chars().next().unwrap(),
                    })
                } else if let Ok(digit) = content_text.parse::<u32>() {
                    Some(ListMarker::ParenthesizedDigit { digit })
                } else {
                    None
                };

                if let Some(m) = marker {
                    assignments.push(extended_sel.finish_with_attr(m));
                }
            }
        }

        assignments
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layered_nlp::{create_line_from_input_tokens, InputToken, LLLineDisplay};

    fn test_line(text: &str) -> layered_nlp::LLLine {
        create_line_from_input_tokens(
            vec![InputToken::text(text.to_string(), Vec::new())],
            |text| text.encode_utf16().count(),
        )
        .run(&ListMarkerResolver::new())
    }

    #[test]
    fn test_parenthesized_letter() {
        let ll_line = test_line("(a) This is item a.");

        let mut display = LLLineDisplay::new(&ll_line);
        display.include::<ListMarker>();

        insta::assert_snapshot!(display, @r###"
        (  a  )     This     is     item     a  .
        ╰─────╯ParenthesizedLetter { letter: 'a' }
        "###);
    }

    #[test]
    fn test_parenthesized_letters_sequence() {
        let ll_line = test_line("(a) first (b) second (c) third");

        let mut display = LLLineDisplay::new(&ll_line);
        display.include::<ListMarker>();

        insta::assert_snapshot!(display, @r###"
        (  a  )     first     (  b  )     second     (  c  )     third
        ╰─────╯ParenthesizedLetter { letter: 'a' }
                              ╰─────╯ParenthesizedLetter { letter: 'b' }
                                                     ╰─────╯ParenthesizedLetter { letter: 'c' }
        "###);
    }

    #[test]
    fn test_parenthesized_roman() {
        let ll_line = test_line("(i) first (ii) second (iii) third");

        let mut display = LLLineDisplay::new(&ll_line);
        display.include::<ListMarker>();

        insta::assert_snapshot!(display, @r###"
        (  i  )     first     (  ii  )     second     (  iii  )     third
        ╰─────╯ParenthesizedRoman { numeral: "i" }
                              ╰──────╯ParenthesizedRoman { numeral: "ii" }
                                                      ╰───────╯ParenthesizedRoman { numeral: "iii" }
        "###);
    }

    #[test]
    fn test_parenthesized_digit() {
        let ll_line = test_line("(1) first (2) second (10) tenth");

        let mut display = LLLineDisplay::new(&ll_line);
        display.include::<ListMarker>();

        insta::assert_snapshot!(display, @r###"
        (  1  )     first     (  2  )     second     (  10  )     tenth
        ╰─────╯ParenthesizedDigit { digit: 1 }
                              ╰─────╯ParenthesizedDigit { digit: 2 }
                                                     ╰──────╯ParenthesizedDigit { digit: 10 }
        "###);
    }

    #[test]
    fn test_numbered_period() {
        let ll_line = test_line("1. First item. 2. Second item.");

        let mut display = LLLineDisplay::new(&ll_line);
        display.include::<ListMarker>();

        insta::assert_snapshot!(display, @r###"
        1  .     First     item  .     2  .     Second     item  .
        ╰──╯NumberedPeriod { number: 1 }
                                       ╰──╯NumberedPeriod { number: 2 }
        "###);
    }

    #[test]
    fn test_mixed_markers() {
        let ll_line = test_line("1. Overview (a) Details (i) Specifics");

        let mut display = LLLineDisplay::new(&ll_line);
        display.include::<ListMarker>();

        insta::assert_snapshot!(display, @r###"
        1  .     Overview     (  a  )     Details     (  i  )     Specifics
        ╰──╯NumberedPeriod { number: 1 }
                              ╰─────╯ParenthesizedLetter { letter: 'a' }
                                                      ╰─────╯ParenthesizedRoman { numeral: "i" }
        "###);
    }

    #[test]
    fn test_roman_numerals_extended() {
        let ll_line = test_line("(iv) four (v) five (ix) nine (x) ten");

        let mut display = LLLineDisplay::new(&ll_line);
        display.include::<ListMarker>();

        insta::assert_snapshot!(display, @r###"
        (  iv  )     four     (  v  )     five     (  ix  )     nine     (  x  )     ten
        ╰──────╯ParenthesizedRoman { numeral: "iv" }
                              ╰─────╯ParenthesizedRoman { numeral: "v" }
                                                   ╰──────╯ParenthesizedRoman { numeral: "ix" }
                                                                         ╰─────╯ParenthesizedRoman { numeral: "x" }
        "###);
    }

    #[test]
    fn test_no_markers() {
        let ll_line = test_line("This text has no list markers.");

        let mut display = LLLineDisplay::new(&ll_line);
        display.include::<ListMarker>();

        insta::assert_snapshot!(display, @"This     text     has     no     list     markers  .");
    }

    #[test]
    fn test_uppercase_letter_becomes_lowercase() {
        let ll_line = test_line("(A) Capital letter marker");

        let mut display = LLLineDisplay::new(&ll_line);
        display.include::<ListMarker>();

        insta::assert_snapshot!(display, @r###"
        (  A  )     Capital     letter     marker
        ╰─────╯ParenthesizedLetter { letter: 'a' }
        "###);
    }

    #[test]
    fn test_is_roman_numeral() {
        assert!(ListMarkerResolver::is_roman_numeral("i"));
        assert!(ListMarkerResolver::is_roman_numeral("ii"));
        assert!(ListMarkerResolver::is_roman_numeral("iii"));
        assert!(ListMarkerResolver::is_roman_numeral("iv"));
        assert!(ListMarkerResolver::is_roman_numeral("v"));
        assert!(ListMarkerResolver::is_roman_numeral("vi"));
        assert!(ListMarkerResolver::is_roman_numeral("vii"));
        assert!(ListMarkerResolver::is_roman_numeral("viii"));
        assert!(ListMarkerResolver::is_roman_numeral("ix"));
        assert!(ListMarkerResolver::is_roman_numeral("x"));
        assert!(ListMarkerResolver::is_roman_numeral("xi"));
        assert!(ListMarkerResolver::is_roman_numeral("xii"));
        assert!(ListMarkerResolver::is_roman_numeral("xx"));
        assert!(ListMarkerResolver::is_roman_numeral("xxx"));

        // Invalid cases
        assert!(!ListMarkerResolver::is_roman_numeral(""));
        assert!(!ListMarkerResolver::is_roman_numeral("a"));
        assert!(!ListMarkerResolver::is_roman_numeral("abc"));
        assert!(!ListMarkerResolver::is_roman_numeral("iiii")); // Not in common list
        assert!(!ListMarkerResolver::is_roman_numeral("hello"));
    }

    #[test]
    fn test_is_single_letter() {
        assert!(ListMarkerResolver::is_single_letter("a"));
        assert!(ListMarkerResolver::is_single_letter("z"));
        assert!(ListMarkerResolver::is_single_letter("m"));

        // Invalid cases
        assert!(!ListMarkerResolver::is_single_letter(""));
        assert!(!ListMarkerResolver::is_single_letter("A"));
        assert!(!ListMarkerResolver::is_single_letter("ab"));
        assert!(!ListMarkerResolver::is_single_letter("1"));
    }
}

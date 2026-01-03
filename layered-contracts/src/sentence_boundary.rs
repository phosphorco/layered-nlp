//! Sentence boundary detection resolver.
//!
//! This resolver identifies sentence boundaries in text using punctuation patterns:
//! - Period, question mark, exclamation point
//! - Optionally semicolons (when `include_semicolons` is enabled)
//!
//! The resolver filters out abbreviations that would otherwise be false positives
//! (e.g., "Dr.", "Mr.", "Inc.").

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver, TextTag};
use std::collections::HashSet;

/// A detected sentence boundary.
#[derive(Clone, PartialEq, Eq)]
pub struct SentenceBoundary {
    pub confidence: SentenceConfidence,
}

impl std::fmt::Debug for SentenceBoundary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SentenceBoundary({:?})", self.confidence)
    }
}

/// Confidence level for a detected sentence boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SentenceConfidence {
    /// Clear sentence boundary (period + capital letter follows)
    High,
    /// Probable boundary (period at end, but no following context)
    Medium,
    /// Possible boundary (needs verification)
    Low,
}

/// Resolver for detecting sentence boundaries in text.
pub struct SentenceBoundaryResolver {
    abbreviations: HashSet<String>,
    /// When true, treat semicolons as sentence boundaries
    include_semicolons: bool,
}

impl SentenceBoundaryResolver {
    pub fn new() -> Self {
        let mut abbreviations = HashSet::new();

        // Common abbreviations that should NOT be treated as sentence boundaries
        let common_abbrevs = [
            "dr", "mr", "mrs", "ms", "prof", "sr", "jr",
            "inc", "ltd", "corp", "co", "llc",
            "e.g", "i.e", "vs", "etc", "approx",
            "u.s", "u.k", "p.m", "a.m",
            "st", "ave", "blvd", "dept", "fig",
        ];

        for abbrev in &common_abbrevs {
            abbreviations.insert(abbrev.to_string());
        }

        SentenceBoundaryResolver {
            abbreviations,
            include_semicolons: false,
        }
    }

    /// Enable semicolon detection as sentence boundaries.
    ///
    /// This is useful for legal contracts where semicolons often separate
    /// independent clauses that function as separate obligations.
    pub fn with_semicolons(mut self) -> Self {
        self.include_semicolons = true;
        self
    }

    pub fn with_custom_abbreviations(mut self, abbreviations: &[&str]) -> Self {
        for abbrev in abbreviations {
            self.abbreviations.insert(abbrev.to_lowercase());
        }
        self
    }

    fn is_sentence_ending_punctuation(&self, text: &str) -> bool {
        if matches!(text, "." | "?" | "!") {
            return true;
        }
        if self.include_semicolons && text == ";" {
            return true;
        }
        false
    }

    fn is_abbreviation(&self, text: &str) -> bool {
        // Remove trailing period if present and check
        let normalized = text.trim_end_matches('.').to_lowercase();
        self.abbreviations.contains(&normalized)
    }

    fn starts_with_uppercase(text: &str) -> bool {
        text.chars().next().map_or(false, |c| c.is_uppercase())
    }

    /// Check if there is a sentence boundary between two selections.
    ///
    /// This scans the text between `earlier` and `later` for any sentence-ending
    /// punctuation (period, question mark, exclamation, and semicolon if enabled).
    ///
    /// Returns `true` if a boundary is found between the two selections.
    pub fn has_boundary_between(&self, earlier: &LLSelection, later: &LLSelection) -> bool {
        // Walk forward from 'earlier' token by token, checking for sentence-ending punctuation
        let mut current = earlier.clone();

        while let Some((next_sel, text)) = current.match_first_forwards(&x::token_text()) {
            // Check if we've reached or passed the later selection.
            // selection_is_before(a, b) = a.split_with(b) gives [Some, None]
            // So "not before" = [None, _] OR [_, Some]
            let [before, after] = next_sel.split_with(later);

            // If next_sel is NOT entirely before later, we've reached/passed it
            if before.is_none() || after.is_some() {
                return false;
            }

            // Check if this token is sentence-ending punctuation.
            // The `text` returned by match_first_forwards is the text of the matched token.
            if self.is_sentence_ending_punctuation(text) {
                return true;
            }

            current = next_sel;
        }

        false
    }
}

impl Default for SentenceBoundaryResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver for SentenceBoundaryResolver {
    type Attr = SentenceBoundary;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        // Find all punctuation tokens
        let punctuation_matches = selection.find_by(&x::all((
            x::attr_eq(&TextTag::PUNC),
            x::token_text(),
        )));

        let mut assignments = Vec::new();

        for (punc_sel, punc_tuple) in punctuation_matches {
            // Extract text from tuple ((), &str)
            let punc_text = punc_tuple.1;

            // Only process sentence-ending punctuation
            if !self.is_sentence_ending_punctuation(punc_text) {
                continue;
            }

            // Check if this is part of an abbreviation
            // Look backwards to get the previous token
            if let Some((_prev_sel, prev_text)) = punc_sel.match_first_backwards(&x::token_text()) {
                if self.is_abbreviation(prev_text) {
                    // This is an abbreviation, not a sentence boundary
                    continue;
                }
            }

            // Semicolons get Medium confidence (clause-level, not sentence-level)
            if punc_text == ";" {
                assignments.push(punc_sel.finish_with_attr(SentenceBoundary {
                    confidence: SentenceConfidence::Medium,
                }));
                continue;
            }

            // Look for the next WORD token in the remainder of the line
            // after() returns everything after the current selection
            let confidence = if let Some(after_punc) = punc_sel.after() {
                // find_by searches ALL matching tokens and returns them in order
                let word_matches = after_punc.find_by(&x::all((
                    x::attr_eq(&TextTag::WORD),
                    x::token_text()
                )));

                // Get the first WORD match (skips any whitespace naturally)
                if let Some((_, (_, next_word))) = word_matches.into_iter().next() {
                    if Self::starts_with_uppercase(next_word) {
                        SentenceConfidence::High
                    } else {
                        SentenceConfidence::Low
                    }
                } else {
                    // No WORD token found after punctuation
                    SentenceConfidence::Medium
                }
            } else {
                // No tokens after punctuation (end of line)
                SentenceConfidence::Medium
            };

            assignments.push(punc_sel.finish_with_attr(SentenceBoundary { confidence }));
        }

        assignments
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layered_nlp::{create_line_from_string, LLLineDisplay};

    fn detect_boundaries(text: &str) -> Vec<SentenceBoundary> {
        let line = create_line_from_string(text).run(&SentenceBoundaryResolver::new());
        line.find(&x::attr::<SentenceBoundary>())
            .into_iter()
            .map(|f| (*f.attr()).clone())
            .collect()
    }

    fn detect_boundaries_with_semicolons(text: &str) -> Vec<SentenceBoundary> {
        let line = create_line_from_string(text)
            .run(&SentenceBoundaryResolver::new().with_semicolons());
        line.find(&x::attr::<SentenceBoundary>())
            .into_iter()
            .map(|f| (*f.attr()).clone())
            .collect()
    }

    #[test]
    fn test_simple_period() {
        let boundaries = detect_boundaries("Hello world. Goodbye.");
        assert_eq!(boundaries.len(), 2);
        assert_eq!(boundaries[0].confidence, SentenceConfidence::High);
        assert_eq!(boundaries[1].confidence, SentenceConfidence::Medium);
    }

    #[test]
    fn test_question_mark() {
        let boundaries = detect_boundaries("How are you? I am fine.");
        assert_eq!(boundaries.len(), 2);
        assert_eq!(boundaries[0].confidence, SentenceConfidence::High);
    }

    #[test]
    fn test_exclamation() {
        let boundaries = detect_boundaries("Stop! Wait for me.");
        assert_eq!(boundaries.len(), 2);
        assert_eq!(boundaries[0].confidence, SentenceConfidence::High);
    }

    #[test]
    fn test_abbreviation_filtering() {
        let boundaries = detect_boundaries("Dr. Smith went to the store.");
        // Should only detect the final period, not "Dr."
        assert_eq!(boundaries.len(), 1);
    }

    #[test]
    fn test_multiple_abbreviations() {
        let boundaries = detect_boundaries("Mr. and Mrs. Jones arrived.");
        // Should only detect the final period
        assert_eq!(boundaries.len(), 1);
    }

    #[test]
    fn test_semicolon_disabled_by_default() {
        let boundaries = detect_boundaries("First clause; second clause.");
        // Only the period should be detected
        assert_eq!(boundaries.len(), 1);
    }

    #[test]
    fn test_semicolon_enabled() {
        let boundaries = detect_boundaries_with_semicolons("First clause; second clause.");
        assert_eq!(boundaries.len(), 2);
        // Semicolon gets Medium confidence
        assert_eq!(boundaries[0].confidence, SentenceConfidence::Medium);
        // Period at end also Medium (no following capital)
        assert_eq!(boundaries[1].confidence, SentenceConfidence::Medium);
    }

    #[test]
    fn test_lowercase_following() {
        let boundaries = detect_boundaries("end of sentence. lowercase start");
        assert_eq!(boundaries.len(), 1);
        assert_eq!(boundaries[0].confidence, SentenceConfidence::Low);
    }

    #[test]
    fn test_display_snapshot() {
        let line = create_line_from_string("Hello world. Goodbye!")
            .run(&SentenceBoundaryResolver::new());
        let mut display = LLLineDisplay::new(&line);
        display.include::<SentenceBoundary>();
        insta::assert_snapshot!(display.to_string(), @r###"
        Hello     world  .     Goodbye  !
                         ╰SentenceBoundary(High)
                                        ╰SentenceBoundary(Medium)
        "###);
    }

    #[test]
    fn test_semicolon_display_snapshot() {
        let line = create_line_from_string("First clause; Second clause.")
            .run(&SentenceBoundaryResolver::new().with_semicolons());
        let mut display = LLLineDisplay::new(&line);
        display.include::<SentenceBoundary>();
        insta::assert_snapshot!(display.to_string(), @r###"
        First     clause  ;     Second     clause  .
                          ╰SentenceBoundary(Medium)
                                                   ╰SentenceBoundary(Medium)
        "###);
    }
}

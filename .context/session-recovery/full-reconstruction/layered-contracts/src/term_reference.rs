//! Term reference resolver for linking mentions to defined terms.
//!
//! This resolver identifies subsequent uses of defined terms and links them back
//! to their definitions. For example, if "Contractor" is defined via
//! `(the "Contractor")`, later bare mentions of `Contractor` get tagged as references.

use std::collections::HashMap;

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver, TextTag};

use crate::defined_term::{DefinedTerm, DefinitionType};
use crate::scored::Scored;

/// A reference to a previously defined term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermReference {
    /// The term name being referenced (matches DefinedTerm.term_name)
    pub term_name: String,
    /// How the original term was defined
    pub definition_type: DefinitionType,
}

/// Resolver for detecting references to defined terms.
///
/// Links subsequent mentions of defined terms back to their definitions.
/// Requires that `DefinedTermResolver` has already been run on the line.
#[derive(Default)]
pub struct TermReferenceResolver;

impl TermReferenceResolver {
    /// Create a new resolver.
    pub fn new() -> Self {
        Self
    }

    /// Calculate confidence score based on case matching and article presence.
    ///
    /// | Scenario | Confidence | Rationale |
    /// |----------|------------|-----------|
    /// | Exact case match + capitalized | 0.90 | Strong signal (proper noun behavior) |
    /// | Case-insensitive match + capitalized | 0.85 | Likely intentional reference |
    /// | Exact match + lowercase | 0.70 | Could be generic word usage |
    /// | Case-insensitive match + lowercase | 0.65 | Weakest signal |
    /// | With article "the" preceding | +0.05 | "the Contractor" is more specific |
    fn calculate_confidence(&self, surface: &str, canonical: &str, has_article: bool) -> f64 {
        let surface_capitalized = surface
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);
        let exact_case = surface == canonical;

        let mut score: f64 = match (exact_case, surface_capitalized) {
            (true, true) => 0.90,
            (false, true) => 0.85,
            (true, false) => 0.70,
            (false, false) => 0.65,
        };

        if has_article {
            score += 0.05;
        }

        score.clamp(0.0, 1.0)
    }

    /// Check if the previous non-whitespace token is a determiner article.
    /// Currently recognizes: the, The, THE, this, such
    fn has_preceding_article(&self, selection: &LLSelection) -> bool {
        // Try to match backwards: skip whitespace, then check for article
        if let Some((ws_sel, _)) = selection.match_first_backwards(&x::whitespace()) {
            if let Some((_, text)) = ws_sel.match_first_backwards(&x::token_text()) {
                return matches!(text.to_lowercase().as_str(), "the" | "this" | "such");
            }
        } else {
            // No whitespace, check directly
            if let Some((_, text)) = selection.match_first_backwards(&x::token_text()) {
                return matches!(text.to_lowercase().as_str(), "the" | "this" | "such");
            }
        }
        false
    }

    /// Match a multi-word term starting from the given selection.
    /// Returns (extended_selection, surface_text) if successful.
    /// The surface_text is the actual text of all matched words joined by spaces.
    fn match_multiword_term(
        &self,
        start_sel: &LLSelection,
        first_word_text: &str,
        term_words: &[&str],
    ) -> Option<(LLSelection, String)> {
        if term_words.len() <= 1 {
            return Some((start_sel.clone(), first_word_text.to_string()));
        }

        let mut current = start_sel.clone();
        let mut surface_parts = vec![first_word_text.to_string()];

        for expected_word in term_words.iter().skip(1) {
            // Skip whitespace
            if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                current = ws_sel;
            } else {
                return None;
            }

            // Match next word
            if let Some((word_sel, (_, text))) =
                current.match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
            {
                if text.to_lowercase() != expected_word.to_lowercase() {
                    return None;
                }
                surface_parts.push(text.to_string());
                current = word_sel;
            } else {
                return None;
            }
        }

        Some((current, surface_parts.join(" ")))
    }

    /// Check if a word selection is contained within any definition selection.
    ///
    /// We compare selections by checking if expanding the word selection backwards and forwards
    /// to the definition boundaries yields the same result as the definition selection.
    fn is_contained_in_definition(
        &self,
        word_sel: &LLSelection,
        definition_sels: &[LLSelection],
    ) -> bool {
        for def_sel in definition_sels {
            // A word is contained in a definition if expanding the word to cover
            // the definition range gives us the definition selection.
            // Since selections are ordered by token index, we can check:
            // - word comes after or at def start
            // - word comes before or at def end
            //
            // We use a simple heuristic: if we can reach from word to definition boundaries
            // by extending, then word is contained. We check by seeing if the definition
            // selection can be "reached" from the word selection.

            // Check if word_sel is a subset by seeing if def_sel "contains" it.
            // We do this by checking if match_backwards from word reaches def start,
            // and match_forwards from word reaches def end.
            //
            // Alternative: just check selection equality when we trim the definition
            // to just contain the word's range.

            // Simple check: the word is contained if def_sel equals word_sel after
            // attempting to shrink def_sel to just the word.
            // Since we can't shrink easily, let's compare by checking if word_sel
            // when expanded equals def_sel.

            // Simpler: Check if word_sel equals def_sel (they're the same span)
            if word_sel == def_sel {
                return true;
            }

            // Check if word is contained by trying to expand word to match def.
            // If word is at def's boundaries already (via match_backwards/forwards),
            // then word must be inside def.

            // We need a containment check. Let's do it by expanding word both ways
            // and seeing if we can reach def's bounds.
            // This is getting complex. Let me use a different strategy:
            // Since both selections share the same LLLine, the only way to compare
            // containment is to try operations that would reveal the relative positions.

            // Approach: Use split_with to see if splitting word by def gives us
            // empty parts (meaning word is inside def).
            let [before, after] = word_sel.split_with(def_sel);

            // If both before and after are None, the word is entirely inside def
            // (or they're equal, which we checked above)
            if before.is_none() && after.is_none() {
                return true;
            }
        }
        false
    }
}

impl Resolver for TermReferenceResolver {
    type Attr = Scored<TermReference>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        // Step 1: Collect all defined terms from Layer 2
        let defined_terms: Vec<_> = selection
            .find_by(&x::attr::<Scored<DefinedTerm>>())
            .into_iter()
            .map(|(sel, scored)| {
                (
                    scored.value.term_name.clone(),
                    scored.value.definition_type.clone(),
                    sel, // Keep the selection for overlap checking
                )
            })
            .collect();

        if defined_terms.is_empty() {
            return vec![];
        }

        // Collect just the definition selections for containment checking
        let definition_sels: Vec<LLSelection> =
            defined_terms.iter().map(|(_, _, sel)| sel.clone()).collect();

        // Step 2: Build lookup map (lowercase first word -> Vec of (full term, original case, type))
        // This handles multi-word terms and allows multiple definitions with same first word
        let mut term_lookup: HashMap<String, Vec<(String, DefinitionType)>> = HashMap::new();

        for (term_name, def_type, _) in &defined_terms {
            let first_word = term_name
                .split_whitespace()
                .next()
                .unwrap_or(term_name)
                .to_lowercase();
            term_lookup
                .entry(first_word)
                .or_default()
                .push((term_name.clone(), def_type.clone()));
        }

        // Step 3: Find word tokens that match term names
        let mut results = Vec::new();
        let word_matches: Vec<_> = selection
            .find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
            .into_iter()
            .collect();

        // Track selections we've already matched (for multi-word terms)
        let mut matched_selections: Vec<LLSelection> = Vec::new();

        for (word_sel, (_, text)) in word_matches {
            // Skip if this word selection is already part of a matched multi-word term
            if matched_selections
                .iter()
                .any(|matched| matched == &word_sel)
            {
                continue;
            }

            // Skip if this word is contained in a definition span
            if self.is_contained_in_definition(&word_sel, &definition_sels) {
                continue;
            }

            let lowercase = text.to_lowercase();

            // Check if this word starts any defined term
            if let Some(candidates) = term_lookup.get(&lowercase) {
                // Try to match the longest term first (multi-word terms)
                // Store: (selection, canonical_term_name, definition_type, surface_text)
                let mut best_match: Option<(LLSelection, &str, &DefinitionType, String)> = None;

                for (term_name, def_type) in candidates {
                    let term_words: Vec<&str> = term_name.split_whitespace().collect();

                    if term_words.len() > 1 {
                        // Try multi-word match
                        if let Some((extended_sel, surface_text)) =
                            self.match_multiword_term(&word_sel, text, &term_words)
                        {
                            // Check if this extended selection overlaps with definitions
                            if !self.is_contained_in_definition(&extended_sel, &definition_sels) {
                                // Prefer longer matches (compare by checking if we got more words)
                                let is_better = best_match.is_none()
                                    || term_words.len()
                                        > best_match.as_ref().unwrap().1.split_whitespace().count();

                                if is_better {
                                    best_match =
                                        Some((extended_sel, term_name, def_type, surface_text));
                                }
                            }
                        }
                    } else {
                        // Single word term - already matched
                        if best_match.is_none() {
                            best_match =
                                Some((word_sel.clone(), term_name, def_type, text.to_string()));
                        }
                    }
                }

                if let Some((matched_sel, term_name, def_type, surface_text)) = best_match {
                    let has_article = self.has_preceding_article(&word_sel);
                    let confidence =
                        self.calculate_confidence(&surface_text, term_name, has_article);

                    // If multi-word, track all word selections that are part of this match
                    if term_name.split_whitespace().count() > 1 {
                        // Mark all words in this multi-word term as matched
                        let mut current = word_sel.clone();
                        matched_selections.push(current.clone());

                        let term_words: Vec<&str> = term_name.split_whitespace().collect();
                        for _ in term_words.iter().skip(1) {
                            if let Some((ws_sel, _)) =
                                current.match_first_forwards(&x::whitespace())
                            {
                                current = ws_sel;
                            }
                            if let Some((next_word_sel, _)) = current.match_first_forwards(
                                &x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
                            ) {
                                matched_selections.push(next_word_sel.clone());
                                current = next_word_sel;
                            }
                        }
                    }

                    results.push(matched_sel.finish_with_attr(Scored::rule_based(
                        TermReference {
                            term_name: term_name.to_string(),
                            definition_type: def_type.clone(),
                        },
                        confidence,
                        "term_reference",
                    )));
                }
            }
        }

        results
    }
}

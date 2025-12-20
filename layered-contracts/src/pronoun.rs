//! Pronoun resolution for contract language analysis.
//!
//! This resolver identifies pronouns and links them to potential antecedents
//! (the nouns/terms they refer to). In contract language, pronouns like "it",
//! "they", "its" typically refer to previously defined parties or terms.
//!
//! Example:
//! ```text
//! The Company shall deliver. It must comply.
//!     ╰─────╯DefinedTerm
//!                            ╰╯PronounReference { "It" -> [Company: 0.85] }
//! ```

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver, TextTag};
use layered_part_of_speech::Tag;

use crate::defined_term::DefinedTerm;
use crate::scored::Scored;
use crate::term_reference::TermReference;

/// A candidate antecedent for a pronoun.
#[derive(Debug, Clone, PartialEq)]
pub struct AntecedentCandidate {
    /// The text of the potential antecedent
    pub text: String,
    /// Whether this candidate is a defined term
    pub is_defined_term: bool,
    /// Distance in tokens from the pronoun (lower = closer)
    pub token_distance: usize,
    /// Individual confidence score for this candidate
    pub confidence: f64,
}

/// A pronoun with its potential antecedents.
#[derive(Debug, Clone, PartialEq)]
pub struct PronounReference {
    /// The pronoun text (e.g., "it", "they", "its")
    pub pronoun: String,
    /// The grammatical person/type of pronoun
    pub pronoun_type: PronounType,
    /// Candidate antecedents, sorted by confidence (highest first)
    pub candidates: Vec<AntecedentCandidate>,
}

/// Classification of pronoun types for agreement checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PronounType {
    /// Third person singular neuter (it, its, itself)
    ThirdSingularNeuter,
    /// Third person singular masculine (he, him, his, himself)
    ThirdSingularMasculine,
    /// Third person singular feminine (she, her, hers, herself)
    ThirdSingularFeminine,
    /// Third person plural (they, them, their, theirs, themselves)
    ThirdPlural,
    /// Relative/demonstrative (this, that, these, those, which, who)
    Relative,
    /// Unknown or unclassified
    Other,
}

impl PronounType {
    /// Classify a pronoun by its text.
    pub fn from_text(text: &str) -> Self {
        match text.to_lowercase().as_str() {
            "it" | "its" | "itself" => Self::ThirdSingularNeuter,
            "he" | "him" | "his" | "himself" => Self::ThirdSingularMasculine,
            "she" | "her" | "hers" | "herself" => Self::ThirdSingularFeminine,
            "they" | "them" | "their" | "theirs" | "themselves" => Self::ThirdPlural,
            "this" | "that" | "these" | "those" | "which" | "who" | "whom" | "whose" => {
                Self::Relative
            }
            _ => Self::Other,
        }
    }

    /// Check if this pronoun type can agree with a singular entity (like a company).
    pub fn can_be_singular(&self) -> bool {
        matches!(
            self,
            Self::ThirdSingularNeuter
                | Self::ThirdSingularMasculine
                | Self::ThirdSingularFeminine
                | Self::Relative
                | Self::Other
        )
    }

    /// Check if this pronoun type can agree with a plural entity.
    pub fn can_be_plural(&self) -> bool {
        matches!(self, Self::ThirdPlural | Self::Relative | Self::Other)
    }
}

/// Resolver for detecting pronouns and their antecedents.
///
/// Requires that `POSTagResolver`, `DefinedTermResolver`, and `TermReferenceResolver`
/// have already been run on the line.
pub struct PronounResolver {
    /// Base confidence for nearest candidate
    base_confidence: f64,
    /// Bonus for being a defined term
    defined_term_bonus: f64,
    /// Bonus for same sentence
    same_sentence_bonus: f64,
    /// Bonus for number/gender agreement
    agreement_bonus: f64,
    /// Penalty when multiple candidates exist
    multiple_candidates_penalty: f64,
}

impl PronounResolver {
    /// Create a new resolver with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a resolver with custom scoring parameters.
    pub fn with_scoring(
        base_confidence: f64,
        defined_term_bonus: f64,
        same_sentence_bonus: f64,
        agreement_bonus: f64,
        multiple_candidates_penalty: f64,
    ) -> Self {
        Self {
            base_confidence,
            defined_term_bonus,
            same_sentence_bonus,
            agreement_bonus,
            multiple_candidates_penalty,
        }
    }

    /// Check if there's a sentence boundary (period, exclamation, question mark)
    /// between two selections.
    fn has_sentence_boundary_between(&self, earlier: &LLSelection, later: &LLSelection) -> bool {
        // Start from the earlier selection and try to find punctuation going forward
        let mut current = earlier.clone();

        while let Some((next_sel, _)) = current.match_first_forwards(&x::token_text()) {
            // Check if we've reached or passed the later selection
            if next_sel == *later {
                return false;
            }

            // Check if this is sentence-ending punctuation
            if let Some((_, punc)) =
                next_sel.find_first_by(&x::all((x::attr_eq(&TextTag::PUNC), x::token_text())))
            {
                if matches!(punc.1, "." | "!" | "?") {
                    return true;
                }
            }

            current = next_sel;
        }

        false
    }

    /// Calculate confidence for a candidate based on scoring factors.
    fn calculate_candidate_confidence(
        &self,
        token_distance: usize,
        is_defined_term: bool,
        same_sentence: bool,
        pronoun_type: PronounType,
        antecedent_text: &str,
    ) -> f64 {
        let mut confidence = self.base_confidence;

        // Distance penalty: closer is better
        // Reduce confidence by ~0.02 per token of distance, up to a max of 0.30 reduction
        let distance_penalty = (token_distance as f64 * 0.02).min(0.30);
        confidence -= distance_penalty;

        // Bonuses
        if is_defined_term {
            confidence += self.defined_term_bonus;
        }
        if same_sentence {
            confidence += self.same_sentence_bonus;
        }

        // Agreement bonus: check if pronoun type matches antecedent number
        if self.check_agreement(pronoun_type, antecedent_text) {
            confidence += self.agreement_bonus;
        }

        confidence.clamp(0.0, 1.0)
    }

    /// Check if a pronoun type agrees with an antecedent.
    ///
    /// In contract language:
    /// - Singular pronouns (it, he, she) typically refer to single entities
    ///   (Company, Contractor, Party)
    /// - Plural pronouns (they) typically refer to plural entities (Parties)
    /// - Relative pronouns (this, that) can refer to either
    fn check_agreement(&self, pronoun_type: PronounType, antecedent_text: &str) -> bool {
        let lower = antecedent_text.to_lowercase();

        // Detect likely plural antecedents - be conservative to avoid false positives
        // Only consider explicit plural patterns common in contracts
        let is_likely_plural = lower.ends_with("ies") // Parties, Companies
            || lower == "parties"
            || lower == "both"
            || lower == "all"
            // Words ending in 's' that are typically plural nouns in contracts
            || (lower.ends_with("ors") && lower.len() > 4) // Contractors, Vendors
            || (lower.ends_with("ees") && lower.len() > 4) // Employees, Licensees
            || (lower.ends_with("ers") && lower.len() > 4); // Members, Partners

        match pronoun_type {
            PronounType::ThirdPlural => is_likely_plural,
            PronounType::ThirdSingularNeuter
            | PronounType::ThirdSingularMasculine
            | PronounType::ThirdSingularFeminine => !is_likely_plural,
            // Relative pronouns can agree with anything
            PronounType::Relative | PronounType::Other => true,
        }
    }

    /// Calculate overall reference confidence based on candidates.
    fn calculate_reference_confidence(&self, candidates: &[AntecedentCandidate]) -> f64 {
        if candidates.is_empty() {
            return 0.0;
        }

        // Start with the best candidate's confidence
        let mut confidence = candidates[0].confidence;

        // Apply penalty if there are multiple plausible candidates
        if candidates.len() > 1 {
            // Only penalize if the second candidate is reasonably confident
            if candidates[1].confidence > 0.4 {
                confidence -= self.multiple_candidates_penalty;
            }
        }

        confidence.clamp(0.0, 1.0)
    }

    /// Check if selection A comes before selection B.
    /// Returns true if A ends before B starts.
    fn selection_is_before(&self, a: &LLSelection, b: &LLSelection) -> bool {
        // split_with(other) returns [before, after] where:
        // - before = part of self before other
        // - after = part of self after other
        //
        // If we call a.split_with(b):
        // - If a is entirely before b: before = Some(a), after = None
        // - If a is entirely after b: before = None, after = Some(a)
        // - If a contains b: before = Some(...), after = Some(...)
        // - If b contains a: before = None, after = None
        //
        // So to check if a is before b, we need after to be None
        // and before to be Some (covering all of a)
        let [before, after] = a.split_with(b);
        before.is_some() && after.is_none()
    }

    /// Collect potential antecedents from defined terms and term references.
    fn collect_antecedents(
        &self,
        selection: &LLSelection,
        pronoun_sel: &LLSelection,
    ) -> Vec<(LLSelection, String, bool)> {
        let mut antecedents = Vec::new();

        // Collect from DefinedTerm spans (the actual definitions)
        for (sel, scored) in selection.find_by(&x::attr::<Scored<DefinedTerm>>()) {
            // Only consider antecedents that appear before the pronoun
            if self.selection_is_before(&sel, pronoun_sel) {
                antecedents.push((sel, scored.value.term_name.clone(), true));
            }
        }

        // Collect from TermReference spans (references to defined terms)
        for (sel, scored) in selection.find_by(&x::attr::<Scored<TermReference>>()) {
            if self.selection_is_before(&sel, pronoun_sel) {
                antecedents.push((sel, scored.value.term_name.clone(), true));
            }
        }

        // Also collect plain nouns/proper nouns that aren't defined terms
        for (sel, (_, text)) in
            selection.find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            if !self.selection_is_before(&sel, pronoun_sel) {
                continue;
            }

            // Check if this word is tagged as a noun or proper noun
            let is_noun = !sel.find_by(&x::attr_eq(&Tag::Noun)).is_empty()
                || !sel.find_by(&x::attr_eq(&Tag::ProperNoun)).is_empty();

            if !is_noun {
                continue;
            }

            // Skip if it's already in our antecedents (as a defined term or reference)
            let already_included = antecedents
                .iter()
                .any(|(_, name, _)| name.to_lowercase() == text.to_lowercase());

            if !already_included {
                // Plain nouns are NOT defined terms (is_defined_term = false)
                antecedents.push((sel, text.to_string(), false));
            }
        }

        antecedents
    }

    /// Estimate token distance between two selections.
    /// Since we can't access internal indices, we count tokens between them.
    fn estimate_token_distance(&self, earlier: &LLSelection, later: &LLSelection) -> usize {
        let mut distance = 0;
        let mut current = earlier.clone();

        while let Some((next_sel, _)) = current.match_first_forwards(&x::token_text()) {
            distance += 1;
            if next_sel == *later {
                return distance;
            }
            current = next_sel;
        }

        distance
    }
}

impl Default for PronounResolver {
    fn default() -> Self {
        Self {
            base_confidence: 0.50,
            defined_term_bonus: 0.30,
            same_sentence_bonus: 0.10,
            agreement_bonus: 0.15,
            multiple_candidates_penalty: 0.20,
        }
    }
}

/// Known pronouns that we resolve (3rd person + relative)
const RESOLVABLE_PRONOUNS: &[&str] = &[
    // 3rd person singular
    "it", "its", "itself",
    "he", "him", "his", "himself",
    "she", "her", "hers", "herself",
    // 3rd person plural
    "they", "them", "their", "theirs", "themselves",
    // Relative/demonstrative
    "this", "that", "these", "those", "which", "who", "whom", "whose",
];

impl Resolver for PronounResolver {
    type Attr = Scored<PronounReference>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        // Find all pronouns - first try POS tags, then fall back to word-list matching
        let pronouns: Vec<_> = selection
            .find_by(&x::all((
                x::attr_eq(&Tag::Pronoun),
                x::all((x::attr_eq(&TextTag::WORD), x::token_text())),
            )))
            .into_iter()
            .collect();

        // If no POS-tagged pronouns found, use word-list matching as fallback
        let pronouns: Vec<_> = if pronouns.is_empty() {
            selection
                .find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
                .into_iter()
                .filter(|(_, (_, text))| {
                    RESOLVABLE_PRONOUNS.contains(&text.to_lowercase().as_str())
                })
                .map(|(sel, (tag, text))| (sel, ((), (tag, text))))
                .collect()
        } else {
            pronouns
        };

        if pronouns.is_empty() {
            return vec![];
        }

        let mut results = Vec::new();

        for (pronoun_sel, (_, (_, pronoun_text))) in pronouns {
            let pronoun_type = PronounType::from_text(pronoun_text);

            // Skip pronouns we can't meaningfully resolve (e.g., "I", "you", "we")
            if matches!(pronoun_type, PronounType::Other) {
                continue;
            }

            // Collect potential antecedents
            let antecedents = self.collect_antecedents(&selection, &pronoun_sel);

            if antecedents.is_empty() {
                continue;
            }

            // Build candidates with confidence scores
            let mut candidates: Vec<AntecedentCandidate> = antecedents
                .into_iter()
                .map(|(ant_sel, text, is_defined_term)| {
                    let token_distance = self.estimate_token_distance(&ant_sel, &pronoun_sel);
                    let same_sentence =
                        !self.has_sentence_boundary_between(&ant_sel, &pronoun_sel);

                    let confidence = self.calculate_candidate_confidence(
                        token_distance,
                        is_defined_term,
                        same_sentence,
                        pronoun_type,
                        &text,
                    );

                    AntecedentCandidate {
                        text,
                        is_defined_term,
                        token_distance,
                        confidence,
                    }
                })
                .collect();

            // Sort by confidence (highest first)
            candidates.sort_by(|a, b| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Keep only the top candidates (limit to avoid noise)
            candidates.truncate(5);

            let reference_confidence = self.calculate_reference_confidence(&candidates);

            results.push(pronoun_sel.finish_with_attr(Scored::rule_based(
                PronounReference {
                    pronoun: pronoun_text.to_string(),
                    pronoun_type,
                    candidates,
                },
                reference_confidence,
                "pronoun_reference",
            )));
        }

        results
    }
}

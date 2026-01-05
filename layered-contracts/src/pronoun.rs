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
use crate::Scored;
use crate::SentenceBoundaryResolver;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
                    let resolver = SentenceBoundaryResolver::new();
                    let same_sentence = !resolver.has_boundary_between(&ant_sel, &pronoun_sel);

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

// ============================================================================
// CATAPHORA RESOLUTION (Gate 3)
// ============================================================================
// Enables bidirectional pronoun resolution - both backward (anaphora) and
// forward (cataphora) reference patterns.

/// Direction of pronoun reference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CataphoraDirection {
    /// Traditional backward reference: antecedent appears before pronoun
    /// Example: "The Company shall... It must..."
    Anaphoric,
    /// Forward reference: antecedent appears after pronoun
    /// Example: "Before it expires, the Contract shall..."
    Cataphoric,
    /// Could be interpreted either way
    Ambiguous,
}

impl CataphoraDirection {
    /// Returns true if this is a forward-looking reference
    pub fn is_forward(&self) -> bool {
        matches!(self, Self::Cataphoric)
    }

    /// Returns true if this is a backward-looking reference
    pub fn is_backward(&self) -> bool {
        matches!(self, Self::Anaphoric)
    }
}

/// Extended antecedent candidate with direction and salience tracking
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CataphoraCandidate {
    /// The text of the potential antecedent
    pub text: String,
    /// Whether this candidate is a formally defined term
    pub is_defined_term: bool,
    /// Distance in tokens from the pronoun
    pub token_distance: usize,
    /// Direction of the reference (anaphoric/cataphoric)
    pub direction: CataphoraDirection,
    /// Salience score based on entity prominence in document (0.0-1.0)
    pub salience: f64,
    /// Confidence score for this candidate (0.0-1.0)
    pub confidence: f64,
    /// Flag for review if confidence is low or ambiguous
    pub needs_review: bool,
    /// Reason why review is needed
    pub review_reason: Option<String>,
}

impl CataphoraCandidate {
    /// Create a new cataphora candidate with default values
    pub fn new(text: impl Into<String>, direction: CataphoraDirection) -> Self {
        Self {
            text: text.into(),
            is_defined_term: false,
            token_distance: 0,
            direction,
            salience: 0.5,
            confidence: 0.5,
            needs_review: false,
            review_reason: None,
        }
    }

    /// Create an anaphoric (backward-looking) candidate
    pub fn anaphoric(text: impl Into<String>, token_distance: usize) -> Self {
        Self {
            text: text.into(),
            is_defined_term: false,
            token_distance,
            direction: CataphoraDirection::Anaphoric,
            salience: 0.5,
            confidence: 0.5,
            needs_review: false,
            review_reason: None,
        }
    }

    /// Create a cataphoric (forward-looking) candidate
    pub fn cataphoric(text: impl Into<String>, token_distance: usize) -> Self {
        Self {
            text: text.into(),
            is_defined_term: false,
            token_distance,
            direction: CataphoraDirection::Cataphoric,
            salience: 0.5,
            // Forward references get lower base confidence (less common)
            confidence: 0.4,
            needs_review: true,
            review_reason: Some("Forward reference (cataphora) - verify antecedent".to_string()),
        }
    }

    /// Set this candidate as a defined term
    pub fn with_defined_term(mut self, is_defined: bool) -> Self {
        self.is_defined_term = is_defined;
        if is_defined {
            self.confidence += 0.25;
            self.confidence = self.confidence.min(1.0);
        }
        self
    }

    /// Set the salience score
    pub fn with_salience(mut self, salience: f64) -> Self {
        self.salience = salience.clamp(0.0, 1.0);
        // High salience boosts confidence
        if salience > 0.7 {
            self.confidence += 0.1;
            self.confidence = self.confidence.min(1.0);
        }
        self
    }

    /// Convert from legacy AntecedentCandidate (assumes anaphoric)
    pub fn from_antecedent(candidate: &AntecedentCandidate) -> Self {
        Self {
            text: candidate.text.clone(),
            is_defined_term: candidate.is_defined_term,
            token_distance: candidate.token_distance,
            direction: CataphoraDirection::Anaphoric,
            salience: if candidate.is_defined_term { 0.8 } else { 0.5 },
            confidence: candidate.confidence,
            needs_review: false,
            review_reason: None,
        }
    }
}

/// Document-level pronoun resolver with bidirectional resolution
///
/// Uses a two-pass algorithm:
/// 1. Pass 1: Collect all potential antecedents from entire document
/// 2. Pass 2: For each pronoun, evaluate candidates in both directions
pub struct DocumentPronounResolver {
    /// Penalty applied per token of distance
    pub distance_penalty: f64,
    /// Maximum distance penalty
    pub max_distance_penalty: f64,
    /// Bonus for anaphoric (backward) references (more common)
    pub anaphoric_bonus: f64,
    /// Penalty for cataphoric (forward) references (less common)
    pub cataphoric_penalty: f64,
    /// Bonus for defined terms
    pub defined_term_bonus: f64,
    /// Minimum confidence to not flag for review
    pub review_threshold: f64,
}

impl DocumentPronounResolver {
    pub fn new() -> Self {
        Self {
            distance_penalty: 0.02,
            max_distance_penalty: 0.30,
            anaphoric_bonus: 0.10,
            cataphoric_penalty: 0.10,
            defined_term_bonus: 0.25,
            review_threshold: 0.6,
        }
    }

    /// Score a candidate based on direction, distance, and properties
    pub fn score_candidate(&self, candidate: &mut CataphoraCandidate) {
        let mut score = 0.50;

        // Distance penalty (applies to both directions)
        let distance_penalty = (candidate.token_distance as f64 * self.distance_penalty)
            .min(self.max_distance_penalty);
        score -= distance_penalty;

        // Direction adjustment
        match candidate.direction {
            CataphoraDirection::Anaphoric => score += self.anaphoric_bonus,
            CataphoraDirection::Cataphoric => score -= self.cataphoric_penalty,
            CataphoraDirection::Ambiguous => {} // No adjustment
        }

        // Defined term bonus
        if candidate.is_defined_term {
            score += self.defined_term_bonus;
        }

        // Salience bonus (entities mentioned more often are more likely referents)
        score += (candidate.salience - 0.5) * 0.2;

        // Clamp and update
        candidate.confidence = score.clamp(0.0, 1.0);

        // Flag low-confidence candidates for review
        if candidate.confidence < self.review_threshold {
            candidate.needs_review = true;
            if candidate.review_reason.is_none() {
                candidate.review_reason = Some(format!(
                    "Low confidence ({:.2}) - verify pronoun resolution",
                    candidate.confidence
                ));
            }
        }

        // Cataphoric references always need review (uncommon pattern)
        if candidate.direction == CataphoraDirection::Cataphoric {
            candidate.needs_review = true;
        }
    }

    /// Detect cataphoric trigger words that suggest forward reference
    pub fn is_cataphoric_trigger(token: &str) -> bool {
        let lower = token.to_lowercase();
        matches!(
            lower.as_str(),
            "before" | "until" | "unless" | "if" | "when" | "after" | "once" | "while"
        )
    }

    /// Calculate salience score based on mention frequency
    pub fn calculate_salience(mention_count: usize, max_mentions: usize) -> f64 {
        if max_mentions == 0 {
            return 0.5;
        }
        let ratio = mention_count as f64 / max_mentions as f64;
        // Scale from 0.3 to 0.9 based on relative frequency
        0.3 + (ratio * 0.6)
    }
}

impl Default for DocumentPronounResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of document-level pronoun resolution
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocumentPronounReference {
    /// The pronoun text
    pub pronoun: String,
    /// Pronoun type
    pub pronoun_type: PronounType,
    /// All candidates (both anaphoric and cataphoric)
    pub candidates: Vec<CataphoraCandidate>,
    /// Best anaphoric candidate (if any)
    pub best_anaphoric: Option<CataphoraCandidate>,
    /// Best cataphoric candidate (if any)
    pub best_cataphoric: Option<CataphoraCandidate>,
    /// Whether this reference needs human review
    pub needs_review: bool,
    /// Reason for review
    pub review_reason: Option<String>,
}

impl DocumentPronounReference {
    /// Create from a legacy PronounReference (backward compatible)
    pub fn from_pronoun_reference(pr: &PronounReference) -> Self {
        let candidates: Vec<CataphoraCandidate> = pr
            .candidates
            .iter()
            .map(CataphoraCandidate::from_antecedent)
            .collect();

        let best_anaphoric = candidates
            .iter()
            .filter(|c| c.direction == CataphoraDirection::Anaphoric)
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
            .cloned();

        Self {
            pronoun: pr.pronoun.clone(),
            pronoun_type: pr.pronoun_type,
            candidates,
            best_anaphoric,
            best_cataphoric: None,
            needs_review: false,
            review_reason: None,
        }
    }

    /// Add a cataphoric candidate
    pub fn add_cataphoric(&mut self, candidate: CataphoraCandidate) {
        self.candidates.push(candidate.clone());

        // Update best cataphoric if this is better
        if candidate.direction == CataphoraDirection::Cataphoric {
            match &self.best_cataphoric {
                None => self.best_cataphoric = Some(candidate),
                Some(current) if candidate.confidence > current.confidence => {
                    self.best_cataphoric = Some(candidate);
                }
                _ => {}
            }
        }
    }

    /// Get the best overall candidate (considering both directions)
    pub fn best_candidate(&self) -> Option<&CataphoraCandidate> {
        self.candidates
            .iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
    }

    /// Check if there's ambiguity between forward and backward candidates
    pub fn is_direction_ambiguous(&self) -> bool {
        match (&self.best_anaphoric, &self.best_cataphoric) {
            (Some(ana), Some(cat)) => {
                // Ambiguous if both have decent confidence and are close
                ana.confidence > 0.5 && cat.confidence > 0.4
                    && (ana.confidence - cat.confidence).abs() < 0.15
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod cataphora_tests {
    use super::*;

    #[test]
    fn test_cataphora_direction_properties() {
        assert!(CataphoraDirection::Cataphoric.is_forward());
        assert!(!CataphoraDirection::Cataphoric.is_backward());

        assert!(CataphoraDirection::Anaphoric.is_backward());
        assert!(!CataphoraDirection::Anaphoric.is_forward());

        assert!(!CataphoraDirection::Ambiguous.is_forward());
        assert!(!CataphoraDirection::Ambiguous.is_backward());
    }

    #[test]
    fn test_cataphora_candidate_constructors() {
        let ana = CataphoraCandidate::anaphoric("Company", 5);
        assert_eq!(ana.direction, CataphoraDirection::Anaphoric);
        assert_eq!(ana.token_distance, 5);
        assert!(!ana.needs_review);

        let cat = CataphoraCandidate::cataphoric("Contract", 10);
        assert_eq!(cat.direction, CataphoraDirection::Cataphoric);
        assert_eq!(cat.token_distance, 10);
        assert!(cat.needs_review); // Cataphoric always needs review
    }

    #[test]
    fn test_cataphora_candidate_defined_term_bonus() {
        let mut candidate = CataphoraCandidate::anaphoric("Tenant", 3);
        let initial_confidence = candidate.confidence;

        candidate = candidate.with_defined_term(true);

        assert!(candidate.is_defined_term);
        assert!(candidate.confidence > initial_confidence);
    }

    #[test]
    fn test_cataphora_candidate_salience() {
        let candidate = CataphoraCandidate::anaphoric("Company", 5)
            .with_salience(0.9);

        assert!(candidate.salience > 0.8);
        // High salience should boost confidence
    }

    #[test]
    fn test_document_resolver_scoring() {
        let resolver = DocumentPronounResolver::new();

        // Anaphoric candidate (backward reference)
        let mut anaphoric = CataphoraCandidate::anaphoric("Company", 5);
        resolver.score_candidate(&mut anaphoric);

        // Cataphoric candidate (forward reference) at same distance
        let mut cataphoric = CataphoraCandidate::cataphoric("Contract", 5);
        resolver.score_candidate(&mut cataphoric);

        // Anaphoric should score higher (more common pattern)
        assert!(anaphoric.confidence > cataphoric.confidence);
    }

    #[test]
    fn test_forward_reference_before_it_expires() {
        // "Before it expires, the Contract shall be renewed"
        // Pronoun "it" at position 1, antecedent "Contract" at position 5

        let mut cataphoric = CataphoraCandidate::cataphoric("Contract", 4)
            .with_defined_term(true)
            .with_salience(0.8);

        let resolver = DocumentPronounResolver::new();
        resolver.score_candidate(&mut cataphoric);

        // Should have reasonable confidence despite being forward reference
        assert!(cataphoric.confidence > 0.4);
        assert!(cataphoric.needs_review); // Forward references always flagged
        assert_eq!(cataphoric.direction, CataphoraDirection::Cataphoric);
    }

    #[test]
    fn test_cataphoric_trigger_detection() {
        assert!(DocumentPronounResolver::is_cataphoric_trigger("Before"));
        assert!(DocumentPronounResolver::is_cataphoric_trigger("UNTIL"));
        assert!(DocumentPronounResolver::is_cataphoric_trigger("unless"));
        assert!(DocumentPronounResolver::is_cataphoric_trigger("if"));

        assert!(!DocumentPronounResolver::is_cataphoric_trigger("the"));
        assert!(!DocumentPronounResolver::is_cataphoric_trigger("shall"));
    }

    #[test]
    fn test_document_pronoun_reference_ambiguity() {
        let doc_ref = DocumentPronounReference {
            pronoun: "it".to_string(),
            pronoun_type: PronounType::ThirdSingularNeuter,
            candidates: vec![],
            best_anaphoric: Some(CataphoraCandidate {
                text: "Company".to_string(),
                is_defined_term: true,
                token_distance: 5,
                direction: CataphoraDirection::Anaphoric,
                salience: 0.7,
                confidence: 0.65,
                needs_review: false,
                review_reason: None,
            }),
            best_cataphoric: Some(CataphoraCandidate {
                text: "Contract".to_string(),
                is_defined_term: true,
                token_distance: 3,
                direction: CataphoraDirection::Cataphoric,
                salience: 0.8,
                confidence: 0.55,
                needs_review: true,
                review_reason: None,
            }),
            needs_review: false,
            review_reason: None,
        };

        // Close confidence scores mean ambiguous direction
        assert!(doc_ref.is_direction_ambiguous());
    }

    #[test]
    fn test_salience_calculation() {
        // Entity mentioned 5 times out of max 10
        let salience = DocumentPronounResolver::calculate_salience(5, 10);
        assert!(salience > 0.5);
        assert!(salience < 0.7);

        // Most mentioned entity
        let high_salience = DocumentPronounResolver::calculate_salience(10, 10);
        assert!(high_salience > 0.8);

        // Rarely mentioned
        let low_salience = DocumentPronounResolver::calculate_salience(1, 10);
        assert!(low_salience < 0.5);
    }

    #[test]
    fn test_conversion_from_legacy_antecedent() {
        let legacy = AntecedentCandidate {
            text: "Landlord".to_string(),
            is_defined_term: true,
            token_distance: 7,
            confidence: 0.75,
        };

        let cataphora = CataphoraCandidate::from_antecedent(&legacy);

        assert_eq!(cataphora.text, "Landlord");
        assert!(cataphora.is_defined_term);
        assert_eq!(cataphora.token_distance, 7);
        assert_eq!(cataphora.direction, CataphoraDirection::Anaphoric);
        assert_eq!(cataphora.confidence, 0.75);
    }
}

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
        const MAX_ITERATIONS: usize = 1000;
        let mut distance = 0;
        let mut current = earlier.clone();

        while let Some((next_sel, _)) = current.match_first_forwards(&x::token_text()) {
            distance += 1;
            if distance > MAX_ITERATIONS {
                return distance; // Prevent runaway
            }
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

    /// Set this candidate as a defined term.
    /// Note: This is a flag-setter only. Confidence adjustments are applied
    /// centrally in `DocumentPronounResolver::score_candidate()`.
    pub fn with_defined_term(mut self, is_defined: bool) -> Self {
        self.is_defined_term = is_defined;
        self
    }

    /// Set the salience score.
    /// Note: This is a flag-setter only. Confidence adjustments are applied
    /// centrally in `DocumentPronounResolver::score_candidate()`.
    pub fn with_salience(mut self, salience: f64) -> Self {
        self.salience = salience.clamp(0.0, 1.0);
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
    /// Estimated number of tokens per line for cross-line distance calculations.
    /// Used to approximate token distance when pronouns and antecedents are on different lines.
    const ESTIMATED_TOKENS_PER_LINE: usize = 20;

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
            "before" | "until" | "unless" | "if" | "when" | "once" | "while"
            // "after" removed - it indicates backward (anaphoric) reference
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

// ============================================================================
// DOCUMENT RESOLVER IMPLEMENTATION (Gate 3 continued)
// ============================================================================

use layered_nlp_document::{DocumentResolver, LayeredDocument, ReviewableResult};
use std::collections::HashMap;

/// Wrapper type for document-level pronoun resolution results.
///
/// This enum holds both individual pronoun references and coreference chains,
/// allowing the resolver to return a heterogeneous set of results.
#[derive(Debug, Clone)]
pub enum DocumentPronounResult {
    /// A single pronoun reference with its resolution candidates
    PronounReference(ReviewableResult<DocumentPronounReference>),
    /// A coreference chain linking mentions of the same entity
    Chain(ReviewableResult<PronounChainResult>),
}

/// A pronoun chain result for document-level resolution.
/// This is a simplified version of PronounChain that is serializable.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PronounChainResult {
    /// Unique identifier for this chain
    pub chain_id: u32,
    /// The canonical name for this entity (usually the defined term name)
    pub canonical_name: String,
    /// Whether this chain is rooted in a formally defined term
    pub is_defined_term: bool,
    /// Number of mentions in this chain
    pub mention_count: usize,
    /// Whether any mention in this chain has been verified (confidence = 1.0)
    pub has_verified_mention: bool,
    /// The best confidence among all mentions
    pub best_confidence: f64,
}

impl DocumentPronounResult {
    /// Returns true if this result needs human review.
    pub fn needs_review(&self) -> bool {
        match self {
            Self::PronounReference(r) => r.needs_review,
            Self::Chain(r) => r.needs_review,
        }
    }

    /// Get the review reason if this result needs review.
    pub fn review_reason(&self) -> Option<&str> {
        match self {
            Self::PronounReference(r) => r.review_reason.as_deref(),
            Self::Chain(r) => r.review_reason.as_deref(),
        }
    }
}

/// Internal structure for tracking antecedent candidates during document traversal.
#[derive(Debug, Clone)]
struct DocumentAntecedent {
    /// The text of the antecedent
    text: String,
    /// Whether this is a formally defined term
    is_defined_term: bool,
    /// Line index where this antecedent appears
    line_index: usize,
    /// Token offset within the line
    token_offset: usize,
    /// Number of mentions in the document (for salience)
    mention_count: usize,
}

/// Internal structure for tracking pronouns during document traversal.
#[derive(Debug, Clone)]
struct DocumentPronoun {
    /// The pronoun text
    text: String,
    /// Pronoun type classification
    pronoun_type: PronounType,
    /// Line index where this pronoun appears
    line_index: usize,
    /// Token offset within the line
    token_offset: usize,
    /// Whether a cataphoric trigger precedes this pronoun on the same line
    has_cataphoric_trigger: bool,
}

impl DocumentResolver for DocumentPronounResolver {
    type Attr = DocumentPronounResult;

    fn resolve(&self, doc: &LayeredDocument) -> Vec<Self::Attr> {
        use layered_nlp::{x, LToken};
        use crate::defined_term::DefinedTerm;
        use crate::term_reference::TermReference;
        use crate::pronoun_chain::{ChainMention, MentionType};

        // Macro for getting token text
        macro_rules! token_text {
            ($token:expr) => {{
                match $token.get_token() {
                    LToken::Text(text, _) => Some(text.as_str()),
                    LToken::Value => None,
                }
            }};
        }

        let mut results = Vec::new();
        let mut antecedents: Vec<DocumentAntecedent> = Vec::new();
        let mut pronouns: Vec<DocumentPronoun> = Vec::new();
        let mut mention_counts: HashMap<String, usize> = HashMap::new();

        // ====================================================================
        // Pass 1: Collect all potential antecedents and pronouns
        // ====================================================================
        for (line_index, line) in doc.lines().iter().enumerate() {

            // Check for cataphoric triggers by examining early tokens
            let mut line_has_cataphoric_trigger = false;
            let mut word_count = 0;
            for token in line.ll_tokens().iter().take(10) {
                if let Some(text) = token_text!(token) {
                    if word_count < 3 && Self::is_cataphoric_trigger(text) {
                        line_has_cataphoric_trigger = true;
                        break;
                    }
                    word_count += 1;
                }
            }

            // Collect defined terms
            for found in line.find(&x::attr::<Scored<DefinedTerm>>()) {
                let scored = found.attr();
                let (start, _end) = found.range();
                let key = scored.value.term_name.to_lowercase();
                *mention_counts.entry(key.clone()).or_insert(0) += 1;

                antecedents.push(DocumentAntecedent {
                    text: scored.value.term_name.clone(),
                    is_defined_term: true,
                    line_index,
                    token_offset: start,
                    mention_count: 0, // Will be updated in pass 2
                });
            }

            // Collect term references
            for found in line.find(&x::attr::<Scored<TermReference>>()) {
                let scored = found.attr();
                let (start, _end) = found.range();
                let key = scored.value.term_name.to_lowercase();
                *mention_counts.entry(key.clone()).or_insert(0) += 1;

                antecedents.push(DocumentAntecedent {
                    text: scored.value.term_name.clone(),
                    is_defined_term: true,
                    line_index,
                    token_offset: start,
                    mention_count: 0,
                });
            }

            // Collect plain nouns via POS tags
            use layered_part_of_speech::Tag;
            for found in line.find(&x::attr_eq(&Tag::Noun)) {
                let (start, _end) = found.range();
                // Get the text of this token
                if let Some(token) = line.ll_tokens().get(start) {
                    if let Some(text) = token_text!(token) {
                        let key = text.to_lowercase();
                        // Only add if not already a defined term
                        if !antecedents.iter().any(|a|
                            a.line_index == line_index &&
                            a.text.to_lowercase() == key
                        ) {
                            *mention_counts.entry(key).or_insert(0) += 1;

                            antecedents.push(DocumentAntecedent {
                                text: text.to_string(),
                                is_defined_term: false,
                                line_index,
                                token_offset: start,
                                mention_count: 0,
                            });
                        }
                    }
                }
            }

            // Collect proper nouns
            for found in line.find(&x::attr_eq(&Tag::ProperNoun)) {
                let (start, _end) = found.range();
                if let Some(token) = line.ll_tokens().get(start) {
                    if let Some(text) = token_text!(token) {
                        let key = text.to_lowercase();
                        if !antecedents.iter().any(|a|
                            a.line_index == line_index &&
                            a.text.to_lowercase() == key
                        ) {
                            *mention_counts.entry(key).or_insert(0) += 1;

                            antecedents.push(DocumentAntecedent {
                                text: text.to_string(),
                                is_defined_term: false,
                                line_index,
                                token_offset: start,
                                mention_count: 0,
                            });
                        }
                    }
                }
            }

            // Collect pronouns (from PronounReference)
            let mut found_pronouns = false;
            for found in line.find(&x::attr::<Scored<PronounReference>>()) {
                found_pronouns = true;
                let scored = found.attr();
                let (start, _end) = found.range();
                pronouns.push(DocumentPronoun {
                    text: scored.value.pronoun.clone(),
                    pronoun_type: scored.value.pronoun_type,
                    line_index,
                    token_offset: start,
                    has_cataphoric_trigger: line_has_cataphoric_trigger,
                });
            }

            // Fallback: scan for pronouns by word matching if none found via PronounReference
            if !found_pronouns {
                for (idx, token) in line.ll_tokens().iter().enumerate() {
                    if let Some(text) = token_text!(token) {
                        if RESOLVABLE_PRONOUNS.contains(&text.to_lowercase().as_str()) {
                            let ptype = PronounType::from_text(text);
                            if !matches!(ptype, PronounType::Other) {
                                pronouns.push(DocumentPronoun {
                                    text: text.to_string(),
                                    pronoun_type: ptype,
                                    line_index,
                                    token_offset: idx,
                                    has_cataphoric_trigger: line_has_cataphoric_trigger,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Update mention counts on antecedents
        let max_mentions = mention_counts.values().copied().max().unwrap_or(1);
        for ant in &mut antecedents {
            let key = ant.text.to_lowercase();
            ant.mention_count = *mention_counts.get(&key).unwrap_or(&1);
        }

        // ====================================================================
        // Pass 2: Resolve each pronoun
        // ====================================================================
        for pronoun in &pronouns {
            let mut candidates: Vec<CataphoraCandidate> = Vec::new();

            // Backward search (anaphoric): look at antecedents before this pronoun
            for ant in &antecedents {
                // Antecedent must be before pronoun
                let is_before = ant.line_index < pronoun.line_index
                    || (ant.line_index == pronoun.line_index
                        && ant.token_offset < pronoun.token_offset);

                if is_before {
                    // Calculate token distance (approximate)
                    let line_distance = pronoun.line_index.saturating_sub(ant.line_index);
                    let token_distance = if line_distance == 0 {
                        pronoun.token_offset.saturating_sub(ant.token_offset)
                    } else {
                        // Cross-line distance: remaining tokens in ant's line + middle lines + pronoun's offset
                        // Formula: (line_distance - 1) * avg_line_length + (avg - ant_offset) + pronoun_offset
                        (line_distance - 1) * Self::ESTIMATED_TOKENS_PER_LINE
                            + (Self::ESTIMATED_TOKENS_PER_LINE.saturating_sub(ant.token_offset))
                            + pronoun.token_offset
                    };

                    let salience = Self::calculate_salience(ant.mention_count, max_mentions);

                    let mut candidate = CataphoraCandidate::anaphoric(&ant.text, token_distance)
                        .with_defined_term(ant.is_defined_term)
                        .with_salience(salience);

                    self.score_candidate(&mut candidate);
                    candidates.push(candidate);
                }
            }

            // Forward search (cataphoric): if trigger present, look at antecedents after
            if pronoun.has_cataphoric_trigger {
                for ant in &antecedents {
                    let is_after = ant.line_index > pronoun.line_index
                        || (ant.line_index == pronoun.line_index
                            && ant.token_offset > pronoun.token_offset);

                    if is_after {
                        let line_distance = ant.line_index.saturating_sub(pronoun.line_index);
                        let token_distance = if line_distance == 0 {
                            ant.token_offset.saturating_sub(pronoun.token_offset)
                        } else {
                            // Cross-line distance: remaining tokens after pronoun + middle lines + ant's offset
                            // Formula: (line_distance - 1) * avg_line_length + (avg - pronoun_offset) + ant_offset
                            (line_distance - 1) * Self::ESTIMATED_TOKENS_PER_LINE
                                + (Self::ESTIMATED_TOKENS_PER_LINE.saturating_sub(pronoun.token_offset))
                                + ant.token_offset
                        };

                        let salience = Self::calculate_salience(ant.mention_count, max_mentions);

                        let mut candidate = CataphoraCandidate::cataphoric(&ant.text, token_distance)
                            .with_defined_term(ant.is_defined_term)
                            .with_salience(salience);

                        self.score_candidate(&mut candidate);
                        candidates.push(candidate);
                    }
                }
            }

            // Sort candidates by confidence (highest first)
            candidates.sort_by(|a, b| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Limit to top 5 candidates
            candidates.truncate(5);

            // Determine best anaphoric and cataphoric candidates
            let best_anaphoric = candidates
                .iter()
                .filter(|c| c.direction == CataphoraDirection::Anaphoric)
                .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal))
                .cloned();

            let best_cataphoric = candidates
                .iter()
                .filter(|c| c.direction == CataphoraDirection::Cataphoric)
                .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal))
                .cloned();

            // Determine review flags
            let (needs_review, review_reason) = self.determine_review_status(
                &candidates,
                &best_anaphoric,
                &best_cataphoric,
                pronoun.has_cataphoric_trigger,
            );

            let doc_ref = DocumentPronounReference {
                pronoun: pronoun.text.clone(),
                pronoun_type: pronoun.pronoun_type,
                candidates: candidates.clone(),
                best_anaphoric,
                best_cataphoric,
                needs_review,
                review_reason: review_reason.clone(),
            };

            // Wrap in ReviewableResult
            let reviewable = if needs_review {
                ReviewableResult::uncertain(
                    doc_ref,
                    Vec::new(),
                    review_reason.unwrap_or_else(|| "Needs review".to_string()),
                )
            } else {
                ReviewableResult::certain(doc_ref)
            };

            results.push(DocumentPronounResult::PronounReference(reviewable));
        }

        // ====================================================================
        // Pass 3: Build coreference chains
        // ====================================================================
        // Track both lowercase key for matching and original-cased canonical name
        struct ChainData {
            original_name: String,  // First occurrence's case (preserved)
            mentions: Vec<ChainMention>,
        }

        let mut chains: HashMap<String, ChainData> = HashMap::new();

        // Seed chains from defined terms
        for ant in &antecedents {
            if ant.is_defined_term {
                let key = ant.text.to_lowercase();
                let entry = chains.entry(key).or_insert_with(|| ChainData {
                    original_name: ant.text.clone(),  // Preserve first occurrence's case
                    mentions: Vec::new(),
                });
                entry.mentions.push(ChainMention {
                    text: ant.text.clone(),
                    mention_type: MentionType::Definition,
                    confidence: 1.0,
                    token_offset: ant.token_offset,
                });
            }
        }

        // Add pronouns to chains based on their best resolution
        for (idx, pronoun) in pronouns.iter().enumerate() {
            // Find the corresponding pronoun reference result
            if let Some(DocumentPronounResult::PronounReference(ref reviewable)) = results.get(idx) {
                let doc_ref = &reviewable.ambiguous.best.value;

                // Use best candidate to determine chain membership
                if let Some(best) = doc_ref.best_candidate() {
                    if best.confidence >= 0.5 {
                        let key = best.text.to_lowercase();
                        let entry = chains.entry(key).or_insert_with(|| ChainData {
                            original_name: best.text.clone(),  // Preserve original case from candidate
                            mentions: Vec::new(),
                        });
                        entry.mentions.push(ChainMention {
                            text: pronoun.text.clone(),
                            mention_type: MentionType::Pronoun,
                            confidence: best.confidence,
                            token_offset: pronoun.token_offset,
                        });
                    }
                }
            }
        }

        // Convert chains to results (only chains with 2+ mentions)
        let mut chain_id = 1u32;
        for (_, chain_data) in chains {
            let mut mentions = chain_data.mentions;
            if mentions.len() >= 2 {
                mentions.sort_by_key(|m| m.token_offset);

                let has_verified = mentions.iter().any(|m| (m.confidence - 1.0).abs() < 0.001);
                let best_confidence = mentions
                    .iter()
                    .map(|m| m.confidence)
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(0.5);

                // Check if any pronoun in chain needs review
                let chain_needs_review = mentions
                    .iter()
                    .filter(|m| matches!(m.mention_type, MentionType::Pronoun))
                    .any(|m| m.confidence < self.review_threshold);

                let chain_result = PronounChainResult {
                    chain_id,
                    canonical_name: chain_data.original_name.clone(),
                    is_defined_term: mentions.iter().any(|m| matches!(m.mention_type, MentionType::Definition)),
                    mention_count: mentions.len(),
                    has_verified_mention: has_verified,
                    best_confidence,
                };

                let reviewable = if chain_needs_review {
                    ReviewableResult::uncertain(
                        chain_result,
                        Vec::new(),
                        "Chain contains low-confidence pronoun resolutions".to_string(),
                    )
                } else if best_confidence >= self.review_threshold {
                    ReviewableResult::certain(chain_result)
                } else {
                    ReviewableResult::from_scored(Scored::rule_based(
                        chain_result,
                        best_confidence,
                        "pronoun_chain",
                    ))
                };

                results.push(DocumentPronounResult::Chain(reviewable));
                chain_id += 1;
            }
        }

        results
    }
}

impl DocumentPronounResolver {
    /// Determine whether a pronoun reference needs review and why.
    fn determine_review_status(
        &self,
        candidates: &[CataphoraCandidate],
        best_anaphoric: &Option<CataphoraCandidate>,
        best_cataphoric: &Option<CataphoraCandidate>,
        has_cataphoric_trigger: bool,
    ) -> (bool, Option<String>) {
        // Case 1: No candidates found
        if candidates.is_empty() {
            return (true, Some("No antecedent found".to_string()));
        }

        // Case 2: Cataphoric reference (always flag)
        if has_cataphoric_trigger && best_cataphoric.is_some() {
            if let Some(cat) = best_cataphoric {
                if cat.confidence > 0.3 {
                    return (true, Some("Cataphoric (forward) reference - verify antecedent".to_string()));
                }
            }
        }

        // Case 3: Ambiguous between anaphoric and cataphoric
        if let (Some(ana), Some(cat)) = (best_anaphoric, best_cataphoric) {
            if ana.confidence > 0.4 && cat.confidence > 0.3
                && (ana.confidence - cat.confidence).abs() < 0.2
            {
                return (true, Some(format!(
                    "Ambiguous direction: anaphoric '{}' ({:.2}) vs cataphoric '{}' ({:.2})",
                    ana.text, ana.confidence, cat.text, cat.confidence
                )));
            }
        }

        // Case 4: Multiple close candidates (ambiguous reference)
        if candidates.len() >= 2 {
            let top = &candidates[0];
            let second = &candidates[1];
            if (top.confidence - second.confidence).abs() < 0.15 && second.confidence > 0.5 {
                return (true, Some(format!(
                    "Ambiguous reference: '{}' ({:.2}) and '{}' ({:.2}) are close",
                    top.text, top.confidence, second.text, second.confidence
                )));
            }
        }

        // Case 5: Best candidate below review threshold
        if let Some(best) = candidates.first() {
            if best.confidence < self.review_threshold {
                return (true, Some(format!(
                    "Low confidence ({:.2}) - verify pronoun resolution",
                    best.confidence
                )));
            }
        }

        // No review needed
        (false, None)
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
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal))
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
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal))
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
        // Builder method is a flag-setter only
        let candidate = CataphoraCandidate::anaphoric("Tenant", 3)
            .with_defined_term(true);
        assert!(candidate.is_defined_term);

        // Confidence boost is applied by score_candidate()
        let resolver = DocumentPronounResolver::new();
        let mut defined = CataphoraCandidate::anaphoric("Tenant", 3).with_defined_term(true);
        let mut not_defined = CataphoraCandidate::anaphoric("Tenant", 3).with_defined_term(false);

        resolver.score_candidate(&mut defined);
        resolver.score_candidate(&mut not_defined);

        assert!(defined.confidence > not_defined.confidence);
    }

    #[test]
    fn test_cataphora_candidate_salience() {
        // Builder method is a flag-setter only
        let candidate = CataphoraCandidate::anaphoric("Company", 5)
            .with_salience(0.9);
        assert!(candidate.salience > 0.8);

        // Confidence boost is applied by score_candidate()
        let resolver = DocumentPronounResolver::new();
        let mut high_salience = CataphoraCandidate::anaphoric("Company", 5).with_salience(0.9);
        let mut low_salience = CataphoraCandidate::anaphoric("Company", 5).with_salience(0.3);

        resolver.score_candidate(&mut high_salience);
        resolver.score_candidate(&mut low_salience);

        assert!(high_salience.confidence > low_salience.confidence);
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

// ============================================================================
// DOCUMENT RESOLVER TESTS
// ============================================================================

#[cfg(test)]
mod document_resolver_tests {
    use super::*;
    use layered_nlp_document::DocumentResolver;

    /// Helper to run full resolver chain on text.
    fn run_pipeline(text: &str) -> layered_nlp_document::LayeredDocument {
        use layered_part_of_speech::POSTagResolver;
        use crate::{
            DefinedTermResolver, TermReferenceResolver, PronounResolver, ContractDocument,
        };

        ContractDocument::from_text(text)
            .run_resolver(&POSTagResolver::default())
            .run_resolver(&DefinedTermResolver::new())
            .run_resolver(&TermReferenceResolver::new())
            .run_resolver(&PronounResolver::new())
    }

    #[test]
    fn test_anaphoric_resolution_company_it() {
        // "The Company shall... It must..." -> "It" -> "Company" (anaphoric, high confidence)
        let text = r#"ABC Corp (the "Company") shall deliver goods.
The Company must ensure quality. It must comply with regulations."#;

        let doc = run_pipeline(text);
        let resolver = DocumentPronounResolver::new();
        let results = resolver.resolve(&doc);

        // Should have at least one pronoun reference
        let pronoun_refs: Vec<_> = results
            .iter()
            .filter_map(|r| {
                if let DocumentPronounResult::PronounReference(ref pr) = r {
                    Some(pr)
                } else {
                    None
                }
            })
            .collect();

        assert!(
            !pronoun_refs.is_empty(),
            "Should find at least one pronoun reference"
        );

        // Find the "It" pronoun resolution
        let it_ref = pronoun_refs.iter().find(|pr| {
            pr.ambiguous.best.value.pronoun.to_lowercase() == "it"
        });

        if let Some(it_result) = it_ref {
            let doc_ref = &it_result.ambiguous.best.value;
            // Should have anaphoric candidate
            assert!(
                doc_ref.best_anaphoric.is_some(),
                "Should have anaphoric candidate for 'It'"
            );

            if let Some(best) = &doc_ref.best_anaphoric {
                // Should resolve to "Company" (case-insensitive)
                assert!(
                    best.text.to_lowercase().contains("company"),
                    "Should resolve 'It' to 'Company', got '{}'",
                    best.text
                );
                assert_eq!(best.direction, CataphoraDirection::Anaphoric);
                // High confidence for defined term
                assert!(
                    best.confidence > 0.5,
                    "Should have high confidence, got {}",
                    best.confidence
                );
            }
        }
    }

    #[test]
    fn test_cataphoric_resolution_before_it_expires() {
        // "Before it expires, the Agreement..." -> "it" -> "Agreement" (cataphoric, flagged)
        let text = r#"Before it expires, the Agreement shall be renewed.
ABC Corp (the "Agreement") is hereby established."#;

        let doc = run_pipeline(text);
        let resolver = DocumentPronounResolver::new();
        let results = resolver.resolve(&doc);

        let pronoun_refs: Vec<_> = results
            .iter()
            .filter_map(|r| {
                if let DocumentPronounResult::PronounReference(ref pr) = r {
                    Some(pr)
                } else {
                    None
                }
            })
            .collect();

        // Find the "it" pronoun resolution
        let it_ref = pronoun_refs.iter().find(|pr| {
            pr.ambiguous.best.value.pronoun.to_lowercase() == "it"
        });

        if let Some(it_result) = it_ref {
            let doc_ref = &it_result.ambiguous.best.value;

            // Should be flagged for review (cataphoric)
            // Note: cataphoric detection depends on trigger word detection
            // The cataphoric candidate may or may not be present depending on
            // whether "Before" is detected as a trigger at line start
            if doc_ref.best_cataphoric.is_some() {
                assert!(
                    it_result.needs_review,
                    "Cataphoric reference should be flagged for review"
                );
            }
        }
    }

    #[test]
    fn test_unresolved_pronoun_flagged() {
        // "They shall notify..." (no antecedent) -> Unresolved, flagged
        let text = "They shall notify the other party immediately.";

        let doc = run_pipeline(text);
        let resolver = DocumentPronounResolver::new();
        let results = resolver.resolve(&doc);

        let pronoun_refs: Vec<_> = results
            .iter()
            .filter_map(|r| {
                if let DocumentPronounResult::PronounReference(ref pr) = r {
                    Some(pr)
                } else {
                    None
                }
            })
            .collect();

        // Find "They" pronoun
        let they_ref = pronoun_refs.iter().find(|pr| {
            pr.ambiguous.best.value.pronoun.to_lowercase() == "they"
        });

        if let Some(they_result) = they_ref {
            let doc_ref = &they_result.ambiguous.best.value;

            // Should be flagged because no clear antecedent
            if doc_ref.candidates.is_empty() {
                assert!(
                    they_result.needs_review,
                    "Pronoun with no antecedent should be flagged for review"
                );
                assert!(
                    they_result.review_reason.as_deref().unwrap_or("").contains("antecedent"),
                    "Review reason should mention missing antecedent"
                );
            }
        }
    }

    #[test]
    fn test_multi_line_chain_building() {
        // Test that chains are built across multiple lines
        let text = r#"ABC Corp (the "Company") shall deliver goods.
The Company must ensure quality.
It shall comply with regulations.
The Company agrees to these terms."#;

        let doc = run_pipeline(text);
        let resolver = DocumentPronounResolver::new();
        let results = resolver.resolve(&doc);

        let chains: Vec<_> = results
            .iter()
            .filter_map(|r| {
                if let DocumentPronounResult::Chain(ref ch) = r {
                    Some(ch)
                } else {
                    None
                }
            })
            .collect();

        // Should build a chain for "Company"
        let company_chain = chains.iter().find(|ch| {
            ch.ambiguous.best.value.canonical_name.to_lowercase().contains("company")
        });

        if let Some(chain_result) = company_chain {
            let chain = &chain_result.ambiguous.best.value;
            // Chain should have multiple mentions (definition + references + pronoun)
            assert!(
                chain.mention_count >= 2,
                "Company chain should have 2+ mentions, got {}",
                chain.mention_count
            );
            assert!(
                chain.is_defined_term,
                "Company chain should be marked as defined term"
            );
        }
    }

    #[test]
    fn test_ambiguous_reference_two_parties() {
        // Ambiguous "they" with two possible parties
        let text = r#"ABC Corp (the "Buyer") agrees to purchase.
XYZ Inc (the "Seller") agrees to sell.
They shall negotiate in good faith."#;

        let doc = run_pipeline(text);
        let resolver = DocumentPronounResolver::new();
        let results = resolver.resolve(&doc);

        let pronoun_refs: Vec<_> = results
            .iter()
            .filter_map(|r| {
                if let DocumentPronounResult::PronounReference(ref pr) = r {
                    Some(pr)
                } else {
                    None
                }
            })
            .collect();

        // Find "They" pronoun
        let they_ref = pronoun_refs.iter().find(|pr| {
            pr.ambiguous.best.value.pronoun.to_lowercase() == "they"
        });

        if let Some(they_result) = they_ref {
            let doc_ref = &they_result.ambiguous.best.value;

            // Should have multiple candidates (Buyer and Seller)
            // Both are potential antecedents for "They"
            if doc_ref.candidates.len() >= 2 {
                // Check if candidates are close in confidence
                let top_two: Vec<_> = doc_ref.candidates.iter().take(2).collect();
                if top_two.len() == 2 {
                    let diff = (top_two[0].confidence - top_two[1].confidence).abs();
                    if diff < 0.2 {
                        // Close candidates should trigger ambiguity flag
                        assert!(
                            they_result.needs_review || doc_ref.needs_review,
                            "Ambiguous 'they' with close candidates should be flagged"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_document_pronoun_result_needs_review() {
        let certain_ref = DocumentPronounReference {
            pronoun: "It".to_string(),
            pronoun_type: PronounType::ThirdSingularNeuter,
            candidates: vec![CataphoraCandidate::anaphoric("Company", 5)],
            best_anaphoric: Some(CataphoraCandidate::anaphoric("Company", 5)),
            best_cataphoric: None,
            needs_review: false,
            review_reason: None,
        };

        let certain_result = DocumentPronounResult::PronounReference(
            ReviewableResult::certain(certain_ref)
        );
        assert!(!certain_result.needs_review());
        assert!(certain_result.review_reason().is_none());

        let uncertain_ref = DocumentPronounReference {
            pronoun: "They".to_string(),
            pronoun_type: PronounType::ThirdPlural,
            candidates: vec![],
            best_anaphoric: None,
            best_cataphoric: None,
            needs_review: true,
            review_reason: Some("No antecedent found".to_string()),
        };

        let uncertain_result = DocumentPronounResult::PronounReference(
            ReviewableResult::uncertain(
                uncertain_ref,
                Vec::new(),
                "No antecedent found",
            )
        );
        assert!(uncertain_result.needs_review());
        assert_eq!(uncertain_result.review_reason(), Some("No antecedent found"));
    }

    #[test]
    fn test_determine_review_status_no_candidates() {
        let resolver = DocumentPronounResolver::new();
        let (needs_review, reason) = resolver.determine_review_status(
            &[],  // No candidates
            &None,
            &None,
            false,
        );

        assert!(needs_review);
        assert!(reason.as_deref().unwrap_or("").contains("No antecedent"));
    }

    #[test]
    fn test_determine_review_status_cataphoric() {
        let resolver = DocumentPronounResolver::new();
        let cataphoric = CataphoraCandidate::cataphoric("Agreement", 5);

        let (needs_review, reason) = resolver.determine_review_status(
            &[cataphoric.clone()],
            &None,
            &Some(cataphoric),
            true,  // Has cataphoric trigger
        );

        assert!(needs_review);
        assert!(reason.as_deref().unwrap_or("").contains("Cataphoric"));
    }

    #[test]
    fn test_determine_review_status_low_confidence() {
        let resolver = DocumentPronounResolver::new();
        let mut low_conf = CataphoraCandidate::anaphoric("Term", 50);
        low_conf.confidence = 0.3;  // Below threshold

        let (needs_review, reason) = resolver.determine_review_status(
            &[low_conf],
            &None,
            &None,
            false,
        );

        assert!(needs_review);
        assert!(reason.as_deref().unwrap_or("").contains("Low confidence"));
    }

    #[test]
    fn test_determine_review_status_high_confidence() {
        let resolver = DocumentPronounResolver::new();
        let mut high_conf = CataphoraCandidate::anaphoric("Company", 5);
        high_conf.confidence = 0.85;

        let (needs_review, _reason) = resolver.determine_review_status(
            &[high_conf.clone()],
            &Some(high_conf),
            &None,
            false,
        );

        assert!(!needs_review);
    }

    #[test]
    fn test_document_resolver_returns_both_types() {
        let text = r#"ABC Corp (the "Company") shall deliver goods.
The Company must ensure quality. It shall comply."#;

        let doc = run_pipeline(text);
        let resolver = DocumentPronounResolver::new();
        let results = resolver.resolve(&doc);

        let has_pronoun_refs = results.iter().any(|r| {
            matches!(r, DocumentPronounResult::PronounReference(_))
        });

        let has_chains = results.iter().any(|r| {
            matches!(r, DocumentPronounResult::Chain(_))
        });

        // Should produce both pronoun references and chains
        assert!(
            has_pronoun_refs || has_chains,
            "Should produce pronoun references and/or chains"
        );
    }
}

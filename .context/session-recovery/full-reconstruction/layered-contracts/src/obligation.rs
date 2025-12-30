//! Obligation phrase detection for contract language analysis.
//!
//! This resolver identifies obligation phrases by anchoring on modal keywords
//! (shall, may, shall not) and linking them to obligors (parties who have the duty)
//! and actions (what they must do).
//!
//! Example:
//! ```text
//! The Company shall deliver goods.
//!     ╰─────╯     ╰───╯
//!     obligor     modal
//!                      ╰────────────╯action
//! ```

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver, TextTag};
use layered_part_of_speech::Tag;

use crate::contract_keyword::ContractKeyword;
use crate::pronoun::PronounReference;
use crate::scored::Scored;
use crate::term_reference::TermReference;

/// The type of obligation expressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObligationType {
    /// "shall", "must" - a required duty
    Duty,
    /// "may", "can" - a permitted action
    Permission,
    /// "shall not", "must not" - a prohibited action
    Prohibition,
}

impl ObligationType {
    /// Convert from a ContractKeyword modal.
    pub fn from_keyword(keyword: &ContractKeyword) -> Option<Self> {
        match keyword {
            ContractKeyword::Shall => Some(Self::Duty),
            ContractKeyword::May => Some(Self::Permission),
            ContractKeyword::ShallNot => Some(Self::Prohibition),
            _ => None,
        }
    }
}

/// Reference to who has the obligation.
#[derive(Debug, Clone, PartialEq)]
pub enum ObligorReference {
    /// Direct reference to a defined term (e.g., "the Company")
    TermRef {
        term_name: String,
        /// Confidence from the TermReference
        confidence: f64,
    },
    /// Resolved through a pronoun (e.g., "It" referring to Company)
    PronounRef {
        pronoun: String,
        /// The best antecedent candidate
        resolved_to: String,
        /// Whether we resolved to a defined term
        is_defined_term: bool,
        /// Combined confidence (pronoun resolution * antecedent)
        confidence: f64,
    },
    /// Plain noun phrase (not a defined term)
    NounPhrase {
        text: String,
    },
}

/// A reference to a condition that qualifies this obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionRef {
    /// The condition keyword type
    pub condition_type: ContractKeyword,
    /// The text following the condition keyword (simplified)
    pub text_preview: String,
}

/// An obligation phrase extracted from contract text.
#[derive(Debug, Clone, PartialEq)]
pub struct ObligationPhrase {
    /// Who has the obligation
    pub obligor: ObligorReference,
    /// What type of obligation
    pub obligation_type: ObligationType,
    /// The action they must/may/must not take
    pub action: String,
    /// Any conditions attached (if/unless/provided)
    pub conditions: Vec<ConditionRef>,
}

/// Resolver for detecting obligation phrases.
///
/// Requires that the following resolvers have already been run:
/// - `ContractKeywordResolver` + `ProhibitionResolver`
/// - `DefinedTermResolver`
/// - `TermReferenceResolver`
/// - `PronounResolver`
pub struct ObligationPhraseResolver {
    /// Base confidence when modal + obligor found
    base_confidence: f64,
    /// Bonus if obligor is a defined term
    defined_term_bonus: f64,
    /// Bonus if resolved through pronoun chain
    pronoun_chain_bonus: f64,
    /// Penalty for multiple obligor candidates
    multiple_obligor_penalty: f64,
    /// Penalty for empty/minimal action span
    empty_action_penalty: f64,
}

impl Default for ObligationPhraseResolver {
    fn default() -> Self {
        Self {
            base_confidence: 0.75,
            defined_term_bonus: 0.10,
            pronoun_chain_bonus: 0.05,
            multiple_obligor_penalty: 0.15,
            empty_action_penalty: 0.10,
        }
    }
}

impl ObligationPhraseResolver {
    /// Create a new resolver with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if selection A comes before selection B.
    fn selection_is_before(&self, a: &LLSelection, b: &LLSelection) -> bool {
        let [before, after] = a.split_with(b);
        before.is_some() && after.is_none()
    }

    /// Find the nearest obligor (TermReference or PronounReference) before the modal.
    fn find_obligor(
        &self,
        selection: &LLSelection,
        modal_sel: &LLSelection,
    ) -> Option<(ObligorReference, bool)> {
        // Returns (obligor, has_multiple_candidates)

        // Look for TermReferences before the modal
        let term_refs: Vec<_> = selection
            .find_by(&x::attr::<Scored<TermReference>>())
            .into_iter()
            .filter(|(sel, _)| self.selection_is_before(sel, modal_sel))
            .collect();

        // Look for PronounReferences before the modal
        let pronoun_refs: Vec<_> = selection
            .find_by(&x::attr::<Scored<PronounReference>>())
            .into_iter()
            .filter(|(sel, _)| self.selection_is_before(sel, modal_sel))
            .collect();

        // Find the nearest candidate (closest to the modal)
        let mut best_term: Option<(&LLSelection, &Scored<TermReference>)> = None;
        let mut best_pronoun: Option<(&LLSelection, &Scored<PronounReference>)> = None;

        // Get the last (nearest to modal) term reference
        if let Some((sel, scored)) = term_refs.last() {
            best_term = Some((sel, scored));
        }

        // Get the last (nearest to modal) pronoun reference
        if let Some((sel, scored)) = pronoun_refs.last() {
            // Only consider pronouns that have resolved candidates
            if !scored.value.candidates.is_empty() {
                best_pronoun = Some((sel, scored));
            }
        }

        // Determine which is closer to the modal
        let has_multiple = term_refs.len() + pronoun_refs.len() > 1;

        match (best_term, best_pronoun) {
            (Some((term_sel, term_ref)), Some((pron_sel, pron_ref))) => {
                // Both exist - pick the one closer to the modal
                // The one that is NOT before the other is closer
                if self.selection_is_before(term_sel, pron_sel) {
                    // Pronoun is closer
                    Some((self.pronoun_to_obligor(pron_ref), has_multiple))
                } else {
                    // Term is closer
                    Some((self.term_to_obligor(term_ref), has_multiple))
                }
            }
            (Some((_, term_ref)), None) => Some((self.term_to_obligor(term_ref), has_multiple)),
            (None, Some((_, pron_ref))) => Some((self.pronoun_to_obligor(pron_ref), has_multiple)),
            (None, None) => {
                // Fall back to looking for plain capitalized nouns
                self.find_noun_obligor(selection, modal_sel)
            }
        }
    }

    /// Convert a TermReference to ObligorReference.
    fn term_to_obligor(&self, term_ref: &Scored<TermReference>) -> ObligorReference {
        ObligorReference::TermRef {
            term_name: term_ref.value.term_name.clone(),
            confidence: term_ref.confidence,
        }
    }

    /// Convert a PronounReference to ObligorReference.
    fn pronoun_to_obligor(&self, pron_ref: &Scored<PronounReference>) -> ObligorReference {
        let best_candidate = &pron_ref.value.candidates[0];
        ObligorReference::PronounRef {
            pronoun: pron_ref.value.pronoun.clone(),
            resolved_to: best_candidate.text.clone(),
            is_defined_term: best_candidate.is_defined_term,
            confidence: pron_ref.confidence,
        }
    }

    /// Find a plain noun phrase to use as obligor (fallback).
    ///
    /// Requires POS tag to be Noun or ProperNoun to avoid mis-tagging words like
    /// "If" or "Performance" (in "If Performance is late...") as obligors.
    ///
    /// Captures multi-word noun phrases like "Service Provider" or "Customer Group"
    /// by peeking ahead for contiguous capitalized nouns.
    fn find_noun_obligor(
        &self,
        selection: &LLSelection,
        modal_sel: &LLSelection,
    ) -> Option<(ObligorReference, bool)> {
        // Look for capitalized words before the modal that are tagged as nouns
        let noun_words: Vec<_> = selection
            .find_by(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
            .into_iter()
            .filter(|(sel, (_, text))| {
                // Must be before the modal
                if !self.selection_is_before(sel, modal_sel) {
                    return false;
                }
                // Must be capitalized
                if !text.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    return false;
                }
                // Must be tagged as a noun or proper noun by POS resolver
                let is_noun = !sel.find_by(&x::attr_eq(&Tag::Noun)).is_empty()
                    || !sel.find_by(&x::attr_eq(&Tag::ProperNoun)).is_empty();
                is_noun
            })
            .collect();

        if noun_words.is_empty() {
            return None;
        }

        // Find the nearest multi-word noun phrase before the modal
        // Start from the last noun (nearest to modal) and look backwards for contiguous nouns
        let mut phrase_parts: Vec<&str> = Vec::new();
        let mut last_sel: Option<&LLSelection> = None;

        // Start from the last noun and work backwards to find contiguous nouns
        for i in (0..noun_words.len()).rev() {
            let (sel, (_, text)) = &noun_words[i];

            if phrase_parts.is_empty() {
                // First word of the phrase (nearest to modal)
                phrase_parts.push(text);
                last_sel = Some(sel);
            } else if let Some(prev_sel) = last_sel {
                // Check if this noun is immediately before the previous one
                // (i.e., they form a contiguous phrase like "Service Provider")
                if self.are_adjacent_words(sel, prev_sel) {
                    phrase_parts.push(text);
                    last_sel = Some(sel);
                } else {
                    // Not contiguous, stop here
                    break;
                }
            }
        }

        // Reverse to get correct order (we collected backwards)
        phrase_parts.reverse();
        let phrase_text = phrase_parts.join(" ");

        Some((
            ObligorReference::NounPhrase { text: phrase_text },
            false,
        ))
    }

    /// Check if two word selections are adjacent (only whitespace between them).
    fn are_adjacent_words(&self, earlier: &LLSelection, later: &LLSelection) -> bool {
        let mut current = earlier.clone();

        // Skip whitespace after earlier
        if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
            current = ws_sel;
        } else {
            return false;
        }

        // Check if the next word matches the text of 'later'
        // We compare by checking that the extended selection ends at the same place as 'later'
        if let Some((extended_sel, _)) =
            current.match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
        {
            // The extended selection goes from current through the word
            // Check if 'later' is contained within the extension (i.e., is the word we found)
            // by verifying that later.split_with(extended_sel) gives [None, None]
            // (meaning later is entirely contained within extended_sel)
            let [before, after] = later.split_with(&extended_sel);
            return before.is_none() && after.is_none();
        }

        false
    }

    /// Extract the action span following the modal.
    fn extract_action(&self, _selection: &LLSelection, modal_sel: &LLSelection) -> String {
        let mut action_words = Vec::new();
        let mut current = modal_sel.clone();

        // Walk forward collecting words until we hit a boundary
        loop {
            // Skip whitespace
            if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                current = ws_sel;
            } else {
                break;
            }

            // Check for punctuation (sentence boundary)
            if let Some((punc_sel, _)) =
                current.match_first_forwards(&x::attr_eq(&TextTag::PUNC))
            {
                // Check if it's sentence-ending punctuation
                if let Some((_, text)) = punc_sel.find_first_by(&x::token_text()) {
                    if matches!(text, "." | ";" | "!" | "?") {
                        break;
                    }
                }
                current = punc_sel;
                continue;
            }

            // Check for another modal (start of new clause)
            if current
                .match_first_forwards(&x::attr_eq(&ContractKeyword::Shall))
                .is_some()
                || current
                    .match_first_forwards(&x::attr_eq(&ContractKeyword::May))
                    .is_some()
                || current
                    .match_first_forwards(&x::attr_eq(&ContractKeyword::ShallNot))
                    .is_some()
            {
                break;
            }

            // Check for condition starter (If, Unless, Provided, SubjectTo)
            if current
                .match_first_forwards(&x::attr_eq(&ContractKeyword::If))
                .is_some()
                || current
                    .match_first_forwards(&x::attr_eq(&ContractKeyword::Unless))
                    .is_some()
                || current
                    .match_first_forwards(&x::attr_eq(&ContractKeyword::Provided))
                    .is_some()
                || current
                    .match_first_forwards(&x::attr_eq(&ContractKeyword::SubjectTo))
                    .is_some()
            {
                break;
            }

            // Get the next word
            if let Some((word_sel, (_, text))) =
                current.match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
            {
                action_words.push(text.to_string());
                current = word_sel;
            } else {
                break;
            }
        }

        action_words.join(" ")
    }

    /// Check if there's a sentence boundary between two selections.
    /// Returns true if no sentence-ending punctuation exists between them.
    fn in_same_sentence(&self, sel_a: &LLSelection, sel_b: &LLSelection) -> bool {
        // Determine which selection comes first
        let (earlier, later) = if self.selection_is_before(sel_a, sel_b) {
            (sel_a, sel_b)
        } else if self.selection_is_before(sel_b, sel_a) {
            (sel_b, sel_a)
        } else {
            // Same position or overlapping - same sentence
            return true;
        };

        // Walk from earlier looking for sentence-ending punctuation before reaching later
        let mut current = earlier.clone();

        while let Some((next_sel, text)) = current.match_first_forwards(&x::token_text()) {
            // Check if we've reached the later selection
            // Use selection_is_before: if later is no longer after next_sel, we've reached/passed it
            if !self.selection_is_before(&next_sel, later) {
                // Reached or passed `later` without finding sentence boundary
                return true;
            }

            // Check for sentence-ending punctuation by looking at the token text directly
            // Note: match_first_forwards returns extended selections, so we check the text
            // we just matched rather than using find_first_by on the selection
            if matches!(text, "." | "!" | "?" | ";") {
                return false;
            }

            current = next_sel;
        }

        // Couldn't find sentence boundary - assume same sentence
        true
    }

    /// Find the next modal keyword after the given position, if any.
    fn find_next_modal(&self, selection: &LLSelection, after_sel: &LLSelection) -> Option<LLSelection> {
        let modals: Vec<_> = selection
            .find_by(&x::attr::<ContractKeyword>())
            .into_iter()
            .filter(|(sel, kw)| {
                matches!(
                    kw,
                    ContractKeyword::Shall | ContractKeyword::May | ContractKeyword::ShallNot
                ) && self.selection_is_before(after_sel, sel)
            })
            .collect();

        modals.first().map(|(sel, _)| sel.clone())
    }

    /// Find conditions (If/Unless/Provided/SubjectTo) that apply to this modal.
    ///
    /// A condition applies if:
    /// 1. It's in the same sentence as the modal
    /// 2. It falls within the modal's clause boundary (between the modal and the
    ///    next modal or sentence end)
    ///
    /// This prevents "If payment is late, the Company shall deliver, and the Vendor
    /// shall refund" from attaching the If condition to both obligations.
    fn find_conditions(&self, selection: &LLSelection, modal_sel: &LLSelection) -> Vec<ConditionRef> {
        let mut conditions = Vec::new();

        // Find the next modal to establish clause boundary
        let next_modal = self.find_next_modal(selection, modal_sel);

        // Look for condition keywords in the selection
        let condition_types = [
            ContractKeyword::If,
            ContractKeyword::Unless,
            ContractKeyword::Provided,
            ContractKeyword::SubjectTo,
        ];

        for condition_type in condition_types {
            for (cond_sel, _) in selection.find_by(&x::attr_eq(&condition_type)) {
                // Only include conditions in the same sentence as the modal
                if !self.in_same_sentence(&cond_sel, modal_sel) {
                    continue;
                }

                // Check clause boundary: condition must be either:
                // 1. Before the modal (prefix condition like "If X, shall Y")
                // 2. After the modal but before the next modal (suffix condition like "shall Y unless Z")
                let cond_before_modal = self.selection_is_before(&cond_sel, modal_sel);
                let cond_after_modal = self.selection_is_before(modal_sel, &cond_sel);

                if cond_before_modal {
                    // Prefix condition: check there's no other modal between condition and this modal
                    // (i.e., the condition should apply to this modal, not a prior one)
                    // We accept prefix conditions that are nearest to this modal
                    // by checking no modal exists between cond_sel and modal_sel
                    let intervening_modal = selection
                        .find_by(&x::attr::<ContractKeyword>())
                        .into_iter()
                        .any(|(sel, kw)| {
                            matches!(
                                kw,
                                ContractKeyword::Shall
                                    | ContractKeyword::May
                                    | ContractKeyword::ShallNot
                            ) && self.selection_is_before(&cond_sel, &sel)
                                && self.selection_is_before(&sel, modal_sel)
                        });

                    if intervening_modal {
                        // There's another modal between condition and this one
                        // The condition belongs to the earlier modal
                        continue;
                    }
                } else if cond_after_modal {
                    // Suffix condition: must be before the next modal (if any)
                    if let Some(ref next) = next_modal {
                        if !self.selection_is_before(&cond_sel, next) {
                            // Condition is after the next modal, doesn't apply to this one
                            continue;
                        }
                    }
                }

                // Get a preview of text following the condition
                let preview = self.extract_condition_preview(&cond_sel);
                if !preview.is_empty() {
                    conditions.push(ConditionRef {
                        condition_type: condition_type.clone(),
                        text_preview: preview,
                    });
                }
            }
        }

        conditions
    }

    /// Extract a preview of text following a condition keyword.
    ///
    /// Includes words (WORD) and numbers (NATN) to preserve references like "Section 5".
    fn extract_condition_preview(&self, cond_sel: &LLSelection) -> String {
        let mut tokens = Vec::new();
        let mut current = cond_sel.clone();
        let max_tokens = 6; // Limit preview length

        for _ in 0..max_tokens {
            // Skip whitespace
            if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                current = ws_sel;
            } else {
                break;
            }

            // Try to get next word
            if let Some((word_sel, (_, text))) =
                current.match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
            {
                tokens.push(text.to_string());
                current = word_sel;
                continue;
            }

            // Try to get next number (NATN)
            if let Some((num_sel, (_, text))) =
                current.match_first_forwards(&x::all((x::attr_eq(&TextTag::NATN), x::token_text())))
            {
                tokens.push(text.to_string());
                current = num_sel;
                continue;
            }

            // Try to get common reference symbols (§, -)
            if let Some((sym_sel, text)) = current.match_first_forwards(&x::token_text()) {
                if matches!(text, "§" | "-" | "(" | ")") {
                    tokens.push(text.to_string());
                    current = sym_sel;
                    continue;
                }
            }

            // No more relevant tokens
            break;
        }

        if tokens.len() == max_tokens {
            tokens.push("...".to_string());
        }

        tokens.join(" ")
    }

    /// Calculate confidence for an obligation phrase.
    ///
    /// Scoring heuristics:
    /// - Base: 0.75 when modal + obligor found
    /// - +0.10 if obligor is a defined term
    /// - +0.05 if resolved through pronoun chain
    /// - -0.15 if multiple obligor candidates compete
    /// - -0.10 if action span is empty/only stop words
    fn calculate_confidence(
        &self,
        obligor: &ObligorReference,
        action: &str,
        has_multiple_candidates: bool,
    ) -> f64 {
        let mut confidence = self.base_confidence;

        // Bonus for defined term obligor
        match obligor {
            ObligorReference::TermRef { .. } => {
                confidence += self.defined_term_bonus;
            }
            ObligorReference::PronounRef { is_defined_term, .. } => {
                confidence += self.pronoun_chain_bonus;
                if *is_defined_term {
                    confidence += self.defined_term_bonus;
                }
            }
            ObligorReference::NounPhrase { .. } => {
                // No bonus for plain noun phrases
            }
        }

        // Penalty for multiple candidates
        if has_multiple_candidates {
            confidence -= self.multiple_obligor_penalty;
        }

        // Penalty for empty/minimal action
        let action_word_count = action.split_whitespace().count();
        if action_word_count == 0 {
            confidence -= self.empty_action_penalty;
        }

        confidence.clamp(0.0, 1.0)
    }
}

impl Resolver for ObligationPhraseResolver {
    type Attr = Scored<ObligationPhrase>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();

        // Find all modal keywords (Shall, May, ShallNot)
        let modals: Vec<_> = selection
            .find_by(&x::attr::<ContractKeyword>())
            .into_iter()
            .filter(|(_, kw)| {
                matches!(
                    kw,
                    ContractKeyword::Shall | ContractKeyword::May | ContractKeyword::ShallNot
                )
            })
            .collect();

        // Collect ShallNot positions to skip standalone Shall that are part of ShallNot
        let shall_not_positions: Vec<_> = modals
            .iter()
            .filter(|(_, kw)| **kw == ContractKeyword::ShallNot)
            .map(|(sel, _)| sel.clone())
            .collect();

        for (modal_sel, keyword) in modals {
            // Skip Shall if it's part of a ShallNot (the Shall token is contained within ShallNot span)
            if *keyword == ContractKeyword::Shall {
                let is_part_of_shall_not = shall_not_positions.iter().any(|shall_not_sel| {
                    // Check if modal_sel is contained within shall_not_sel
                    let [before, after] = modal_sel.split_with(shall_not_sel);
                    before.is_none() && after.is_none()
                });
                if is_part_of_shall_not {
                    continue;
                }
            }
            // Determine obligation type
            let obligation_type = match ObligationType::from_keyword(keyword) {
                Some(t) => t,
                None => continue,
            };

            // Find the obligor
            let (obligor, has_multiple) = match self.find_obligor(&selection, &modal_sel) {
                Some(o) => o,
                None => continue, // Skip if no obligor found
            };

            // Extract the action
            let action = self.extract_action(&selection, &modal_sel);

            // Find conditions
            let conditions = self.find_conditions(&selection, &modal_sel);

            // Calculate confidence
            let confidence = self.calculate_confidence(&obligor, &action, has_multiple);

            let phrase = ObligationPhrase {
                obligor,
                obligation_type,
                action,
                conditions,
            };

            results.push(modal_sel.finish_with_attr(Scored::rule_based(
                phrase,
                confidence,
                "obligation_phrase",
            )));
        }

        results
    }
}

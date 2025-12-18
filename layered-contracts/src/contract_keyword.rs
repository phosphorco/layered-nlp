//! Contract keyword detection resolver.
//!
//! This resolver identifies contract-specific keywords that signal:
//! - Obligations (shall, must, may)
//! - Definitions (means, includes, hereinafter)
//! - Conditionals (if, unless, provided)
//! - Party references (party, parties)

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};

/// Contract-specific keywords that carry semantic meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractKeyword {
    // Obligation modals
    /// "shall", "must" - indicates a duty/obligation
    Shall,
    /// "may", "can" - indicates permission
    May,
    /// "shall not", "must not" - indicates prohibition
    ShallNot,

    // Definition signals
    /// "means", "refers to", "is defined as"
    Means,
    /// "includes", "including"
    Includes,
    /// "hereinafter", "hereafter"
    Hereinafter,

    // Conditionals
    /// "if", "when", "upon", "in the event"
    If,
    /// "unless", "except"
    Unless,
    /// "provided", "provided that", "provided, however"
    Provided,
    /// "subject to"
    SubjectTo,

    // Party indicators
    /// "party", "parties"
    Party,
}

/// Configuration for contract keyword detection.
pub struct ContractKeywordResolver {
    /// Keywords that map to Shall (obligation)
    shall_keywords: Vec<&'static str>,
    /// Keywords that map to May (permission)
    may_keywords: Vec<&'static str>,
    /// Keywords that map to Means (definition)
    means_keywords: Vec<&'static str>,
    /// Keywords that map to Includes
    includes_keywords: Vec<&'static str>,
    /// Keywords that map to Hereinafter
    hereinafter_keywords: Vec<&'static str>,
    /// Keywords that map to If (conditional)
    if_keywords: Vec<&'static str>,
    /// Keywords that map to Unless
    unless_keywords: Vec<&'static str>,
    /// Keywords that map to Provided
    provided_keywords: Vec<&'static str>,
    /// Keywords that map to SubjectTo
    subject_to_keywords: Vec<&'static str>,
    /// Keywords that map to Party
    party_keywords: Vec<&'static str>,
}

impl Default for ContractKeywordResolver {
    fn default() -> Self {
        Self {
            shall_keywords: vec!["shall", "must"],
            may_keywords: vec!["may", "can"],
            means_keywords: vec!["means", "mean"],
            includes_keywords: vec!["includes", "including", "include"],
            hereinafter_keywords: vec!["hereinafter", "hereafter"],
            if_keywords: vec!["if", "when", "upon"],
            unless_keywords: vec!["unless", "except"],
            provided_keywords: vec!["provided"],
            subject_to_keywords: vec!["subject"],
            party_keywords: vec!["party", "parties"],
        }
    }
}

impl ContractKeywordResolver {
    /// Create a new resolver with default keyword lists.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a resolver with custom keyword lists.
    #[allow(clippy::too_many_arguments)]
    pub fn with_keywords(
        shall: &[&'static str],
        may: &[&'static str],
        means: &[&'static str],
        includes: &[&'static str],
        hereinafter: &[&'static str],
        if_kw: &[&'static str],
        unless: &[&'static str],
        provided: &[&'static str],
        subject_to: &[&'static str],
        party: &[&'static str],
    ) -> Self {
        Self {
            shall_keywords: shall.to_vec(),
            may_keywords: may.to_vec(),
            means_keywords: means.to_vec(),
            includes_keywords: includes.to_vec(),
            hereinafter_keywords: hereinafter.to_vec(),
            if_keywords: if_kw.to_vec(),
            unless_keywords: unless.to_vec(),
            provided_keywords: provided.to_vec(),
            subject_to_keywords: subject_to.to_vec(),
            party_keywords: party.to_vec(),
        }
    }

    /// Match a single token to a keyword.
    fn match_single_token(&self, text: &str) -> Option<ContractKeyword> {
        let lower = text.to_lowercase();
        let lower_str = lower.as_str();

        if self.shall_keywords.contains(&lower_str) {
            Some(ContractKeyword::Shall)
        } else if self.may_keywords.contains(&lower_str) {
            Some(ContractKeyword::May)
        } else if self.means_keywords.contains(&lower_str) {
            Some(ContractKeyword::Means)
        } else if self.includes_keywords.contains(&lower_str) {
            Some(ContractKeyword::Includes)
        } else if self.hereinafter_keywords.contains(&lower_str) {
            Some(ContractKeyword::Hereinafter)
        } else if self.if_keywords.contains(&lower_str) {
            Some(ContractKeyword::If)
        } else if self.unless_keywords.contains(&lower_str) {
            Some(ContractKeyword::Unless)
        } else if self.provided_keywords.contains(&lower_str) {
            Some(ContractKeyword::Provided)
        } else if self.subject_to_keywords.contains(&lower_str) {
            Some(ContractKeyword::SubjectTo)
        } else if self.party_keywords.contains(&lower_str) {
            Some(ContractKeyword::Party)
        } else {
            None
        }
    }
}

impl Resolver for ContractKeywordResolver {
    type Attr = ContractKeyword;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();

        // Single token matching
        for (sel, text) in selection.find_by(&x::token_text()) {
            if let Some(keyword) = self.match_single_token(text) {
                // Special handling for "subject to" - check if next word is "to"
                if keyword == ContractKeyword::SubjectTo {
                    // Skip whitespace, then check for "to"
                    if let Some((ws_sel, _)) = sel.match_first_forwards(&x::whitespace()) {
                        if let Some((extended_sel, next_text)) =
                            ws_sel.match_first_forwards(&x::token_text())
                        {
                            if next_text.to_lowercase() == "to" {
                                results
                                    .push(extended_sel.finish_with_attr(ContractKeyword::SubjectTo));
                                continue;
                            }
                        }
                    }
                    // "subject" alone is not a contract keyword
                    continue;
                }

                results.push(sel.finish_with_attr(keyword));
            }
        }

        results
    }
}

/// Resolver for detecting "shall not" / "must not" prohibition patterns.
/// Run this after ContractKeywordResolver to upgrade Shall to ShallNot when followed by "not".
pub struct ProhibitionResolver;

impl Default for ProhibitionResolver {
    fn default() -> Self {
        Self
    }
}

impl ProhibitionResolver {
    pub fn new() -> Self {
        Self
    }
}

impl Resolver for ProhibitionResolver {
    type Attr = ContractKeyword;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        // Find Shall keywords and check if followed by "not"
        selection
            .find_by(&x::attr_eq(&ContractKeyword::Shall))
            .into_iter()
            .filter_map(|(sel, _)| {
                // Try to match whitespace then "not"
                sel.match_first_forwards(&x::whitespace())
                    .and_then(|(ws_sel, _)| {
                        ws_sel.match_first_forwards(&x::token_text()).and_then(
                            |(not_sel, text)| {
                                if text.to_lowercase() == "not" {
                                    Some(not_sel.finish_with_attr(ContractKeyword::ShallNot))
                                } else {
                                    None
                                }
                            },
                        )
                    })
            })
            .collect()
    }
}

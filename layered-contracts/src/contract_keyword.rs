//! Contract keyword detection resolver.
//!
//! This resolver identifies contract-specific keywords that signal:
//! - Obligations (shall, must, may)
//! - Definitions (means, includes, hereinafter)
//! - Conditionals (if, unless, provided)
//! - Party references (party, parties)

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};

/// Contract-specific keywords that carry semantic meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractKeyword {
    // Obligation modals
    /// "shall" - indicates a duty/obligation
    Shall,
    /// "must" - indicates a duty/obligation
    Must,
    /// "may" - indicates permission
    May,
    /// "can" - indicates permission
    Can,
    /// "will" - indicates future commitment/duty
    Will,
    /// "shall not" - indicates prohibition
    ShallNot,
    /// "must not" - indicates prohibition
    MustNot,
    /// "cannot" / "can not" - indicates prohibition
    Cannot,
    /// "will not" - indicates prohibition
    WillNot,

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
    /// Keywords that map to Must (obligation)
    must_keywords: Vec<&'static str>,
    /// Keywords that map to May (permission)
    may_keywords: Vec<&'static str>,
    /// Keywords that map to Can (permission)
    can_keywords: Vec<&'static str>,
    /// Keywords that map to Will (future commitment)
    will_keywords: Vec<&'static str>,
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
            shall_keywords: vec!["shall"],
            must_keywords: vec!["must"],
            may_keywords: vec!["may"],
            can_keywords: vec!["can"],
            will_keywords: vec!["will"],
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

    /// Match a single token to a keyword.
    fn match_single_token(&self, text: &str) -> Option<ContractKeyword> {
        let lower = text.to_lowercase();
        let lower_str = lower.as_str();

        // Check for "cannot" first (single token compound)
        if lower_str == "cannot" {
            return Some(ContractKeyword::Cannot);
        }

        if self.shall_keywords.contains(&lower_str) {
            Some(ContractKeyword::Shall)
        } else if self.must_keywords.contains(&lower_str) {
            Some(ContractKeyword::Must)
        } else if self.may_keywords.contains(&lower_str) {
            Some(ContractKeyword::May)
        } else if self.can_keywords.contains(&lower_str) {
            Some(ContractKeyword::Can)
        } else if self.will_keywords.contains(&lower_str) {
            Some(ContractKeyword::Will)
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

/// Resolver for detecting prohibition patterns like "shall not", "must not", "will not", "can not".
/// Run this after ContractKeywordResolver to upgrade modals to their negated forms when followed by "not".
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

    /// Check if a modal keyword followed by "not" should produce a negated keyword
    fn get_negated_keyword(keyword: &ContractKeyword) -> Option<ContractKeyword> {
        match keyword {
            ContractKeyword::Shall => Some(ContractKeyword::ShallNot),
            ContractKeyword::Must => Some(ContractKeyword::MustNot),
            ContractKeyword::Will => Some(ContractKeyword::WillNot),
            ContractKeyword::Can => Some(ContractKeyword::Cannot),
            _ => None,
        }
    }
}

impl Resolver for ProhibitionResolver {
    type Attr = ContractKeyword;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();

        // Check each modal keyword type for "not" following
        let modals_to_check = [
            ContractKeyword::Shall,
            ContractKeyword::Must,
            ContractKeyword::Will,
            ContractKeyword::Can,
        ];

        for modal in modals_to_check {
            for (sel, _) in selection.find_by(&x::attr_eq(&modal)) {
                // Try to match whitespace then "not"
                if let Some((ws_sel, _)) = sel.match_first_forwards(&x::whitespace()) {
                    if let Some((not_sel, text)) = ws_sel.match_first_forwards(&x::token_text()) {
                        if text.to_lowercase() == "not" {
                            if let Some(negated) = Self::get_negated_keyword(&modal) {
                                results.push(not_sel.finish_with_attr(negated));
                            }
                        }
                    }
                }
            }
        }

        results
    }
}

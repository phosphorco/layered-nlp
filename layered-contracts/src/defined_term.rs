//! Defined term detection resolver.
//!
//! This resolver identifies formally defined terms in contract text using patterns:
//! - `"Term" means ...` - Quoted term followed by "means" keyword
//! - `ABC Corp (the "Term")` - Parenthetical definition
//! - `ABC Corp, hereinafter "Term"` - Hereinafter pattern

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver, TextTag};

use crate::contract_keyword::ContractKeyword;
use crate::Scored;

/// Represents a formally defined term in a contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinedTerm {
    /// The term name as it appears in quotes (e.g., "Contractor")
    pub term_name: String,
    /// How this term was defined
    pub definition_type: DefinitionType,
}

/// The type of formal definition pattern detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionType {
    /// Pattern: "Term" means ... (quoted term followed by means keyword)
    /// Example: "Company" means ABC Corporation
    QuotedMeans,

    /// Pattern: ABC Corp (the "Term") - parenthetical definition
    /// Example: ABC Corporation (the "Company")
    Parenthetical,

    /// Pattern: ABC Corp, hereinafter "Term" - hereinafter pattern
    /// Example: ABC Corporation, hereinafter referred to as the "Contractor"
    Hereinafter,
}

/// Resolver for detecting formally defined terms in contract text.
pub struct DefinedTermResolver {
    /// Base confidence for QuotedMeans pattern (highest - most explicit)
    quoted_means_confidence: f64,
    /// Base confidence for Parenthetical pattern
    parenthetical_confidence: f64,
    /// Base confidence for Hereinafter pattern
    hereinafter_confidence: f64,
}

impl Default for DefinedTermResolver {
    fn default() -> Self {
        Self {
            quoted_means_confidence: 0.95,
            parenthetical_confidence: 0.90,
            hereinafter_confidence: 0.90,
        }
    }
}

impl DefinedTermResolver {
    /// Create a new resolver with default confidence scores.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a resolver with custom confidence scores.
    pub fn with_confidence(
        quoted_means: f64,
        parenthetical: f64,
        hereinafter: f64,
    ) -> Self {
        Self {
            quoted_means_confidence: quoted_means,
            parenthetical_confidence: parenthetical,
            hereinafter_confidence: hereinafter,
        }
    }

    /// Extract term name from a selection starting after opening quote.
    /// Returns (extended_selection_to_closing_quote, term_name) or None if no valid term found.
    fn extract_quoted_term_forwards(
        &self,
        start_sel: &LLSelection,
    ) -> Option<(LLSelection, String)> {
        let mut current = start_sel.clone();
        let mut term_parts = Vec::new();

        loop {
            // Skip optional whitespace between words
            if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                current = ws_sel;
            }

            // Try to match a word
            if let Some((word_sel, (_, text))) = current
                .match_first_forwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
            {
                term_parts.push(text.to_string());
                current = word_sel;
            } else {
                break;
            }
        }

        // Skip optional whitespace before closing quote
        if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
            current = ws_sel;
        }

        // Check for closing quote
        if let Some((close_quote_sel, _)) = current.match_first_forwards(&x::attr_eq(&'"')) {
            if !term_parts.is_empty() {
                let term_name = term_parts.join(" ");
                return Some((close_quote_sel, term_name));
            }
        }

        None
    }

    /// Extract term name backwards from a selection ending before closing quote.
    /// Returns (extended_selection_to_opening_quote, term_name) or None if no valid term found.
    fn extract_quoted_term_backwards(
        &self,
        start_sel: &LLSelection,
    ) -> Option<(LLSelection, String)> {
        let mut current = start_sel.clone();
        let mut term_parts = Vec::new();

        loop {
            // Skip optional whitespace between words
            if let Some((ws_sel, _)) = current.match_first_backwards(&x::whitespace()) {
                current = ws_sel;
            }

            // Try to match a word
            if let Some((word_sel, (_, text))) = current
                .match_first_backwards(&x::all((x::attr_eq(&TextTag::WORD), x::token_text())))
            {
                term_parts.push(text.to_string());
                current = word_sel;
            } else {
                break;
            }
        }

        // Skip optional whitespace before opening quote
        if let Some((ws_sel, _)) = current.match_first_backwards(&x::whitespace()) {
            current = ws_sel;
        }

        // Check for opening quote
        if let Some((open_quote_sel, _)) = current.match_first_backwards(&x::attr_eq(&'"')) {
            if !term_parts.is_empty() {
                // Reverse since we collected backwards
                term_parts.reverse();
                let term_name = term_parts.join(" ");
                return Some((open_quote_sel, term_name));
            }
        }

        None
    }

    /// Pattern 1: "Term" means ...
    /// Find ContractKeyword::Means, then look backwards for a quoted term.
    fn find_quoted_means_patterns(
        &self,
        selection: &LLSelection,
    ) -> Vec<LLCursorAssignment<Scored<DefinedTerm>>> {
        selection
            .find_by(&x::attr_eq(&ContractKeyword::Means))
            .into_iter()
            .filter_map(|(means_sel, _)| {
                // Match backwards: whitespace -> closing quote
                means_sel
                    .match_first_backwards(&x::whitespace())
                    .and_then(|(ws_sel, _)| ws_sel.match_first_backwards(&x::attr_eq(&'"')))
                    .and_then(|(close_quote_sel, _)| {
                        // Extract term backwards from closing quote
                        self.extract_quoted_term_backwards(&close_quote_sel)
                    })
                    .map(|(full_sel, term_name)| {
                        full_sel.finish_with_attr(Scored::rule_based(
                            DefinedTerm {
                                term_name,
                                definition_type: DefinitionType::QuotedMeans,
                            },
                            self.quoted_means_confidence,
                            "quoted_means",
                        ))
                    })
            })
            .collect()
    }

    /// Pattern 2: ABC Corp (the "Term")
    /// Find opening paren, then look for optional "the" + quoted term + closing paren.
    fn find_parenthetical_patterns(
        &self,
        selection: &LLSelection,
    ) -> Vec<LLCursorAssignment<Scored<DefinedTerm>>> {
        selection
            .find_by(&x::attr_eq(&'('))
            .into_iter()
            .filter_map(|(paren_sel, _)| {
                let mut current = paren_sel.clone();

                // Skip optional whitespace
                if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                    current = ws_sel;
                }

                // Match optional "the"
                if let Some((the_sel, text)) = current.match_first_forwards(&x::token_text()) {
                    if text.to_lowercase() == "the" {
                        current = the_sel;
                        // Skip whitespace after "the"
                        if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                            current = ws_sel;
                        }
                    }
                }

                // Match opening quote
                current
                    .match_first_forwards(&x::attr_eq(&'"'))
                    .and_then(|(open_quote_sel, _)| {
                        // Extract term forwards from opening quote
                        self.extract_quoted_term_forwards(&open_quote_sel)
                    })
                    .and_then(|(after_close_quote, term_name)| {
                        // Skip optional whitespace
                        let mut check_sel = after_close_quote.clone();
                        if let Some((ws_sel, _)) = check_sel.match_first_forwards(&x::whitespace())
                        {
                            check_sel = ws_sel;
                        }

                        // Check for closing paren
                        check_sel
                            .match_first_forwards(&x::attr_eq(&')'))
                            .map(|(close_paren_sel, _)| {
                                close_paren_sel.finish_with_attr(Scored::rule_based(
                                    DefinedTerm {
                                        term_name,
                                        definition_type: DefinitionType::Parenthetical,
                                    },
                                    self.parenthetical_confidence,
                                    "parenthetical",
                                ))
                            })
                    })
            })
            .collect()
    }

    /// Pattern 3: ABC Corp, hereinafter "Term"
    /// Find ContractKeyword::Hereinafter, then look forwards for quoted term.
    fn find_hereinafter_patterns(
        &self,
        selection: &LLSelection,
    ) -> Vec<LLCursorAssignment<Scored<DefinedTerm>>> {
        selection
            .find_by(&x::attr_eq(&ContractKeyword::Hereinafter))
            .into_iter()
            .filter_map(|(hereinafter_sel, _)| {
                let mut current = hereinafter_sel.clone();

                // Skip optional whitespace
                if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                    current = ws_sel;
                }

                // Skip optional "referred"
                if let Some((word_sel, text)) = current.match_first_forwards(&x::token_text()) {
                    if text.to_lowercase() == "referred" {
                        current = word_sel;
                        // Skip whitespace + "to"
                        if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                            current = ws_sel;
                        }
                        if let Some((to_sel, text)) = current.match_first_forwards(&x::token_text())
                        {
                            if text.to_lowercase() == "to" {
                                current = to_sel;
                                // Skip whitespace + "as"
                                if let Some((ws_sel, _)) =
                                    current.match_first_forwards(&x::whitespace())
                                {
                                    current = ws_sel;
                                }
                                if let Some((as_sel, text)) =
                                    current.match_first_forwards(&x::token_text())
                                {
                                    if text.to_lowercase() == "as" {
                                        current = as_sel;
                                    }
                                }
                            }
                        }
                    }
                }

                // Skip optional whitespace
                if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                    current = ws_sel;
                }

                // Skip optional "the"
                if let Some((the_sel, text)) = current.match_first_forwards(&x::token_text()) {
                    if text.to_lowercase() == "the" {
                        current = the_sel;
                        // Skip whitespace after "the"
                        if let Some((ws_sel, _)) = current.match_first_forwards(&x::whitespace()) {
                            current = ws_sel;
                        }
                    }
                }

                // Match opening quote
                current
                    .match_first_forwards(&x::attr_eq(&'"'))
                    .and_then(|(open_quote_sel, _)| {
                        // Extract term forwards from opening quote
                        self.extract_quoted_term_forwards(&open_quote_sel)
                    })
                    .map(|(full_sel, term_name)| {
                        full_sel.finish_with_attr(Scored::rule_based(
                            DefinedTerm {
                                term_name,
                                definition_type: DefinitionType::Hereinafter,
                            },
                            self.hereinafter_confidence,
                            "hereinafter",
                        ))
                    })
            })
            .collect()
    }
}

impl Resolver for DefinedTermResolver {
    type Attr = Scored<DefinedTerm>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();

        // Pattern 1: "Term" means ...
        results.extend(self.find_quoted_means_patterns(&selection));

        // Pattern 2: (the "Term")
        results.extend(self.find_parenthetical_patterns(&selection));

        // Pattern 3: hereinafter "Term"
        results.extend(self.find_hereinafter_patterns(&selection));

        results
    }
}

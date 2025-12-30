//! Terms of art detection for contract language analysis.
//!
//! This resolver identifies multi-word legal expressions that should be treated
//! as atomic units rather than parsed compositionally:
//!
//! - **Legal doctrines**: "force majeure", "res judicata", "prima facie"
//! - **Obligation phrases**: "indemnify and hold harmless", "represent and warrant"
//! - **Payment terms**: "net 30", "cash on delivery"
//! - **Contract mechanisms**: "material adverse change", "right of first refusal"
//! - **Allocation terms**: "pro rata", "pari passu"
//! - **Interpretive phrases**: "time is of the essence", "without prejudice"
//!
//! # Example
//!
//! ```ignore
//! use layered_contracts::TermsOfArtResolver;
//! use layered_nlp::create_line_from_string;
//!
//! let line = create_line_from_string("The Company shall indemnify and hold harmless the Buyer.")
//!     .run(&TermsOfArtResolver::new());
//! // "indemnify and hold harmless" detected as single TermOfArt
//! ```
//!
//! # Why This Matters
//!
//! Without term-of-art detection, "indemnify and hold harmless" might be parsed as
//! two separate obligations or split incorrectly. Marking these phrases as atomic
//! prevents downstream resolvers from misinterpreting their structure.

use std::collections::HashMap;

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};

/// A detected term of art (legal multi-word expression).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermOfArt {
    /// The canonical form of the term (e.g., "force majeure")
    pub canonical: String,
    /// Category for downstream processing
    pub category: TermOfArtCategory,
}

/// Categories of legal terms of art.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TermOfArtCategory {
    /// Legal doctrines: "force majeure", "res judicata", "prima facie"
    LegalDoctrine,
    /// Obligation phrases: "indemnify and hold harmless", "represent and warrant"
    ObligationPhrase,
    /// Payment terms: "net 30", "cash on delivery", "COD"
    PaymentTerm,
    /// Contract mechanisms: "material adverse change", "change of control"
    ContractMechanism,
    /// Allocation/ranking terms: "pro rata", "pari passu"
    AllocationTerm,
    /// Interpretive phrases: "time is of the essence", "without prejudice"
    InterpretivePhrase,
}

impl TermOfArtCategory {
    /// Returns a human-readable description of the category.
    pub fn description(&self) -> &'static str {
        match self {
            TermOfArtCategory::LegalDoctrine => "Legal doctrine",
            TermOfArtCategory::ObligationPhrase => "Obligation phrase",
            TermOfArtCategory::PaymentTerm => "Payment term",
            TermOfArtCategory::ContractMechanism => "Contract mechanism",
            TermOfArtCategory::AllocationTerm => "Allocation term",
            TermOfArtCategory::InterpretivePhrase => "Interpretive phrase",
        }
    }
}

/// Resolver for detecting legal terms of art.
///
/// Uses a dictionary-based approach where terms are keyed by their first word
/// for efficient O(1) lookup, then matched by remaining words.
#[derive(Debug, Clone)]
pub struct TermsOfArtResolver {
    /// Maps first word (lowercased) → list of (remaining words, TermOfArt)
    dictionary: HashMap<String, Vec<(Vec<String>, TermOfArt)>>,
}

impl Default for TermsOfArtResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl TermsOfArtResolver {
    /// Creates a new resolver with the default legal terms dictionary.
    pub fn new() -> Self {
        let mut resolver = Self {
            dictionary: HashMap::new(),
        };
        resolver.add_defaults();
        resolver
    }

    /// Creates an empty resolver (for custom dictionaries).
    pub fn empty() -> Self {
        Self {
            dictionary: HashMap::new(),
        }
    }

    /// Adds a term to the dictionary.
    ///
    /// The phrase is split on whitespace and lowercased for matching.
    pub fn add(&mut self, phrase: &str, category: TermOfArtCategory) {
        let words: Vec<String> = phrase.split_whitespace().map(|s| s.to_lowercase()).collect();

        if let Some(first) = words.first() {
            let remaining = words[1..].to_vec();
            let term = TermOfArt {
                canonical: phrase.to_string(),
                category,
            };

            self.dictionary
                .entry(first.clone())
                .or_default()
                .push((remaining, term));
        }
    }

    /// Returns the number of terms in the dictionary.
    pub fn len(&self) -> usize {
        self.dictionary.values().map(|v| v.len()).sum()
    }

    /// Returns true if the dictionary is empty.
    pub fn is_empty(&self) -> bool {
        self.dictionary.is_empty()
    }

    /// Populates the dictionary with default legal terms.
    fn add_defaults(&mut self) {
        // Legal doctrines (Latin/French terms)
        self.add("force majeure", TermOfArtCategory::LegalDoctrine);
        self.add("res judicata", TermOfArtCategory::LegalDoctrine);
        self.add("stare decisis", TermOfArtCategory::LegalDoctrine);
        self.add("prima facie", TermOfArtCategory::LegalDoctrine);
        self.add("ex parte", TermOfArtCategory::LegalDoctrine);
        self.add("bona fide", TermOfArtCategory::LegalDoctrine);
        self.add("ab initio", TermOfArtCategory::LegalDoctrine);
        self.add("de facto", TermOfArtCategory::LegalDoctrine);
        self.add("de jure", TermOfArtCategory::LegalDoctrine);
        self.add("ipso facto", TermOfArtCategory::LegalDoctrine);
        self.add("sui generis", TermOfArtCategory::LegalDoctrine);
        self.add("ultra vires", TermOfArtCategory::LegalDoctrine);

        // Obligation phrases (single concept, not multiple obligations)
        self.add(
            "indemnify and hold harmless",
            TermOfArtCategory::ObligationPhrase,
        );
        self.add("represent and warrant", TermOfArtCategory::ObligationPhrase);
        self.add(
            "covenant not to compete",
            TermOfArtCategory::ObligationPhrase,
        );
        self.add("acknowledge and agree", TermOfArtCategory::ObligationPhrase);
        self.add(
            "release and discharge",
            TermOfArtCategory::ObligationPhrase,
        );
        self.add("waive and release", TermOfArtCategory::ObligationPhrase);

        // Payment terms
        self.add("net 30", TermOfArtCategory::PaymentTerm);
        self.add("net 60", TermOfArtCategory::PaymentTerm);
        self.add("net 90", TermOfArtCategory::PaymentTerm);
        self.add("net 10", TermOfArtCategory::PaymentTerm);
        self.add("net 15", TermOfArtCategory::PaymentTerm);
        self.add("net 45", TermOfArtCategory::PaymentTerm);
        self.add("cash on delivery", TermOfArtCategory::PaymentTerm);
        self.add("due on receipt", TermOfArtCategory::PaymentTerm);

        // Contract mechanisms
        self.add(
            "material adverse change",
            TermOfArtCategory::ContractMechanism,
        );
        self.add(
            "material adverse effect",
            TermOfArtCategory::ContractMechanism,
        );
        self.add("change of control", TermOfArtCategory::ContractMechanism);
        self.add(
            "right of first refusal",
            TermOfArtCategory::ContractMechanism,
        );
        self.add("right of first offer", TermOfArtCategory::ContractMechanism);
        self.add("change in control", TermOfArtCategory::ContractMechanism);
        self.add("liquidated damages", TermOfArtCategory::ContractMechanism);
        self.add("limitation of liability", TermOfArtCategory::ContractMechanism);

        // Allocation/ranking terms (Latin)
        self.add("pro rata", TermOfArtCategory::AllocationTerm);
        self.add("pari passu", TermOfArtCategory::AllocationTerm);
        self.add("mutatis mutandis", TermOfArtCategory::AllocationTerm);
        self.add("inter alia", TermOfArtCategory::AllocationTerm);
        self.add("pro tanto", TermOfArtCategory::AllocationTerm);
        self.add("pro forma", TermOfArtCategory::AllocationTerm);

        // Interpretive phrases
        self.add(
            "time is of the essence",
            TermOfArtCategory::InterpretivePhrase,
        );
        self.add("without prejudice", TermOfArtCategory::InterpretivePhrase);
        self.add(
            "for the avoidance of doubt",
            TermOfArtCategory::InterpretivePhrase,
        );
        self.add(
            "notwithstanding the foregoing",
            TermOfArtCategory::InterpretivePhrase,
        );
        self.add(
            "notwithstanding anything to the contrary",
            TermOfArtCategory::InterpretivePhrase,
        );
        self.add(
            "subject to the foregoing",
            TermOfArtCategory::InterpretivePhrase,
        );
        self.add("as the case may be", TermOfArtCategory::InterpretivePhrase);
    }

    /// Attempts to match remaining words after the first word.
    ///
    /// Returns the extended selection if all remaining words match,
    /// or None if the match fails.
    fn try_match_phrase(&self, start: &LLSelection, remaining: &[String]) -> Option<LLSelection> {
        // If no remaining words, the first word is the complete term
        if remaining.is_empty() {
            return Some(start.clone());
        }

        let mut current = start.clone();

        for expected in remaining {
            // Skip whitespace
            let (ws_sel, _) = current.match_first_forwards(&x::whitespace())?;
            current = ws_sel;

            // Match next word
            let (word_sel, text) = current.match_first_forwards(&x::token_text())?;
            if text.to_lowercase() != *expected {
                return None;
            }
            current = word_sel;
        }

        Some(current)
    }
}

impl Resolver for TermsOfArtResolver {
    type Attr = TermOfArt;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();

        for (sel, text) in selection.find_by(&x::token_text()) {
            let lower = text.to_lowercase();

            if let Some(candidates) = self.dictionary.get(&lower) {
                for (remaining, term) in candidates {
                    if let Some(extended) = self.try_match_phrase(&sel, remaining) {
                        results.push(extended.finish_with_attr(term.clone()));
                    }
                }
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layered_nlp::{create_line_from_string, x};

    // Helper to run resolver and extract terms
    fn extract_terms(text: &str) -> Vec<TermOfArt> {
        let line = create_line_from_string(text).run(&TermsOfArtResolver::new());
        line.find(&x::attr::<TermOfArt>())
            .into_iter()
            .map(|f| (*f.attr()).clone())
            .collect()
    }

    // Gate 1: Type and dictionary tests

    #[test]
    fn test_term_of_art_category_description() {
        assert_eq!(
            TermOfArtCategory::LegalDoctrine.description(),
            "Legal doctrine"
        );
        assert_eq!(
            TermOfArtCategory::ObligationPhrase.description(),
            "Obligation phrase"
        );
        assert_eq!(TermOfArtCategory::PaymentTerm.description(), "Payment term");
        assert_eq!(
            TermOfArtCategory::ContractMechanism.description(),
            "Contract mechanism"
        );
        assert_eq!(
            TermOfArtCategory::AllocationTerm.description(),
            "Allocation term"
        );
        assert_eq!(
            TermOfArtCategory::InterpretivePhrase.description(),
            "Interpretive phrase"
        );
    }

    #[test]
    fn test_term_of_art_equality() {
        let term1 = TermOfArt {
            canonical: "force majeure".to_string(),
            category: TermOfArtCategory::LegalDoctrine,
        };
        let term2 = TermOfArt {
            canonical: "force majeure".to_string(),
            category: TermOfArtCategory::LegalDoctrine,
        };
        let term3 = TermOfArt {
            canonical: "force majeure".to_string(),
            category: TermOfArtCategory::ObligationPhrase,
        };

        assert_eq!(term1, term2);
        assert_ne!(term1, term3);
    }

    #[test]
    fn test_resolver_default_dictionary_not_empty() {
        let resolver = TermsOfArtResolver::new();
        assert!(!resolver.is_empty());
        assert!(resolver.len() > 30); // We added ~40 terms
    }

    #[test]
    fn test_resolver_empty_dictionary() {
        let resolver = TermsOfArtResolver::empty();
        assert!(resolver.is_empty());
        assert_eq!(resolver.len(), 0);
    }

    #[test]
    fn test_add_custom_term() {
        let mut resolver = TermsOfArtResolver::empty();
        resolver.add("custom term", TermOfArtCategory::LegalDoctrine);
        assert_eq!(resolver.len(), 1);
    }

    #[test]
    fn test_dictionary_keyed_by_first_word() {
        let resolver = TermsOfArtResolver::new();
        // "force majeure" should be keyed under "force"
        assert!(resolver.dictionary.contains_key("force"));
        // "net 30" and "net 60" should both be under "net"
        let net_entries = resolver.dictionary.get("net").unwrap();
        assert!(net_entries.len() >= 2);
    }

    // Gate 2: Integration tests - Resolver matching

    #[test]
    fn test_detect_force_majeure() {
        let terms = extract_terms("The force majeure clause applies.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "force majeure");
        assert_eq!(terms[0].category, TermOfArtCategory::LegalDoctrine);
    }

    #[test]
    fn test_detect_indemnify_and_hold_harmless() {
        let terms = extract_terms("The Company shall indemnify and hold harmless the Buyer.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "indemnify and hold harmless");
        assert_eq!(terms[0].category, TermOfArtCategory::ObligationPhrase);
    }

    #[test]
    fn test_detect_payment_term_net_30() {
        let terms = extract_terms("Payment is due net 30 from invoice date.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "net 30");
        assert_eq!(terms[0].category, TermOfArtCategory::PaymentTerm);
    }

    #[test]
    fn test_detect_material_adverse_change() {
        let terms = extract_terms("Upon any material adverse change, the agreement terminates.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "material adverse change");
        assert_eq!(terms[0].category, TermOfArtCategory::ContractMechanism);
    }

    #[test]
    fn test_detect_pro_rata() {
        let terms = extract_terms("Distributions shall be made pro rata.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "pro rata");
        assert_eq!(terms[0].category, TermOfArtCategory::AllocationTerm);
    }

    #[test]
    fn test_detect_time_is_of_the_essence() {
        let terms = extract_terms("Time is of the essence in this agreement.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "time is of the essence");
        assert_eq!(terms[0].category, TermOfArtCategory::InterpretivePhrase);
    }

    #[test]
    fn test_case_insensitive_matching() {
        let terms = extract_terms("FORCE MAJEURE shall excuse performance.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "force majeure");
    }

    #[test]
    fn test_multiple_terms_in_one_line() {
        let terms = extract_terms(
            "The pro rata distribution shall apply, notwithstanding the foregoing.",
        );
        assert_eq!(terms.len(), 2);
        let canonicals: Vec<&str> = terms.iter().map(|t| t.canonical.as_str()).collect();
        assert!(canonicals.contains(&"pro rata"));
        assert!(canonicals.contains(&"notwithstanding the foregoing"));
    }

    #[test]
    fn test_no_partial_match() {
        // "net" alone should not match "net 30"
        let terms = extract_terms("The net amount is due.");
        assert!(terms.is_empty());
    }

    #[test]
    fn test_no_false_positive_on_similar_words() {
        // "force" alone should not match
        let terms = extract_terms("The company shall force compliance.");
        assert!(terms.is_empty());
    }

    #[test]
    fn test_right_of_first_refusal() {
        let terms = extract_terms("Seller grants Buyer the right of first refusal.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "right of first refusal");
        assert_eq!(terms[0].category, TermOfArtCategory::ContractMechanism);
    }

    #[test]
    fn test_bona_fide() {
        let terms = extract_terms("The offer must be bona fide.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "bona fide");
        assert_eq!(terms[0].category, TermOfArtCategory::LegalDoctrine);
    }

    #[test]
    fn test_represent_and_warrant() {
        let terms = extract_terms("The parties represent and warrant that they have authority.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "represent and warrant");
        assert_eq!(terms[0].category, TermOfArtCategory::ObligationPhrase);
    }

    #[test]
    fn test_for_the_avoidance_of_doubt() {
        let terms =
            extract_terms("For the avoidance of doubt, this section applies to all parties.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "for the avoidance of doubt");
        assert_eq!(terms[0].category, TermOfArtCategory::InterpretivePhrase);
    }

    #[test]
    fn test_custom_resolver_with_added_term() {
        let mut resolver = TermsOfArtResolver::empty();
        resolver.add("custom legal term", TermOfArtCategory::LegalDoctrine);

        let line = create_line_from_string("This is a custom legal term example.").run(&resolver);
        let terms: Vec<_> = line
            .find(&x::attr::<TermOfArt>())
            .into_iter()
            .map(|f| (*f.attr()).clone())
            .collect();

        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "custom legal term");
    }

    // Gate 3: Pipeline integration tests

    #[test]
    fn test_pipeline_detects_terms_of_art() {
        use crate::pipeline::Pipeline;

        let doc = Pipeline::standard()
            .run_on_text("The force majeure clause shall excuse performance.")
            .unwrap();

        // Check that terms of art are detected in the document
        let line = &doc.lines()[0];
        let terms: Vec<_> = line
            .find(&x::attr::<TermOfArt>())
            .into_iter()
            .map(|f| (*f.attr()).clone())
            .collect();

        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "force majeure");
    }

    #[test]
    fn test_pipeline_with_multiple_terms() {
        use crate::pipeline::Pipeline;

        let doc = Pipeline::standard()
            .run_on_text(
                "Payment is due net 30. Time is of the essence. \
                 The Company shall indemnify and hold harmless the Buyer.",
            )
            .unwrap();

        // Collect all terms from all lines
        let mut all_terms: Vec<TermOfArt> = Vec::new();
        for line in doc.lines() {
            for f in line.find(&x::attr::<TermOfArt>()) {
                all_terms.push((*f.attr()).clone());
            }
        }

        assert_eq!(all_terms.len(), 3);
        let canonicals: Vec<&str> = all_terms.iter().map(|t| t.canonical.as_str()).collect();
        assert!(canonicals.contains(&"net 30"));
        assert!(canonicals.contains(&"time is of the essence"));
        assert!(canonicals.contains(&"indemnify and hold harmless"));
    }

    // Gate 4: Edge cases and comprehensive category coverage

    #[test]
    fn test_all_legal_doctrine_terms() {
        // Latin/French legal doctrines
        let doctrines = [
            "force majeure",
            "res judicata",
            "stare decisis",
            "prima facie",
            "ex parte",
            "bona fide",
            "ab initio",
            "de facto",
            "de jure",
            "ipso facto",
            "sui generis",
            "ultra vires",
        ];

        for doctrine in doctrines {
            let text = format!("This is a {} case.", doctrine);
            let terms = extract_terms(&text);
            assert_eq!(
                terms.len(),
                1,
                "Failed to detect doctrine: {}",
                doctrine
            );
            assert_eq!(
                terms[0].category,
                TermOfArtCategory::LegalDoctrine,
                "Wrong category for: {}",
                doctrine
            );
        }
    }

    #[test]
    fn test_all_obligation_phrase_terms() {
        let phrases = [
            "indemnify and hold harmless",
            "represent and warrant",
            "covenant not to compete",
            "acknowledge and agree",
            "release and discharge",
            "waive and release",
        ];

        for phrase in phrases {
            let text = format!("The party shall {}.", phrase);
            let terms = extract_terms(&text);
            assert_eq!(
                terms.len(),
                1,
                "Failed to detect obligation phrase: {}",
                phrase
            );
            assert_eq!(
                terms[0].category,
                TermOfArtCategory::ObligationPhrase,
                "Wrong category for: {}",
                phrase
            );
        }
    }

    #[test]
    fn test_all_payment_term_terms() {
        let payment_terms = [
            "net 10",
            "net 15",
            "net 30",
            "net 45",
            "net 60",
            "net 90",
            "cash on delivery",
            "due on receipt",
        ];

        for term in payment_terms {
            let text = format!("Payment is {}.", term);
            let terms = extract_terms(&text);
            assert_eq!(
                terms.len(),
                1,
                "Failed to detect payment term: {}",
                term
            );
            assert_eq!(
                terms[0].category,
                TermOfArtCategory::PaymentTerm,
                "Wrong category for: {}",
                term
            );
        }
    }

    #[test]
    fn test_all_contract_mechanism_terms() {
        let mechanisms = [
            "material adverse change",
            "material adverse effect",
            "change of control",
            "change in control",
            "right of first refusal",
            "right of first offer",
            "liquidated damages",
            "limitation of liability",
        ];

        for mechanism in mechanisms {
            let text = format!("Upon {}, the agreement terminates.", mechanism);
            let terms = extract_terms(&text);
            assert_eq!(
                terms.len(),
                1,
                "Failed to detect contract mechanism: {}",
                mechanism
            );
            assert_eq!(
                terms[0].category,
                TermOfArtCategory::ContractMechanism,
                "Wrong category for: {}",
                mechanism
            );
        }
    }

    #[test]
    fn test_all_allocation_term_terms() {
        let allocation_terms = [
            "pro rata",
            "pari passu",
            "mutatis mutandis",
            "inter alia",
            "pro tanto",
            "pro forma",
        ];

        for term in allocation_terms {
            let text = format!("Distributions shall be made {}.", term);
            let terms = extract_terms(&text);
            assert_eq!(
                terms.len(),
                1,
                "Failed to detect allocation term: {}",
                term
            );
            assert_eq!(
                terms[0].category,
                TermOfArtCategory::AllocationTerm,
                "Wrong category for: {}",
                term
            );
        }
    }

    #[test]
    fn test_all_interpretive_phrase_terms() {
        let phrases = [
            "time is of the essence",
            "without prejudice",
            "for the avoidance of doubt",
            "notwithstanding the foregoing",
            "notwithstanding anything to the contrary",
            "subject to the foregoing",
            "as the case may be",
        ];

        for phrase in phrases {
            let text = format!("{}, this applies.", phrase);
            let terms = extract_terms(&text);
            assert_eq!(
                terms.len(),
                1,
                "Failed to detect interpretive phrase: {}",
                phrase
            );
            assert_eq!(
                terms[0].category,
                TermOfArtCategory::InterpretivePhrase,
                "Wrong category for: {}",
                phrase
            );
        }
    }

    #[test]
    fn test_term_at_start_of_line() {
        let terms = extract_terms("Pro rata distribution applies here.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "pro rata");
    }

    #[test]
    fn test_term_at_end_of_line() {
        let terms = extract_terms("This contract operates ab initio");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "ab initio");
    }

    #[test]
    fn test_mixed_case_in_middle() {
        let terms = extract_terms("The FORCE MAJEURE event occurred.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "force majeure");
    }

    #[test]
    fn test_term_with_punctuation_after() {
        let terms = extract_terms("Under pro rata, the distribution is fair.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "pro rata");
    }

    #[test]
    fn test_similar_but_not_matching() {
        // "material" alone shouldn't match "material adverse change"
        let terms = extract_terms("The material is important.");
        assert!(terms.is_empty());
    }

    #[test]
    fn test_overlapping_potential_matches() {
        // "right of first" is not a term, only "right of first refusal/offer"
        let terms = extract_terms("Seller grants right of first refusal.");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].canonical, "right of first refusal");
    }

    #[test]
    fn test_consecutive_terms_different_categories() {
        let terms = extract_terms("Force majeure, pro rata distribution applies.");
        assert_eq!(terms.len(), 2);

        let legal_doctrine = terms.iter().find(|t| t.canonical == "force majeure");
        let allocation = terms.iter().find(|t| t.canonical == "pro rata");

        assert!(legal_doctrine.is_some());
        assert!(allocation.is_some());
        assert_eq!(
            legal_doctrine.unwrap().category,
            TermOfArtCategory::LegalDoctrine
        );
        assert_eq!(allocation.unwrap().category, TermOfArtCategory::AllocationTerm);
    }
}

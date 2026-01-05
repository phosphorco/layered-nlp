//! Relative clause detection and attachment.
//!
//! Detects relative clauses (clauses that modify nouns) and links them
//! to their head nouns. Distinguishes relative clauses from conditionals.
//!
//! Example: "the tenant who fails to pay rent"
//! - Head noun: "tenant"
//! - Relative clause: "who fails to pay rent"
//! - Relative pronoun: "who"

use layered_nlp_document::DocSpan;
use serde::{Deserialize, Serialize};

/// Types of relative pronouns
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelativePronoun {
    /// "who" - typically for persons
    Who,
    /// "whom" - objective case for persons
    Whom,
    /// "whose" - possessive
    Whose,
    /// "which" - for things/concepts
    Which,
    /// "that" - for persons or things (restrictive)
    That,
    /// "where" - for places
    Where,
    /// "when" - for times
    When,
    /// Zero relative / contact clause - "the man [that] I saw"
    Zero,
}

impl RelativePronoun {
    /// Parse a token as a relative pronoun
    pub fn from_token(token: &str) -> Option<Self> {
        let lower = token.to_lowercase();
        match lower.as_str() {
            "who" => Some(Self::Who),
            "whom" => Some(Self::Whom),
            "whose" => Some(Self::Whose),
            "which" => Some(Self::Which),
            "that" => Some(Self::That),
            "where" => Some(Self::Where),
            "when" => Some(Self::When),
            _ => None,
        }
    }

    /// Check if this pronoun typically refers to persons
    pub fn is_personal(&self) -> bool {
        matches!(self, Self::Who | Self::Whom | Self::Whose)
    }

    /// Check if this pronoun typically refers to things
    pub fn is_nonpersonal(&self) -> bool {
        matches!(self, Self::Which | Self::Where | Self::When)
    }

    /// Check if this pronoun can be either (restrictive)
    pub fn is_neutral(&self) -> bool {
        matches!(self, Self::That | Self::Zero)
    }
}

/// Type of relative clause
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelativeClauseType {
    /// Restrictive - essential to identify the noun (no commas)
    /// "The tenant who fails to pay will be evicted"
    Restrictive,
    /// Non-restrictive - additional information (with commas)
    /// "The Landlord, who owns the property, may terminate"
    NonRestrictive,
    /// Ambiguous - cannot determine without more context
    Ambiguous,
}

/// A detected relative clause and its attachment
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelativeClauseAttachment {
    /// The span of the entire relative clause
    pub clause_span: DocSpan,
    /// The span of the head noun this modifies
    pub head_noun_span: DocSpan,
    /// The text of the head noun
    pub head_noun_text: String,
    /// The relative pronoun used
    pub pronoun: RelativePronoun,
    /// The span of the relative pronoun
    pub pronoun_span: DocSpan,
    /// Type of relative clause (restrictive/non-restrictive)
    pub clause_type: RelativeClauseType,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Whether this needs human review
    pub needs_review: bool,
    /// Reason for review if applicable
    pub review_reason: Option<String>,
}

impl RelativeClauseAttachment {
    /// Create a new relative clause attachment
    pub fn new(
        clause_span: DocSpan,
        head_noun_span: DocSpan,
        head_noun_text: impl Into<String>,
        pronoun: RelativePronoun,
        pronoun_span: DocSpan,
    ) -> Self {
        Self {
            clause_span,
            head_noun_span,
            head_noun_text: head_noun_text.into(),
            pronoun,
            pronoun_span,
            clause_type: RelativeClauseType::Ambiguous,
            confidence: 0.7,
            needs_review: false,
            review_reason: None,
        }
    }

    /// Set the clause type with confidence adjustment
    pub fn with_clause_type(mut self, clause_type: RelativeClauseType) -> Self {
        self.clause_type = clause_type;
        // Non-restrictive clauses are easier to detect (comma-bounded)
        if clause_type == RelativeClauseType::NonRestrictive {
            self.confidence = (self.confidence + 0.1).min(1.0);
        }
        self
    }

    /// Flag for review with reason
    pub fn flag_for_review(mut self, reason: impl Into<String>) -> Self {
        self.needs_review = true;
        self.review_reason = Some(reason.into());
        self
    }
}

/// Detects relative clauses and distinguishes them from conditionals
pub struct RelativeClauseDetector {
    /// Words that indicate conditional clauses (not relative)
    pub conditional_markers: Vec<&'static str>,
    /// Words that can start relative clauses
    pub relative_markers: Vec<&'static str>,
}

impl RelativeClauseDetector {
    pub fn new() -> Self {
        Self {
            conditional_markers: vec![
                "if", "unless", "provided", "providing", "assuming",
                "given", "suppose", "supposing", "whether",
            ],
            relative_markers: vec![
                "who", "whom", "whose", "which", "that", "where", "when",
            ],
        }
    }

    /// Check if a token is a relative pronoun
    pub fn is_relative_marker(&self, token: &str) -> bool {
        let lower = token.to_lowercase();
        self.relative_markers.contains(&lower.as_str())
    }

    /// Check if a token is a conditional marker
    pub fn is_conditional_marker(&self, token: &str) -> bool {
        let lower = token.to_lowercase();
        self.conditional_markers.contains(&lower.as_str())
    }

    /// Distinguish "that" as relative vs. complementizer/demonstrative
    ///
    /// Relative "that": follows a noun directly - "the tenant that fails"
    /// Complementizer "that": follows a verb - "I know that he left"
    /// Demonstrative "that": stands alone - "that is wrong"
    pub fn is_relative_that(&self, preceding_token: Option<&str>, following_token: Option<&str>) -> bool {
        // If preceded by a noun/determiner and followed by a verb/pronoun, likely relative
        // This is a heuristic - full parsing would give better results

        let preceded_by_noun = preceding_token.map_or(false, |t| {
            let lower = t.to_lowercase();
            // Check for common noun-ending patterns or determiners
            lower.ends_with("er") || lower.ends_with("or") || lower.ends_with("ant")
                || lower.ends_with("ent") || lower.ends_with("ion")
                || lower.ends_with("ty") || lower.ends_with("ies")
                || matches!(lower.as_str(), "party" | "company" | "tenant" | "landlord"
                    | "buyer" | "seller" | "contract" | "agreement" | "property")
        });

        let followed_by_verb_like = following_token.map_or(false, |t| {
            let lower = t.to_lowercase();
            // Common verb starts or pronouns that indicate relative clause
            lower.ends_with("s") || lower.ends_with("ed") || lower.ends_with("ing")
                || matches!(lower.as_str(), "is" | "are" | "was" | "were" | "has" | "have"
                    | "shall" | "may" | "will" | "can" | "must" | "fails" | "does")
        });

        preceded_by_noun && followed_by_verb_like
    }

    /// Check if this looks like a non-restrictive clause (comma-bounded)
    pub fn is_likely_non_restrictive(preceding_is_comma: bool, has_closing_comma: bool) -> bool {
        preceding_is_comma && has_closing_comma
    }

    /// Detect relative clause type from context
    pub fn detect_clause_type(
        &self,
        preceded_by_comma: bool,
        followed_by_comma: bool,
        pronoun: &RelativePronoun,
    ) -> RelativeClauseType {
        // "which" with commas is almost always non-restrictive
        if *pronoun == RelativePronoun::Which && preceded_by_comma {
            return RelativeClauseType::NonRestrictive;
        }

        // "that" is almost always restrictive
        if *pronoun == RelativePronoun::That {
            return RelativeClauseType::Restrictive;
        }

        // Commas suggest non-restrictive
        if preceded_by_comma && followed_by_comma {
            return RelativeClauseType::NonRestrictive;
        }

        // No commas with who/which - likely restrictive
        if !preceded_by_comma && matches!(pronoun, RelativePronoun::Who | RelativePronoun::Which) {
            return RelativeClauseType::Restrictive;
        }

        RelativeClauseType::Ambiguous
    }
}

impl Default for RelativeClauseDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use layered_nlp_document::DocPosition;

    fn make_span(start_line: usize, start_token: usize, end_line: usize, end_token: usize) -> DocSpan {
        DocSpan {
            start: DocPosition { line: start_line, token: start_token },
            end: DocPosition { line: end_line, token: end_token },
        }
    }

    #[test]
    fn test_relative_pronoun_parsing() {
        assert_eq!(RelativePronoun::from_token("who"), Some(RelativePronoun::Who));
        assert_eq!(RelativePronoun::from_token("WHO"), Some(RelativePronoun::Who));
        assert_eq!(RelativePronoun::from_token("which"), Some(RelativePronoun::Which));
        assert_eq!(RelativePronoun::from_token("that"), Some(RelativePronoun::That));
        assert_eq!(RelativePronoun::from_token("if"), None);
    }

    #[test]
    fn test_relative_pronoun_properties() {
        assert!(RelativePronoun::Who.is_personal());
        assert!(RelativePronoun::Whom.is_personal());
        assert!(!RelativePronoun::Which.is_personal());

        assert!(RelativePronoun::Which.is_nonpersonal());
        assert!(!RelativePronoun::Who.is_nonpersonal());

        assert!(RelativePronoun::That.is_neutral());
        assert!(!RelativePronoun::Who.is_neutral());
    }

    #[test]
    fn test_detector_markers() {
        let detector = RelativeClauseDetector::new();

        assert!(detector.is_relative_marker("who"));
        assert!(detector.is_relative_marker("which"));
        assert!(detector.is_relative_marker("that"));
        assert!(!detector.is_relative_marker("if"));

        assert!(detector.is_conditional_marker("if"));
        assert!(detector.is_conditional_marker("unless"));
        assert!(!detector.is_conditional_marker("who"));
    }

    #[test]
    fn test_relative_that_detection() {
        let detector = RelativeClauseDetector::new();

        // "the tenant that fails" - relative
        assert!(detector.is_relative_that(Some("tenant"), Some("fails")));

        // "the company that owns" - relative
        assert!(detector.is_relative_that(Some("company"), Some("owns")));

        // "know that he" - complementizer (no noun before)
        assert!(!detector.is_relative_that(Some("know"), Some("he")));
    }

    #[test]
    fn test_clause_type_detection() {
        let detector = RelativeClauseDetector::new();

        // "which" with comma = non-restrictive
        let clause_type = detector.detect_clause_type(true, false, &RelativePronoun::Which);
        assert_eq!(clause_type, RelativeClauseType::NonRestrictive);

        // "that" = restrictive
        let clause_type = detector.detect_clause_type(false, false, &RelativePronoun::That);
        assert_eq!(clause_type, RelativeClauseType::Restrictive);

        // "who" without commas = likely restrictive
        let clause_type = detector.detect_clause_type(false, false, &RelativePronoun::Who);
        assert_eq!(clause_type, RelativeClauseType::Restrictive);
    }

    #[test]
    fn test_relative_clause_attachment_creation() {
        let clause_span = make_span(0, 3, 0, 8);
        let head_span = make_span(0, 1, 0, 2);
        let pronoun_span = make_span(0, 3, 0, 4);

        let attachment = RelativeClauseAttachment::new(
            clause_span,
            head_span,
            "tenant",
            RelativePronoun::Who,
            pronoun_span,
        );

        assert_eq!(attachment.head_noun_text, "tenant");
        assert_eq!(attachment.pronoun, RelativePronoun::Who);
        assert!(!attachment.needs_review);
    }

    #[test]
    fn test_relative_clause_with_type() {
        let clause_span = make_span(0, 3, 0, 8);
        let head_span = make_span(0, 1, 0, 2);
        let pronoun_span = make_span(0, 3, 0, 4);

        let attachment = RelativeClauseAttachment::new(
            clause_span,
            head_span,
            "Company",
            RelativePronoun::Which,
            pronoun_span,
        ).with_clause_type(RelativeClauseType::NonRestrictive);

        assert_eq!(attachment.clause_type, RelativeClauseType::NonRestrictive);
        // Non-restrictive gets confidence boost
        assert!(attachment.confidence > 0.7);
    }

    #[test]
    fn test_flag_for_review() {
        let clause_span = make_span(0, 3, 0, 8);
        let head_span = make_span(0, 1, 0, 2);
        let pronoun_span = make_span(0, 3, 0, 4);

        let attachment = RelativeClauseAttachment::new(
            clause_span,
            head_span,
            "entity",
            RelativePronoun::That,
            pronoun_span,
        ).flag_for_review("Ambiguous attachment - could be complementizer");

        assert!(attachment.needs_review);
        assert!(attachment.review_reason.is_some());
    }

    #[test]
    fn test_non_restrictive_detection() {
        // Comma-bounded = non-restrictive
        assert!(RelativeClauseDetector::is_likely_non_restrictive(true, true));

        // No commas = likely restrictive
        assert!(!RelativeClauseDetector::is_likely_non_restrictive(false, false));
    }
}

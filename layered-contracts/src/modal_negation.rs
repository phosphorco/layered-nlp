//! Modal-negation classification for contract obligations.
//!
//! Maps combinations of modal keywords and polarity to obligation types,
//! handling the nuances of legal language where negations can flip or modify meaning.
//!
//! Key insight: "shall not be required to" creates discretion (permission to decline),
//! while "shall not" alone creates prohibition.

use crate::{ContractKeyword, Polarity, PolarityContext};
use layered_nlp_document::DocSpan;
use serde::{Deserialize, Serialize};

/// Extended obligation type including discretion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModalObligationType {
    /// "shall", "must" - a required duty
    Duty,
    /// "may", "can" - a permitted action
    Permission,
    /// "shall not", "must not", "may not" - a prohibited action
    Prohibition,
    /// "shall not be required to", "need not" - discretion to act or not
    Discretion,
}

impl ModalObligationType {
    /// Check if this obligation type creates a binding requirement
    pub fn is_binding(&self) -> bool {
        matches!(self, Self::Duty | Self::Prohibition)
    }

    /// Check if this obligation type grants freedom
    pub fn grants_freedom(&self) -> bool {
        matches!(self, Self::Permission | Self::Discretion)
    }
}

/// Patterns that indicate discretion rather than prohibition
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiscretionPattern {
    /// "shall not be required to" - explicit release from duty
    ShallNotBeRequiredTo,
    /// "need not" - no necessity
    NeedNot,
    /// "is not required to" - passive release from duty
    IsNotRequiredTo,
    /// "shall not be obligated to" - explicit release from obligation
    ShallNotBeObligatedTo,
    /// "may decline to" - explicit permission to refuse
    MayDeclineTo,
    /// Other discretion-granting pattern
    Other(String),
}

impl DiscretionPattern {
    /// Human-readable description
    pub fn description(&self) -> &str {
        match self {
            Self::ShallNotBeRequiredTo => "Release from duty: 'shall not be required to'",
            Self::NeedNot => "No necessity: 'need not'",
            Self::IsNotRequiredTo => "Passive release: 'is not required to'",
            Self::ShallNotBeObligatedTo => "Release from obligation: 'shall not be obligated to'",
            Self::MayDeclineTo => "Permission to refuse: 'may decline to'",
            Self::Other(s) => s.as_str(),
        }
    }
}

/// Result of modal-negation classification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModalNegationClassification {
    /// The span being classified
    pub span: DocSpan,
    /// Detected obligation type
    pub obligation_type: ModalObligationType,
    /// The modal keyword that triggered classification (stored as string for serialization)
    pub modal: String,
    /// Polarity context used in classification
    pub polarity: Polarity,
    /// Whether this classification is ambiguous and needs review
    pub is_ambiguous: bool,
    /// If ambiguous, the reason for ambiguity
    pub ambiguity_reason: Option<String>,
    /// If discretion, the detected pattern
    pub discretion_pattern: Option<DiscretionPattern>,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
}

impl ModalNegationClassification {
    /// Create a clear, unambiguous classification
    pub fn clear(
        span: DocSpan,
        obligation_type: ModalObligationType,
        modal: &ContractKeyword,
        polarity: Polarity,
    ) -> Self {
        Self {
            span,
            obligation_type,
            modal: format!("{:?}", modal),
            polarity,
            is_ambiguous: false,
            ambiguity_reason: None,
            discretion_pattern: None,
            confidence: 0.95,
        }
    }

    /// Create a classification that needs review
    pub fn ambiguous(
        span: DocSpan,
        obligation_type: ModalObligationType,
        modal: &ContractKeyword,
        polarity: Polarity,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            span,
            obligation_type,
            modal: format!("{:?}", modal),
            polarity,
            is_ambiguous: true,
            ambiguity_reason: Some(reason.into()),
            discretion_pattern: None,
            confidence: 0.6,
        }
    }

    /// Create a discretion classification
    pub fn discretion(
        span: DocSpan,
        modal: &ContractKeyword,
        pattern: DiscretionPattern,
    ) -> Self {
        Self {
            span,
            obligation_type: ModalObligationType::Discretion,
            modal: format!("{:?}", modal),
            polarity: Polarity::Negative, // Discretion typically involves negation
            is_ambiguous: true, // Discretion patterns often need human verification
            ambiguity_reason: Some("Discretion pattern detected - verify intent".to_string()),
            discretion_pattern: Some(pattern),
            confidence: 0.75,
        }
    }
}

/// Classifies modal + negation combinations into obligation types
pub struct ModalNegationClassifier {
    /// Confidence penalty for ambiguous polarity
    pub ambiguous_polarity_penalty: f64,
    /// Confidence penalty for complex negation patterns
    pub complex_negation_penalty: f64,
}

impl ModalNegationClassifier {
    pub fn new() -> Self {
        Self {
            ambiguous_polarity_penalty: 0.3,
            complex_negation_penalty: 0.2,
        }
    }

    /// Classify based on modal keyword and polarity context
    pub fn classify(
        &self,
        span: DocSpan,
        modal: &ContractKeyword,
        polarity_ctx: &PolarityContext,
    ) -> ModalNegationClassification {
        // If polarity is ambiguous, flag for review
        if polarity_ctx.polarity == Polarity::Ambiguous || polarity_ctx.needs_review {
            let reason = polarity_ctx
                .review_reason
                .clone()
                .unwrap_or_else(|| "Polarity context is ambiguous".to_string());
            return ModalNegationClassification::ambiguous(
                span,
                self.guess_obligation_type(modal, &polarity_ctx.polarity),
                modal,
                polarity_ctx.polarity,
                reason,
            );
        }

        // Standard classification based on modal + polarity
        match (modal, &polarity_ctx.polarity) {
            // Positive modals (no negation in scope)
            (ContractKeyword::Shall | ContractKeyword::Must, Polarity::Positive) => {
                ModalNegationClassification::clear(
                    span,
                    ModalObligationType::Duty,
                    modal,
                    Polarity::Positive,
                )
            }

            (ContractKeyword::May | ContractKeyword::Can, Polarity::Positive) => {
                ModalNegationClassification::clear(
                    span,
                    ModalObligationType::Permission,
                    modal,
                    Polarity::Positive,
                )
            }

            // Negative modals - prohibition
            (ContractKeyword::ShallNot | ContractKeyword::MustNot, Polarity::Negative) |
            (ContractKeyword::Shall | ContractKeyword::Must, Polarity::Negative) => {
                ModalNegationClassification::clear(
                    span,
                    ModalObligationType::Prohibition,
                    modal,
                    Polarity::Negative,
                )
            }

            // "may not" - prohibition (not permission)
            (ContractKeyword::May, Polarity::Negative) => {
                ModalNegationClassification::clear(
                    span,
                    ModalObligationType::Prohibition,
                    modal,
                    Polarity::Negative,
                )
            }

            // "cannot" / "can not" - prohibition
            (ContractKeyword::Cannot | ContractKeyword::Can, Polarity::Negative) => {
                ModalNegationClassification::clear(
                    span,
                    ModalObligationType::Prohibition,
                    modal,
                    Polarity::Negative,
                )
            }

            // Will modals - statements of future action
            (ContractKeyword::Will, Polarity::Positive) => {
                ModalNegationClassification::clear(
                    span,
                    ModalObligationType::Duty, // "will" implies commitment
                    modal,
                    Polarity::Positive,
                )
            }

            (ContractKeyword::Will | ContractKeyword::WillNot, Polarity::Negative) => {
                ModalNegationClassification::clear(
                    span,
                    ModalObligationType::Prohibition,
                    modal,
                    Polarity::Negative,
                )
            }

            // ShallNot or MustNot with positive polarity is unusual - flag
            (ContractKeyword::ShallNot | ContractKeyword::MustNot, Polarity::Positive) => {
                ModalNegationClassification::ambiguous(
                    span,
                    ModalObligationType::Prohibition,
                    modal,
                    Polarity::Positive,
                    "Compound negative modal with positive polarity - double negation?",
                )
            }

            // Catch-all for other combinations
            (_, Polarity::Ambiguous) => ModalNegationClassification::ambiguous(
                span,
                self.guess_obligation_type(modal, &Polarity::Ambiguous),
                modal,
                Polarity::Ambiguous,
                "Cannot determine polarity",
            ),

            // Default case - flag as ambiguous
            _ => ModalNegationClassification::ambiguous(
                span,
                ModalObligationType::Permission,
                modal,
                polarity_ctx.polarity,
                "Unexpected modal-polarity combination",
            ),
        }
    }

    /// Best-guess obligation type when polarity is unclear
    fn guess_obligation_type(&self, modal: &ContractKeyword, _polarity: &Polarity) -> ModalObligationType {
        match modal {
            ContractKeyword::Shall | ContractKeyword::Must | ContractKeyword::Will => {
                ModalObligationType::Duty
            }
            ContractKeyword::May | ContractKeyword::Can => ModalObligationType::Permission,
            ContractKeyword::ShallNot
            | ContractKeyword::MustNot
            | ContractKeyword::Cannot
            | ContractKeyword::WillNot => ModalObligationType::Prohibition,
            _ => ModalObligationType::Permission,
        }
    }

    /// Detect discretion patterns from tokens following a modal
    pub fn detect_discretion_pattern(tokens: &[&str]) -> Option<DiscretionPattern> {
        let text_lower: Vec<String> = tokens.iter().map(|t| t.to_lowercase()).collect();
        let joined = text_lower.join(" ");

        // Check for discretion patterns
        if joined.contains("shall not be required to")
            || joined.contains("must not be required to")
        {
            return Some(DiscretionPattern::ShallNotBeRequiredTo);
        }

        if joined.contains("shall not be obligated to")
            || joined.contains("shall not be obliged to")
        {
            return Some(DiscretionPattern::ShallNotBeObligatedTo);
        }

        if joined.contains("is not required to") || joined.contains("are not required to") {
            return Some(DiscretionPattern::IsNotRequiredTo);
        }

        if joined.contains("need not") || joined.contains("needn't") {
            return Some(DiscretionPattern::NeedNot);
        }

        if joined.contains("may decline to") || joined.contains("may refuse to") {
            return Some(DiscretionPattern::MayDeclineTo);
        }

        None
    }

    /// Classify with discretion pattern detection
    pub fn classify_with_discretion(
        &self,
        span: DocSpan,
        modal: &ContractKeyword,
        polarity_ctx: &PolarityContext,
        following_tokens: &[&str],
    ) -> ModalNegationClassification {
        // First check for discretion patterns
        if let Some(pattern) = Self::detect_discretion_pattern(following_tokens) {
            return ModalNegationClassification::discretion(span, modal, pattern);
        }

        // Fall back to standard classification
        self.classify(span, modal, polarity_ctx)
    }
}

impl Default for ModalNegationClassifier {
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

    fn positive_ctx(span: DocSpan) -> PolarityContext {
        PolarityContext {
            span,
            polarity: Polarity::Positive,
            negation_count: 0,
            negation_spans: Vec::new(),
            has_double_negative: false,
            confidence: 1.0,
            needs_review: false,
            review_reason: None,
        }
    }

    fn negative_ctx(span: DocSpan) -> PolarityContext {
        PolarityContext {
            span,
            polarity: Polarity::Negative,
            negation_count: 1,
            negation_spans: vec![span],
            has_double_negative: false,
            confidence: 0.95,
            needs_review: false,
            review_reason: None,
        }
    }

    fn ambiguous_ctx(span: DocSpan, reason: &str) -> PolarityContext {
        PolarityContext {
            span,
            polarity: Polarity::Ambiguous,
            negation_count: 2,
            negation_spans: Vec::new(),
            has_double_negative: true,
            confidence: 0.5,
            needs_review: true,
            review_reason: Some(reason.to_string()),
        }
    }

    #[test]
    fn test_shall_positive_is_duty() {
        let classifier = ModalNegationClassifier::new();
        let span = make_span(0, 0, 0, 5);
        let ctx = positive_ctx(span);

        let result = classifier.classify(span, &ContractKeyword::Shall, &ctx);

        assert_eq!(result.obligation_type, ModalObligationType::Duty);
        assert!(!result.is_ambiguous);
        assert!(result.confidence > 0.9);
    }

    #[test]
    fn test_may_positive_is_permission() {
        let classifier = ModalNegationClassifier::new();
        let span = make_span(0, 0, 0, 5);
        let ctx = positive_ctx(span);

        let result = classifier.classify(span, &ContractKeyword::May, &ctx);

        assert_eq!(result.obligation_type, ModalObligationType::Permission);
        assert!(!result.is_ambiguous);
    }

    #[test]
    fn test_shall_negative_is_prohibition() {
        let classifier = ModalNegationClassifier::new();
        let span = make_span(0, 0, 0, 5);
        let ctx = negative_ctx(span);

        let result = classifier.classify(span, &ContractKeyword::Shall, &ctx);

        assert_eq!(result.obligation_type, ModalObligationType::Prohibition);
        assert!(!result.is_ambiguous);
    }

    #[test]
    fn test_may_not_is_prohibition() {
        let classifier = ModalNegationClassifier::new();
        let span = make_span(0, 0, 0, 5);
        let ctx = negative_ctx(span);

        let result = classifier.classify(span, &ContractKeyword::May, &ctx);

        assert_eq!(result.obligation_type, ModalObligationType::Prohibition);
        assert!(!result.is_ambiguous);
    }

    #[test]
    fn test_ambiguous_polarity_flags_for_review() {
        let classifier = ModalNegationClassifier::new();
        let span = make_span(0, 0, 0, 5);
        let ctx = ambiguous_ctx(span, "Double negative detected");

        let result = classifier.classify(span, &ContractKeyword::Shall, &ctx);

        assert!(result.is_ambiguous);
        assert!(result.ambiguity_reason.is_some());
        assert!(result.confidence < 0.9);
    }

    #[test]
    fn test_detect_shall_not_be_required_to() {
        let tokens = ["shall", "not", "be", "required", "to", "provide"];
        let pattern = ModalNegationClassifier::detect_discretion_pattern(&tokens);

        assert!(matches!(pattern, Some(DiscretionPattern::ShallNotBeRequiredTo)));
    }

    #[test]
    fn test_detect_need_not() {
        let tokens = ["The", "tenant", "need", "not", "provide"];
        let pattern = ModalNegationClassifier::detect_discretion_pattern(&tokens);

        assert!(matches!(pattern, Some(DiscretionPattern::NeedNot)));
    }

    #[test]
    fn test_detect_may_decline_to() {
        let tokens = ["The", "landlord", "may", "decline", "to", "accept"];
        let pattern = ModalNegationClassifier::detect_discretion_pattern(&tokens);

        assert!(matches!(pattern, Some(DiscretionPattern::MayDeclineTo)));
    }

    #[test]
    fn test_classify_with_discretion_pattern() {
        let classifier = ModalNegationClassifier::new();
        let span = make_span(0, 0, 0, 10);
        let ctx = negative_ctx(span);
        let tokens = ["shall", "not", "be", "required", "to", "provide"];

        let result = classifier.classify_with_discretion(span, &ContractKeyword::Shall, &ctx, &tokens);

        assert_eq!(result.obligation_type, ModalObligationType::Discretion);
        assert!(result.discretion_pattern.is_some());
    }

    #[test]
    fn test_obligation_type_properties() {
        assert!(ModalObligationType::Duty.is_binding());
        assert!(ModalObligationType::Prohibition.is_binding());
        assert!(!ModalObligationType::Permission.is_binding());
        assert!(!ModalObligationType::Discretion.is_binding());

        assert!(ModalObligationType::Permission.grants_freedom());
        assert!(ModalObligationType::Discretion.grants_freedom());
        assert!(!ModalObligationType::Duty.grants_freedom());
        assert!(!ModalObligationType::Prohibition.grants_freedom());
    }

    #[test]
    fn test_cannot_is_prohibition() {
        let classifier = ModalNegationClassifier::new();
        let span = make_span(0, 0, 0, 5);
        let ctx = negative_ctx(span);

        let result = classifier.classify(span, &ContractKeyword::Cannot, &ctx);

        assert_eq!(result.obligation_type, ModalObligationType::Prohibition);
    }

    #[test]
    fn test_will_positive_is_duty() {
        let classifier = ModalNegationClassifier::new();
        let span = make_span(0, 0, 0, 5);
        let ctx = positive_ctx(span);

        let result = classifier.classify(span, &ContractKeyword::Will, &ctx);

        assert_eq!(result.obligation_type, ModalObligationType::Duty);
    }

    #[test]
    fn test_classification_constructors() {
        let span = make_span(0, 0, 0, 5);

        let clear = ModalNegationClassification::clear(
            span,
            ModalObligationType::Duty,
            &ContractKeyword::Shall,
            Polarity::Positive,
        );
        assert!(!clear.is_ambiguous);
        assert!(clear.confidence > 0.9);
        assert_eq!(clear.modal, "Shall");

        let ambiguous = ModalNegationClassification::ambiguous(
            span,
            ModalObligationType::Prohibition,
            &ContractKeyword::May,
            Polarity::Negative,
            "test reason",
        );
        assert!(ambiguous.is_ambiguous);
        assert_eq!(ambiguous.ambiguity_reason, Some("test reason".to_string()));
        assert_eq!(ambiguous.modal, "May");

        let discretion = ModalNegationClassification::discretion(
            span,
            &ContractKeyword::Shall,
            DiscretionPattern::ShallNotBeRequiredTo,
        );
        assert_eq!(discretion.obligation_type, ModalObligationType::Discretion);
        assert!(discretion.discretion_pattern.is_some());
        assert_eq!(discretion.modal, "Shall");
    }
}

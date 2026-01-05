//! Polarity tracking for contract clauses.
//!
//! Tracks negation count to determine overall polarity:
//! - Even negations = Positive polarity
//! - Odd negations = Negative polarity
//! - Ambiguous when double-negatives or complex patterns detected

use layered_nlp_document::{DocSpan, NegationKind};
use serde::{Deserialize, Serialize};

/// Polarity classification for a clause or phrase
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Polarity {
    /// Affirmative/positive meaning (even number of negations including 0)
    Positive,
    /// Negative meaning (odd number of negations)
    Negative,
    /// Ambiguous due to complex patterns (flag for review)
    Ambiguous,
}

/// Context about how polarity was determined
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolarityContext {
    /// The span this polarity applies to
    pub span: DocSpan,
    /// Detected polarity
    pub polarity: Polarity,
    /// Number of negation markers found
    pub negation_count: usize,
    /// The negation operators that contributed
    pub negation_spans: Vec<DocSpan>,
    /// Whether a double-negative pattern was detected
    pub has_double_negative: bool,
    /// Confidence score (0.0-1.0)
    pub confidence: f64,
    /// Flag indicating this needs human review
    pub needs_review: bool,
    /// Optional explanation for why review is needed
    pub review_reason: Option<String>,
}

impl PolarityContext {
    /// Create a simple positive polarity context
    pub fn positive(span: DocSpan) -> Self {
        Self {
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

    /// Create a simple negative polarity context
    pub fn negative(span: DocSpan, negation_span: DocSpan) -> Self {
        Self {
            span,
            polarity: Polarity::Negative,
            negation_count: 1,
            negation_spans: vec![negation_span],
            has_double_negative: false,
            confidence: 0.95,
            needs_review: false,
            review_reason: None,
        }
    }

    /// Create an ambiguous polarity context that needs review
    pub fn ambiguous(span: DocSpan, reason: impl Into<String>) -> Self {
        Self {
            span,
            polarity: Polarity::Ambiguous,
            negation_count: 0,
            negation_spans: Vec::new(),
            has_double_negative: false,
            confidence: 0.5,
            needs_review: true,
            review_reason: Some(reason.into()),
        }
    }
}

/// Double-negative patterns that require special handling
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DoubleNegativePattern {
    /// "unless not" - exception with negation
    UnlessNot,
    /// "cannot fail to" - negative modal + negative verb
    CannotFailTo,
    /// "not without" - negation + negative preposition
    NotWithout,
    /// "never not" - double explicit negation
    NeverNot,
    /// "no ... without" - quantifier negation + preposition
    NoWithout,
    /// Other detected double-negative pattern
    Other(String),
}

impl DoubleNegativePattern {
    /// Human-readable description of the pattern
    pub fn description(&self) -> &str {
        match self {
            Self::UnlessNot => "Double negative: 'unless not' pattern",
            Self::CannotFailTo => "Double negative: 'cannot fail to' pattern",
            Self::NotWithout => "Double negative: 'not without' pattern",
            Self::NeverNot => "Double negative: 'never not' pattern",
            Self::NoWithout => "Double negative: 'no ... without' pattern",
            Self::Other(s) => s.as_str(),
        }
    }
}

/// Tracks polarity within a clause or phrase by counting negations
pub struct PolarityTracker {
    negations: Vec<(DocSpan, NegationKind)>,
    double_negatives: Vec<(DocSpan, DoubleNegativePattern)>,
}

impl PolarityTracker {
    pub fn new() -> Self {
        Self {
            negations: Vec::new(),
            double_negatives: Vec::new(),
        }
    }

    /// Add a negation operator to track
    pub fn add_negation(&mut self, span: DocSpan, kind: NegationKind) {
        self.negations.push((span, kind));
    }

    /// Record a double-negative pattern
    pub fn add_double_negative(&mut self, span: DocSpan, pattern: DoubleNegativePattern) {
        self.double_negatives.push((span, pattern));
    }

    /// Get the current negation count
    pub fn negation_count(&self) -> usize {
        self.negations.len()
    }

    /// Calculate polarity from tracked negations
    pub fn polarity(&self, span: DocSpan) -> PolarityContext {
        let negation_count = self.negations.len();
        let negation_spans: Vec<_> = self.negations.iter().map(|(s, _)| *s).collect();
        let has_double_negative = !self.double_negatives.is_empty();

        // Double negatives require review - don't auto-resolve
        if has_double_negative {
            let all_patterns: Vec<&str> = self
                .double_negatives
                .iter()
                .map(|(_, p)| p.description())
                .collect();
            let combined_reason = all_patterns.join("; ");
            return PolarityContext {
                span,
                polarity: Polarity::Ambiguous,
                negation_count,
                negation_spans,
                has_double_negative: true,
                confidence: 0.6,
                needs_review: true,
                review_reason: Some(combined_reason),
            };
        }

        // Check for correlative negations (neither...nor) which need special handling
        let has_correlative = self
            .negations
            .iter()
            .any(|(_, kind)| *kind == NegationKind::Correlative);
        if has_correlative {
            // Correlative negations have complex scope - flag for review
            // but don't change polarity calculation
            let polarity = if negation_count % 2 == 0 {
                Polarity::Positive
            } else {
                Polarity::Negative
            };
            return PolarityContext {
                span,
                polarity,
                negation_count,
                negation_spans,
                has_double_negative: false,
                confidence: 0.7,
                needs_review: true,
                review_reason: Some("Correlative negation detected (neither...nor)".to_string()),
            };
        }

        // Simple case: even negations = positive, odd = negative
        let polarity = if negation_count % 2 == 0 {
            Polarity::Positive
        } else {
            Polarity::Negative
        };

        // High confidence for simple cases, lower for multiple negations
        let confidence = match negation_count {
            0 => 1.0,
            1 => 0.95,
            2 => 0.7, // Two negations without detected pattern - might be error
            _ => 0.5, // 3+ negations are suspicious
        };

        let needs_review = negation_count >= 2;
        let review_reason = if needs_review {
            Some(format!(
                "Multiple negations detected ({}) - verify polarity",
                negation_count
            ))
        } else {
            None
        };

        PolarityContext {
            span,
            polarity,
            negation_count,
            negation_spans,
            has_double_negative: false,
            confidence,
            needs_review,
            review_reason,
        }
    }

    /// Reset tracker for new clause
    pub fn reset(&mut self) {
        self.negations.clear();
        self.double_negatives.clear();
    }
}

impl Default for PolarityTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolver that detects polarity contexts in a document
pub struct PolarityResolver;

impl PolarityResolver {
    /// Detect double-negative patterns from text tokens
    pub fn detect_double_negative_patterns(tokens: &[&str]) -> Vec<(usize, DoubleNegativePattern)> {
        let mut patterns = Vec::new();
        let text_lower: Vec<String> = tokens.iter().map(|t| t.to_lowercase()).collect();

        for i in 0..text_lower.len() {
            // "unless not"
            if text_lower[i] == "unless" && i + 1 < text_lower.len() && text_lower[i + 1] == "not" {
                patterns.push((i, DoubleNegativePattern::UnlessNot));
            }

            // "cannot fail to" / "can not fail to"
            if (text_lower[i] == "cannot" || text_lower[i] == "can't")
                && i + 2 < text_lower.len()
                && text_lower[i + 1] == "fail"
                && text_lower[i + 2] == "to"
            {
                patterns.push((i, DoubleNegativePattern::CannotFailTo));
            }

            // "not without"
            if text_lower[i] == "not" && i + 1 < text_lower.len() && text_lower[i + 1] == "without"
            {
                patterns.push((i, DoubleNegativePattern::NotWithout));
            }

            // "never not"
            if text_lower[i] == "never" && i + 1 < text_lower.len() && text_lower[i + 1] == "not" {
                patterns.push((i, DoubleNegativePattern::NeverNot));
            }

            // "no ... without" (within 5 tokens)
            if text_lower[i] == "no" {
                for j in (i + 1)..std::cmp::min(i + 6, text_lower.len()) {
                    if text_lower[j] == "without" {
                        patterns.push((i, DoubleNegativePattern::NoWithout));
                        break;
                    }
                }
            }
        }

        patterns
    }

    /// Check if a token is a negation word
    pub fn is_negation_word(token: &str) -> bool {
        let lower = token.to_lowercase();
        matches!(
            lower.as_str(),
            "not" | "no" | "never" | "neither" | "nor" | "without" | "cannot" | "can't" | "won't"
                | "wouldn't" | "shouldn't" | "couldn't" | "don't" | "doesn't" | "didn't"
                | "isn't" | "aren't" | "wasn't" | "weren't" | "hasn't" | "haven't" | "hadn't"
        )
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
    fn test_no_negations_positive() {
        let tracker = PolarityTracker::new();
        let ctx = tracker.polarity(make_span(0, 0, 0, 5));

        assert_eq!(ctx.polarity, Polarity::Positive);
        assert_eq!(ctx.negation_count, 0);
        assert!(!ctx.needs_review);
        assert_eq!(ctx.confidence, 1.0);
    }

    #[test]
    fn test_single_negation_negative() {
        let mut tracker = PolarityTracker::new();
        tracker.add_negation(make_span(0, 2, 0, 3), NegationKind::Simple);

        let ctx = tracker.polarity(make_span(0, 0, 0, 5));

        assert_eq!(ctx.polarity, Polarity::Negative);
        assert_eq!(ctx.negation_count, 1);
        assert!(!ctx.needs_review);
        assert_eq!(ctx.confidence, 0.95);
    }

    #[test]
    fn test_double_negation_positive() {
        let mut tracker = PolarityTracker::new();
        tracker.add_negation(make_span(0, 2, 0, 3), NegationKind::Simple);
        tracker.add_negation(make_span(0, 4, 0, 5), NegationKind::Simple);

        let ctx = tracker.polarity(make_span(0, 0, 0, 6));

        // Two negations = positive, but needs review
        assert_eq!(ctx.polarity, Polarity::Positive);
        assert_eq!(ctx.negation_count, 2);
        assert!(ctx.needs_review);
        assert!(ctx.confidence < 1.0);
    }

    #[test]
    fn test_double_negative_pattern_ambiguous() {
        let mut tracker = PolarityTracker::new();
        tracker.add_double_negative(make_span(0, 0, 0, 2), DoubleNegativePattern::UnlessNot);

        let ctx = tracker.polarity(make_span(0, 0, 0, 5));

        assert_eq!(ctx.polarity, Polarity::Ambiguous);
        assert!(ctx.has_double_negative);
        assert!(ctx.needs_review);
        assert!(ctx.review_reason.is_some());
    }

    #[test]
    fn test_detect_unless_not() {
        let tokens = ["The", "tenant", "unless", "not", "permitted"];
        let patterns = PolarityResolver::detect_double_negative_patterns(&tokens);

        assert_eq!(patterns.len(), 1);
        assert!(matches!(patterns[0].1, DoubleNegativePattern::UnlessNot));
    }

    #[test]
    fn test_detect_cannot_fail_to() {
        let tokens = ["The", "landlord", "cannot", "fail", "to", "provide"];
        let patterns = PolarityResolver::detect_double_negative_patterns(&tokens);

        assert_eq!(patterns.len(), 1);
        assert!(matches!(patterns[0].1, DoubleNegativePattern::CannotFailTo));
    }

    #[test]
    fn test_detect_not_without() {
        let tokens = ["shall", "not", "without", "consent"];
        let patterns = PolarityResolver::detect_double_negative_patterns(&tokens);

        assert_eq!(patterns.len(), 1);
        assert!(matches!(patterns[0].1, DoubleNegativePattern::NotWithout));
    }

    #[test]
    fn test_detect_no_without() {
        let tokens = ["no", "assignment", "shall", "be", "without", "approval"];
        let patterns = PolarityResolver::detect_double_negative_patterns(&tokens);

        assert_eq!(patterns.len(), 1);
        assert!(matches!(patterns[0].1, DoubleNegativePattern::NoWithout));
    }

    #[test]
    fn test_is_negation_word() {
        assert!(PolarityResolver::is_negation_word("not"));
        assert!(PolarityResolver::is_negation_word("NOT"));
        assert!(PolarityResolver::is_negation_word("never"));
        assert!(PolarityResolver::is_negation_word("cannot"));
        assert!(PolarityResolver::is_negation_word("won't"));

        assert!(!PolarityResolver::is_negation_word("shall"));
        assert!(!PolarityResolver::is_negation_word("may"));
        assert!(!PolarityResolver::is_negation_word("the"));
    }

    #[test]
    fn test_polarity_context_constructors() {
        let span = make_span(0, 0, 0, 5);

        let positive = PolarityContext::positive(span);
        assert_eq!(positive.polarity, Polarity::Positive);
        assert!(!positive.needs_review);

        let negative = PolarityContext::negative(span, make_span(0, 2, 0, 3));
        assert_eq!(negative.polarity, Polarity::Negative);
        assert_eq!(negative.negation_count, 1);

        let ambiguous = PolarityContext::ambiguous(span, "test reason");
        assert_eq!(ambiguous.polarity, Polarity::Ambiguous);
        assert!(ambiguous.needs_review);
        assert_eq!(ambiguous.review_reason, Some("test reason".to_string()));
    }

    #[test]
    fn test_tracker_reset() {
        let mut tracker = PolarityTracker::new();
        tracker.add_negation(make_span(0, 2, 0, 3), NegationKind::Simple);

        assert_eq!(tracker.negation_count(), 1);

        tracker.reset();

        assert_eq!(tracker.negation_count(), 0);
    }

    #[test]
    fn test_detect_patterns_empty_input() {
        let tokens: [&str; 0] = [];
        let patterns = PolarityResolver::detect_double_negative_patterns(&tokens);
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_correlative_negation_needs_review() {
        let mut tracker = PolarityTracker::new();
        tracker.add_negation(make_span(0, 0, 0, 1), NegationKind::Correlative);
        tracker.add_negation(make_span(0, 3, 0, 4), NegationKind::Correlative);

        let ctx = tracker.polarity(make_span(0, 0, 0, 5));

        // Two correlative negations = positive, but needs review
        assert_eq!(ctx.polarity, Polarity::Positive);
        assert_eq!(ctx.negation_count, 2);
        assert!(ctx.needs_review);
        assert!(ctx.review_reason.unwrap().contains("Correlative negation"));
    }

    #[test]
    fn test_multiple_double_negative_patterns_combined() {
        let mut tracker = PolarityTracker::new();
        tracker.add_double_negative(make_span(0, 0, 0, 2), DoubleNegativePattern::UnlessNot);
        tracker.add_double_negative(make_span(0, 5, 0, 7), DoubleNegativePattern::NotWithout);

        let ctx = tracker.polarity(make_span(0, 0, 0, 10));

        assert_eq!(ctx.polarity, Polarity::Ambiguous);
        assert!(ctx.has_double_negative);
        assert!(ctx.needs_review);

        let reason = ctx.review_reason.unwrap();
        assert!(reason.contains("unless not"));
        assert!(reason.contains("not without"));
        assert!(reason.contains("; ")); // Joined with semicolon
    }
}

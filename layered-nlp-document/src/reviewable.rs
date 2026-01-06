//! ReviewableResult<T> infrastructure for human-in-the-loop review workflows.
//!
//! This module builds on [`Ambiguous<T>`] to add explicit review tracking,
//! enabling workflows where AI-produced results can be flagged for human verification.
//!
//! # Example
//!
//! ```
//! use layered_nlp_document::{ReviewableResult, Scored, compose_confidence};
//!
//! // Create a certain result (no review needed)
//! let certain = ReviewableResult::certain("Known term".to_string());
//! assert!(!certain.needs_review);
//! assert_eq!(certain.confidence(), 1.0);
//!
//! // Create from a low-confidence scored value (auto-flags for review)
//! let scored = Scored::rule_based("Maybe term".to_string(), 0.5, "heuristic");
//! let uncertain = ReviewableResult::from_scored(scored);
//! assert!(uncertain.needs_review);
//!
//! // Compose confidence scores
//! assert!((compose_confidence(&[0.8, 0.9]) - 0.72).abs() < 0.001);
//! ```

use crate::{Ambiguous, AmbiguityFlag, Scored};

/// Threshold below which `from_scored` automatically flags for review.
const REVIEW_THRESHOLD: f64 = 0.6;

/// Minimum confidence floor for composed scores.
const CONFIDENCE_FLOOR: f64 = 0.1;

/// A result that may need human review before being trusted.
///
/// Composes [`Ambiguous<T>`] with explicit review tracking, enabling
/// workflows where uncertain AI-produced results are surfaced for verification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(bound = "T: serde::Serialize + serde::de::DeserializeOwned")]
pub struct ReviewableResult<T> {
    /// The ambiguous result with best value and alternatives
    pub ambiguous: Ambiguous<T>,
    /// Whether this result should be reviewed by a human
    pub needs_review: bool,
    /// Optional explanation of why review is needed
    pub review_reason: Option<String>,
}

impl<T> ReviewableResult<T> {
    /// Create a certain result that needs no review.
    ///
    /// Uses confidence 1.0 and `ScoreSource::Derived` since the certainty
    /// is programmatically determined rather than from a specific source.
    ///
    /// # Example
    ///
    /// ```
    /// use layered_nlp_document::ReviewableResult;
    ///
    /// let result = ReviewableResult::certain("Verified value".to_string());
    /// assert!(!result.needs_review);
    /// assert_eq!(result.confidence(), 1.0);
    /// ```
    pub fn certain(value: T) -> Self {
        let scored = Scored::derived(value, 1.0);
        let ambiguous = Ambiguous {
            best: scored,
            alternatives: Vec::new(),
            flag: AmbiguityFlag::None,
        };
        Self {
            ambiguous,
            needs_review: false,
            review_reason: None,
        }
    }

    /// Create an uncertain result flagged for human review.
    ///
    /// The first value becomes the best candidate, with additional alternatives
    /// provided for the reviewer to consider.
    ///
    /// # Example
    ///
    /// ```
    /// use layered_nlp_document::{ReviewableResult, Scored};
    ///
    /// let alternatives = vec![
    ///     Scored::rule_based("Alternative 1".to_string(), 0.7, "rule_a"),
    ///     Scored::rule_based("Alternative 2".to_string(), 0.6, "rule_b"),
    /// ];
    /// let result = ReviewableResult::uncertain(
    ///     "Best guess".to_string(),
    ///     alternatives,
    ///     "Multiple competing interpretations",
    /// );
    /// assert!(result.needs_review);
    /// assert_eq!(result.review_reason.as_deref(), Some("Multiple competing interpretations"));
    /// ```
    pub fn uncertain(
        value: T,
        alternatives: Vec<Scored<T>>,
        reason: impl Into<String>,
    ) -> Self {
        // Use a moderate confidence for the best value since it's uncertain
        let best_confidence = if alternatives.is_empty() {
            0.5
        } else {
            // Slightly higher than best alternative, but still uncertain
            alternatives
                .iter()
                .map(|a| a.confidence)
                .fold(0.0_f64, |a, b| a.max(b))
                .max(0.5)
                .min(0.9)
        };

        let scored = Scored::derived(value, best_confidence);
        let flag = if !alternatives.is_empty() {
            AmbiguityFlag::CompetingAlternatives
        } else {
            AmbiguityFlag::LowConfidence
        };

        let ambiguous = Ambiguous {
            best: scored,
            alternatives,
            flag,
        };

        Self {
            ambiguous,
            needs_review: true,
            review_reason: Some(reason.into()),
        }
    }

    /// Create a ReviewableResult from a single scored value.
    ///
    /// Automatically determines `needs_review` based on confidence:
    /// - Confidence >= 0.6: no review needed
    /// - Confidence < 0.6: flagged for review
    ///
    /// # Example
    ///
    /// ```
    /// use layered_nlp_document::{ReviewableResult, Scored};
    ///
    /// // High confidence - no review
    /// let high = Scored::rule_based("value", 0.8, "rule");
    /// let result = ReviewableResult::from_scored(high);
    /// assert!(!result.needs_review);
    ///
    /// // Low confidence - needs review
    /// let low = Scored::rule_based("value", 0.5, "rule");
    /// let result = ReviewableResult::from_scored(low);
    /// assert!(result.needs_review);
    /// ```
    pub fn from_scored(scored: Scored<T>) -> Self {
        let needs_review = scored.confidence < REVIEW_THRESHOLD;
        let review_reason = if needs_review {
            Some(format!(
                "Low confidence ({:.2}) below threshold ({:.2})",
                scored.confidence, REVIEW_THRESHOLD
            ))
        } else {
            None
        };

        let flag = if needs_review {
            AmbiguityFlag::LowConfidence
        } else {
            AmbiguityFlag::None
        };

        let ambiguous = Ambiguous {
            best: scored,
            alternatives: Vec::new(),
            flag,
        };

        Self {
            ambiguous,
            needs_review,
            review_reason,
        }
    }

    /// Extract the best value, discarding confidence and review information.
    ///
    /// # Example
    ///
    /// ```
    /// use layered_nlp_document::ReviewableResult;
    ///
    /// let result = ReviewableResult::certain(42);
    /// assert_eq!(result.into_value(), 42);
    /// ```
    pub fn into_value(self) -> T {
        self.ambiguous.into_best()
    }

    /// Get the confidence of the best value.
    ///
    /// # Example
    ///
    /// ```
    /// use layered_nlp_document::{ReviewableResult, Scored};
    ///
    /// let scored = Scored::rule_based("value", 0.85, "rule");
    /// let result = ReviewableResult::from_scored(scored);
    /// assert!((result.confidence() - 0.85).abs() < 0.001);
    /// ```
    pub fn confidence(&self) -> f64 {
        self.ambiguous.best.confidence
    }

    /// Transform the inner value while preserving review status.
    ///
    /// # Example
    ///
    /// ```
    /// use layered_nlp_document::ReviewableResult;
    ///
    /// let result = ReviewableResult::certain(42);
    /// let mapped = result.map(|x| x.to_string());
    /// assert_eq!(mapped.into_value(), "42");
    /// ```
    pub fn map<U, F>(self, f: F) -> ReviewableResult<U>
    where
        F: FnOnce(T) -> U,
    {
        let mapped_best = self.ambiguous.best.map(f);

        // Note: We can't map alternatives without Clone on the function,
        // so we discard them. This is intentional - mapping transforms
        // the semantic meaning, so alternatives may no longer be valid.
        let ambiguous = Ambiguous {
            best: mapped_best,
            alternatives: Vec::new(),
            flag: self.ambiguous.flag,
        };

        ReviewableResult {
            ambiguous,
            needs_review: self.needs_review,
            review_reason: self.review_reason,
        }
    }
}

/// Compose multiple confidence scores by multiplication with a floor.
///
/// This models the intuition that sequential uncertain steps compound:
/// if step A has 80% confidence and step B has 90% confidence, the
/// combined confidence is 72% (0.8 * 0.9).
///
/// A floor of 0.1 prevents vanishingly small scores that would be
/// indistinguishable in practice.
///
/// # Examples
///
/// ```
/// use layered_nlp_document::compose_confidence;
///
/// // Single score passes through
/// assert!((compose_confidence(&[0.9]) - 0.9).abs() < 0.001);
///
/// // Two scores multiply
/// assert!((compose_confidence(&[0.8, 0.9]) - 0.72).abs() < 0.001);
///
/// // Three scores multiply
/// assert!((compose_confidence(&[0.8, 0.9, 0.7]) - 0.504).abs() < 0.001);
///
/// // Very low product hits the floor
/// assert!((compose_confidence(&[0.1, 0.05, 0.1]) - 0.1).abs() < 0.001);
///
/// // Empty slice returns floor
/// assert!((compose_confidence(&[]) - 0.1).abs() < 0.001);
/// ```
pub fn compose_confidence(scores: &[f64]) -> f64 {
    if scores.is_empty() {
        return CONFIDENCE_FLOOR;
    }

    let product = scores.iter().product::<f64>();
    product.max(CONFIDENCE_FLOOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_certain_creates_high_confidence() {
        let result = ReviewableResult::certain("test".to_string());

        assert_eq!(result.confidence(), 1.0);
        assert!(!result.needs_review);
        assert!(result.review_reason.is_none());
        assert_eq!(result.ambiguous.flag, AmbiguityFlag::None);
        assert!(result.ambiguous.alternatives.is_empty());
    }

    #[test]
    fn test_uncertain_flags_for_review() {
        let alts = vec![
            Scored::rule_based("alt1".to_string(), 0.7, "rule1"),
            Scored::rule_based("alt2".to_string(), 0.6, "rule2"),
        ];
        let result = ReviewableResult::uncertain(
            "best".to_string(),
            alts,
            "competing interpretations",
        );

        assert!(result.needs_review);
        assert_eq!(
            result.review_reason.as_deref(),
            Some("competing interpretations")
        );
        assert_eq!(result.ambiguous.flag, AmbiguityFlag::CompetingAlternatives);
        assert_eq!(result.ambiguous.alternatives.len(), 2);
    }

    #[test]
    fn test_from_scored_high_confidence_no_review() {
        let scored = Scored::rule_based("value".to_string(), 0.8, "rule");
        let result = ReviewableResult::from_scored(scored);

        assert!(!result.needs_review);
        assert!(result.review_reason.is_none());
        assert_eq!(result.confidence(), 0.8);
    }

    #[test]
    fn test_from_scored_low_confidence_needs_review() {
        let scored = Scored::rule_based("value".to_string(), 0.5, "rule");
        let result = ReviewableResult::from_scored(scored);

        assert!(result.needs_review);
        assert!(result.review_reason.is_some());
        assert_eq!(result.ambiguous.flag, AmbiguityFlag::LowConfidence);
    }

    #[test]
    fn test_from_scored_at_threshold_no_review() {
        let scored = Scored::rule_based("value".to_string(), 0.6, "rule");
        let result = ReviewableResult::from_scored(scored);

        assert!(!result.needs_review);
    }

    #[test]
    fn test_from_scored_just_below_threshold_needs_review() {
        let scored = Scored::rule_based("value".to_string(), 0.59, "rule");
        let result = ReviewableResult::from_scored(scored);

        assert!(result.needs_review);
    }

    #[test]
    fn test_into_value() {
        let result = ReviewableResult::certain(42);
        assert_eq!(result.into_value(), 42);
    }

    #[test]
    fn test_map_preserves_review_status() {
        let result = ReviewableResult::certain(42);
        let mapped = result.map(|x| x * 2);

        assert!(!mapped.needs_review);
        assert_eq!(mapped.into_value(), 84);
    }

    #[test]
    fn test_map_preserves_needs_review_true() {
        let scored = Scored::rule_based(10, 0.4, "rule");
        let result = ReviewableResult::from_scored(scored);
        let mapped = result.map(|x| x.to_string());

        assert!(mapped.needs_review);
        assert_eq!(mapped.into_value(), "10");
    }

    // compose_confidence tests

    #[test]
    fn test_compose_single_score() {
        let result = compose_confidence(&[0.9]);
        assert!((result - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_compose_two_scores() {
        let result = compose_confidence(&[0.8, 0.9]);
        assert!((result - 0.72).abs() < 0.001);
    }

    #[test]
    fn test_compose_three_scores() {
        let result = compose_confidence(&[0.8, 0.9, 0.7]);
        assert!((result - 0.504).abs() < 0.001);
    }

    #[test]
    fn test_compose_very_low_hits_floor() {
        // 0.1 * 0.05 * 0.1 = 0.0005, which is below the 0.1 floor
        let result = compose_confidence(&[0.1, 0.05, 0.1]);
        assert!((result - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_compose_empty_returns_floor() {
        let result = compose_confidence(&[]);
        assert!((result - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_compose_product_above_floor() {
        // 0.5 * 0.5 = 0.25, above 0.1 floor
        let result = compose_confidence(&[0.5, 0.5]);
        assert!((result - 0.25).abs() < 0.001);
    }
}

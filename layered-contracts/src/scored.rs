//! `Scored<T>` infrastructure for confidence-based attributes.
//!
//! The [`Scored<T>`] wrapper represents values with associated confidence scores.
//! - `confidence = 1.0` means the value has been verified (certain)
//! - `confidence < 1.0` means the value needs verification (uncertain)
//!
//! Scores can come from different sources tracked via [`ScoreSource`]:
//! - Rule-based formulas
//! - LLM passes
//! - Human verification
//! - Derived from other scores

use std::fmt;

/// A value with an associated confidence score.
///
/// # Confidence Semantics
/// - `1.0`: Verified/certain - no further verification needed
/// - `0.0 - 0.99`: Needs verification - the lower the score, the less certain
///
/// # Example
/// ```
/// use layered_contracts::{Scored, ScoreSource};
///
/// // Create a rule-based score
/// let term = Scored::rule_based("Contractor".to_string(), 0.85, "capitalized_noun");
/// assert!(!term.is_verified());
/// assert!(term.needs_verification());
///
/// // Create a verified score
/// let verified = Scored::verified("Company".to_string());
/// assert!(verified.is_verified());
/// ```
#[derive(Clone)]
pub struct Scored<T> {
    /// The actual value
    pub value: T,
    /// Confidence score from 0.0 to 1.0
    pub confidence: f64,
    /// Where this score came from
    pub source: ScoreSource,
}

/// The source of a confidence score.
#[derive(Clone, Debug, PartialEq)]
pub enum ScoreSource {
    /// Score produced by a rule-based formula
    RuleBased {
        /// Name of the rule that produced this score
        rule_name: String,
    },
    /// Score produced by an LLM pass
    LLMPass {
        /// Model identifier (e.g., "gpt-4", "claude-3")
        model: String,
        /// Unique identifier for this pass
        pass_id: String,
    },
    /// Score set by human verification
    HumanVerified {
        /// Identifier for the verifier
        verifier_id: String,
    },
    /// Score derived from combining other scores
    Derived,
}

impl<T> Scored<T> {
    /// Create a new scored value with explicit confidence and source.
    pub fn new(value: T, confidence: f64, source: ScoreSource) -> Self {
        Self {
            value,
            confidence: confidence.clamp(0.0, 1.0),
            source,
        }
    }

    /// Create a scored value from a rule-based formula.
    pub fn rule_based(value: T, confidence: f64, rule_name: &str) -> Self {
        Self::new(
            value,
            confidence,
            ScoreSource::RuleBased {
                rule_name: rule_name.to_string(),
            },
        )
    }

    /// Create a scored value from an LLM pass.
    pub fn llm_pass(value: T, confidence: f64, model: &str, pass_id: &str) -> Self {
        Self::new(
            value,
            confidence,
            ScoreSource::LLMPass {
                model: model.to_string(),
                pass_id: pass_id.to_string(),
            },
        )
    }

    /// Create a verified scored value (confidence = 1.0).
    pub fn verified(value: T) -> Self {
        Self::new(
            value,
            1.0,
            ScoreSource::HumanVerified {
                verifier_id: "external".to_string(),
            },
        )
    }

    /// Create a verified scored value with a specific verifier ID.
    pub fn verified_by(value: T, verifier_id: &str) -> Self {
        Self::new(
            value,
            1.0,
            ScoreSource::HumanVerified {
                verifier_id: verifier_id.to_string(),
            },
        )
    }

    /// Create a derived scored value (from combining other scores).
    pub fn derived(value: T, confidence: f64) -> Self {
        Self::new(value, confidence, ScoreSource::Derived)
    }

    /// Returns true if this value has been verified (confidence = 1.0).
    pub fn is_verified(&self) -> bool {
        (self.confidence - 1.0).abs() < f64::EPSILON
    }

    /// Returns true if this value needs verification (confidence < 1.0).
    pub fn needs_verification(&self) -> bool {
        !self.is_verified()
    }

    /// Map the inner value while preserving confidence and source.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Scored<U> {
        Scored {
            value: f(self.value),
            confidence: self.confidence,
            source: self.source,
        }
    }

    /// Get a reference to the inner value.
    pub fn as_ref(&self) -> Scored<&T> {
        Scored {
            value: &self.value,
            confidence: self.confidence,
            source: self.source.clone(),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for Scored<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Compact format for snapshot tests: Scored(value, conf: 0.85)
        write!(
            f,
            "Scored({:?}, conf: {:.2})",
            self.value, self.confidence
        )
    }
}

impl<T: PartialEq> PartialEq for Scored<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && (self.confidence - other.confidence).abs() < f64::EPSILON
            && self.source == other.source
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_based_score() {
        let scored = Scored::rule_based("test", 0.75, "my_rule");
        assert_eq!(scored.confidence, 0.75);
        assert!(scored.needs_verification());
        assert!(!scored.is_verified());
        assert!(matches!(
            scored.source,
            ScoreSource::RuleBased { rule_name } if rule_name == "my_rule"
        ));
    }

    #[test]
    fn test_verified_score() {
        let scored = Scored::verified("test");
        assert_eq!(scored.confidence, 1.0);
        assert!(scored.is_verified());
        assert!(!scored.needs_verification());
    }

    #[test]
    fn test_confidence_clamping() {
        let high = Scored::rule_based("test", 1.5, "rule");
        assert_eq!(high.confidence, 1.0);

        let low = Scored::rule_based("test", -0.5, "rule");
        assert_eq!(low.confidence, 0.0);
    }

    #[test]
    fn test_map() {
        let scored = Scored::rule_based(42, 0.8, "rule");
        let mapped = scored.map(|x| x.to_string());
        assert_eq!(mapped.value, "42");
        assert_eq!(mapped.confidence, 0.8);
    }

    #[test]
    fn test_debug_format() {
        let scored = Scored::rule_based("Contractor", 0.85, "capitalized");
        let debug = format!("{:?}", scored);
        assert_eq!(debug, r#"Scored("Contractor", conf: 0.85)"#);
    }
}

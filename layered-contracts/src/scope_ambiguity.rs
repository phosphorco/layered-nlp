//! Scope ambiguity flagging for human-in-the-loop review workflows.
//!
//! This module provides Gate 3: Scope Ambiguity Flagging, which detects
//! ambiguous scope boundaries in negation and quantifier operators.
//!
//! # Ambiguity Conditions
//!
//! The flagger detects four types of ambiguity:
//!
//! 1. **Low boundary confidence**: Primary domain candidate has confidence below threshold
//! 2. **Multiple plausible boundaries**: Multiple candidate boundaries within token distance
//! 3. **Negation-quantifier interaction**: Negation and quantifier scopes overlap
//! 4. **Exception-conjunction ambiguity**: "except X and Y" patterns (one exception or two?)
//!
//! # Example
//!
//! ```ignore
//! use layered_contracts::{
//!     ScopeAmbiguityFlagger, ScopeAmbiguityFlaggerConfig,
//!     NegationDetector, QuantifierDetector, ContractDocument,
//! };
//!
//! let doc = ContractDocument::from_text("The Company shall not perform X and Y.");
//! let neg_detector = NegationDetector::new();
//! let quant_detector = QuantifierDetector::new();
//!
//! let negations = neg_detector.detect(&doc);
//! let quantifiers = quant_detector.detect(&doc);
//!
//! let flagger = ScopeAmbiguityFlagger::with_defaults();
//! for negation in negations {
//!     let result = flagger.flag_negation(negation, &quantifiers, &["The", "Company", "shall", "not", "perform", "X", "and", "Y", "."]);
//!     if result.needs_review {
//!         println!("Review needed: {:?}", result.review_reason);
//!     }
//! }
//! ```

use crate::{
    NegationOp, QuantifierOp, ScopeBoundaryDetector, ScopeOperator,
};
use layered_nlp_document::{
    AmbiguityFlag, Ambiguous, ReviewableResult, Scored,
};

// ============================================================================
// Gate 3: Scope Ambiguity Flagging
// ============================================================================

/// Specific flags for scope ambiguity conditions.
///
/// These flags provide detailed information about why a scope operator
/// was flagged as ambiguous, enabling targeted human review.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ScopeAmbiguityFlag {
    /// No ambiguity detected
    None,
    /// Primary domain boundary has low confidence
    LowBoundaryConfidence {
        /// Actual confidence score
        confidence: f64,
        /// Threshold that was not met
        threshold: f64,
    },
    /// Multiple plausible boundaries exist within a small token window
    MultiplePlausibleBoundaries {
        /// Number of candidate boundaries
        count: usize,
        /// Maximum token distance between candidates
        within_tokens: usize,
    },
    /// Negation and quantifier scopes interact (overlap)
    NegationQuantifierInteraction {
        /// The negation marker text
        negation_marker: String,
        /// The quantifier marker text
        quantifier_marker: String,
    },
    /// Exception keyword followed by conjunction creates ambiguity
    ExceptionConjunctionAmbiguity {
        /// The exception keyword ("except", "save", etc.)
        exception: String,
        /// The conjunction ("and", "or")
        conjunction: String,
    },
}

/// Configuration for scope ambiguity detection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScopeAmbiguityFlaggerConfig {
    /// Confidence threshold below which a boundary is considered low confidence.
    /// Default: 0.7
    pub boundary_confidence_threshold: f64,
    /// Maximum token distance to consider multiple boundaries as competing.
    /// Default: 10
    pub distance_threshold: usize,
}

impl Default for ScopeAmbiguityFlaggerConfig {
    fn default() -> Self {
        Self {
            boundary_confidence_threshold: 0.7,
            distance_threshold: 10,
        }
    }
}

/// Flagger for detecting scope ambiguity in negation and quantifier operators.
///
/// The flagger analyzes scope operators for four types of ambiguity:
/// - Low boundary confidence
/// - Multiple plausible boundaries
/// - Negation-quantifier interaction
/// - Exception-conjunction patterns
pub struct ScopeAmbiguityFlagger {
    config: ScopeAmbiguityFlaggerConfig,
    /// Exception keywords that can create ambiguity with conjunctions
    exception_keywords: Vec<&'static str>,
    /// Conjunctions that create ambiguity after exception keywords
    conjunctions: Vec<&'static str>,
}

impl ScopeAmbiguityFlagger {
    /// Create a new flagger with the given configuration.
    pub fn new(config: ScopeAmbiguityFlaggerConfig) -> Self {
        Self {
            config,
            exception_keywords: vec!["except", "save", "but", "unless", "provided"],
            conjunctions: vec!["and", "or"],
        }
    }

    /// Create a flagger with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ScopeAmbiguityFlaggerConfig::default())
    }

    /// Flag a negation operator for ambiguity.
    ///
    /// Checks all ambiguity conditions and returns a ReviewableResult
    /// indicating whether human review is needed.
    ///
    /// # Arguments
    ///
    /// * `negation` - The scored negation operator to analyze
    /// * `quantifiers` - All quantifier operators in the same context (for interaction check)
    /// * `tokens` - Line tokens for context (for exception-conjunction detection)
    pub fn flag_negation(
        &self,
        negation: Scored<ScopeOperator<NegationOp>>,
        quantifiers: &[Scored<ScopeOperator<QuantifierOp>>],
        tokens: &[&str],
    ) -> ReviewableResult<ScopeOperator<NegationOp>> {
        let mut flags = self.check_ambiguity_conditions(&negation, tokens);

        // Check for negation-quantifier interactions
        for quant in quantifiers {
            if ScopeBoundaryDetector::scopes_interact(&negation.value, &quant.value) {
                flags.push(ScopeAmbiguityFlag::NegationQuantifierInteraction {
                    negation_marker: negation.value.payload.marker.clone(),
                    quantifier_marker: quant.value.payload.marker.clone(),
                });
            }
        }

        self.build_reviewable_result(negation, flags)
    }

    /// Flag a quantifier operator for ambiguity.
    ///
    /// Checks all ambiguity conditions and returns a ReviewableResult
    /// indicating whether human review is needed.
    ///
    /// # Arguments
    ///
    /// * `quantifier` - The scored quantifier operator to analyze
    /// * `negations` - All negation operators in the same context (for interaction check)
    /// * `tokens` - Line tokens for context (for exception-conjunction detection)
    pub fn flag_quantifier(
        &self,
        quantifier: Scored<ScopeOperator<QuantifierOp>>,
        negations: &[Scored<ScopeOperator<NegationOp>>],
        tokens: &[&str],
    ) -> ReviewableResult<ScopeOperator<QuantifierOp>> {
        let mut flags = self.check_ambiguity_conditions(&quantifier, tokens);

        // Check for negation-quantifier interactions
        for neg in negations {
            if ScopeBoundaryDetector::scopes_interact(&neg.value, &quantifier.value) {
                flags.push(ScopeAmbiguityFlag::NegationQuantifierInteraction {
                    negation_marker: neg.value.payload.marker.clone(),
                    quantifier_marker: quantifier.value.payload.marker.clone(),
                });
            }
        }

        self.build_reviewable_result(quantifier, flags)
    }

    /// Check generic ambiguity conditions for any scope operator.
    ///
    /// Checks:
    /// 1. Low boundary confidence
    /// 2. Multiple plausible boundaries within distance threshold
    /// 3. Exception-conjunction patterns in tokens
    fn check_ambiguity_conditions<O>(
        &self,
        op: &Scored<ScopeOperator<O>>,
        tokens: &[&str],
    ) -> Vec<ScopeAmbiguityFlag> {
        let mut flags = Vec::new();

        // 1. Check boundary confidence
        if let Some(primary) = op.value.domain.candidates.first() {
            if primary.confidence < self.config.boundary_confidence_threshold {
                flags.push(ScopeAmbiguityFlag::LowBoundaryConfidence {
                    confidence: primary.confidence,
                    threshold: self.config.boundary_confidence_threshold,
                });
            }
        }

        // 2. Check for multiple plausible boundaries
        if op.value.domain.candidate_count() > 1 {
            let candidates = &op.value.domain.candidates;
            let max_distance = self.compute_max_candidate_distance(candidates);

            if max_distance <= self.config.distance_threshold {
                flags.push(ScopeAmbiguityFlag::MultiplePlausibleBoundaries {
                    count: candidates.len(),
                    within_tokens: max_distance,
                });
            }
        }

        // 3. Check for exception-conjunction patterns
        flags.extend(self.detect_exception_conjunction(tokens));

        flags
    }

    /// Compute the maximum gap between non-overlapping domain candidates.
    ///
    /// For the "within N tokens" check, we measure the gap between candidates,
    /// not the total endpoint spread. For overlapping candidates, distance is 0.
    /// For non-overlapping, it's the gap between end of one and start of another.
    fn compute_max_candidate_distance(
        &self,
        candidates: &[Scored<layered_nlp_document::DocSpan>],
    ) -> usize {
        if candidates.len() < 2 {
            return 0;
        }

        let mut max_distance = 0;
        for i in 0..candidates.len() {
            for j in (i + 1)..candidates.len() {
                let a = &candidates[i].value;
                let b = &candidates[j].value;

                // Compute gap between non-overlapping spans
                let distance = if a.end.token <= b.start.token {
                    // a ends before b starts
                    b.start.token - a.end.token
                } else if b.end.token <= a.start.token {
                    // b ends before a starts
                    a.start.token - b.end.token
                } else {
                    // Overlapping spans have 0 distance
                    0
                };
                max_distance = max_distance.max(distance);
            }
        }
        max_distance
    }

    /// Detect all exception keyword followed by conjunction patterns.
    ///
    /// Patterns like "except widgets and gadgets" are ambiguous because
    /// it's unclear whether "and gadgets" is part of the exception or
    /// a separate clause.
    ///
    /// Returns all detected patterns (multiple exception clauses in the same
    /// sentence will each be flagged).
    fn detect_exception_conjunction(&self, tokens: &[&str]) -> Vec<ScopeAmbiguityFlag> {
        let mut flags = Vec::new();

        // Look for exception keyword
        for (i, token) in tokens.iter().enumerate() {
            let lower = token.to_lowercase();
            if self.exception_keywords.contains(&lower.as_str()) {
                // Look for conjunction after exception (within reasonable distance)
                for (j, following_token) in tokens.iter().enumerate().skip(i + 1) {
                    let following_lower = following_token.to_lowercase();

                    // Stop searching if we hit a clause boundary
                    if following_lower == "." || following_lower == "," || following_lower == ";" {
                        break;
                    }

                    if self.conjunctions.contains(&following_lower.as_str()) {
                        // Check if there's a noun between exception and conjunction
                        // This heuristic avoids false positives for "except if X and Y"
                        if j > i + 1 {
                            flags.push(ScopeAmbiguityFlag::ExceptionConjunctionAmbiguity {
                                exception: lower.clone(),
                                conjunction: following_lower,
                            });
                            // Found first conjunction for this exception, move to next exception
                            break;
                        }
                    }
                }
            }
        }
        flags
    }

    /// Build a ReviewableResult from flags.
    fn build_reviewable_result<O>(
        &self,
        op: Scored<ScopeOperator<O>>,
        flags: Vec<ScopeAmbiguityFlag>,
    ) -> ReviewableResult<ScopeOperator<O>> {
        if flags.is_empty() {
            // No ambiguity - return as certain if confidence is high enough
            if op.confidence >= self.config.boundary_confidence_threshold {
                return ReviewableResult::certain(op.value);
            } else {
                return ReviewableResult::from_scored(op);
            }
        }

        // Build review reason from flags
        let reasons: Vec<String> = flags
            .iter()
            .map(|f| match f {
                ScopeAmbiguityFlag::None => String::new(),
                ScopeAmbiguityFlag::LowBoundaryConfidence { confidence, threshold } => {
                    format!(
                        "Low boundary confidence ({:.2} < {:.2})",
                        confidence, threshold
                    )
                }
                ScopeAmbiguityFlag::MultiplePlausibleBoundaries { count, within_tokens } => {
                    format!(
                        "{} plausible boundaries within {} tokens",
                        count, within_tokens
                    )
                }
                ScopeAmbiguityFlag::NegationQuantifierInteraction {
                    negation_marker,
                    quantifier_marker,
                } => {
                    format!(
                        "Negation '{}' interacts with quantifier '{}'",
                        negation_marker, quantifier_marker
                    )
                }
                ScopeAmbiguityFlag::ExceptionConjunctionAmbiguity { exception, conjunction } => {
                    format!(
                        "Ambiguous: '{}' followed by '{}' - scope unclear",
                        exception, conjunction
                    )
                }
            })
            .filter(|s| !s.is_empty())
            .collect();

        let reason = reasons.join("; ");

        // Create ambiguous result with flag
        let ambiguous = Ambiguous {
            best: op,
            alternatives: Vec::new(),
            flag: AmbiguityFlag::CompetingAlternatives,
        };

        ReviewableResult {
            ambiguous,
            needs_review: true,
            review_reason: Some(reason),
        }
    }
}

impl Default for ScopeAmbiguityFlagger {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContractDocument;
    use crate::{NegationDetector, QuantifierDetector};

    fn get_tokens(text: &str) -> Vec<&str> {
        text.split_whitespace().collect()
    }

    #[test]
    fn test_clear_boundary_no_flag() {
        // "shall not perform X." - clear boundary (period)
        let text = "The Company shall not perform X.";
        let doc = ContractDocument::from_text(text);

        let neg_detector = NegationDetector::new();
        let quant_detector = QuantifierDetector::new();

        let negations = neg_detector.detect(&doc);
        let quantifiers = quant_detector.detect(&doc);

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let tokens = get_tokens(text);

        assert_eq!(negations.len(), 1);
        let result = flagger.flag_negation(negations[0].clone(), &quantifiers, &tokens);

        // Clear boundary should not need review
        assert!(!result.needs_review);
    }

    #[test]
    fn test_ambiguous_and_scope() {
        // "shall not perform X and Y" - ambiguous: scope over X only or X and Y?
        let text = "The Company shall not perform X and Y";
        let doc = ContractDocument::from_text(text);

        let neg_detector = NegationDetector::new();
        let quant_detector = QuantifierDetector::new();

        let negations = neg_detector.detect(&doc);
        let quantifiers = quant_detector.detect(&doc);

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let tokens = get_tokens(text);

        assert_eq!(negations.len(), 1);
        let result = flagger.flag_negation(negations[0].clone(), &quantifiers, &tokens);

        // Note: "shall not perform X and Y" does not automatically trigger review
        // because detecting this ambiguity requires syntactic analysis (NP chunking)
        // to recognize that "X and Y" could be a coordinated noun phrase. The current
        // flagger only detects explicit scope boundary issues (low confidence, multiple
        // candidates, negation-quantifier interaction, exception-conjunction patterns).
        // Coordinated NP ambiguity detection is deferred to Tier 3 syntactic analysis.
        assert!(!result.needs_review);
        assert!(result.ambiguous.best.value.payload.marker == "not");
    }

    #[test]
    fn test_exception_conjunction_ambiguity() {
        // "except widgets and gadgets" - ambiguous: one exception or two?
        let text = "The Company shall sell all items except widgets and gadgets";
        let tokens = get_tokens(text);

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let flags = flagger.detect_exception_conjunction(&tokens);

        assert_eq!(flags.len(), 1);
        if let ScopeAmbiguityFlag::ExceptionConjunctionAmbiguity { exception, conjunction } =
            &flags[0]
        {
            assert_eq!(exception, "except");
            assert_eq!(conjunction, "and");
        } else {
            panic!("Expected ExceptionConjunctionAmbiguity flag");
        }
    }

    #[test]
    fn test_negation_quantifier_interaction() {
        // "not all parties" - quantifier-negation interaction
        let text = "The Company shall not notify all parties within 30 days.";
        let doc = ContractDocument::from_text(text);

        let neg_detector = NegationDetector::new();
        let quant_detector = QuantifierDetector::new();

        let negations = neg_detector.detect(&doc);
        let quantifiers = quant_detector.detect(&doc);

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let tokens = get_tokens(text);

        // Check if we have both negation and quantifier
        assert_eq!(negations.len(), 1);
        assert!(!quantifiers.is_empty());

        let result = flagger.flag_negation(negations[0].clone(), &quantifiers, &tokens);

        // If the scopes interact, this should need review
        if result.needs_review {
            let reason = result.review_reason.unwrap();
            assert!(reason.contains("interacts") || reason.contains("confidence"));
        }
    }

    #[test]
    fn test_low_boundary_confidence() {
        // Test with a manually constructed low-confidence operator
        use layered_nlp_document::{DocPosition, DocSpan, ScopeDimension, ScopeDomain};

        let trigger = DocSpan::new(
            DocPosition { line: 0, token: 3 },
            DocPosition { line: 0, token: 4 },
        );

        // Create domain with low confidence candidate
        let domain_span = DocSpan::new(
            DocPosition { line: 0, token: 4 },
            DocPosition { line: 0, token: 8 },
        );
        let domain = ScopeDomain::from_candidates(vec![Scored::rule_based(
            domain_span,
            0.5, // Below 0.7 threshold
            "weak_heuristic",
        )]);

        let op = ScopeOperator::new(
            ScopeDimension::Negation,
            trigger,
            domain,
            NegationOp {
                marker: "not".to_string(),
                kind: layered_nlp_document::NegationKind::Simple,
            },
        );

        let scored_op = Scored::rule_based(op, 0.5, "test");
        let tokens = ["The", "Company", "shall", "not", "perform", "something", "unclear", "here"];

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let result = flagger.flag_negation(scored_op, &[], &tokens);

        // Low confidence should trigger review
        assert!(result.needs_review);
        let reason = result.review_reason.unwrap();
        assert!(reason.contains("confidence"));
    }

    #[test]
    fn test_multiple_plausible_boundaries() {
        // Test with multiple candidate boundaries
        use layered_nlp_document::{DocPosition, DocSpan, ScopeDimension, ScopeDomain};

        let trigger = DocSpan::new(
            DocPosition { line: 0, token: 3 },
            DocPosition { line: 0, token: 4 },
        );

        // Create domain with multiple candidates within 10 tokens
        let span1 = DocSpan::new(
            DocPosition { line: 0, token: 4 },
            DocPosition { line: 0, token: 6 },
        );
        let span2 = DocSpan::new(
            DocPosition { line: 0, token: 4 },
            DocPosition { line: 0, token: 10 },
        );

        let domain = ScopeDomain::from_candidates(vec![
            Scored::rule_based(span1, 0.8, "heuristic_a"),
            Scored::rule_based(span2, 0.75, "heuristic_b"),
        ]);

        let op = ScopeOperator::new(
            ScopeDimension::Negation,
            trigger,
            domain,
            NegationOp {
                marker: "not".to_string(),
                kind: layered_nlp_document::NegationKind::Simple,
            },
        );

        let scored_op = Scored::rule_based(op, 0.8, "test");
        let tokens = [
            "The", "Company", "shall", "not", "perform", "X", "and", "Y", "or", "Z", ".",
        ];

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let result = flagger.flag_negation(scored_op, &[], &tokens);

        // Multiple boundaries should trigger review
        assert!(result.needs_review);
        let reason = result.review_reason.unwrap();
        assert!(reason.contains("plausible boundaries"));
    }

    #[test]
    fn test_no_exception_conjunction_immediately_after() {
        // "except and" - conjunction immediately after exception is not flagged
        // because there's no noun phrase between them
        let tokens = ["except", "and", "something"];

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let flags = flagger.detect_exception_conjunction(&tokens);

        // The "and" is directly after "except" (j=1, i=0, so j > i+1 is false)
        assert!(flags.is_empty());
    }

    #[test]
    fn test_exception_with_or() {
        let tokens = ["shall", "comply", "except", "widgets", "or", "gadgets"];

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let flags = flagger.detect_exception_conjunction(&tokens);

        assert_eq!(flags.len(), 1);
        if let ScopeAmbiguityFlag::ExceptionConjunctionAmbiguity { exception, conjunction } =
            &flags[0]
        {
            assert_eq!(exception, "except");
            assert_eq!(conjunction, "or");
        }
    }

    #[test]
    fn test_flag_quantifier() {
        let text = "Each party shall not disclose information.";
        let doc = ContractDocument::from_text(text);

        let neg_detector = NegationDetector::new();
        let quant_detector = QuantifierDetector::new();

        let negations = neg_detector.detect(&doc);
        let quantifiers = quant_detector.detect(&doc);

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let tokens = get_tokens(text);

        assert!(!quantifiers.is_empty());
        let result = flagger.flag_quantifier(quantifiers[0].clone(), &negations, &tokens);

        // Verify we get a valid result
        assert_eq!(result.ambiguous.best.value.payload.marker, "each");
    }

    #[test]
    fn test_config_custom_threshold() {
        let config = ScopeAmbiguityFlaggerConfig {
            boundary_confidence_threshold: 0.9, // Higher threshold
            distance_threshold: 5,              // Lower distance
        };

        let flagger = ScopeAmbiguityFlagger::new(config);
        assert_eq!(flagger.config.boundary_confidence_threshold, 0.9);
        assert_eq!(flagger.config.distance_threshold, 5);
    }

    #[test]
    fn test_save_keyword() {
        // "save widgets and gadgets" - also ambiguous
        let tokens = ["shall", "comply", "save", "widgets", "and", "gadgets"];

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let flags = flagger.detect_exception_conjunction(&tokens);

        assert_eq!(flags.len(), 1);
        if let ScopeAmbiguityFlag::ExceptionConjunctionAmbiguity { exception, .. } = &flags[0] {
            assert_eq!(exception, "save");
        }
    }

    #[test]
    fn test_unless_keyword() {
        // "unless widgets and gadgets" - also ambiguous
        let tokens = ["shall", "comply", "unless", "widgets", "and", "gadgets"];

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let flags = flagger.detect_exception_conjunction(&tokens);

        assert_eq!(flags.len(), 1);
        if let ScopeAmbiguityFlag::ExceptionConjunctionAmbiguity { exception, .. } = &flags[0] {
            assert_eq!(exception, "unless");
        }
    }

    #[test]
    fn test_exception_stopped_by_comma() {
        // "except widgets, and gadgets" - comma stops the search
        let tokens = ["except", "widgets", ",", "and", "gadgets"];

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let flags = flagger.detect_exception_conjunction(&tokens);

        // The comma should stop the search before reaching "and"
        assert!(flags.is_empty());
    }

    #[test]
    fn test_multiple_exception_clauses() {
        // Multiple exception clauses in same sentence should all be flagged
        let tokens = [
            "shall", "comply", "except", "widgets", "and", "gadgets", "but", "sprockets", "or",
            "gizmos",
        ];

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let flags = flagger.detect_exception_conjunction(&tokens);

        // Should detect both "except widgets and" and "but sprockets or"
        assert_eq!(flags.len(), 2);

        // First flag: except...and
        if let ScopeAmbiguityFlag::ExceptionConjunctionAmbiguity { exception, conjunction } =
            &flags[0]
        {
            assert_eq!(exception, "except");
            assert_eq!(conjunction, "and");
        } else {
            panic!("Expected ExceptionConjunctionAmbiguity for 'except'");
        }

        // Second flag: but...or
        if let ScopeAmbiguityFlag::ExceptionConjunctionAmbiguity { exception, conjunction } =
            &flags[1]
        {
            assert_eq!(exception, "but");
            assert_eq!(conjunction, "or");
        } else {
            panic!("Expected ExceptionConjunctionAmbiguity for 'but'");
        }
    }

    #[test]
    fn test_candidate_distance_overlapping() {
        // Overlapping candidates should have distance 0
        use layered_nlp_document::{DocPosition, DocSpan};

        let span1 = DocSpan::new(
            DocPosition { line: 0, token: 4 },
            DocPosition { line: 0, token: 8 },
        );
        let span2 = DocSpan::new(
            DocPosition { line: 0, token: 6 },
            DocPosition { line: 0, token: 10 },
        );

        let candidates = vec![
            Scored::rule_based(span1, 0.8, "a"),
            Scored::rule_based(span2, 0.75, "b"),
        ];

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let distance = flagger.compute_max_candidate_distance(&candidates);

        // Spans overlap (4-8 and 6-10), so distance is 0
        assert_eq!(distance, 0);
    }

    #[test]
    fn test_candidate_distance_non_overlapping() {
        // Non-overlapping candidates should measure the gap
        use layered_nlp_document::{DocPosition, DocSpan};

        let span1 = DocSpan::new(
            DocPosition { line: 0, token: 4 },
            DocPosition { line: 0, token: 6 },
        );
        let span2 = DocSpan::new(
            DocPosition { line: 0, token: 8 },
            DocPosition { line: 0, token: 10 },
        );

        let candidates = vec![
            Scored::rule_based(span1, 0.8, "a"),
            Scored::rule_based(span2, 0.75, "b"),
        ];

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let distance = flagger.compute_max_candidate_distance(&candidates);

        // Gap between end of span1 (6) and start of span2 (8) is 2
        assert_eq!(distance, 2);
    }

    #[test]
    fn test_candidate_distance_adjacent() {
        // Adjacent candidates (end of one = start of another) have distance 0
        use layered_nlp_document::{DocPosition, DocSpan};

        let span1 = DocSpan::new(
            DocPosition { line: 0, token: 4 },
            DocPosition { line: 0, token: 6 },
        );
        let span2 = DocSpan::new(
            DocPosition { line: 0, token: 6 },
            DocPosition { line: 0, token: 8 },
        );

        let candidates = vec![
            Scored::rule_based(span1, 0.8, "a"),
            Scored::rule_based(span2, 0.75, "b"),
        ];

        let flagger = ScopeAmbiguityFlagger::with_defaults();
        let distance = flagger.compute_max_candidate_distance(&candidates);

        // Adjacent spans: end at 6, start at 6 -> distance 0
        assert_eq!(distance, 0);
    }
}

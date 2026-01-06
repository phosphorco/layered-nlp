//! Modal Scope Analyzer for contract obligations.
//!
//! Gate 4: Combines modal classification, polarity context, and scope information
//! to produce fully analyzed obligations with confidence scoring and review flagging.
//!
//! # Architecture
//!
//! The analyzer composes outputs from earlier gates:
//! - Modal classification (Duty/Permission/Prohibition/Discretion)
//! - Polarity context (positive/negative/ambiguous)
//! - Scope boundaries (optional ScopeOperator)
//! - Scope ambiguity flags
//!
//! # Review Flagging
//!
//! Results are flagged for human review when:
//! - Overall confidence falls below threshold (default 0.6)
//! - Modal classification is ambiguous
//! - Polarity context needs review (double negatives, etc.)
//! - Scope ambiguity flag is present
//!
//! # Example
//!
//! ```ignore
//! use layered_contracts::{
//!     ModalScopeAnalyzer, ObligationPhrase, ModalNegationClassification,
//!     PolarityContext, Polarity, ModalObligationType,
//! };
//! use layered_nlp_document::{Scored, ScopeOperator, NegationOp};
//!
//! let analyzer = ModalScopeAnalyzer::new();
//!
//! let result = analyzer.analyze(
//!     obligation,
//!     &modal_classification,
//!     &polarity_ctx,
//!     scope,
//!     scope_flag,
//! );
//!
//! if result.needs_review {
//!     println!("Review needed: {:?}", result.review_reason);
//! }
//! ```

use crate::{
    ModalNegationClassification, ModalObligationType, ObligationPhrase, Polarity, PolarityContext,
    ScopeAmbiguityFlag,
};
use layered_nlp_document::{
    compose_confidence, NegationOp, ReviewableResult, ScopeOperator, Scored,
};
use serde::{Deserialize, Serialize};

// ============================================================================
// ScopedObligation - The main output type
// ============================================================================

/// A fully analyzed obligation with modal type, polarity, and scope information.
///
/// This is the output of Gate 4, combining information from:
/// - ObligationPhraseResolver (obligor, action, conditions)
/// - ModalNegationClassifier (modal type, discretion patterns)
/// - PolarityTracker (negation count, polarity)
/// - ScopeAmbiguityFlagger (scope boundaries, ambiguity)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopedObligation {
    /// The obligation phrase (from ObligationPhraseResolver)
    pub obligation: Scored<ObligationPhrase>,

    /// Modal obligation type (Duty/Permission/Prohibition/Discretion)
    pub modal_type: ModalObligationType,

    /// Negation polarity in the scope
    pub polarity: Polarity,

    /// The scope operator that governs this obligation (if present)
    pub scope: Option<ScopeOperator<NegationOp>>,

    /// Confidence in the scope boundary detection
    pub scope_confidence: f64,

    /// Ambiguity flag from scope analysis
    pub scope_flag: Option<ScopeAmbiguityFlag>,

    /// Overall composed confidence from all sources
    pub overall_confidence: f64,
}

impl ScopedObligation {
    /// Check if this obligation creates a binding requirement.
    pub fn is_binding(&self) -> bool {
        self.modal_type.is_binding()
    }

    /// Check if this obligation grants freedom (permission or discretion).
    pub fn grants_freedom(&self) -> bool {
        self.modal_type.grants_freedom()
    }

    /// Check if this obligation has an associated scope.
    pub fn has_scope(&self) -> bool {
        self.scope.is_some()
    }

    /// Check if there are any scope ambiguity concerns.
    pub fn has_scope_ambiguity(&self) -> bool {
        matches!(&self.scope_flag, Some(flag) if !matches!(flag, ScopeAmbiguityFlag::None))
    }
}

// ============================================================================
// ModalScopeAnalyzer - The analyzer
// ============================================================================

/// Analyzes obligations in context of modal classification and scope.
///
/// The analyzer is stateless except for configuration. It composes confidence
/// scores from multiple sources and determines whether human review is needed.
#[derive(Debug, Clone)]
pub struct ModalScopeAnalyzer {
    /// Threshold below which results are flagged for review (default 0.6)
    review_threshold: f64,
}

impl ModalScopeAnalyzer {
    /// Create a new analyzer with default threshold (0.6).
    pub fn new() -> Self {
        Self {
            review_threshold: 0.6,
        }
    }

    /// Create an analyzer with a custom review threshold.
    pub fn with_threshold(threshold: f64) -> Self {
        Self {
            review_threshold: threshold.clamp(0.0, 1.0),
        }
    }

    /// Analyze an obligation in context of modal classification and scope.
    ///
    /// Composes confidence from:
    /// - obligation.confidence
    /// - modal_classification.confidence
    /// - polarity_ctx.confidence
    /// - scope_confidence (or 1.0 if no scope present)
    ///
    /// Flags for review when:
    /// - overall_confidence < review_threshold
    /// - modal_classification.is_ambiguous is true
    /// - polarity_ctx.needs_review is true
    /// - scope_flag is not None
    pub fn analyze(
        &self,
        obligation: Scored<ObligationPhrase>,
        modal_classification: &ModalNegationClassification,
        polarity_ctx: &PolarityContext,
        scope: Option<ScopeOperator<NegationOp>>,
        scope_flag: Option<ScopeAmbiguityFlag>,
    ) -> ReviewableResult<ScopedObligation> {
        // Calculate scope confidence from the scope's domain, or 1.0 if no scope
        let scope_confidence = scope
            .as_ref()
            .and_then(|s| s.domain.candidates.first())
            .map(|c| c.confidence)
            .unwrap_or(1.0);

        // Compose confidence from all sources
        let overall_confidence = compose_confidence(&[
            obligation.confidence,
            modal_classification.confidence,
            polarity_ctx.confidence,
            scope_confidence,
        ]);

        // Build the scoped obligation
        let scoped = ScopedObligation {
            obligation,
            modal_type: modal_classification.obligation_type,
            polarity: polarity_ctx.polarity,
            scope,
            scope_confidence,
            scope_flag: scope_flag.clone(),
            overall_confidence,
        };

        // Determine if review is needed
        let (needs_review, review_reason) =
            self.determine_review_status(&scoped, modal_classification, polarity_ctx, &scope_flag);

        if needs_review {
            let reason = review_reason.unwrap_or_else(|| "Unknown review reason".to_string());
            ReviewableResult::uncertain(scoped, vec![], reason)
        } else {
            ReviewableResult::certain(scoped)
        }
    }

    /// Determine if review is needed and why.
    fn determine_review_status(
        &self,
        scoped: &ScopedObligation,
        modal_classification: &ModalNegationClassification,
        polarity_ctx: &PolarityContext,
        scope_flag: &Option<ScopeAmbiguityFlag>,
    ) -> (bool, Option<String>) {
        let mut reasons = Vec::new();

        // Check overall confidence
        if scoped.overall_confidence < self.review_threshold {
            reasons.push(format!(
                "Low overall confidence ({:.2} < {:.2})",
                scoped.overall_confidence, self.review_threshold
            ));
        }

        // Check modal ambiguity
        if modal_classification.is_ambiguous {
            let modal_reason = modal_classification
                .ambiguity_reason
                .clone()
                .unwrap_or_else(|| "Modal classification is ambiguous".to_string());
            reasons.push(modal_reason);
        }

        // Check polarity review flag
        if polarity_ctx.needs_review {
            let polarity_reason = polarity_ctx
                .review_reason
                .clone()
                .unwrap_or_else(|| "Polarity context needs review".to_string());
            reasons.push(polarity_reason);
        }

        // Check scope ambiguity flag
        if let Some(flag) = scope_flag {
            if !matches!(flag, ScopeAmbiguityFlag::None) {
                let scope_reason = self.scope_flag_to_reason(flag);
                reasons.push(scope_reason);
            }
        }

        if reasons.is_empty() {
            (false, None)
        } else {
            (true, Some(reasons.join("; ")))
        }
    }

    /// Convert a scope ambiguity flag to a human-readable reason.
    fn scope_flag_to_reason(&self, flag: &ScopeAmbiguityFlag) -> String {
        match flag {
            ScopeAmbiguityFlag::None => String::new(),
            ScopeAmbiguityFlag::LowBoundaryConfidence {
                confidence,
                threshold,
            } => {
                format!(
                    "Scope boundary confidence low ({:.2} < {:.2})",
                    confidence, threshold
                )
            }
            ScopeAmbiguityFlag::MultiplePlausibleBoundaries {
                count,
                within_tokens,
            } => {
                format!(
                    "{} plausible scope boundaries within {} tokens",
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
            ScopeAmbiguityFlag::ExceptionConjunctionAmbiguity {
                exception,
                conjunction,
            } => {
                format!(
                    "Ambiguous: '{}' followed by '{}' - scope unclear",
                    exception, conjunction
                )
            }
        }
    }
}

impl Default for ModalScopeAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConditionRef, DiscretionPattern, ObligorReference, ObligationType};
    use layered_nlp_document::{DocPosition, DocSpan, NegationKind, ScopeDimension, ScopeDomain};

    // ========================================================================
    // Test helpers
    // ========================================================================

    fn make_span(start_line: usize, start_token: usize, end_line: usize, end_token: usize) -> DocSpan {
        DocSpan {
            start: DocPosition {
                line: start_line,
                token: start_token,
            },
            end: DocPosition {
                line: end_line,
                token: end_token,
            },
        }
    }

    fn make_obligation(action: &str) -> Scored<ObligationPhrase> {
        let phrase = ObligationPhrase {
            obligor: ObligorReference::TermRef {
                term_name: "Tenant".to_string(),
                confidence: 0.9,
            },
            obligation_type: ObligationType::Duty,
            action: action.to_string(),
            conditions: Vec::new(),
        };
        Scored::rule_based(phrase, 0.85, "obligation_phrase")
    }

    fn make_modal_clear(
        obligation_type: ModalObligationType,
        polarity: Polarity,
    ) -> ModalNegationClassification {
        ModalNegationClassification {
            span: make_span(0, 0, 0, 5),
            obligation_type,
            modal: "shall".to_string(),
            polarity,
            is_ambiguous: false,
            ambiguity_reason: None,
            discretion_pattern: None,
            confidence: 0.95,
        }
    }

    fn make_modal_ambiguous(
        obligation_type: ModalObligationType,
        polarity: Polarity,
        reason: &str,
    ) -> ModalNegationClassification {
        ModalNegationClassification {
            span: make_span(0, 0, 0, 5),
            obligation_type,
            modal: "shall".to_string(),
            polarity,
            is_ambiguous: true,
            ambiguity_reason: Some(reason.to_string()),
            discretion_pattern: None,
            confidence: 0.6,
        }
    }

    fn make_modal_discretion(pattern: DiscretionPattern) -> ModalNegationClassification {
        ModalNegationClassification {
            span: make_span(0, 0, 0, 10),
            obligation_type: ModalObligationType::Discretion,
            modal: "shall".to_string(),
            polarity: Polarity::Negative,
            is_ambiguous: false, // Known discretion patterns are not ambiguous
            ambiguity_reason: None,
            discretion_pattern: Some(pattern),
            confidence: 0.85,
        }
    }

    fn make_polarity_positive(span: DocSpan) -> PolarityContext {
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

    fn make_polarity_negative(span: DocSpan) -> PolarityContext {
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

    fn make_polarity_double_negative(span: DocSpan) -> PolarityContext {
        PolarityContext {
            span,
            polarity: Polarity::Positive, // Double negative resolves to positive
            negation_count: 2,
            negation_spans: vec![span, span],
            has_double_negative: true,
            confidence: 0.6,
            needs_review: true,
            review_reason: Some("Double negative detected".to_string()),
        }
    }

    fn make_scope(marker: &str) -> ScopeOperator<NegationOp> {
        let trigger = make_span(0, 3, 0, 4);
        let domain_span = make_span(0, 4, 0, 8);
        let domain = ScopeDomain::from_candidates(vec![Scored::rule_based(
            domain_span,
            0.9,
            "boundary_heuristic",
        )]);

        ScopeOperator::new(
            ScopeDimension::Negation,
            trigger,
            domain,
            NegationOp {
                marker: marker.to_string(),
                kind: NegationKind::Simple,
            },
        )
    }

    // ========================================================================
    // Test cases from the plan
    // ========================================================================

    #[test]
    fn test_tenant_shall_pay_rent() {
        // "Tenant shall pay rent" - Duty, Positive, not flagged
        let analyzer = ModalScopeAnalyzer::new();
        let span = make_span(0, 0, 0, 5);

        let obligation = make_obligation("pay rent");
        let modal = make_modal_clear(ModalObligationType::Duty, Polarity::Positive);
        let polarity = make_polarity_positive(span);

        let result = analyzer.analyze(obligation, &modal, &polarity, None, None);

        assert!(!result.needs_review, "Simple duty should not need review");
        let scoped = result.into_value();
        assert_eq!(scoped.modal_type, ModalObligationType::Duty);
        assert_eq!(scoped.polarity, Polarity::Positive);
        assert!(scoped.is_binding());
    }

    #[test]
    fn test_tenant_shall_not_sublease() {
        // "Tenant shall not sublease" - Prohibition, Negative, not flagged
        let analyzer = ModalScopeAnalyzer::new();
        let span = make_span(0, 0, 0, 5);

        let obligation = make_obligation("sublease");
        let modal = make_modal_clear(ModalObligationType::Prohibition, Polarity::Negative);
        let polarity = make_polarity_negative(span);
        let scope = Some(make_scope("not"));

        let result = analyzer.analyze(obligation, &modal, &polarity, scope, None);

        assert!(
            !result.needs_review,
            "Simple prohibition should not need review"
        );
        let scoped = result.into_value();
        assert_eq!(scoped.modal_type, ModalObligationType::Prohibition);
        assert_eq!(scoped.polarity, Polarity::Negative);
        assert!(scoped.is_binding());
        assert!(scoped.has_scope());
    }

    #[test]
    fn test_tenant_shall_not_be_required_to_insure() {
        // "Tenant shall not be required to insure" - Discretion, Positive (known pattern), not flagged
        let analyzer = ModalScopeAnalyzer::new();
        let span = make_span(0, 0, 0, 10);

        let obligation = make_obligation("insure");
        let modal = make_modal_discretion(DiscretionPattern::ShallNotBeRequiredTo);
        // Even though there's negation in text, discretion patterns resolve to granting freedom
        let polarity = make_polarity_positive(span);

        let result = analyzer.analyze(obligation, &modal, &polarity, None, None);

        assert!(
            !result.needs_review,
            "Known discretion pattern should not need review"
        );
        let scoped = result.into_value();
        assert_eq!(scoped.modal_type, ModalObligationType::Discretion);
        assert!(scoped.grants_freedom());
        assert!(!scoped.is_binding());
    }

    #[test]
    fn test_unless_tenant_does_not_pay() {
        // "Unless tenant does not pay" - Duty, Positive (double neg), flagged
        let analyzer = ModalScopeAnalyzer::new();
        let span = make_span(0, 0, 0, 8);

        let mut obligation = make_obligation("deliver goods");
        // Add condition
        obligation.value.conditions.push(ConditionRef {
            condition_type: crate::ContractKeyword::Unless,
            text_preview: "tenant does not pay".to_string(),
        });

        let modal = make_modal_clear(ModalObligationType::Duty, Polarity::Positive);
        let polarity = make_polarity_double_negative(span);

        let result = analyzer.analyze(obligation, &modal, &polarity, None, None);

        assert!(
            result.needs_review,
            "Double negative should trigger review"
        );
        let reason = result.review_reason.as_ref().unwrap();
        assert!(
            reason.contains("Double negative"),
            "Review reason should mention double negative"
        );
    }

    #[test]
    fn test_tenant_may_not_assign() {
        // "Tenant may not assign" - Prohibition, Negative, not flagged
        let analyzer = ModalScopeAnalyzer::new();
        let span = make_span(0, 0, 0, 5);

        let obligation = make_obligation("assign");
        let modal = make_modal_clear(ModalObligationType::Prohibition, Polarity::Negative);
        let polarity = make_polarity_negative(span);
        let scope = Some(make_scope("not"));

        let result = analyzer.analyze(obligation, &modal, &polarity, scope, None);

        assert!(
            !result.needs_review,
            "'may not' prohibition should not need review"
        );
        let scoped = result.into_value();
        assert_eq!(scoped.modal_type, ModalObligationType::Prohibition);
        assert!(scoped.is_binding());
    }

    // ========================================================================
    // Additional test cases
    // ========================================================================

    #[test]
    fn test_low_overall_confidence_triggers_review() {
        let analyzer = ModalScopeAnalyzer::new();
        let span = make_span(0, 0, 0, 5);

        // Create obligation with low confidence
        let phrase = ObligationPhrase {
            obligor: ObligorReference::NounPhrase {
                text: "someone".to_string(),
            },
            obligation_type: ObligationType::Duty,
            action: "do something".to_string(),
            conditions: Vec::new(),
        };
        let obligation = Scored::rule_based(phrase, 0.4, "weak_heuristic");

        let modal = make_modal_clear(ModalObligationType::Duty, Polarity::Positive);
        let polarity = make_polarity_positive(span);

        let result = analyzer.analyze(obligation, &modal, &polarity, None, None);

        assert!(
            result.needs_review,
            "Low obligation confidence should trigger review"
        );
        let reason = result.review_reason.as_ref().unwrap();
        assert!(
            reason.contains("confidence"),
            "Review reason should mention confidence"
        );
    }

    #[test]
    fn test_modal_ambiguity_triggers_review() {
        let analyzer = ModalScopeAnalyzer::new();
        let span = make_span(0, 0, 0, 5);

        let obligation = make_obligation("do something");
        let modal = make_modal_ambiguous(
            ModalObligationType::Permission,
            Polarity::Ambiguous,
            "Cannot determine modal intent",
        );
        let polarity = make_polarity_positive(span);

        let result = analyzer.analyze(obligation, &modal, &polarity, None, None);

        assert!(result.needs_review, "Modal ambiguity should trigger review");
        let reason = result.review_reason.as_ref().unwrap();
        assert!(
            reason.contains("modal"),
            "Review reason should mention modal"
        );
    }

    #[test]
    fn test_scope_flag_triggers_review() {
        let analyzer = ModalScopeAnalyzer::new();
        let span = make_span(0, 0, 0, 5);

        let obligation = make_obligation("perform X and Y");
        let modal = make_modal_clear(ModalObligationType::Prohibition, Polarity::Negative);
        let polarity = make_polarity_negative(span);
        let scope = Some(make_scope("not"));
        let scope_flag = Some(ScopeAmbiguityFlag::MultiplePlausibleBoundaries {
            count: 2,
            within_tokens: 5,
        });

        let result = analyzer.analyze(obligation, &modal, &polarity, scope, scope_flag);

        assert!(
            result.needs_review,
            "Scope ambiguity flag should trigger review"
        );
        let reason = result.review_reason.as_ref().unwrap();
        assert!(
            reason.contains("plausible") && reason.contains("boundaries"),
            "Review reason should mention scope boundaries"
        );
    }

    #[test]
    fn test_negation_quantifier_interaction_flag() {
        let analyzer = ModalScopeAnalyzer::new();
        let span = make_span(0, 0, 0, 8);

        let obligation = make_obligation("notify all parties");
        let modal = make_modal_clear(ModalObligationType::Prohibition, Polarity::Negative);
        let polarity = make_polarity_negative(span);
        let scope = Some(make_scope("not"));
        let scope_flag = Some(ScopeAmbiguityFlag::NegationQuantifierInteraction {
            negation_marker: "not".to_string(),
            quantifier_marker: "all".to_string(),
        });

        let result = analyzer.analyze(obligation, &modal, &polarity, scope, scope_flag);

        assert!(
            result.needs_review,
            "Negation-quantifier interaction should trigger review"
        );
        let reason = result.review_reason.as_ref().unwrap();
        assert!(
            reason.contains("interacts"),
            "Review reason should mention interaction"
        );
    }

    #[test]
    fn test_exception_conjunction_ambiguity_flag() {
        let analyzer = ModalScopeAnalyzer::new();
        let span = make_span(0, 0, 0, 10);

        let obligation = make_obligation("sell items");
        let modal = make_modal_clear(ModalObligationType::Duty, Polarity::Positive);
        let polarity = make_polarity_positive(span);
        let scope_flag = Some(ScopeAmbiguityFlag::ExceptionConjunctionAmbiguity {
            exception: "except".to_string(),
            conjunction: "and".to_string(),
        });

        let result = analyzer.analyze(obligation, &modal, &polarity, None, scope_flag);

        assert!(
            result.needs_review,
            "Exception-conjunction ambiguity should trigger review"
        );
        let reason = result.review_reason.as_ref().unwrap();
        assert!(
            reason.contains("except") && reason.contains("and"),
            "Review reason should mention the exception and conjunction"
        );
    }

    #[test]
    fn test_custom_review_threshold() {
        let span = make_span(0, 0, 0, 5);
        let obligation = make_obligation("pay rent");
        let modal = make_modal_clear(ModalObligationType::Duty, Polarity::Positive);
        let polarity = make_polarity_positive(span);

        // With stricter threshold (0.9), even good results need review
        let strict_analyzer = ModalScopeAnalyzer::with_threshold(0.9);
        let result = strict_analyzer.analyze(obligation.clone(), &modal, &polarity, None, None);
        // Overall confidence is compose_confidence([0.85, 0.95, 1.0, 1.0]) = ~0.8075
        // This is below 0.9 threshold
        assert!(
            result.needs_review,
            "Strict threshold should trigger review for moderate confidence"
        );

        // With lenient threshold (0.3), poor results don't need review
        let lenient_analyzer = ModalScopeAnalyzer::with_threshold(0.3);
        let result = lenient_analyzer.analyze(obligation, &modal, &polarity, None, None);
        assert!(
            !result.needs_review,
            "Lenient threshold should not trigger review"
        );
    }

    #[test]
    fn test_scoped_obligation_properties() {
        let analyzer = ModalScopeAnalyzer::new();
        let span = make_span(0, 0, 0, 5);

        let obligation = make_obligation("pay rent");
        let modal = make_modal_clear(ModalObligationType::Duty, Polarity::Positive);
        let polarity = make_polarity_positive(span);

        let result = analyzer.analyze(obligation, &modal, &polarity, None, None);
        let scoped = result.into_value();

        assert!(scoped.is_binding());
        assert!(!scoped.grants_freedom());
        assert!(!scoped.has_scope());
        assert!(!scoped.has_scope_ambiguity());
    }

    #[test]
    fn test_scope_none_flag_does_not_trigger_review() {
        let analyzer = ModalScopeAnalyzer::new();
        let span = make_span(0, 0, 0, 5);

        let obligation = make_obligation("pay rent");
        let modal = make_modal_clear(ModalObligationType::Duty, Polarity::Positive);
        let polarity = make_polarity_positive(span);
        let scope_flag = Some(ScopeAmbiguityFlag::None);

        let result = analyzer.analyze(obligation, &modal, &polarity, None, scope_flag);

        assert!(
            !result.needs_review,
            "ScopeAmbiguityFlag::None should not trigger review"
        );
    }

    #[test]
    fn test_multiple_review_reasons_combined() {
        let analyzer = ModalScopeAnalyzer::new();
        let span = make_span(0, 0, 0, 5);

        // Create scenario with multiple issues
        let phrase = ObligationPhrase {
            obligor: ObligorReference::NounPhrase {
                text: "someone".to_string(),
            },
            obligation_type: ObligationType::Duty,
            action: "do something".to_string(),
            conditions: Vec::new(),
        };
        let obligation = Scored::rule_based(phrase, 0.3, "weak_heuristic");

        let modal = make_modal_ambiguous(
            ModalObligationType::Permission,
            Polarity::Ambiguous,
            "Modal ambiguous",
        );
        let polarity = make_polarity_double_negative(span);
        let scope_flag = Some(ScopeAmbiguityFlag::LowBoundaryConfidence {
            confidence: 0.4,
            threshold: 0.7,
        });

        let result = analyzer.analyze(obligation, &modal, &polarity, None, scope_flag);

        assert!(result.needs_review, "Multiple issues should trigger review");
        let reason = result.review_reason.as_ref().unwrap();

        // Should have multiple reasons joined by "; "
        assert!(reason.contains("; "), "Multiple reasons should be joined");
        assert!(
            reason.contains("confidence") || reason.contains("Modal") || reason.contains("Double"),
            "Reason should contain at least one specific issue"
        );
    }

    #[test]
    fn test_confidence_composition() {
        let analyzer = ModalScopeAnalyzer::new();
        let span = make_span(0, 0, 0, 5);

        let obligation = make_obligation("pay rent"); // 0.85 confidence
        let modal = make_modal_clear(ModalObligationType::Duty, Polarity::Positive); // 0.95 confidence
        let polarity = make_polarity_negative(span); // 0.95 confidence
        let scope = Some(make_scope("not")); // 0.9 confidence in domain

        let result = analyzer.analyze(obligation, &modal, &polarity, scope, None);
        let scoped = result.into_value();

        // Expected: 0.85 * 0.95 * 0.95 * 0.9 = ~0.69
        let expected = 0.85 * 0.95 * 0.95 * 0.9;
        assert!(
            (scoped.overall_confidence - expected).abs() < 0.01,
            "Expected confidence ~{:.3}, got {:.3}",
            expected,
            scoped.overall_confidence
        );
    }
}

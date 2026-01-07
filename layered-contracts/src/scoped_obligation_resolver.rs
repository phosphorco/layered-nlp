//! ScopedObligation resolver for making obligations queryable via x::attr().
//!
//! This resolver wraps ModalScopeAnalyzer to produce ScopedObligation attributes
//! that can be queried from the line layer. It composes inputs from:
//! - ObligationPhraseResolver (obligor, action, conditions)
//! - Modal negation classification (derived from ObligationType)
//! - Polarity context (default positive for now)
//! - Scope ambiguity flags (if present)
//!
//! # Example
//!
//! ```ignore
//! use layered_contracts::{ScopedObligationResolver, ObligationPhraseResolver};
//! use layered_nlp::x;
//!
//! let ll_line = create_line_from_string("The Tenant shall pay rent.")
//!     .run(&ObligationPhraseResolver::new())
//!     .run(&ScopedObligationResolver::new());
//!
//! // Query for scoped obligations
//! for find in ll_line.find(&x::attr::<Scored<ScopedObligation>>()) {
//!     let scoped = find.attr();
//!     println!("{:?} - binding: {}", scoped.modal_type, scoped.is_binding());
//! }
//! ```

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};
use layered_nlp_document::{DocPosition, DocSpan, ReviewableResult, Scored};

use crate::{
    modal_negation::{ModalNegationClassification, ModalObligationType},
    modal_scope::{ModalScopeAnalyzer, ScopedObligation},
    obligation::{ObligationPhrase, ObligationType},
    polarity::{Polarity, PolarityContext},
    scope_ambiguity::ScopeAmbiguityFlag,
    ContractKeyword,
};

/// Resolver that produces ScopedObligation attributes from ObligationPhrase spans.
///
/// This resolver:
/// 1. Finds all Scored<ObligationPhrase> spans
/// 2. Derives modal classification from the obligation type
/// 3. Creates default polarity context
/// 4. Calls ModalScopeAnalyzer.analyze() to produce ScopedObligation
/// 5. Returns assignments at the same span as the source ObligationPhrase
#[derive(Debug, Clone)]
pub struct ScopedObligationResolver {
    /// The analyzer that composes modal, polarity, and scope information
    analyzer: ModalScopeAnalyzer,
}

impl ScopedObligationResolver {
    /// Create a new resolver with default analyzer settings.
    pub fn new() -> Self {
        Self {
            analyzer: ModalScopeAnalyzer::new(),
        }
    }

    /// Create a resolver with a custom review threshold.
    pub fn with_threshold(threshold: f64) -> Self {
        Self {
            analyzer: ModalScopeAnalyzer::with_threshold(threshold),
        }
    }

    /// Derive ModalObligationType from ObligationType.
    fn derive_modal_type(obligation_type: ObligationType) -> ModalObligationType {
        match obligation_type {
            ObligationType::Duty => ModalObligationType::Duty,
            ObligationType::Permission => ModalObligationType::Permission,
            ObligationType::Prohibition => ModalObligationType::Prohibition,
        }
    }

    /// Derive ContractKeyword from ObligationType (for modal classification).
    fn derive_contract_keyword(obligation_type: ObligationType) -> ContractKeyword {
        match obligation_type {
            ObligationType::Duty => ContractKeyword::Shall,
            ObligationType::Permission => ContractKeyword::May,
            ObligationType::Prohibition => ContractKeyword::ShallNot,
        }
    }

    /// Create a default DocSpan for placeholder purposes.
    ///
    /// Since we're operating at the line level, we use a placeholder span.
    /// Full document-level positioning is handled by the document resolver layer.
    fn placeholder_span() -> DocSpan {
        DocSpan {
            start: DocPosition { line: 0, token: 0 },
            end: DocPosition { line: 0, token: 1 },
        }
    }

    /// Create a ModalNegationClassification from an ObligationPhrase.
    fn create_modal_classification(
        &self,
        obligation: &ObligationPhrase,
    ) -> ModalNegationClassification {
        let modal_type = Self::derive_modal_type(obligation.obligation_type);
        let keyword = Self::derive_contract_keyword(obligation.obligation_type);
        let span = Self::placeholder_span();

        // Derive polarity from obligation type
        let polarity = match obligation.obligation_type {
            ObligationType::Duty | ObligationType::Permission => Polarity::Positive,
            ObligationType::Prohibition => Polarity::Negative,
        };

        ModalNegationClassification::clear(span, modal_type, &keyword, polarity)
    }

    /// Create a default PolarityContext.
    fn create_polarity_context(&self, obligation: &ObligationPhrase) -> PolarityContext {
        let span = Self::placeholder_span();

        match obligation.obligation_type {
            ObligationType::Duty | ObligationType::Permission => PolarityContext::positive(span),
            ObligationType::Prohibition => {
                // For prohibition, create a negative context
                PolarityContext::negative(span, span)
            }
        }
    }
}

impl Default for ScopedObligationResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver for ScopedObligationResolver {
    type Attr = Scored<ScopedObligation>;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();

        // Find all ObligationPhrase spans
        let obligations = selection.find_by(&x::attr::<Scored<ObligationPhrase>>());

        for (obligation_sel, scored_obligation) in obligations {
            // Create modal classification from the obligation
            let modal_classification =
                self.create_modal_classification(&scored_obligation.value);

            // Create polarity context
            let polarity_ctx = self.create_polarity_context(&scored_obligation.value);

            // Query for ScopeAmbiguityFlag if present on this span
            let scope_flag: Option<ScopeAmbiguityFlag> = obligation_sel
                .find_first_by(&x::attr::<ScopeAmbiguityFlag>())
                .map(|(_, flag)| flag.clone());

            // Call analyzer to produce ScopedObligation
            let reviewable_result: ReviewableResult<ScopedObligation> = self.analyzer.analyze(
                scored_obligation.clone(),
                &modal_classification,
                &polarity_ctx,
                None, // No scope operator for now - deferred to later gate
                scope_flag,
            );

            // Extract the ScopedObligation and wrap in Scored
            let confidence = reviewable_result.confidence();
            let needs_review = reviewable_result.needs_review;
            let scoped = reviewable_result.into_value();

            // Determine source based on whether review is needed
            let scored_scoped = if needs_review {
                Scored::rule_based(scoped, confidence, "scoped_obligation_uncertain")
            } else {
                Scored::rule_based(scoped, confidence, "scoped_obligation")
            };

            // Assign at the same span as the source ObligationPhrase
            results.push(obligation_sel.assign(scored_scoped).build());
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ContractKeywordResolver, DefinedTermResolver, ObligationPhraseResolver,
        ProhibitionResolver, PronounResolver, TermReferenceResolver,
    };
    use layered_nlp::create_line_from_string;
    use layered_part_of_speech::POSTagResolver;

    /// Run the full pipeline to get a line with ScopedObligation attributes.
    fn run_pipeline(text: &str) -> layered_nlp::LLLine {
        create_line_from_string(text)
            .run(&POSTagResolver::default())
            .run(&ContractKeywordResolver::new())
            .run(&ProhibitionResolver::new())
            .run(&DefinedTermResolver::new())
            .run(&TermReferenceResolver::new())
            .run(&PronounResolver::new())
            .run(&ObligationPhraseResolver::new())
            .run(&ScopedObligationResolver::new())
    }

    #[test]
    fn test_duty_resolved() {
        // Use defined term pattern to ensure obligor detection
        let line = run_pipeline(r#"ABC Corp (the "Company") shall deliver goods."#);

        let scoped: Vec<_> = line.find(&x::attr::<Scored<ScopedObligation>>());

        assert_eq!(scoped.len(), 1);
        let attr = scoped[0].attr();
        assert_eq!(attr.value.modal_type, ModalObligationType::Duty);
        assert!(attr.value.is_binding());
        assert!(!attr.value.grants_freedom());
    }

    #[test]
    fn test_permission_resolved() {
        let line = run_pipeline(r#"ABC Corp (the "Tenant") may sublease the premises."#);

        let scoped: Vec<_> = line.find(&x::attr::<Scored<ScopedObligation>>());

        assert_eq!(scoped.len(), 1);
        let attr = scoped[0].attr();
        assert_eq!(attr.value.modal_type, ModalObligationType::Permission);
        assert!(!attr.value.is_binding());
        assert!(attr.value.grants_freedom());
    }

    #[test]
    fn test_prohibition_resolved() {
        let line = run_pipeline(r#"ABC Corp (the "Tenant") shall not sublease."#);

        let scoped: Vec<_> = line.find(&x::attr::<Scored<ScopedObligation>>());

        assert_eq!(scoped.len(), 1);
        let attr = scoped[0].attr();
        assert_eq!(attr.value.modal_type, ModalObligationType::Prohibition);
        assert!(attr.value.is_binding());
        assert!(!attr.value.grants_freedom());
    }

    #[test]
    fn test_multiple_obligations() {
        let line = run_pipeline(r#"ABC Corp (the "Company") shall deliver and XYZ Inc (the "Vendor") shall refund."#);

        let scoped: Vec<_> = line.find(&x::attr::<Scored<ScopedObligation>>());

        // Should find both obligations
        assert_eq!(scoped.len(), 2);
        assert!(scoped.iter().all(|s| s.attr().value.modal_type == ModalObligationType::Duty));
    }

    #[test]
    fn test_no_obligations_produces_empty() {
        let line = run_pipeline("This is a simple sentence without modal verbs.");

        let scoped: Vec<_> = line.find(&x::attr::<Scored<ScopedObligation>>());

        assert!(scoped.is_empty());
    }

    #[test]
    fn test_confidence_preserved() {
        let line = run_pipeline(r#"ABC Corp (the "Company") shall deliver goods."#);

        let scoped: Vec<_> = line.find(&x::attr::<Scored<ScopedObligation>>());

        assert_eq!(scoped.len(), 1);
        // Confidence should be composed from obligation and modal classification
        // Both are high confidence, so overall should be > 0.6
        assert!(scoped[0].attr().confidence > 0.6);
    }
}

//! LinkedObligation resolver for making linked obligations queryable via x::attr().
//!
//! This resolver wraps ObligationPartyLinker to produce LinkedObligation attributes
//! that can be queried from the line layer. It processes Gate 2 output (ScopedObligation)
//! and links each obligation to its parties (obligor and beneficiary).
//!
//! # Architecture
//!
//! Gate 3 in the accountability graph pipeline:
//! - Input: Scored<ScopedObligation> from Gate 2 (ScopedObligationResolver)
//! - Input: PronounChainResult[] for pronoun resolution context
//! - Output: LinkedObligation with identified parties
//!
//! # Example
//!
//! ```ignore
//! use layered_contracts::{
//!     LinkedObligationResolver, LinkedObligationResolverConfig,
//!     ScopedObligationResolver, ObligationPhraseResolver,
//! };
//! use layered_nlp::x;
//!
//! // Assume pronoun chains were built from document-level analysis
//! let chains = vec![]; // PronounChainResult from document resolver
//!
//! let config = LinkedObligationResolverConfig {
//!     pronoun_chains: chains,
//! };
//!
//! let ll_line = create_line_from_string("The Tenant shall pay Landlord.")
//!     .run(&ObligationPhraseResolver::new())
//!     .run(&ScopedObligationResolver::new())
//!     .run(&LinkedObligationResolver::new(config));
//!
//! // Query for linked obligations
//! for find in ll_line.find(&x::attr::<LinkedObligation>()) {
//!     let linked = find.attr();
//!     println!("Obligor: {}, Confidence: {:.2}",
//!         linked.obligor_name(), linked.overall_confidence);
//! }
//! ```

use layered_nlp::{x, LLCursorAssignment, LLSelection, Resolver};
use layered_nlp_document::Scored;

use crate::{
    modal_scope::ScopedObligation,
    obligation_linker::{LinkedObligation, ObligationPartyLinker},
    pronoun::PronounChainResult,
};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the LinkedObligationResolver.
///
/// Holds document-level context needed for party linking, primarily the
/// pronoun chains that enable pronoun resolution.
#[derive(Debug, Clone, Default)]
pub struct LinkedObligationResolverConfig {
    /// Pronoun chains from document-level analysis.
    ///
    /// These are used to resolve pronouns in obligation phrases to their
    /// canonical referents (e.g., "it" -> "Company").
    pub pronoun_chains: Vec<PronounChainResult>,
}

impl LinkedObligationResolverConfig {
    /// Create a new config with the given pronoun chains.
    pub fn with_chains(chains: Vec<PronounChainResult>) -> Self {
        Self {
            pronoun_chains: chains,
        }
    }
}

// ============================================================================
// Resolver
// ============================================================================

/// Resolver that produces LinkedObligation attributes from ScopedObligation spans.
///
/// This resolver:
/// 1. Finds all Scored<ScopedObligation> spans from Gate 2
/// 2. Extracts the ScopedObligation from the Scored wrapper
/// 3. Calls ObligationPartyLinker.link() with pronoun chains for context
/// 4. Returns LinkedObligation assignments at the same span
///
/// Note: LinkedObligation is NOT wrapped in Scored because it has its own
/// `overall_confidence` field that incorporates confidence from all components.
#[derive(Debug, Clone)]
pub struct LinkedObligationResolver {
    /// The linker that extracts parties from obligations
    linker: ObligationPartyLinker,
    /// Configuration with document-level context
    config: LinkedObligationResolverConfig,
}

impl LinkedObligationResolver {
    /// Create a new resolver with the given configuration.
    pub fn new(config: LinkedObligationResolverConfig) -> Self {
        Self {
            linker: ObligationPartyLinker::new(),
            config,
        }
    }

    /// Create a new resolver with default (empty) configuration.
    ///
    /// Use this when pronoun chain context is not available.
    pub fn default_config() -> Self {
        Self::new(LinkedObligationResolverConfig::default())
    }
}

impl Resolver for LinkedObligationResolver {
    type Attr = LinkedObligation;

    fn go(&self, selection: LLSelection) -> Vec<LLCursorAssignment<Self::Attr>> {
        let mut results = Vec::new();

        // Find all Scored<ScopedObligation> spans from Gate 2
        let scoped_obligations = selection.find_by(&x::attr::<Scored<ScopedObligation>>());

        for (scoped_sel, scored_scoped) in scoped_obligations {
            // Extract the ScopedObligation from the Scored wrapper
            let scoped_obligation = scored_scoped.value.clone();

            // Call linker to produce LinkedObligation
            let linked = self
                .linker
                .link(scoped_obligation, &self.config.pronoun_chains);

            // Assign at the same span as the source ScopedObligation
            results.push(scoped_sel.assign(linked).build());
        }

        results
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ContractKeywordResolver, DefinedTermResolver, ObligationPhraseResolver,
        ProhibitionResolver, PronounResolver, ScopedObligationResolver, TermReferenceResolver,
    };
    use layered_nlp::create_line_from_string;
    use layered_part_of_speech::POSTagResolver;

    /// Run the full pipeline to get a line with LinkedObligation attributes.
    fn run_pipeline(text: &str) -> layered_nlp::LLLine {
        run_pipeline_with_chains(text, vec![])
    }

    /// Run the pipeline with pronoun chains for resolution context.
    fn run_pipeline_with_chains(
        text: &str,
        chains: Vec<PronounChainResult>,
    ) -> layered_nlp::LLLine {
        let config = LinkedObligationResolverConfig::with_chains(chains);

        create_line_from_string(text)
            .run(&POSTagResolver::default())
            .run(&ContractKeywordResolver::new())
            .run(&ProhibitionResolver::new())
            .run(&DefinedTermResolver::new())
            .run(&TermReferenceResolver::new())
            .run(&PronounResolver::new())
            .run(&ObligationPhraseResolver::new())
            .run(&ScopedObligationResolver::new())
            .run(&LinkedObligationResolver::new(config))
    }

    // ========================================================================
    // Direct obligor/beneficiary tests
    // ========================================================================

    #[test]
    fn test_direct_obligor_term_ref() {
        // Direct defined term as obligor
        let line = run_pipeline(r#"ABC Corp (the "Tenant") shall pay rent."#);

        let linked: Vec<_> = line.find(&x::attr::<LinkedObligation>());

        assert_eq!(linked.len(), 1);
        let attr = linked[0].attr();
        assert_eq!(attr.obligor_name(), "Tenant");
        assert!(!attr.obligor.needs_review);
        assert!(attr.overall_confidence > 0.6);
    }

    #[test]
    fn test_direct_obligor_with_beneficiary() {
        // Direct obligor with beneficiary in action text
        let line = run_pipeline(r#"ABC Corp (the "Tenant") shall pay Landlord rent."#);

        let linked: Vec<_> = line.find(&x::attr::<LinkedObligation>());

        assert_eq!(linked.len(), 1);
        let attr = linked[0].attr();

        // Obligor should be Tenant
        assert_eq!(attr.obligor_name(), "Tenant");

        // Beneficiary should be detected as Landlord
        assert!(attr.beneficiary.is_some());
        assert_eq!(attr.beneficiary_name(), Some("Landlord"));
    }

    #[test]
    fn test_beneficiary_to_pattern() {
        // "to X" pattern for beneficiary detection
        let line = run_pipeline(r#"XYZ Inc (the "Company") shall deliver goods to Vendor."#);

        let linked: Vec<_> = line.find(&x::attr::<LinkedObligation>());

        assert_eq!(linked.len(), 1);
        let attr = linked[0].attr();

        assert_eq!(attr.obligor_name(), "Company");
        assert!(attr.beneficiary.is_some());
        assert_eq!(attr.beneficiary_name(), Some("Vendor"));
    }

    // ========================================================================
    // Pronoun resolution tests
    // ========================================================================

    #[test]
    fn test_pronoun_with_chain_context() {
        // Pronoun resolution with chain context
        let chains = vec![PronounChainResult {
            chain_id: 1,
            canonical_name: "Company".to_string(),
            is_defined_term: true,
            mention_count: 3,
            has_verified_mention: true,
            best_confidence: 0.95,
        }];

        // Note: The pronoun "It" must be preceded by a defined term for resolution
        let line =
            run_pipeline_with_chains(r#"ABC Corp (the "Company") exists. It shall deliver."#, chains);

        let linked: Vec<_> = line.find(&x::attr::<LinkedObligation>());

        // Should find an obligation with pronoun resolved
        assert!(!linked.is_empty());
    }

    #[test]
    fn test_pronoun_resolution_uses_chains() {
        // Test that pronoun chains are passed to the linker
        // The actual pronoun resolution happens in earlier pipeline stages,
        // but the chains provide additional context for confidence boosting

        let chains = vec![PronounChainResult {
            chain_id: 1,
            canonical_name: "Entity".to_string(),
            is_defined_term: true,
            mention_count: 3,
            has_verified_mention: true,
            best_confidence: 0.95,
        }];

        let line = run_pipeline_with_chains(
            r#"Something (the "Entity") shall perform duties."#,
            chains,
        );

        let linked: Vec<_> = line.find(&x::attr::<LinkedObligation>());

        // Should find an obligation - the chain context doesn't change detection,
        // but does affect confidence when pronouns are resolved
        assert_eq!(linked.len(), 1);
        let attr = linked[0].attr();

        // The obligor should be Entity (defined term)
        assert_eq!(attr.obligor_name(), "Entity");
    }

    // ========================================================================
    // No scoped obligations case
    // ========================================================================

    #[test]
    fn test_no_scoped_obligations_produces_empty() {
        // Text without any modal verbs should produce no linked obligations
        let line = run_pipeline("This is a simple sentence without modal verbs.");

        let linked: Vec<_> = line.find(&x::attr::<LinkedObligation>());

        assert!(linked.is_empty());
    }

    #[test]
    fn test_empty_chains_still_works() {
        // Should work even without pronoun chain context
        let line = run_pipeline(r#"ABC Corp (the "Seller") shall deliver goods."#);

        let linked: Vec<_> = line.find(&x::attr::<LinkedObligation>());

        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].attr().obligor_name(), "Seller");
    }

    // ========================================================================
    // Multiple obligations test
    // ========================================================================

    #[test]
    fn test_multiple_obligations_linked() {
        // Multiple obligations in same line should all be linked
        let line = run_pipeline(
            r#"ABC Corp (the "Buyer") shall pay and XYZ Inc (the "Seller") shall deliver."#,
        );

        let linked: Vec<_> = line.find(&x::attr::<LinkedObligation>());

        // Should find both obligations
        assert_eq!(linked.len(), 2);

        let obligors: Vec<_> = linked.iter().map(|f| f.attr().obligor_name()).collect();
        assert!(obligors.contains(&"Buyer"));
        assert!(obligors.contains(&"Seller"));
    }

    // ========================================================================
    // Confidence propagation test
    // ========================================================================

    #[test]
    fn test_confidence_composition() {
        // Overall confidence should compose from all components
        let line = run_pipeline(r#"ABC Corp (the "Company") shall deliver goods."#);

        let linked: Vec<_> = line.find(&x::attr::<LinkedObligation>());

        assert_eq!(linked.len(), 1);
        let attr = linked[0].attr();

        // Overall confidence should be composed from:
        // - ScopedObligation confidence
        // - Obligor confidence
        // - Beneficiary confidence (if any)
        assert!(
            attr.overall_confidence > 0.0 && attr.overall_confidence <= 1.0,
            "Overall confidence should be in valid range"
        );
    }

    // ========================================================================
    // Review status tests
    // ========================================================================

    #[test]
    fn test_needs_review_propagation() {
        // needs_review should reflect component status
        let line = run_pipeline(r#"ABC Corp (the "Company") shall pay rent."#);

        let linked: Vec<_> = line.find(&x::attr::<LinkedObligation>());

        assert_eq!(linked.len(), 1);
        let attr = linked[0].attr();

        // High confidence term ref should not need review
        assert!(!attr.needs_review());
        assert!(attr.review_reasons().is_empty());
    }
}

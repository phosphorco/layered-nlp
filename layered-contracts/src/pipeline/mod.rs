//! Pipeline presets for running contract analysis resolvers in correct order.
//!
//! The Pipeline struct provides preset configurations that automatically order
//! resolvers based on their dependencies.

use crate::{
    ContractDocument, ContractKeywordResolver, DefinedTermResolver, ObligationPhraseResolver,
    ProcessError, PronounChainResolver, PronounResolver, SectionHeaderResolver,
    SectionReferenceResolver, TemporalExpressionResolver, TermReferenceResolver,
    TermsOfArtResolver,
};

/// Pipeline preset for running contract analysis resolvers.
///
/// Provides preset configurations that run resolvers in correct dependency order:
/// - `structure_only()` - Section/header detection only
/// - `fast()` - Quick structure + keywords
/// - `standard()` - Full core analysis (keywords, terms, obligations)
#[derive(Debug, Clone)]
pub struct Pipeline {
    /// Resolvers to run (in order)
    resolvers: Vec<ResolverType>,
}

#[derive(Debug, Clone, Copy)]
enum ResolverType {
    SectionHeader,
    SectionReference,
    ContractKeyword,
    TermsOfArt,
    DefinedTerm,
    TermReference,
    Temporal,
    Pronoun,
    PronounChain,
    Obligation,
}

impl Pipeline {
    /// Section/header detection only - fastest preset.
    pub fn structure_only() -> Self {
        Self {
            resolvers: vec![ResolverType::SectionHeader, ResolverType::SectionReference],
        }
    }

    /// Quick structure + keywords.
    pub fn fast() -> Self {
        Self {
            resolvers: vec![
                ResolverType::SectionHeader,
                ResolverType::SectionReference,
                ResolverType::ContractKeyword,
            ],
        }
    }

    /// Full core analysis with proper resolver ordering.
    ///
    /// Runs resolvers in dependency order:
    /// 1. SectionHeader - section structure (no deps)
    /// 2. SectionReference - cross-references (needs headers)
    /// 3. ContractKeyword - modal verbs (no deps)
    /// 4. TermsOfArt - multi-word expressions (no deps, prevents splitting)
    /// 5. DefinedTerm - term definitions (no deps)
    /// 6. TermReference - term usage (needs DefinedTerm)
    /// 7. Temporal - time expressions (no deps)
    /// 8. Pronoun - pronoun resolution (needs DefinedTerm)
    /// 9. PronounChain - pronoun chains (needs Pronoun, DefinedTerm)
    /// 10. Obligation - obligation phrases (needs TermReference, PronounChain)
    pub fn standard() -> Self {
        Self {
            resolvers: vec![
                ResolverType::SectionHeader,
                ResolverType::SectionReference,
                ResolverType::ContractKeyword,
                ResolverType::TermsOfArt,
                ResolverType::DefinedTerm,
                ResolverType::TermReference,
                ResolverType::Temporal,
                ResolverType::Pronoun,
                ResolverType::PronounChain,
                ResolverType::Obligation,
            ],
        }
    }

    /// Run the pipeline on text, returning a fully analyzed ContractDocument.
    pub fn run_on_text(&self, text: &str) -> Result<ContractDocument, ProcessError> {
        let mut doc = ContractDocument::from_text(text);

        for resolver_type in &self.resolvers {
            doc = match resolver_type {
                ResolverType::SectionHeader => doc.run_resolver(&SectionHeaderResolver::new()),
                ResolverType::SectionReference => {
                    doc.run_resolver(&SectionReferenceResolver::new())
                }
                ResolverType::ContractKeyword => doc.run_resolver(&ContractKeywordResolver::new()),
                ResolverType::TermsOfArt => doc.run_resolver(&TermsOfArtResolver::new()),
                ResolverType::DefinedTerm => doc.run_resolver(&DefinedTermResolver::new()),
                ResolverType::TermReference => doc.run_resolver(&TermReferenceResolver::new()),
                ResolverType::Temporal => doc.run_resolver(&TemporalExpressionResolver::new()),
                ResolverType::Pronoun => doc.run_resolver(&PronounResolver::new()),
                ResolverType::PronounChain => doc.run_resolver(&PronounChainResolver::new()),
                ResolverType::Obligation => doc.run_resolver(&ObligationPhraseResolver::new()),
            };
        }

        Ok(doc)
    }
}

#![doc(
    html_logo_url = "https://raw.githubusercontent.com/storyscript/layered-nlp/main/assets/layered-nlp.svg",
    issue_tracker_base_url = "https://github.com/storyscript/layered-nlp/issues/"
)]

//! Contract language analysis plugin for layered-nlp.
//!
//! This crate provides a comprehensive pipeline for analyzing legal contracts:
//!
//! ## Per-Line Resolvers
//!
//! - [`ContractKeywordResolver`] - Detects modal verbs (shall, may, must)
//! - [`DefinedTermResolver`] - Extracts defined terms ("Company" means...)
//! - [`TermReferenceResolver`] - Links term references to definitions
//! - [`ObligationPhraseResolver`] - Detects obligation phrases with obligor/action
//! - [`PronounResolver`] - Resolves pronouns to antecedents
//! - [`SectionHeaderResolver`] - Parses section headers (Section 3.1, Article IV)
//! - [`SectionReferenceResolver`] - Detects references to sections
//! - [`TemporalExpressionResolver`] - Extracts time expressions (within 30 days)
//!
//! ## Document-Level Processing
//!
//! - [`ContractDocument`] - Multi-line document abstraction
//! - [`DocumentStructureBuilder`] - Builds hierarchical section tree
//! - [`SectionReferenceLinker`] - Resolves section references to targets
//!
//! ## Contract Comparison (Semantic Diff)
//!
//! - [`DocumentAligner`] - Aligns sections between document versions
//! - [`SemanticDiffEngine`] - Detects semantic changes with risk classification
//!
//! ## Confidence Scoring
//!
//! All resolvers use a [`Scored<T>`] wrapper to represent confidence levels,
//! where `confidence < 1.0` means the result needs verification, and
//! `confidence = 1.0` means the result has been verified.
//!
//! ## Example
//!
//! ```ignore
//! use layered_contracts::{ContractDocument, SectionHeaderResolver};
//!
//! let doc = ContractDocument::from_text("Section 1.1 Definitions\n...")
//!     .run_resolver(&SectionHeaderResolver::new());
//! ```

mod accountability_analytics;
mod accountability_graph;
mod conflict_detector;
mod contract_clause;
mod clause_aggregate;
mod contract_keyword;
mod defined_term;
mod deictic;
mod document;
mod document_aligner;
mod document_structure;
mod obligation;
mod pronoun;
mod pronoun_chain;
mod scored;
mod section_header;
mod section_reference;
mod section_reference_linker;
mod semantic_diff;
mod temporal;
mod term_reference;
mod token_diff;
mod utils;
mod verification;

// Pipeline presets for running resolvers in dependency order
pub mod pipeline;

pub use accountability_analytics::{
    AccountabilityNodePayload, AccountabilityPayload, BeneficiaryDescriptor, BeneficiaryGroup,
    BeneficiaryPayload, ClausePayload, ConditionPayload, ObligationGraph, PartyAnalytics,
    PartySummary, VerificationQueueDetails, VerificationQueueItem,
};
pub use accountability_graph::{
    AccountabilityGraphResolver, BeneficiaryLink, ConditionLink, ObligationNode,
};
pub use clause_aggregate::{
    ClauseAggregate, ClauseAggregateEntry, ClauseAggregationResolver,
};
pub use conflict_detector::{
    Conflict, ConflictDetector, ConflictType, NormalizedObligation, ObligationNormalizer,
    ObligationTopic, TopicClassifier, group_by_topic,
};
pub use contract_clause::{
    ClauseCondition, ClauseDuty, ClauseParty, ContractClause, ContractClauseResolver,
};
pub use contract_keyword::{ContractKeyword, ContractKeywordResolver, ProhibitionResolver};
pub use defined_term::{DefinedTerm, DefinedTermResolver, DefinitionType};
pub use obligation::{
    ConditionRef, ObligationPhrase, ObligationPhraseResolver, ObligationType, ObligorReference,
};
pub use pronoun::{AntecedentCandidate, PronounReference, PronounResolver, PronounType};
pub use pronoun_chain::{ChainMention, MentionType, PronounChain, PronounChainResolver};
pub use scored::{ScoreSource, Scored};
pub use term_reference::{TermReference, TermReferenceResolver};
pub use verification::{
    apply_verification_action, VerificationAction, VerificationNote, VerificationTarget,
};

// Document-level abstractions (new)
pub use document::{ContractDocument, DocPosition, DocSpan, ProcessError, ProcessResult};
pub use document_aligner::{
    AlignedPair, AlignmentCandidate, AlignmentCandidates, AlignmentHint, AlignmentResult,
    AlignmentSignal, AlignmentStats, AlignmentType, DocumentAligner, HintType, SectionRef,
    SimilarityConfig,
};
pub use document_structure::{DocumentProcessor, DocumentStructure, DocumentStructureBuilder, SectionNode};
pub use section_header::{SectionHeader, SectionHeaderResolver, SectionIdentifier, SectionKind};
pub use section_reference::{
    ReferencePurpose, ReferenceType, RelativeReference, SectionReference, SectionReferenceResolver,
};
pub use section_reference_linker::{
    LinkedReference, LinkedReferences, ReferenceResolution, SectionReferenceLinker,
};
pub use temporal::{
    DeadlineType, DurationUnit, NormalizedTiming, TemporalConverter, TemporalExpression,
    TemporalExpressionResolver, TemporalType, TimeRelation, TimeUnit,
};
pub use semantic_diff::{
    AffectedReference, ChangeSignal, ConditionChange, DiffConfig, DiffHint, DiffHintType,
    DiffReviewCandidates, DiffSummary, ImpactDirection, ObligationModalChange, PartyChange,
    PartyImpact, PartySummaryDiff, ReferenceUsageType, RiskLevel, SemanticChange,
    SemanticChangeType, SemanticDiffEngine, SemanticDiffResult, TemporalChange, TemporalSnapshot,
    TermChange, TermChangeClass,
};
pub use token_diff::{
    AlignedTokenPair, TokenAlignment, TokenAlignmentConfig, TokenAligner, TokenRef,
    TokenRelation, WhitespaceMode,
};
// Note: token_diff::AlignmentStats is accessible but not re-exported to avoid
// name collision with document_aligner::AlignmentStats

// Deictic mapping resolver
pub use deictic::DeicticResolver;

// Re-export layered_deixis types for convenience
pub use layered_deixis::{
    DeicticCategory, DeicticReference, DeicticSource, DeicticSubcategory, ResolvedReferent,
    // Simple resolvers for gap-filling
    DiscourseMarkerResolver, PersonPronounResolver, PlaceDeicticResolver, SimpleTemporalResolver,
};

#[cfg(test)]
mod tests {
    mod accountability_analytics;
    mod clause_aggregate;
    mod accountability_graph;
    mod contract_clause;
    mod contract_keyword;
    mod defined_term;
    mod document_aligner;
    mod obligation;
    mod pronoun;
    mod pronoun_chain;
    mod semantic_diff;
    mod term_reference;
}

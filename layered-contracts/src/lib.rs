#![doc(
    html_logo_url = "https://raw.githubusercontent.com/storyscript/layered-nlp/main/assets/layered-nlp.svg",
    issue_tracker_base_url = "https://github.com/storyscript/layered-nlp/issues/"
)]

//! Contract language analysis plugin for layered-nlp.
//!
//! This crate provides resolvers for analyzing contract language, including:
//! - Contract keywords (shall, may, means, etc.)
//! - Defined terms and their references
//! - Pronoun resolution and antecedent tracking
//! - Obligation phrase detection
//!
//! All resolvers use a [`Scored<T>`] wrapper to represent confidence levels,
//! where `confidence < 1.0` means the result needs verification, and
//! `confidence = 1.0` means the result has been verified.

mod accountability_analytics;
mod accountability_graph;
mod contract_clause;
mod clause_aggregate;
mod contract_keyword;
mod defined_term;
mod obligation;
mod pronoun;
mod pronoun_chain;
mod scored;
mod term_reference;
mod utils;
mod verification;

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

#[cfg(test)]
mod tests {
    mod accountability_analytics;
    mod clause_aggregate;
    mod accountability_graph;
    mod contract_clause;
    mod contract_keyword;
    mod defined_term;
    mod obligation;
    mod pronoun;
    mod pronoun_chain;
    mod term_reference;
}

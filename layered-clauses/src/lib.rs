#![doc(
    html_logo_url = "https://raw.githubusercontent.com/storyscript/layered-nlp/main/assets/layered-nlp.svg",
    issue_tracker_base_url = "https://github.com/storyscript/layered-nlp/issues/"
)]

mod clause_keyword;
mod clause_link;
mod clause_link_resolver;
mod clause_participant;
mod clause_query;
mod clauses;
mod list_marker;
mod relative_clause;
mod sentence_boundary;

pub use clause_keyword::{ClauseKeyword, ClauseKeywordResolver};
pub use clause_link::ClauseLinkBuilder;
pub use clause_link_resolver::{ClauseLink, ClauseLinkResolver, ClauseSpan, CoordinationType, LinkConfidence};
pub use clause_participant::{ClauseParticipant, ClauseParticipants, ParticipantDetector, ParticipantRole};
pub use clause_query::ClauseQueryAPI;
pub use clauses::{Clause, ClauseResolver};
pub use relative_clause::{
    RelativeClauseAttachment, RelativeClauseDetector, RelativeClauseType, RelativePronoun,
};

// Re-export from layered-contracts for convenience
pub use layered_contracts::ObligationType;
pub use list_marker::{ListMarker, ListMarkerResolver};
pub use sentence_boundary::{SentenceBoundary, SentenceBoundaryResolver, SentenceConfidence};

#[cfg(test)]
mod tests {
    mod clauses;
    mod cross_line;
    mod cross_reference;
    mod exception_scope;
    mod integration;
    mod list_marker;
    mod obligation_integration;
    mod sentence_boundary;
}

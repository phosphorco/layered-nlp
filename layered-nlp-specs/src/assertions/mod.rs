//! Per-type assertion implementations.

mod obligation;
mod pronoun;
mod defined_term;
mod term_reference;
mod clause;
mod clause_link;

pub use obligation::ObligationAssertion;
pub use pronoun::PronounAssertion;
pub use defined_term::DefinedTermAssertion;
pub use term_reference::TermReferenceAssertion;
pub use clause::ClauseAssertion;
pub use clause_link::{ClauseLinkAssertion, ClauseLinkMatch};

//! Per-type assertion implementations.

mod obligation;
mod pronoun;
mod defined_term;

pub use obligation::ObligationAssertion;
pub use pronoun::PronounAssertion;
pub use defined_term::DefinedTermAssertion;

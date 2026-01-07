#![doc(
    html_logo_url = "https://raw.githubusercontent.com/storyscript/layered-nlp/main/assets/layered-nlp.svg",
    issue_tracker_base_url = "https://github.com/storyscript/layered-nlp/issues/"
)]

//! Specification-driven testing infrastructure for layered-nlp.
//!
//! This crate provides a test harness where test cases are defined
//! declaratively in `.nlp` fixture files with inline span annotations.
//!
//! ## Overview
//!
//! The spec system allows defining expected NLP annotations directly in
//! fixture files, enabling visual verification of span coverage and
//! regression testing.
//!
//! ## Modules
//!
//! - [`parser`] - Parses `.nlp` fixture files with inline annotations
//! - [`fixture`] - Fixture file loading and management
//! - [`assertion`] - Assertion types for span matching
//! - [`matcher`] - Pattern matching for span attributes
//! - [`runner`] - Test runner for executing fixtures
//! - [`errors`] - Error types for the spec system
//! - [`formatter`] - Rich error formatting with field-level diagnostics
//! - [`failures`] - Expected failures tracking via TOML

pub mod assertion;
pub mod assertions;
pub mod config;
pub mod context;
pub mod errors;
pub mod failures;
pub mod fixture;
pub mod formatter;
pub mod loader;
pub mod matcher;
pub mod parser;
pub mod runner;

// Re-exports for convenient access to core types
pub use assertion::{
    AssertionMismatch, AssertionSpec, FieldMismatch, FieldOperator, FieldValue, MismatchSeverity,
    ParseError, SpanAssertion, TypeFieldCheck,
};
pub use assertions::{DefinedTermAssertion, ObligationAssertion, PronounAssertion};
pub use errors::{SpecError, SpecResult};
pub use fixture::{
    Assertion, AssertionBody, CompareOp, FieldCheck, NlpFixture, RefTarget, SpanMarker,
};
pub use matcher::{
    MatchResult, AssertionResult, AssertionOutcome,
    check_obligation, check_pronoun, check_defined_term,
    is_supported_type, valid_fields_for_type,
};
pub use parser::{parse_fixture, parse_spans};
pub use loader::{load_fixture, load_all_fixtures};
pub use context::DocumentContext;
pub use config::PipelineConfig;
pub use runner::{run_fixture, check_fixture_assertions, PipelineResult};
pub use formatter::{format_failure, format_summary};
pub use failures::{ExpectedFailures, FailureEntry, FailureState, HarnessResult};

#[cfg(test)]
mod tests;

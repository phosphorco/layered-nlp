//! Error types for the spec system.
//!
//! This module defines error types for parsing, fixture loading,
//! and assertion failures.

use thiserror::Error;

/// Errors that can occur during spec processing.
#[derive(Debug, Error)]
pub enum SpecError {
    /// Error parsing a fixture file.
    #[error("parse error at line {line}: {message}")]
    Parse { line: usize, message: String },

    /// Error loading a fixture file.
    #[error("failed to load fixture: {path}: {message}")]
    Load { path: String, message: String },

    /// Assertion failure when comparing spans.
    #[error("assertion failed: {message}")]
    Assertion { message: String },
}

/// Result type for spec operations.
pub type SpecResult<T> = Result<T, SpecError>;

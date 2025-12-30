//! Document-level abstractions for layered-nlp.
//!
//! This crate provides infrastructure for multi-line document processing,
//! confidence scoring, and snapshot testing.
//!
//! ## Core Types
//!
//! - [`LayeredDocument`] - Multi-line document abstraction
//! - [`DocPosition`] / [`DocSpan`] - Position within documents
//! - [`Scored<T>`] - Values with confidence scores
//! - [`Ambiguous<T>`] - N-best alternatives with ambiguity detection
//!
//! ## Example
//!
//! ```
//! use layered_nlp_document::{LayeredDocument, Scored};
//!
//! let doc = LayeredDocument::from_text("Line 1\nLine 2\nLine 3");
//! assert_eq!(doc.line_count(), 3);
//!
//! let scored = Scored::rule_based("value", 0.85, "my_rule");
//! assert!(scored.needs_verification());
//! ```

mod document;
mod scored;
mod ambiguity;
mod span_link;
mod scope_operator;
mod scope_index;

// Document types
pub use document::{
    DocPosition,
    DocSpan,
    LayeredDocument,
    ProcessError,
    ProcessResult,
};

// Scoring infrastructure
pub use scored::{
    Scored,
    ScoreSource,
};

// Ambiguity infrastructure
pub use ambiguity::{
    AmbiguityFlag,
    AmbiguityConfig,
    Ambiguous,
};

// Span link infrastructure
pub use span_link::{
    SpanLink,
    DocSpanLink,
    ClauseRole,
    AttachmentRole,
    SemanticRole,
    ConflictRole,
};

// Scope operator infrastructure
pub use scope_operator::{
    ScopeDimension,
    ScopeDomain,
    ScopeOperator,
    NegationOp,
    NegationKind,
    QuantifierOp,
    QuantifierKind,
    PrecedenceOp,
    DeicticFrame,
};

// Scope index
pub use scope_index::ScopeIndex;

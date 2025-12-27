//! Dual-layer test snapshot system (FR-010).
//!
//! This module provides infrastructure for test snapshots that separates:
//! - **Storage** — machine-readable RON format as the canonical source of truth
//! - **Rendering** — human-readable views derived from storage
//!
//! # Key Types
//!
//! - [`Snapshot`] — Canonical storage format (RON-serializable)
//! - [`SpanData`] — Individual span with ID, position, value, associations
//! - [`SnapshotSpanId`] — Stable identifier (e.g., "dt-0", "ob-0")
//! - [`SnapshotKind`] — Trait for type-specific prefixes
//!
//! # Example
//!
//! ```ignore
//! use layered_contracts::snapshot::{Snapshot, SnapshotBuilder};
//!
//! let doc = ContractDocument::from_text("...")
//!     .run_resolver(&SectionHeaderResolver::new());
//!
//! let snapshot = SnapshotBuilder::from_document(&doc).build();
//! let ron_string = snapshot.to_ron_string().unwrap();
//! ```

mod types;
mod construction;
mod semantic;
pub mod display;
pub mod graph;
pub mod config;
#[macro_use]
mod macros;

pub use types::{
    AssociationData, InputSource, SnapshotSpanId, SnapshotDocPos, SnapshotDocSpan, SnapshotKind,
    Snapshot, SpanData,
};
pub use construction::SnapshotBuilder;
pub use semantic::{classify_type_name, SemanticCategory, SnapshotRenderer};
pub use display::{DocDisplay, index_to_label};
pub use graph::GraphRenderer;
pub use config::{RenderMode, SnapshotConfig};

// Re-export macros for convenience
pub use crate::{assert_contract_snapshot, assert_contract_snapshot_data, assert_contract_snapshot_view};

#[cfg(test)]
mod tests;

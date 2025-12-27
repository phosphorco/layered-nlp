//! Snapshot test macros.
//!
//! This module provides macros for creating dual-layer test snapshots.

/// Assert that a contract document matches expected snapshots.
///
/// This macro produces two snapshot files:
/// - `{name}_data.snap` — Canonical RON storage format (redacted for stability)
/// - `{name}_view.snap` — Human-readable semantic summary (not redacted)
///
/// # Arguments
///
/// - `name`: A string **literal** for the snapshot name (not an expression)
/// - `doc`: A `ContractDocument` to snapshot
/// - `renderer` (optional): A custom `SnapshotRenderer`
///
/// # Example
///
/// ```ignore
/// use layered_contracts::snapshot::assert_contract_snapshot;
///
/// #[test]
/// fn test_my_contract() {
///     let doc = ContractDocument::from_text("Section 1.1 Test")
///         .run_resolver(&SectionHeaderResolver::new());
///     
///     assert_contract_snapshot!("my_test", doc);
/// }
/// ```
///
/// # Redaction
///
/// The macro automatically applies basic redaction to remove non-deterministic
/// content (like LLM pass IDs) from the RON snapshot. The view is rendered
/// from the original (non-redacted) snapshot to preserve semantic detail.
#[macro_export]
macro_rules! assert_contract_snapshot {
    ($name:literal, $doc:expr) => {{
        use $crate::snapshot::{Snapshot, SnapshotRenderer};
        
        let raw_snapshot = Snapshot::from_document(&$doc);
        let redacted_snapshot = raw_snapshot.clone().redact();
        let renderer = SnapshotRenderer::new();
        
        // Create RON data snapshot (redacted for stability)
        let ron_data = redacted_snapshot.to_ron_string().expect("RON serialization failed");
        insta::assert_snapshot!(concat!($name, "_data"), ron_data);
        
        // Create semantic view snapshot (from raw for full detail)
        let view = renderer.render_semantic(&raw_snapshot);
        insta::assert_snapshot!(concat!($name, "_view"), view);
    }};
    
    // Variant with custom renderer
    ($name:literal, $doc:expr, $renderer:expr) => {{
        use $crate::snapshot::Snapshot;
        
        let raw_snapshot = Snapshot::from_document(&$doc);
        let redacted_snapshot = raw_snapshot.clone().redact();
        
        // Create RON data snapshot (redacted for stability)
        let ron_data = redacted_snapshot.to_ron_string().expect("RON serialization failed");
        insta::assert_snapshot!(concat!($name, "_data"), ron_data);
        
        // Create semantic view snapshot (from raw for full detail)
        let view = $renderer.render_semantic(&raw_snapshot);
        insta::assert_snapshot!(concat!($name, "_view"), view);
    }};
}

/// Assert only the RON data snapshot (without view).
///
/// Useful when you only care about the canonical storage format.
/// The snapshot is redacted for stability.
///
/// # Arguments
///
/// - `name`: A string **literal** for the snapshot name
/// - `doc`: A `ContractDocument` to snapshot
#[macro_export]
macro_rules! assert_contract_snapshot_data {
    ($name:literal, $doc:expr) => {{
        use $crate::snapshot::Snapshot;
        
        let snapshot = Snapshot::from_document(&$doc).redact();
        let ron_data = snapshot.to_ron_string().expect("RON serialization failed");
        insta::assert_snapshot!($name, ron_data);
    }};
}

/// Assert only the semantic view snapshot (without RON).
///
/// Useful when you primarily care about the human-readable output.
/// The view is rendered from the non-redacted snapshot.
///
/// # Arguments
///
/// - `name`: A string **literal** for the snapshot name
/// - `doc`: A `ContractDocument` to snapshot
/// - `renderer` (optional): A custom `SnapshotRenderer`
#[macro_export]
macro_rules! assert_contract_snapshot_view {
    ($name:literal, $doc:expr) => {{
        use $crate::snapshot::{Snapshot, SnapshotRenderer};
        
        let snapshot = Snapshot::from_document(&$doc);
        let renderer = SnapshotRenderer::new();
        let view = renderer.render_semantic(&snapshot);
        insta::assert_snapshot!($name, view);
    }};
    
    ($name:literal, $doc:expr, $renderer:expr) => {{
        use $crate::snapshot::Snapshot;
        
        let snapshot = Snapshot::from_document(&$doc);
        let view = $renderer.render_semantic(&snapshot);
        insta::assert_snapshot!($name, view);
    }};
}

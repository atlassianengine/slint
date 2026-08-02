// Rust guideline compliant 2026-02-21

//! Background daemon services module.

pub mod capability_resolver;
pub mod search_runtime;
pub mod workspace_runtime;

#[expect(
    unused_imports,
    reason = "Public API re-exports for capability resolution and agent indexing"
)]
pub use capability_resolver::{
    build_native_workspace_index, format_manifest_uri, format_workspace_uri, parse_capability_uri,
    CapabilityTarget,
};
pub use search_runtime::{SearchRuntime, SearchRuntimeHandle};
pub use workspace_runtime::WorkspaceRuntime;

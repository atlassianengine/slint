// Rust guideline compliant 2026-02-21

//! Domain module grouping all AI and Agent-related controllers.

pub mod agent_room;
pub mod agent_runner;
pub mod mcp_router;
pub mod memory_indexer;
pub mod rag_search;

use crate::services::SearchRuntimeHandle;

/// Installs all AI sub-controllers.
pub fn install(app: &crate::AppWindow, search: SearchRuntimeHandle) {
    agent_runner::install(app);
    agent_room::install(app);
    rag_search::install(app, search.clone());
    memory_indexer::install(app, search);
    mcp_router::install(app);
}

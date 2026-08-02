// Rust guideline compliant 2026-02-21

//! Memory Indexer controller tracking background indexes.

use crate::services::SearchRuntimeHandle;

pub fn install(app: &crate::AppWindow, search: SearchRuntimeHandle) {
    let memory = search.clone();
    app.on_memory_index_requested(move |id, text| {
        let id = if id.trim().is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            id.to_string()
        };
        memory.upsert_memory(id, text.to_string());
    });
    let rebuild = search.clone();
    app.on_search_shadow_rebuild_requested(move || rebuild.rebuild());
}

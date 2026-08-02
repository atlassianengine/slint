// Rust guideline compliant 2026-02-21

//! RAG Search controller querying hybrid vector and graph database nodes.

use crate::services::SearchRuntimeHandle;
use slint::ComponentHandle;

pub fn install(app: &crate::AppWindow, search: SearchRuntimeHandle) {
    let weak = app.as_weak();
    app.on_rag_search_requested(move |query| {
        let weak = weak.clone();
        search.recall(query.to_string(), move |result| {
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade() {
                    match result {
                        Ok(hits) => {
                            app.set_rag_search_result_count(
                                hits.len().min(i32::MAX as usize) as i32
                            );
                            app.set_rag_search_preview(
                                hits.first()
                                    .map(|hit| hit.text.as_str())
                                    .unwrap_or("")
                                    .into(),
                            );
                        }
                        Err(error) => app
                            .set_rag_search_preview(format!("Search unavailable: {error}").into()),
                    }
                }
            });
        });
    });
}

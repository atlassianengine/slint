// Rust guideline compliant 2026-02-21

//! Service layer binding the background indexer thread pool with the Slint UI event loop.

use retrospect_indexer::{run_indexer_loop, IdleGate, SubspaceIndexerRegion};
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Handles the active background indexer threads and task channels.
pub struct IndexerService {
    _runtime: Runtime,
    region: Arc<SubspaceIndexerRegion>,
}

impl IndexerService {
    /// Starts the background indexing runtime and registers the filesystem monitoring loop.
    pub fn start() -> Self {
        let runtime = Runtime::new().expect("Failed to create Tokio background runtime");
        let (region, rx) = SubspaceIndexerRegion::spawn("default_subspace");
        let region = Arc::new(region);
        let idle_gate = region.idle_gate.clone();

        // Spawn the background indexing worker thread loop
        runtime.spawn(async move {
            run_indexer_loop(idle_gate, rx).await;
        });

        Self {
            _runtime: runtime,
            region,
        }
    }

    /// Triggers indexing catch-up for a modified file.
    pub fn notify_file_changed(&self, path: &str) {
        let region = self.region.clone();
        let path = path.to_owned();
        tokio::task::block_in_place(move || {
            let _ = futures::executor::block_on(region.notify_file_changed(path));
        });
    }
}

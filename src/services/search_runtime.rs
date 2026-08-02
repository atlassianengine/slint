//! Provides Slint with a Tokio-backed handle to the package-owned authority.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use document_retrieval::RetrospectMemoryChunkIngestRow;
use execution_workspace::FileChange;
use retrospect_persistence_authority::{
    PersistenceDocumentManifest, PersistenceFileIndexChange, PersistenceMemoryChunk,
    PersistenceSearchMaintenance, PersistenceSearchRuntime, PersistenceSearchRuntimeConfig,
    RecallHit, SearchShadowHealth,
};
use retrospect_persistence_contracts::{CanvasEdgeRecord, CanvasNodeWrite, CanvasSnapshot};
use slint::ComponentHandle;
use tokio::runtime::{Handle, Runtime};

const DEFAULT_TENANT_ID: &str = "default-tenant";
const DEFAULT_WORKSPACE_ID: &str = "default-workspace";
const LEDGER_DRAIN_INTERVAL: Duration = Duration::from_secs(3);

/// Keeps the native Tokio runtime alive for the application lifetime.
pub struct SearchRuntime {
    _runtime: Runtime,
    handle: SearchRuntimeHandle,
    maintenance: Option<PersistenceSearchMaintenance>,
}

/// Provides controller-safe access to the package-owned persistence authority.
#[derive(Clone)]
pub struct SearchRuntimeHandle {
    runtime: Handle,
    authority: Arc<PersistenceSearchRuntime>,
}

impl SearchRuntime {
    /// Opens the package-owned authority runtime and its search-shadow lanes.
    ///
    /// Slint owns only the Tokio executor and this typed handle. Database
    /// opening, writes, search-shadow maintenance, and file-index draining
    /// remain inside `retrospect-persistence-authority`.
    ///
    /// # Errors
    ///
    /// Returns an error when the Tokio runtime, authority database, or search
    /// shadow cannot be initialized.
    pub fn start() -> Result<Self, String> {
        let runtime = Runtime::new().map_err(|error| error.to_string())?;
        let config = PersistenceSearchRuntimeConfig::new(
            app_data_root()?,
            DEFAULT_TENANT_ID,
            DEFAULT_WORKSPACE_ID,
        )
        .map_err(|error| error.to_string())?;
        let authority = runtime
            .block_on(PersistenceSearchRuntime::open(config))
            .map_err(|error| error.to_string())?;
        let handle = SearchRuntimeHandle {
            runtime: runtime.handle().clone(),
            authority: Arc::new(authority),
        };
        Ok(Self {
            _runtime: runtime,
            handle,
            maintenance: None,
        })
    }

    /// Returns a cloneable controller handle.
    pub fn handle(&self) -> SearchRuntimeHandle {
        self.handle.clone()
    }

    /// Starts owned background maintenance.
    ///
    /// # Errors
    ///
    /// Returns an error if maintenance has already been started.
    pub fn start_background_drains(&mut self) -> Result<(), String> {
        if self.maintenance.is_some() {
            return Err("authority background maintenance already started".into());
        }
        self.maintenance = Some(
            self.handle
                .authority
                .start_background_drains(&self.handle.runtime),
        );
        Ok(())
    }
}

impl Drop for SearchRuntime {
    fn drop(&mut self) {
        if let Some(maintenance) = self.maintenance.take() {
            self._runtime.block_on(maintenance.shutdown());
        }
        if let Err(error) = self._runtime.block_on(self.handle.authority.flush()) {
            eprintln!("authority write flush failed: {error}");
        }
    }
}

impl SearchRuntimeHandle {
    /// Loads the durable renderer-neutral canvas snapshot.
    pub fn load_canvas_snapshot(&self) -> Result<CanvasSnapshot, String> {
        self.runtime
            .block_on(self.authority.load_canvas_snapshot())
            .map_err(|error| error.to_string())
    }

    /// Initializes the authority-owned canvas snapshot.
    pub fn initialize_canvas_snapshot(&self, snapshot: CanvasSnapshot) -> Result<(), String> {
        self.runtime
            .block_on(self.authority.initialize_canvas_snapshot(snapshot))
            .map_err(|error| error.to_string())
    }

    /// Queues a canvas node through the authority's serialized write lane.
    pub fn persist_canvas_node(&self, node: CanvasNodeWrite) {
        if let Err(error) = self.authority.try_upsert_canvas_node(node) {
            eprintln!("canvas node authority enqueue failed: {error}");
        }
    }

    /// Queues a complete canvas-edge replacement through the authority.
    pub fn persist_canvas_edges(&self, edges: Vec<CanvasEdgeRecord>) {
        if let Err(error) = self.authority.try_replace_canvas_edges(edges) {
            eprintln!("canvas edge authority enqueue failed: {error}");
        }
    }

    /// Queues a memory write through the authority-owned writer.
    pub fn upsert_memory(&self, id: String, text: String) {
        let authority = Arc::clone(&self.authority);
        self.runtime.spawn(async move {
            let normalized = text.to_ascii_lowercase();
            let _ = authority
                .upsert_memory(PersistenceMemoryChunk {
                    id: id.clone(),
                    workspace_id: DEFAULT_WORKSPACE_ID.into(),
                    text,
                    memory_kind: "document".into(),
                    source_type: "slint".into(),
                    source_id: id,
                    search_tokens: normalized.clone(),
                    ngram_target: normalized,
                })
                .await;
        });
    }

    /// Queues native document retrieval units through the authority.
    pub fn upsert_document_chunks(&self, chunks: Vec<RetrospectMemoryChunkIngestRow>) {
        let authority = Arc::clone(&self.authority);
        self.runtime.spawn(async move {
            for chunk in chunks {
                let _ = authority.upsert_retrieval_chunk(chunk).await;
            }
        });
    }

    /// Supersedes obsolete document retrieval units through the authority.
    pub fn supersede_document_chunks(&self, unit_ids: Vec<String>) {
        let authority = Arc::clone(&self.authority);
        self.runtime.spawn(async move {
            let _ = authority.supersede_retrieval_chunks(unit_ids).await;
        });
    }

    /// Persists a document agent manifest through the authority.
    pub fn upsert_document_manifest(
        &self,
        manifest_id: String,
        document_id: String,
        manifest_hash: String,
        structural_hash: String,
        manifest_json: String,
    ) {
        let authority = Arc::clone(&self.authority);
        self.runtime.spawn(async move {
            let _ = authority
                .upsert_document_manifest(PersistenceDocumentManifest {
                    manifest_id,
                    workspace_id: DEFAULT_WORKSPACE_ID.into(),
                    document_id,
                    manifest_hash,
                    structural_hash,
                    manifest_json,
                })
                .await;
        });
    }

    /// Runs authority-owned hybrid recall and returns the result to the UI.
    pub fn recall(
        &self,
        query: String,
        on_complete: impl FnOnce(Result<Vec<RecallHit>, String>) + Send + 'static,
    ) {
        let authority = Arc::clone(&self.authority);
        self.runtime.spawn(async move {
            on_complete(
                authority
                    .recall(&query)
                    .await
                    .map_err(|error| error.to_string()),
            );
        });
    }

    /// Rebuilds the authority-owned inactive search replica.
    pub fn rebuild(&self) {
        let authority = Arc::clone(&self.authority);
        self.runtime.spawn(async move {
            let _ = authority.rebuild().await;
        });
    }

    /// Queues a saved document for authority-owned background indexing.
    pub fn enqueue_file_change(&self, change: FileChange) {
        let authority = Arc::clone(&self.authority);
        self.runtime.spawn(async move {
            let _ = authority
                .enqueue_file_change(PersistenceFileIndexChange {
                    workspace_id: DEFAULT_WORKSPACE_ID.into(),
                    execution_workspace_id: "slint-editor".into(),
                    document_id: change.document_id.as_str().into(),
                    relative_path: change.path.as_path().to_string_lossy().into_owned(),
                    content_digest: change.digest,
                    operation: "upsert".into(),
                })
                .await;
        });
    }

    /// Publishes authority search health snapshots to the Slint event loop.
    pub fn bind_health(&self, app: &crate::AppWindow) {
        let authority = Arc::clone(&self.authority);
        let weak = app.as_weak();
        self.runtime.spawn(async move {
            loop {
                let health = authority.health().await.map_err(|error| error.to_string());
                let weak = weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = weak.upgrade() {
                        apply_health(&app, health);
                    }
                });
                tokio::time::sleep(LEDGER_DRAIN_INTERVAL).await;
            }
        });
    }
}

fn apply_health(app: &crate::AppWindow, health: Result<SearchShadowHealth, String>) {
    match health {
        Ok(status) => {
            let slot = status
                .active_slot
                .map(|slot| format!("{slot:?}"))
                .unwrap_or_default();
            app.set_search_shadow_status(if status.pending_mutations > 0 {
                "CATCHING UP".into()
            } else {
                "READY".into()
            });
            app.set_search_shadow_active_slot(slot.into());
            app.set_search_shadow_pending(status.pending_mutations.clamp(0, i32::MAX as i64) as i32);
            app.set_search_shadow_maintenance(status.maintenance_active);
        }
        Err(error) => {
            app.set_search_shadow_status(format!("OFFLINE: {error}").into());
        }
    }
}

fn app_data_root() -> Result<PathBuf, String> {
    if let Some(root) = std::env::var_os("RETROSPECT_APP_DATA_DIR") {
        return Ok(PathBuf::from(root));
    }
    if let Some(root) = std::env::var_os("APPDATA") {
        return Ok(PathBuf::from(root).join("Retrospect"));
    }
    if let Some(root) = std::env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(root).join("Retrospect"));
    }
    if let Some(root) = std::env::var_os("HOME") {
        let root = PathBuf::from(root);
        if cfg!(target_os = "macos") {
            return Ok(root
                .join("Library")
                .join("Application Support")
                .join("Retrospect"));
        }
        return Ok(root.join(".local").join("share").join("Retrospect"));
    }
    std::env::current_dir()
        .map(|root| root.join(".retrospect"))
        .map_err(|error| error.to_string())
}

// Rust guideline compliant 2026-02-21

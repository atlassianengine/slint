//! Owns the native Slint execution-workspace gateway.

use std::path::PathBuf;

use execution_workspace::{CapabilityGrant, ExecutionWorkspaceId, RelativePath, WorkspaceGateway};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use super::SearchRuntimeHandle;

const DEFAULT_TENANT_ID: &str = "default-tenant";
const DEFAULT_WORKSPACE_ID: &str = "default-workspace";
const EDITOR_EXECUTION_WORKSPACE_ID: &str = "slint-editor";
const EDITOR_DOCUMENT_PATH: &str = "documents/retrospect.rs";

/// Keeps the scoped native document gateway available to controllers.
pub struct WorkspaceRuntime {
    gateway: WorkspaceGateway,
    editor_document: RelativePath,
    _watcher: RecommendedWatcher,
}

impl WorkspaceRuntime {
    /// Creates the application-owned editor workspace and gateway.
    ///
    /// # Errors
    /// Returns an error when the app-data root or seed document cannot be prepared.
    pub fn start(search: SearchRuntimeHandle) -> Result<Self, String> {
        let root = app_data_root()?
            .join("tenants")
            .join(DEFAULT_TENANT_ID)
            .join("workspaces")
            .join(DEFAULT_WORKSPACE_ID)
            .join("execution-workspaces")
            .join(EDITOR_EXECUTION_WORKSPACE_ID);
        std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let editor_document =
            RelativePath::parse(EDITOR_DOCUMENT_PATH).map_err(|error| error.to_string())?;
        let document_path = root.join(editor_document.as_path());
        if !document_path.exists() {
            let parent = document_path
                .parent()
                .ok_or_else(|| "editor document has no parent".to_string())?;
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            std::fs::write(&document_path, seed_document()).map_err(|error| error.to_string())?;
        }
        let gateway = WorkspaceGateway::open(
            ExecutionWorkspaceId::parse(EDITOR_EXECUTION_WORKSPACE_ID)
                .map_err(|error| error.to_string())?,
            root,
            CapabilityGrant::editor(),
        )
        .map_err(|error| error.to_string())?;
        let watch_gateway = gateway.clone();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let Ok(event) = event else {
                    return;
                };
                if !event.kind.is_modify() && !event.kind.is_create() {
                    return;
                }
                for path in event.paths {
                    let Ok(relative) = path.strip_prefix(watch_gateway.root()) else {
                        continue;
                    };
                    let Ok(relative) = RelativePath::parse(relative) else {
                        continue;
                    };
                    if relative.as_path().starts_with(".retrospect") {
                        continue;
                    }
                    if relative
                        .as_path()
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with(".tmp"))
                    {
                        continue;
                    }
                    if let Ok(document) = watch_gateway.read_text(relative.clone()) {
                        search.enqueue_file_change(execution_workspace::FileChange {
                            document_id: document.id,
                            path: relative,
                            digest: document.digest,
                        });
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        watcher
            .watch(gateway.root(), RecursiveMode::Recursive)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            gateway,
            editor_document,
            _watcher: watcher,
        })
    }

    /// Returns the cloneable shared workspace gateway.
    pub fn gateway(&self) -> WorkspaceGateway {
        self.gateway.clone()
    }

    /// Returns the initial editor document path.
    pub fn editor_document(&self) -> RelativePath {
        self.editor_document.clone()
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
        return Ok(if cfg!(target_os = "macos") {
            root.join("Library")
                .join("Application Support")
                .join("Retrospect")
        } else {
            root.join(".local").join("share").join("Retrospect")
        });
    }
    std::env::current_dir()
        .map(|root| root.join(".retrospect"))
        .map_err(|error| error.to_string())
}

fn seed_document() -> &'static str {
    concat!(
        "fn describe_workspace(name: &str) -> String {\n",
        "    format!(\"Retrospect workspace: {name}\")\n",
        "}\n\n",
        "fn main() {\n",
        "    println!(\"{}\", describe_workspace(\"native\"));\n",
        "}\n",
    )
}

// Rust guideline compliant 2026-02-21

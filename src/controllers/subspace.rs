// Rust guideline compliant 2026-07-20

//! Subspace navigator controller — seeds the navigator panel with subspaces
//! and the file tree of the active subspace root, and handles subspace creation.

use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::path::Path;
use std::rc::Rc;

use crate::{AppWindow, NavigatorFileItem, SubspaceItem};

/// Installs the subspace navigator controller onto the main Slint `AppWindow`.
pub fn install(app: &AppWindow) {
    // Seed demo subspaces for the navigator panel
    let subspaces_model = Rc::new(VecModel::from(vec![
        SubspaceItem {
            id: "sp-retrospect".into(),
            name: "retrospect-slint".into(),
            path: "D:/App Dev/retrospect-slint".into(),
            active: true,
        },
        SubspaceItem {
            id: "sp-packages".into(),
            name: "packages".into(),
            path: "D:/App Dev/packages".into(),
            active: false,
        },
        SubspaceItem {
            id: "sp-canvas".into(),
            name: "retrospect-canvas".into(),
            path: "D:/App Dev/retrospect-canvas".into(),
            active: false,
        },
    ]));

    // Populate files from the active subspace root — owns the model for the
    // lifetime of the app; callbacks mutate it in-place via set_vec().
    let files_model = Rc::new(VecModel::from(scan_directory(
        "D:/App Dev/retrospect-slint",
        0,
        2,
    )));

    app.set_navigator_subspaces(ModelRc::from(subspaces_model.clone()));
    app.set_navigator_files(ModelRc::from(files_model.clone()));

    // When a subspace is selected, mark it active and refresh the file tree
    // by mutating the existing model in-place (no set_navigator_files call —
    // that would create a re-entrant borrow of Slint's internal RefCell).
    {
        let subspaces_model = subspaces_model.clone();
        let files_model = files_model.clone();
        app.on_navigator_subspace_selected(move |id| {
            let mut new_root = String::new();
            for idx in 0..subspaces_model.row_count() {
                if let Some(mut sp) = subspaces_model.row_data(idx) {
                    let is_selected = sp.id == id;
                    sp.active = is_selected;
                    subspaces_model.set_row_data(idx, sp.clone());
                    if is_selected {
                        new_root = sp.path.to_string();
                    }
                }
            }
            if !new_root.is_empty() {
                files_model.set_vec(scan_directory(&new_root, 0, 2));
            }
        });
    }

    // Handle file click — future: open in code/document editor
    app.on_navigator_file_clicked(|path| {
        println!("Navigator file clicked: {path}");
    });

    // Handle new subspace creation — mutate models in-place only
    {
        let subspaces_model = subspaces_model.clone();
        let files_model = files_model.clone();
        app.on_new_subspace_create_requested(move |name, path, _tenant, _user, _board| {
            let id: slint::SharedString = format!("sp-{}", name.to_lowercase().replace(' ', "-"))
                .as_str()
                .into();
            // Deactivate all existing subspaces
            for idx in 0..subspaces_model.row_count() {
                if let Some(mut sp) = subspaces_model.row_data(idx) {
                    sp.active = false;
                    subspaces_model.set_row_data(idx, sp);
                }
            }
            // Append new subspace
            subspaces_model.push(SubspaceItem {
                id: id.clone(),
                name: name.clone(),
                path: path.clone(),
                active: true,
            });
            // Refresh file tree in-place
            files_model.set_vec(scan_directory(&path.to_string(), 0, 2));
        });
    }

    // Native folder picker — rfd::FileDialog::pick_folder() is blocking and
    // must NOT call back into Slint setters while the Slint event loop holds
    // its RefCell. We spawn it on a thread and emit the result via
    // invoke_from_event_loop so the property write happens between frames.
    {
        let app_weak = app.as_weak();
        app.on_new_subspace_browse_folder_requested(move || {
            let app_weak = app_weak.clone();
            std::thread::spawn(move || {
                if let Some(dir) = rfd::FileDialog::new()
                    .set_title("Select subspace root folder")
                    .pick_folder()
                {
                    let picked = dir.to_string_lossy().replace('\\', "/");
                    let picked_shared: slint::SharedString = picked.as_str().into();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(app) = app_weak.upgrade() {
                            // Re-open the modal (it may have closed) and log path
                            app.set_new_subspace_modal_open(true);
                            println!("Folder selected: {picked_shared}");
                        }
                    });
                }
            });
        });
    }
}

/// Scans a directory tree up to `max_depth` and returns a flat sorted list
/// of `NavigatorFileItem` entries suitable for display in the file panel.
fn scan_directory(root: &str, depth: i32, max_depth: i32) -> Vec<NavigatorFileItem> {
    let mut items = Vec::new();
    let path = Path::new(root);

    let read_dir = match std::fs::read_dir(path) {
        Ok(rd) => rd,
        Err(_) => return items,
    };

    let mut entries: Vec<std::fs::DirEntry> = read_dir.flatten().collect();
    // Directories first, then files, both alphabetically
    entries.sort_by_key(|e| {
        let is_file = e.file_type().map(|t| t.is_file()).unwrap_or(true);
        (is_file, e.file_name())
    });

    for entry in entries {
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files and common build/cache dirs
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }

        let is_dir = entry.metadata().map(|m| m.is_dir()).unwrap_or(false);
        let path_str = entry_path.to_string_lossy().replace('\\', "/");

        items.push(NavigatorFileItem {
            name: name.as_str().into(),
            path: path_str.as_str().into(),
            is_dir,
            depth,
        });

        // Recurse into directories up to max_depth
        if is_dir && depth < max_depth {
            items.extend(scan_directory(&path_str, depth + 1, max_depth));
        }
    }

    items
}

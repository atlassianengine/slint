// Rust guideline compliant 2026-02-21

//! Unified native controllers module routing Slint UI callbacks directly to the backend crates.

pub mod agent_hub;
pub mod ai;
pub mod canvas;
pub mod canvas_properties;
pub mod data_views;
pub mod git_diff;
pub mod hub;
pub mod planner;
pub mod settings;
pub mod subspace;
pub mod workspace;

use crate::services::SearchRuntimeHandle;
use crate::AppWindow;

/// Installs native controllers with service-backed runtime handles only.
pub fn install_all(app: &AppWindow, search: SearchRuntimeHandle) {
    planner::install(app);
    canvas::install(app);
    ai::install(app, search.clone());
    workspace::install(app);
    settings::install(app);
    agent_hub::install(app);
    data_views::install(app);
    git_diff::install(app);
    canvas_properties::install(app);
    hub::install(app);
    subspace::install(app);
}

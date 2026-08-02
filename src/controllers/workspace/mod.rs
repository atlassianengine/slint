// Rust guideline compliant 2026-02-21

//! Domain module grouping all Document and Analytics controllers.

pub mod analytics_feed;
pub mod lexical_editor;

/// Installs all workspace sub-controllers.
pub fn install(app: &crate::AppWindow) {
    lexical_editor::install(app);
    analytics_feed::install(app);
}

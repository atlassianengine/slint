// Rust guideline compliant 2026-02-21

//! Domain module grouping all Whiteboard, Brush, and Geometry controllers.

pub mod boolean_export;
pub mod brush_tools;
pub mod whiteboard;

/// Installs all canvas sub-controllers.
pub fn install(app: &crate::AppWindow) {
    whiteboard::install(app);
    brush_tools::install(app);
    boolean_export::install(app);
}

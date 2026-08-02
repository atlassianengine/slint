// Rust guideline compliant 2026-02-21

use canvas_core::CanvasSession;

use crate::{color::accent_color, WireRoute};

use super::presentation::CanvasPresentation;

pub(super) fn rows(session: &CanvasSession, presentation: &CanvasPresentation) -> Vec<WireRoute> {
    rows_for_routes(session.graph().cubic_routes(), presentation)
}

pub(super) fn rows_for_node(
    session: &CanvasSession,
    id: &str,
    presentation: &CanvasPresentation,
) -> Vec<WireRoute> {
    rows_for_routes(
        session
            .graph()
            .cubic_routes()
            .into_iter()
            .filter(|route| route.from == id || route.to == id)
            .collect(),
        presentation,
    )
}

fn rows_for_routes(
    routes: Vec<canvas_core::CanvasRoute>,
    presentation: &CanvasPresentation,
) -> Vec<WireRoute> {
    routes
        .into_iter()
        .map(|route| {
            let source_accent = accent_color(&presentation.accent(&route.from));
            let target_accent = accent_color(&presentation.accent(&route.to));
            // Slint follows CSS's 90deg-right convention; canvas y grows down.
            let gradient_angle =
                (-((route.end.y - route.start.y).atan2(route.end.x - route.start.x))).to_degrees();

            WireRoute {
                from: route.from.into(),
                to: route.to.into(),
                commands: route.commands.into(),
                source_accent,
                target_accent,
                gradient_angle,
                min_x: route.min_x,
                min_y: route.min_y,
                width: route.width,
                height: route.height,
                end_angle: route.end_angle_deg,
                has_target_arrow: route.has_target_arrow,
            }
        })
        .collect()
}

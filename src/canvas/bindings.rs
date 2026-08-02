use std::{cell::RefCell, rc::Rc};

use canvas_core::{CanvasPortSide, CanvasRect, CanvasSession};
use slint::{ComponentHandle, DataTransfer, Model, SharedString};

use crate::services::SearchRuntimeHandle;
use crate::{canvas_backdrop::CanvasBackdrop, AppWindow};
use retrospect_persistence_contracts::CanvasEdgeRecord;

use super::presentation::CanvasPresentation;

/// UI intent bindings. Each callback mutates canvas-core first, then projects
/// only the state affected by that transition.
pub(super) fn install(
    app: &AppWindow,
    session: Rc<RefCell<CanvasSession>>,
    presentation: Rc<CanvasPresentation>,
    backdrop: CanvasBackdrop,
    search: SearchRuntimeHandle,
) {
    {
        let session = session.clone();
        let presentation = presentation.clone();
        let app_weak = app.as_weak();
        app.on_node_selected(move |id| {
            {
                let mut session = session.borrow_mut();
                session.select_node(&id);
                presentation.sync_nodes(&session);
            } // session borrow released before any app mutation

            // Defer surface switch to the next event-loop tick so this callback
            // has fully unwound before Slint re-borrows the component RefCell.
            if id == "doc-node" || id == "journal-node" {
                let app_weak = app_weak.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_weak.upgrade() {
                        app.set_active_surface(crate::WorkbenchSurface::Document);
                    }
                });
            }
        });
    }
    {
        let session = session.clone();
        let presentation = presentation.clone();
        app.on_node_dragged(move |id, screen_dx, screen_dy| {
            let mut session = session.borrow_mut();
            if session.move_node_by_screen(&id, screen_dx, screen_dy) {
                presentation.sync_node(&session, &id);
            }
        });
    }
    {
        let session = session.clone();
        let presentation = presentation.clone();
        let search = search.clone();
        app.on_node_drag_finished(move |id, screen_dx, screen_dy| {
            let mut session = session.borrow_mut();
            if session.move_node_by_screen(&id, screen_dx, screen_dy) {
                presentation.sync_node(&session, &id);
                presentation.sync_wires_for_node(&session, &id);
                if let Some(node) = session.graph().node(&id) {
                    search.persist_canvas_node(presentation.node_write(node));
                }
            }
        });
    }
    app.on_connection_drag_data(|id, side| {
        DataTransfer::from(SharedString::from(format!("{id}\t{side}")))
    });
    {
        let session = session.clone();
        let presentation = presentation.clone();
        let search = search.clone();
        app.on_connection_dropped(move |target_id, target_side, data| {
            let Ok(source) = data.plain_text() else {
                return;
            };
            let Some((source_id, source_side)) = parse_connection_source(&source) else {
                return;
            };
            let Some(target_side) = CanvasPortSide::from_str(&target_side) else {
                return;
            };
            let mut session = session.borrow_mut();
            if session.begin_connection_from(source_id, source_side)
                && session.complete_connection_at(&target_id, target_side)
            {
                presentation.refresh(&session);
                search.persist_canvas_edges(
                    session
                        .graph()
                        .edges()
                        .iter()
                        .map(|edge| CanvasEdgeRecord {
                            from_node: edge.from.clone(),
                            to_node: edge.to.clone(),
                            from_port: port_side_name(edge.from_port).to_owned(),
                            to_port: port_side_name(edge.to_port).to_owned(),
                        })
                        .collect(),
                );
            }
        });
    }
    {
        let session = session.clone();
        app.on_connection_begin(move |id, side| {
            if let Some(side) = CanvasPortSide::from_str(&side) {
                session.borrow_mut().begin_connection_from(&id, side);
            }
        });
    }
    {
        let session = session.clone();
        let presentation = presentation.clone();
        app.on_box_selected(move |x, y, width, height| {
            let mut session = session.borrow_mut();
            session.select_screen_rect(CanvasRect {
                x,
                y,
                width,
                height,
            });
            presentation.refresh(&session);
        });
    }
    {
        let session = session.clone();
        let presentation = presentation.clone();
        let backdrop = backdrop.clone();
        let callback_app = app.as_weak();
        app.on_camera_panned(move |delta_x, delta_y| {
            let mut session = session.borrow_mut();
            session.pan_by_screen(delta_x, delta_y);
            if let Some(app) = callback_app.upgrade() {
                presentation.sync_camera(&app, session.camera());
                backdrop.sync_camera(session.camera());
                app.window().request_redraw();
            }
        });
    }
    {
        let presentation = presentation.clone();
        let backdrop = backdrop.clone();
        let callback_app = app.as_weak();
        app.on_zoom_requested(move |delta| {
            let mut session = session.borrow_mut();
            session.zoom_by(delta);
            if let Some(app) = callback_app.upgrade() {
                // Projection remains a camera-only change, like the prior
                // adapter: no node or wire model is rebuilt for zoom.
                presentation.sync_camera(&app, session.camera());
                backdrop.sync_camera(session.camera());
                app.window().request_redraw();
            }
        });
    }
}

fn port_side_name(side: CanvasPortSide) -> &'static str {
    match side {
        CanvasPortSide::Left => "left",
        CanvasPortSide::Right => "right",
        CanvasPortSide::Top => "top",
        CanvasPortSide::Bottom => "bottom",
    }
}

fn parse_connection_source(source: &str) -> Option<(&str, CanvasPortSide)> {
    let (id, side) = source.split_once('\t')?;
    Some((id, CanvasPortSide::from_str(side)?))
}

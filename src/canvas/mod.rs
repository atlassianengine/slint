mod appearance;
mod bindings;
mod fixture;
mod persistence;
mod presentation;
mod wire_routes;

use std::{cell::RefCell, rc::Rc};

use canvas_core::CanvasSession;

use crate::{canvas_backdrop::CanvasBackdrop, services::SearchRuntimeHandle, AppWindow};

pub fn install(
    app: &AppWindow,
    backdrop: CanvasBackdrop,
    search: &SearchRuntimeHandle,
) -> Result<(), String> {
    appearance::install(app);
    let snapshot = match persistence::load_snapshot(search)? {
        Some(mut snapshot) => {
            if fixture::upgrade_legacy_descriptors(&mut snapshot) {
                persistence::initialize_snapshot(search, snapshot.clone())?;
            }
            snapshot
        }
        None => {
            // Fixtures seed only a new workspace; a nonempty durable graph is authoritative.
            let snapshot = fixture::snapshot();
            persistence::initialize_snapshot(search, snapshot.clone())?;
            snapshot
        }
    };
    let graph = persistence::graph_from(&snapshot);
    let session = Rc::new(RefCell::new(CanvasSession::new(graph)));
    let presentation = Rc::new(presentation::CanvasPresentation::new(snapshot.nodes));

    presentation.attach(app);
    presentation.refresh(&session.borrow());
    presentation.sync_camera(app, session.borrow().camera());
    backdrop.sync_camera(session.borrow().camera());
    bindings::install(app, session, presentation, backdrop, search.clone());
    Ok(())
}

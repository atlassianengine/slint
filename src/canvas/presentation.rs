use std::{cell::RefCell, collections::HashMap, rc::Rc};

use canvas_core::{CanvasCamera, CanvasNode, CanvasPortSide, CanvasSession};
use slint::{Model, ModelRc, SharedString, VecModel};

use crate::{color::accent_color, AppWindow, Node, WireRoute};
use retrospect_persistence_contracts::{CanvasNodeRecord, CanvasNodeWrite};

use super::wire_routes;

/// Mutable Slint models and the deterministic projection from canvas-core.
/// Domain state remains in CanvasSession; this type owns no graph state.
pub(super) struct CanvasPresentation {
    nodes: Rc<VecModel<Node>>,
    wires: Rc<VecModel<WireRoute>>,
    descriptors: RefCell<HashMap<String, CanvasNodeRecord>>,
}

impl CanvasPresentation {
    pub(super) fn new(descriptors: Vec<CanvasNodeRecord>) -> Self {
        Self {
            nodes: Rc::new(VecModel::from(Vec::new())),
            wires: Rc::new(VecModel::from(Vec::new())),
            descriptors: RefCell::new(
                descriptors
                    .into_iter()
                    .map(|node| (node.id.clone(), node))
                    .collect(),
            ),
        }
    }

    pub(super) fn attach(&self, app: &AppWindow) {
        app.set_nodes(ModelRc::from(self.nodes.clone()));
        app.set_wire_routes(ModelRc::from(self.wires.clone()));
    }

    pub(super) fn refresh(&self, session: &CanvasSession) {
        self.nodes.set_vec(
            session
                .graph()
                .nodes()
                .iter()
                .map(|node| self.node_row(node, session))
                .collect::<Vec<_>>(),
        );
        self.sync_wires(session);
    }

    pub(super) fn sync_node(&self, session: &CanvasSession, id: &str) {
        let Some(node) = session.graph().node(id) else {
            return;
        };
        for row in 0..self.nodes.row_count() {
            if self
                .nodes
                .row_data(row)
                .is_some_and(|existing| existing.id == id)
            {
                self.nodes.set_row_data(row, self.node_row(node, session));
                return;
            }
        }
    }

    pub(super) fn sync_nodes(&self, session: &CanvasSession) {
        let ids = session
            .graph()
            .nodes()
            .iter()
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            self.sync_node(session, &id);
        }
    }

    pub(super) fn sync_wires(&self, session: &CanvasSession) {
        self.wires.set_vec(wire_routes::rows(session, self));
    }

    pub(super) fn sync_wires_for_node(&self, session: &CanvasSession, id: &str) {
        let mut updates = wire_routes::rows_for_node(session, id, self).into_iter();

        for row in 0..self.wires.row_count() {
            let Some(existing) = self.wires.row_data(row) else {
                continue;
            };
            if existing.from != id && existing.to != id {
                continue;
            }
            let Some(wire) = updates.next() else {
                return;
            };
            self.wires.set_row_data(row, wire);
        }
    }

    pub(super) fn sync_camera(&self, app: &AppWindow, camera: CanvasCamera) {
        app.set_camera_x(camera.x);
        app.set_camera_y(camera.y);
        app.set_zoom(camera.zoom);
    }

    pub(super) fn node_write(&self, node: &CanvasNode) -> CanvasNodeWrite {
        let descriptors = self.descriptors.borrow();
        let descriptor = descriptors.get(&node.id);
        CanvasNodeWrite {
            id: node.id.clone(),
            kind: descriptor.map_or_else(|| "unknown".into(), |value| value.kind.clone()),
            title: descriptor.map_or_else(|| node.id.clone(), |value| value.title.clone()),
            subtitle: descriptor
                .map_or_else(|| "Unknown node".into(), |value| value.subtitle.clone()),
            accent_hex: descriptor
                .map_or_else(|| "#64748b".into(), |value| value.accent_hex.clone()),
            running: descriptor.is_some_and(|value| value.running),
            left_port_enabled: descriptor.map_or(true, |value| value.left_port_enabled),
            right_port_enabled: descriptor.map_or(true, |value| value.right_port_enabled),
            top_port_enabled: descriptor.map_or(true, |value| value.top_port_enabled),
            bottom_port_enabled: descriptor.map_or(true, |value| value.bottom_port_enabled),
            x: f64::from(node.bounds.x),
            y: f64::from(node.bounds.y),
            width: f64::from(node.bounds.width),
            height: f64::from(node.bounds.height),
        }
    }

    pub(super) fn accent(&self, id: &str) -> String {
        self.descriptors
            .borrow()
            .get(id)
            .map_or_else(|| "#64748b".into(), |value| value.accent_hex.clone())
    }

    fn node_row(&self, node: &CanvasNode, session: &CanvasSession) -> Node {
        let descriptors = self.descriptors.borrow();
        node_row(node, descriptors.get(&node.id), session)
    }
}

fn node_row(
    node: &CanvasNode,
    presentation: Option<&CanvasNodeRecord>,
    session: &CanvasSession,
) -> Node {
    let kind = presentation.map_or("unknown", |value| value.kind.as_str());
    let title = presentation.map_or(node.id.as_str(), |value| value.title.as_str());
    let subtitle = presentation.map_or("Unknown node", |value| value.subtitle.as_str());
    let accent = presentation.map_or("#64748b", |value| value.accent_hex.as_str());
    Node {
        id: SharedString::from(node.id.as_str()),
        kind: SharedString::from(kind),
        title: SharedString::from(title),
        subtitle: SharedString::from(subtitle),
        x: node.bounds.x,
        y: node.bounds.y,
        width: node.bounds.width,
        height: node.bounds.height,
        accent: accent_color(accent),
        selected: session.is_selected(&node.id),
        running: presentation.is_some_and(|value| value.running),
        left_port_enabled: presentation.map_or(true, |value| value.left_port_enabled),
        right_port_enabled: presentation.map_or(true, |value| value.right_port_enabled),
        top_port_enabled: presentation.map_or(true, |value| value.top_port_enabled),
        bottom_port_enabled: presentation.map_or(true, |value| value.bottom_port_enabled),
        left_connected: session
            .graph()
            .port_is_connected(&node.id, CanvasPortSide::Left),
        right_connected: session
            .graph()
            .port_is_connected(&node.id, CanvasPortSide::Right),
        top_connected: session
            .graph()
            .port_is_connected(&node.id, CanvasPortSide::Top),
        bottom_connected: session
            .graph()
            .port_is_connected(&node.id, CanvasPortSide::Bottom),
    }
}

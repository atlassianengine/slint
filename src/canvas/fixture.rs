use canvas_core::CanvasPortSide;
use retrospect_persistence_contracts::{CanvasEdgeRecord, CanvasNodeRecord, CanvasSnapshot};
use retrospect_node_contracts::{parity_fixture, NodePresentation};
use std::sync::OnceLock;

#[derive(Clone)]
pub struct FixtureNodePresentation {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub accent: String,
    pub running: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub left_port_enabled: bool,
    pub right_port_enabled: bool,
    pub top_port_enabled: bool,
    pub bottom_port_enabled: bool,
}

static PRESENTATIONS: OnceLock<Vec<FixtureNodePresentation>> = OnceLock::new();

fn presentations() -> &'static [FixtureNodePresentation] {
    PRESENTATIONS.get_or_init(|| {
        let fixture = parity_fixture();
        fixture
            .presentations
            .iter()
            .map(|presentation| project_presentation(presentation, &fixture))
            .collect()
    })
}

fn project_presentation(presentation: &NodePresentation, fixture: &retrospect_node_contracts::ParityFixture) -> FixtureNodePresentation {
    let contract = fixture.nodes.iter().find(|node| node.type_id == presentation.type_id);
    FixtureNodePresentation {
        id: presentation.id.clone(),
        kind: presentation.type_id.clone(),
        title: presentation.title.clone(),
        subtitle: presentation.subtitle.clone(),
        accent: presentation.accent.clone(),
        running: presentation.running,
        x: presentation.x,
        y: presentation.y,
        width: presentation.width,
        height: presentation.height,
        left_port_enabled: contract.is_some_and(|node| !node.input_data_ports.is_empty()),
        right_port_enabled: contract.is_some_and(|node| !node.output_data_ports.is_empty()),
        top_port_enabled: false,
        bottom_port_enabled: false,
    }
}

pub fn snapshot() -> CanvasSnapshot {
    CanvasSnapshot {
        nodes: presentations().iter().map(node).collect(),
        edges: vec![
            edge("research", "thesis"),
            edge("thesis", "agent"),
            edge("agent", "review"),
            edge("agent", "risk"),
            edge("risk", "ship"),
            edge("review", "ship"),
            edge("notes", "risk"),
            edge("doc-node", "journal-node"),
            edge("journal-node", "task-node"),
        ],
    }
}

pub fn upgrade_legacy_descriptors(snapshot: &mut CanvasSnapshot) -> bool {
    let mut changed = false;
    for node in &mut snapshot.nodes {
        if node.kind != "unknown" || !node.title.is_empty() {
            continue;
        }
        let Some(fixture) = presentation(&node.id) else {
            continue;
        };
        node.kind = fixture.kind.clone();
        node.title = fixture.title.clone();
        node.subtitle = fixture.subtitle.clone();
        node.accent_hex = fixture.accent.clone();
        node.running = fixture.running;
        node.left_port_enabled = fixture.left_port_enabled;
        node.right_port_enabled = fixture.right_port_enabled;
        node.top_port_enabled = fixture.top_port_enabled;
        node.bottom_port_enabled = fixture.bottom_port_enabled;
        changed = true;
    }
    changed
}

pub fn presentation(id: &str) -> Option<&'static FixtureNodePresentation> {
    presentations().iter().find(|presentation| presentation.id == id)
}

fn node(presentation: &FixtureNodePresentation) -> CanvasNodeRecord {
    CanvasNodeRecord {
        id: presentation.id.clone(),
        kind: presentation.kind.clone(),
        title: presentation.title.clone(),
        subtitle: presentation.subtitle.clone(),
        accent_hex: presentation.accent.clone(),
        running: presentation.running,
        x: f64::from(presentation.x),
        y: f64::from(presentation.y),
        width: f64::from(presentation.width),
        height: f64::from(presentation.height),
        left_port_enabled: presentation.left_port_enabled,
        right_port_enabled: presentation.right_port_enabled,
        top_port_enabled: presentation.top_port_enabled,
        bottom_port_enabled: presentation.bottom_port_enabled,
    }
}

fn edge(from: &str, to: &str) -> CanvasEdgeRecord {
    let (from_port, to_port) = match (from, to) {
        ("agent", "risk") | ("review", "ship") => (CanvasPortSide::Bottom, CanvasPortSide::Top),
        _ => (CanvasPortSide::Right, CanvasPortSide::Left),
    };
    CanvasEdgeRecord {
        from_node: from.to_owned(),
        to_node: to.to_owned(),
        from_port: port_side_name(from_port).to_owned(),
        to_port: port_side_name(to_port).to_owned(),
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
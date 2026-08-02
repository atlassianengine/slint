use canvas_core::{CanvasEdge, CanvasGraph, CanvasNode, CanvasPortSide, CanvasRect};
use retrospect_persistence_contracts::CanvasSnapshot;

use crate::services::SearchRuntimeHandle;

pub(super) fn load_snapshot(
    search: &SearchRuntimeHandle,
) -> Result<Option<CanvasSnapshot>, String> {
    let snapshot = search.load_canvas_snapshot()?;
    if snapshot.nodes.is_empty() {
        if !snapshot.edges.is_empty() {
            return Err("persisted canvas contains edges but no nodes".into());
        }
        return Ok(None);
    }
    Ok(Some(snapshot))
}

pub(super) fn graph_from(snapshot: &CanvasSnapshot) -> CanvasGraph {
    let nodes = snapshot
        .nodes
        .iter()
        .map(|node| CanvasNode {
            id: node.id.clone(),
            bounds: CanvasRect {
                x: node.x as f32,
                y: node.y as f32,
                width: node.width.max(1.0) as f32,
                height: node.height.max(1.0) as f32,
            },
        })
        .collect();
    let edges = snapshot
        .edges
        .iter()
        .map(|edge| CanvasEdge {
            from: edge.from_node.clone(),
            to: edge.to_node.clone(),
            from_port: CanvasPortSide::from_str(&edge.from_port).expect("validated port"),
            to_port: CanvasPortSide::from_str(&edge.to_port).expect("validated port"),
        })
        .collect();
    CanvasGraph::new(nodes, edges)
}

pub(super) fn initialize_snapshot(
    search: &SearchRuntimeHandle,
    snapshot: CanvasSnapshot,
) -> Result<(), String> {
    search.initialize_canvas_snapshot(snapshot)
}

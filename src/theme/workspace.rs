//! Retrospect-only canvas and glass extension of the shared UIKit contract.
//!
//! These are deliberately not generic `Colors`: they describe the workbench
//! canvas, navigator, and glass treatment which do not exist in every app.

use slint::ComponentHandle;
use ui_kit_slint::Oklch;

use crate::{AppWindow, WorkspacePalette};

#[derive(Clone, Copy)]
struct WorkspacePack {
    workspace_glow: Oklch,
    glass_fill: Oklch,
    glass_border: Oklch,
    glass_blur: f32,
    hover_fill: Oklch,
    active_fill: Oklch,
    active_fill_muted: Oklch,
    active_fill_quiet: Oklch,
    navigator_fill: Oklch,
    navigator_selection: Oklch,
    canvas_input: Oklch,
    canvas_border: Oklch,
    canvas_grid_dot: Oklch,
    canvas_grid_glow: Oklch,
    connection_halo: Oklch,
    selection_fill: Oklch,
    node_card_fill: Oklch,
    node_card_border: Oklch,
    node_card_border_hover: Oklch,
    node_card_glint: Oklch,
    node_card_glint_hover: Oklch,
    node_card_shadow: Oklch,
    node_card_shadow_hover: Oklch,
}

const DARK: WorkspacePack = WorkspacePack {
    workspace_glow: Oklch::new(0.14, 0.002, 285.0),
    // Rails need enough translucency for the native scene capture behind them
    // to remain legible; an almost opaque fill makes backdrop blur invisible.
    glass_fill: Oklch::new(0.16, 0.006, 285.0).with_alpha(0.62),
    glass_border: Oklch::new(0.0, 0.0, 0.0).with_alpha(0.82),
    glass_blur: 18.0,
    hover_fill: Oklch::new(1.0, 0.0, 0.0).with_alpha(0.05),
    active_fill: Oklch::new(0.64, 0.145, 155.0),
    active_fill_muted: Oklch::new(0.64, 0.145, 155.0).with_alpha(0.60),
    active_fill_quiet: Oklch::new(0.64, 0.145, 155.0).with_alpha(0.40),
    navigator_fill: Oklch::new(0.17, 0.005, 285.0).with_alpha(0.95),
    navigator_selection: Oklch::new(0.21, 0.005, 285.0),
    canvas_input: Oklch::new(0.14, 0.002, 285.0),
    canvas_border: Oklch::new(0.29, 0.006, 285.0),
    // Original Retrospect's dot field is near-neutral violet, not blue. The
    // brighter role is only consumed while the pointer field is active.
    canvas_grid_dot: Oklch::new(0.42, 0.010, 285.0),
    canvas_grid_glow: Oklch::new(0.72, 0.065, 285.0),
    connection_halo: Oklch::new(0.63, 0.2, 260.0).with_alpha(0.20),
    selection_fill: Oklch::new(0.63, 0.2, 260.0).with_alpha(0.13),
    node_card_fill: Oklch::new(0.17, 0.006, 285.0).with_alpha(0.95),
    node_card_border: Oklch::new(0.0, 0.0, 0.0).with_alpha(0.76),
    node_card_border_hover: Oklch::new(0.0, 0.0, 0.0).with_alpha(0.90),
    node_card_glint: Oklch::new(1.0, 0.0, 0.0).with_alpha(0.055),
    node_card_glint_hover: Oklch::new(1.0, 0.0, 0.0).with_alpha(0.08),
    node_card_shadow: Oklch::new(0.0, 0.0, 0.0).with_alpha(0.20),
    node_card_shadow_hover: Oklch::new(0.0, 0.0, 0.0).with_alpha(0.26),
};

const LIGHT: WorkspacePack = WorkspacePack {
    workspace_glow: Oklch::new(0.98, 0.004, 80.0),
    glass_fill: Oklch::new(1.0, 0.0, 0.0).with_alpha(0.85),
    glass_border: Oklch::new(0.0, 0.0, 0.0).with_alpha(0.18),
    glass_blur: 18.0,
    hover_fill: Oklch::new(0.0, 0.0, 0.0).with_alpha(0.04),
    active_fill: Oklch::new(0.64, 0.145, 155.0),
    active_fill_muted: Oklch::new(0.64, 0.145, 155.0).with_alpha(0.60),
    active_fill_quiet: Oklch::new(0.64, 0.145, 155.0).with_alpha(0.40),
    navigator_fill: Oklch::new(0.975, 0.004, 80.0),
    navigator_selection: Oklch::new(0.94, 0.006, 80.0),
    canvas_input: Oklch::new(0.98, 0.004, 80.0),
    canvas_border: Oklch::new(0.84, 0.008, 80.0),
    canvas_grid_dot: Oklch::new(0.48, 0.010, 285.0),
    canvas_grid_glow: Oklch::new(0.58, 0.060, 285.0),
    connection_halo: Oklch::new(0.50, 0.18, 260.0).with_alpha(0.25),
    selection_fill: Oklch::new(0.50, 0.18, 260.0).with_alpha(0.12),
    node_card_fill: Oklch::new(1.0, 0.0, 0.0).with_alpha(0.90),
    node_card_border: Oklch::new(0.0, 0.0, 0.0).with_alpha(0.16),
    node_card_border_hover: Oklch::new(0.0, 0.0, 0.0).with_alpha(0.24),
    node_card_glint: Oklch::new(1.0, 0.0, 0.0).with_alpha(0.56),
    node_card_glint_hover: Oklch::new(1.0, 0.0, 0.0).with_alpha(0.66),
    node_card_shadow: Oklch::new(0.0, 0.0, 0.0).with_alpha(0.06),
    node_card_shadow_hover: Oklch::new(0.0, 0.0, 0.0).with_alpha(0.10),
};

fn color(token: Oklch) -> slint::Color {
    slint::Color::from_oklch(token.lightness, token.chroma, token.hue, token.alpha)
}

pub(super) fn apply(app: &AppWindow, appearance: &str) -> Result<(), String> {
    let pack = match appearance {
        "dark" => DARK,
        "light" => LIGHT,
        _ => {
            return Err(format!(
                "unknown Retrospect workspace appearance: {appearance}"
            ))
        }
    };
    let palette = app.global::<WorkspacePalette>();
    palette.set_workspace_glow(color(pack.workspace_glow));
    palette.set_glass_fill(color(pack.glass_fill));
    palette.set_glass_border(color(pack.glass_border));
    palette.set_glass_blur(pack.glass_blur);
    palette.set_hover_fill(color(pack.hover_fill));
    palette.set_active_fill(color(pack.active_fill));
    palette.set_active_fill_muted(color(pack.active_fill_muted));
    palette.set_active_fill_quiet(color(pack.active_fill_quiet));
    palette.set_navigator_fill(color(pack.navigator_fill));
    palette.set_navigator_selection(color(pack.navigator_selection));
    palette.set_canvas_input(color(pack.canvas_input));
    palette.set_canvas_border(color(pack.canvas_border));
    palette.set_canvas_grid_dot(color(pack.canvas_grid_dot));
    palette.set_canvas_grid_glow(color(pack.canvas_grid_glow));
    palette.set_connection_halo(color(pack.connection_halo));
    palette.set_selection_fill(color(pack.selection_fill));
    palette.set_node_card_fill(color(pack.node_card_fill));
    palette.set_node_card_border(color(pack.node_card_border));
    palette.set_node_card_border_hover(color(pack.node_card_border_hover));
    palette.set_node_card_glint(color(pack.node_card_glint));
    palette.set_node_card_glint_hover(color(pack.node_card_glint_hover));
    palette.set_node_card_shadow(color(pack.node_card_shadow));
    palette.set_node_card_shadow_hover(color(pack.node_card_shadow_hover));
    Ok(())
}

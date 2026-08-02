mod foundation;
mod packs;
mod workspace;

use slint::{ComponentHandle, SharedString};
use ui_kit_slint::{Oklch, ThemePack};

use crate::{
    AppWindow, Borders, Colors, Controls, Focus, Motion, Overlays, Shadows, Skeletons,
    ThemeSelection, Typography,
};

pub fn apply(app: &AppWindow) {
    apply_selection(app, "retrospect", "dark")
        .expect("Retrospect's bundled dark theme pack must be present");
}

/// Applies an explicitly resolved Retrospect-owned pack. Persistence and
/// selection UI remain product concerns; this boundary maps a chosen pack to
/// shared globals and Retrospect's workspace extension.
pub fn apply_selection(app: &AppWindow, theme_id: &str, appearance: &str) -> Result<(), String> {
    let pack = resolve(theme_id, appearance)
        .ok_or_else(|| format!("unknown Retrospect theme: {theme_id}/{appearance}"))?;
    apply_pack(app, &pack);
    Ok(())
}

fn resolve(theme_id: &str, appearance: &str) -> Option<ThemePack> {
    packs::resolve(theme_id, appearance)
}

fn color(token: Oklch) -> slint::Color {
    slint::Color::from_oklch(token.lightness, token.chroma, token.hue, token.alpha)
}

fn push_metrics(app: &AppWindow, pack: &ThemePack) {
    let metrics = pack.metrics;
    let borders = app.global::<Borders>();
    borders.set_width_thin(metrics.borders.width_thin);
    borders.set_width(metrics.borders.width);
    borders.set_width_normal(metrics.borders.width_normal);
    borders.set_width_strong(metrics.borders.width_strong);
    borders.set_radius_none(metrics.borders.radius_none);
    borders.set_radius_xs(metrics.borders.radius_xs);
    borders.set_radius_sm(metrics.borders.radius_sm);
    borders.set_radius_md(metrics.borders.radius_md);
    borders.set_radius_lg(metrics.borders.radius_lg);
    borders.set_radius_xl(metrics.borders.radius_xl);
    borders.set_radius_2xl(metrics.borders.radius_2xl);
    borders.set_radius_pill(metrics.borders.radius_pill);
    borders.set_radius_button(metrics.borders.radius_button);
    borders.set_radius_card(metrics.borders.radius_card);
    borders.set_radius_input(metrics.borders.radius_input);
    borders.set_radius_modal(metrics.borders.radius_modal);
    borders.set_radius_popover(metrics.borders.radius_popover);

    let motion = app.global::<Motion>();
    motion.set_duration_instant(metrics.motion.duration_instant);
    motion.set_duration_fast(metrics.motion.duration_fast);
    motion.set_duration_base(metrics.motion.duration_base);
    motion.set_duration_moderate(metrics.motion.duration_moderate);
    motion.set_duration_slow(metrics.motion.duration_slow);
    motion.set_duration_crawl(metrics.motion.duration_crawl);
    motion.set_scale_press(metrics.motion.scale_press);
    motion.set_scale_press_subtle(metrics.motion.scale_press_subtle);
    motion.set_scale_hover(metrics.motion.scale_hover);

    let shadows = app.global::<Shadows>();
    shadows.set_card_blur(metrics.shadows.card_blur);
    shadows.set_card_offset_y(metrics.shadows.card_offset_y);
    shadows.set_card_opacity(metrics.shadows.card_opacity);
    shadows.set_overlay_blur(metrics.shadows.overlay_blur);
    shadows.set_overlay_offset_y(metrics.shadows.overlay_offset_y);
    shadows.set_overlay_opacity(metrics.shadows.overlay_opacity);
    shadows.set_modal_blur(metrics.shadows.modal_blur);
    shadows.set_modal_offset_y(metrics.shadows.modal_offset_y);
    shadows.set_modal_opacity(metrics.shadows.modal_opacity);
    shadows.set_glow_md_blur(metrics.shadows.glow_md_blur);
    shadows.set_glow_md_opacity(metrics.shadows.glow_md_opacity);
    shadows.set_glow_lg_blur(metrics.shadows.glow_lg_blur);
    shadows.set_glow_lg_opacity(metrics.shadows.glow_lg_opacity);

    let controls = app.global::<Controls>();
    controls.set_button_height_xs(metrics.controls.button_height_xs);
    controls.set_button_height_sm(metrics.controls.button_height_sm);
    controls.set_button_height_md(metrics.controls.button_height_md);
    controls.set_button_height_lg(metrics.controls.button_height_lg);
    controls.set_input_height(metrics.controls.input_height);
    controls.set_select_height(metrics.controls.select_height);
    controls.set_icon_size_sm(metrics.controls.icon_size_sm);
    controls.set_icon_size_md(metrics.controls.icon_size_md);
    controls.set_icon_size_lg(metrics.controls.icon_size_lg);

    let focus = app.global::<Focus>();
    focus.set_ring_width(metrics.focus.ring_width);
    focus.set_ring_offset(metrics.focus.ring_offset);

    let overlays = app.global::<Overlays>();
    overlays.set_scrim_subtle_opacity(metrics.overlays.scrim_subtle_opacity);
    overlays.set_scrim_opacity(metrics.overlays.scrim_opacity);
    overlays.set_scrim_strong_opacity(metrics.overlays.scrim_strong_opacity);

    let skeleton = app.global::<Skeletons>();
    skeleton.set_base_opacity(metrics.skeleton.base_opacity);
    skeleton.set_highlight_opacity(metrics.skeleton.highlight_opacity);
    skeleton.set_shimmer_duration(metrics.skeleton.shimmer_duration);
    skeleton.set_pulse_duration(metrics.skeleton.pulse_duration);
}

fn apply_pack(app: &AppWindow, pack: &ThemePack) {
    let selection = app.global::<ThemeSelection>();
    selection.set_brand_id(SharedString::from(pack.brand_id));
    selection.set_theme_id(SharedString::from(pack.theme_id));
    selection.set_theme_name(SharedString::from(pack.theme_name));
    selection.set_appearance(SharedString::from(pack.appearance));

    let colors = app.global::<Colors>();
    colors.set_paper_body(color(pack.paper_body));
    colors.set_paper_surface(color(pack.paper_surface));
    colors.set_paper_raised(color(pack.paper_raised));
    colors.set_paper_sunken(color(pack.paper_sunken));
    colors.set_paper_chrome(color(pack.paper_chrome));
    colors.set_ink_headers(color(pack.ink_headers));
    colors.set_ink_subheaders(color(pack.ink_subheaders));
    colors.set_ink_body(color(pack.ink_body));
    colors.set_ink_info(color(pack.ink_info));
    colors.set_ink_muted(color(pack.ink_muted));
    colors.set_ink_disabled(color(pack.ink_disabled));
    colors.set_ink_hover(color(pack.ink_hover));
    colors.set_ink_inverse(color(pack.ink_inverse));
    colors.set_primary(color(pack.primary));
    colors.set_secondary(color(pack.secondary));
    colors.set_accent(color(pack.accent));
    colors.set_status_success(color(pack.status_success));
    colors.set_status_warning(color(pack.status_warning));
    colors.set_status_critical(color(pack.status_critical));
    colors.set_status_info(color(pack.status_info));
    colors.set_status_caution(color(pack.status_caution));
    colors.set_border_subtle(color(pack.border_subtle));
    colors.set_border_strong(color(pack.border_strong));

    push_metrics(app, pack);

    let typography = app.global::<Typography>();
    typography.set_font_display(SharedString::from(pack.font_display));
    typography.set_font_body(SharedString::from(pack.font_body));
    typography.set_font_mono(SharedString::from(pack.font_mono));

    workspace::apply(app, pack.appearance)
        .expect("Retrospect workspace extension must support every bundled appearance");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_resolver_owns_both_retrospect_appearances() {
        assert_eq!(
            resolve("retrospect", "dark").unwrap().primary,
            Oklch::new(0.75540, 0.11877, 285.42)
        );
        assert_eq!(
            resolve("retrospect", "light").unwrap().primary,
            Oklch::new(0.64, 0.145, 155.0)
        );
        assert!(resolve("retrospect", "system").is_none());
    }

    #[test]
    fn brand_metrics_remain_identical_across_appearances() {
        assert_eq!(
            resolve("retrospect", "dark")
                .unwrap()
                .metrics
                .borders
                .radius_card,
            resolve("retrospect", "light")
                .unwrap()
                .metrics
                .borders
                .radius_card
        );
        assert_eq!(
            resolve("retrospect", "dark").unwrap().font_body,
            resolve("retrospect", "light").unwrap().font_body
        );
    }
}

//! Retrospect's own semantic colour palette.

use ui_kit_slint::{compose_theme, Oklch, ThemePack, ThemePalette};

use crate::theme::foundation::RETROSPECT;

const DARK: ThemePalette = ThemePalette {
    theme_id: "retrospect",
    theme_name: "Retrospect",
    appearance: "dark",
    paper_body: Oklch::new(0.15515, 0.01438, 293.31),
    paper_surface: Oklch::new(0.20693, 0.02435, 293.68),
    paper_raised: Oklch::new(0.245, 0.030, 293.0),
    paper_sunken: Oklch::new(0.17743, 0.01900, 292.13),
    paper_chrome: Oklch::new(0.15515, 0.01438, 293.31),
    ink_headers: Oklch::new(0.98415, 0.00331, 288.90),
    ink_subheaders: Oklch::new(0.92876, 0.01294, 290.51),
    ink_body: Oklch::new(0.98415, 0.00331, 288.90),
    ink_info: Oklch::new(0.92876, 0.01294, 290.51),
    ink_muted: Oklch::new(0.71067, 0.03627, 290.77),
    ink_disabled: Oklch::new(0.71067, 0.03627, 290.77).with_alpha(0.55),
    ink_hover: Oklch::new(0.98415, 0.00331, 288.90),
    ink_inverse: Oklch::new(1.0, 0.0, 0.0),
    primary: Oklch::new(0.75540, 0.11877, 285.42),
    secondary: Oklch::new(0.82759, 0.07923, 284.10),
    accent: Oklch::new(0.82759, 0.07923, 284.10),
    status_success: Oklch::new(0.39253, 0.05450, 129.72),
    status_warning: Oklch::new(0.792, 0.132, 80.0),
    status_critical: Oklch::new(0.50542, 0.08579, 90.71),
    status_info: Oklch::new(0.75540, 0.11877, 285.42),
    status_caution: Oklch::new(0.792, 0.132, 80.0),
    border_subtle: Oklch::new(0.31452, 0.03787, 291.06),
    border_strong: Oklch::new(0.37170, 0.04055, 290.86),
};

const LIGHT: ThemePalette = ThemePalette {
    theme_id: "retrospect",
    theme_name: "Retrospect",
    appearance: "light",
    paper_body: Oklch::new(0.98, 0.004, 80.0),
    paper_surface: Oklch::new(1.0, 0.0, 0.0),
    paper_raised: Oklch::new(0.96, 0.006, 80.0),
    paper_sunken: Oklch::new(0.95, 0.008, 80.0),
    paper_chrome: Oklch::new(0.95, 0.008, 80.0),
    ink_headers: Oklch::new(0.15, 0.01, 80.0),
    ink_subheaders: Oklch::new(0.22, 0.01, 80.0),
    ink_body: Oklch::new(0.22, 0.01, 80.0),
    ink_info: Oklch::new(0.45, 0.01, 80.0),
    ink_muted: Oklch::new(0.45, 0.01, 80.0),
    ink_disabled: Oklch::new(0.45, 0.01, 80.0).with_alpha(0.55),
    ink_hover: Oklch::new(0.15, 0.01, 80.0),
    ink_inverse: Oklch::new(1.0, 0.0, 0.0),
    primary: Oklch::new(0.64, 0.145, 155.0),
    secondary: Oklch::new(0.63, 0.2, 260.0),
    accent: Oklch::new(0.71, 0.155, 72.0),
    status_success: Oklch::new(0.64, 0.145, 155.0),
    status_warning: Oklch::new(0.71, 0.155, 72.0),
    status_critical: Oklch::new(0.55, 0.2, 27.0),
    status_info: Oklch::new(0.63, 0.2, 260.0),
    status_caution: Oklch::new(0.71, 0.155, 72.0),
    border_subtle: Oklch::new(0.0, 0.0, 0.0).with_alpha(0.12),
    border_strong: Oklch::new(0.0, 0.0, 0.0).with_alpha(0.18),
};

/// Resolves Retrospect's local palette with its durable foundation.
pub fn resolve(appearance: &str) -> Option<ThemePack> {
    match appearance {
        "dark" => Some(compose_theme(RETROSPECT, DARK)),
        "light" => Some(compose_theme(RETROSPECT, LIGHT)),
        _ => None,
    }
}

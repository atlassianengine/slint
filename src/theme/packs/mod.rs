//! Retrospect's explicit theme allow-list.

mod retrospect;

use ui_kit_slint::ThemePack;

/// Resolves an allowed Retrospect pack.
pub fn resolve(theme_id: &str, appearance: &str) -> Option<ThemePack> {
    match theme_id {
        "retrospect" => retrospect::resolve(appearance),
        _ => None,
    }
}

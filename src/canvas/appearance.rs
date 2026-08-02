//! Board appearance state for the native fixture host.
//!
//! This is deliberately Rust-owned: the Slint sheet emits user intent while
//! this module owns the selected path, decoded image, and clamped dim value.

use std::{cell::RefCell, path::PathBuf, rc::Rc};

use slint::{ComponentHandle, Image};

use crate::AppWindow;

const DEFAULT_BACKGROUND_DIM: f32 = 0.55;

#[derive(Default)]
struct BoardAppearance {
    path: Option<PathBuf>,
    image: Image,
    dim: f32,
}

pub(super) fn install(app: &AppWindow) {
    let appearance = Rc::new(RefCell::new(BoardAppearance {
        dim: DEFAULT_BACKGROUND_DIM,
        ..Default::default()
    }));
    sync(app, &appearance.borrow());

    let choose_app = app.as_weak();
    let choose_state = appearance.clone();
    app.on_canvas_background_select_requested(move || {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Choose Retrospect canvas background")
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "bmp"])
            .pick_file()
        else {
            return;
        };
        let Ok(image) = Image::load_from_path(&path) else {
            return;
        };
        let mut appearance = choose_state.borrow_mut();
        appearance.path = Some(path);
        appearance.image = image;
        if let Some(app) = choose_app.upgrade() {
            sync(&app, &appearance);
        }
    });

    let clear_app = app.as_weak();
    let clear_state = appearance.clone();
    app.on_canvas_background_clear_requested(move || {
        let mut appearance = clear_state.borrow_mut();
        appearance.path = None;
        appearance.image = Image::default();
        if let Some(app) = clear_app.upgrade() {
            sync(&app, &appearance);
        }
    });

    let dim_app = app.as_weak();
    app.on_canvas_background_dim_changed(move |dim| {
        let mut appearance = appearance.borrow_mut();
        appearance.dim = dim.clamp(0.0, 1.0);
        if let Some(app) = dim_app.upgrade() {
            sync(&app, &appearance);
        }
    });
}

fn sync(app: &AppWindow, appearance: &BoardAppearance) {
    app.set_board_background_image(appearance.image.clone());
    app.set_board_background_path(
        appearance
            .path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .into(),
    );
    app.set_board_background_dim(appearance.dim);
}

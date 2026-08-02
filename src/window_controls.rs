use slint::{winit_030::WinitWindowAccessor, ComponentHandle};

use crate::AppWindow;

/// Owns platform-window effects for the custom Slint title rail.
pub fn install(app: &AppWindow) {
    let weak = app.as_weak();
    app.on_minimize_requested(move || {
        let weak = weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = weak.upgrade() {
                app.window().set_minimized(true);
            }
        });
    });

    let weak = app.as_weak();
    app.on_maximize_requested(move || {
        let weak = weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = weak.upgrade() {
                let window = app.window();
                window.set_maximized(!window.is_maximized());
            }
        });
    });

    let weak = app.as_weak();
    app.on_close_requested(move || {
        let weak = weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = weak.upgrade() {
                let _ = app.hide();
            }
        });
    });

    let weak = app.as_weak();
    app.on_window_drag_requested(move || {
        if let Some(app) = weak.upgrade() {
            // Winit must receive this while handling the pointer press. Slint
            // emits the callback directly from the custom title rail.
            let _ = app
                .window()
                .with_winit_window(|window| window.drag_window());
        }
    });
}

slint::include_modules!();

mod canvas;
mod canvas_backdrop;
mod code_editor;
mod color;
mod controllers;
mod document_editor;
mod planner;
mod services;
mod theme;
mod window_controls;

fn main() -> Result<(), slint::PlatformError> {
    let mut search = services::SearchRuntime::start().map_err(slint::PlatformError::from)?;
    let search_handle = search.handle();
    let workspace = services::WorkspaceRuntime::start(search_handle.clone())
        .map_err(slint::PlatformError::from)?;

    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .require_wgpu_29(Default::default())
        .select()?;

    let app = AppWindow::new()?;
    app.set_build_mode(if cfg!(debug_assertions) {
        "DEBUG".into()
    } else {
        "RELEASE".into()
    });
    theme::apply(&app);
    let backdrop = canvas_backdrop::install(&app)
        .map_err(|error| slint::PlatformError::from(error.to_string()))?;
    canvas::install(&app, backdrop, &search_handle).map_err(slint::PlatformError::from)?;
    search
        .start_background_drains()
        .map_err(slint::PlatformError::from)?;
    code_editor::install(
        &app,
        workspace.gateway(),
        workspace.editor_document(),
        search_handle.clone(),
    );
    document_editor::install(&app, workspace.gateway(), search_handle.clone());
    search_handle.bind_health(&app);
    controllers::install_all(&app, search_handle);
    window_controls::install(&app);
    app.run()
}

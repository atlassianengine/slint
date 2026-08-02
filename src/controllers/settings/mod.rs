// Rust guideline compliant 2026-02-21

//! Settings & OAuth controller managing workbench configuration and connections.

mod connection_items;
mod constants;
mod credentials;
mod probes;

use crate::AppWindow;
use slint::{ComponentHandle, ModelRc};

use connection_items::{
    active_ai_provider_for_connection, create_connection_items, provider_display_name,
    set_model_provider_connection_state, set_provider_connection_state, set_provider_status,
};
use constants::{
    AI_PROVIDER_CREDENTIALS, CONNECTING_STATUS, DEFAULT_AI_PROVIDER, DISCONNECTED_STATUS,
    INVALID_CONFIGURATION_PREFIX, KEY_REMOVAL_HINT, NO_PROVIDER_CREDENTIALS_STATUS,
    UNKNOWN_PROVIDER_NAME,
};

pub(crate) use constants::{
    CLAUDE_PROVIDER_ID, GEMINI_PROVIDER_ID, LOCAL_CLI_PROVIDER_ID, OPENAI_PROVIDER_ID,
};
use credentials::{hydrate_ai_provider_credentials, sync_ai_provider_credential};
use probes::{build_connection_probe, run_connection_probe};

pub(crate) use probes::build_active_provider_readiness_snapshot;

/// Installs the settings controller onto the main Slint `AppWindow`.
pub fn install(app: &AppWindow) {
    let connections = create_connection_items();

    app.set_settings_connections(ModelRc::from(connections.clone()));
    app.set_settings_active_tab("appearance".into());
    app.set_settings_active_theme_id("retrospect".into());
    app.set_settings_active_ai_provider(DEFAULT_AI_PROVIDER.into());
    app.set_settings_turso_db_url("".into());
    app.set_settings_turso_auth_token("".into());
    app.set_settings_vault_status_text(NO_PROVIDER_CREDENTIALS_STATUS.into());
    hydrate_ai_provider_credentials(&app, &connections);

    {
        let app_weak = app.as_weak();
        app.on_settings_theme_selected(move |theme_id| {
            if let Some(app) = app_weak.upgrade() {
                app.set_settings_active_theme_id(theme_id.clone());
                let selected_theme = theme_id.to_string();
                if let Err(error) = crate::theme::apply_selection(&app, &selected_theme, "dark") {
                    app.set_settings_vault_status_text(
                        format!("Theme '{selected_theme}' unavailable: {error}").into(),
                    );
                    return;
                }
                app.set_settings_vault_status_text(
                    format!("Theme changed to '{selected_theme}'.").into(),
                );
            }
        });
    }

    {
        let connections = connections.clone();
        let app_weak = app.as_weak();
        app.on_settings_connect_provider_requested(move |provider_id| {
            let provider_id = provider_id.to_string();
            set_provider_connection_state(&connections, &provider_id, false, CONNECTING_STATUS);
            let probe = app_weak
                .upgrade()
                .and_then(|app| build_connection_probe(&provider_id, &app));
            if probe.is_none() {
                if let Some(app) = app_weak.upgrade() {
                    app.set_settings_vault_status_text(
                        format!(
                            "Unsupported provider: {}",
                            provider_display_name(&provider_id)
                        )
                        .into(),
                    );
                }
                set_provider_connection_state(
                    &connections,
                    &provider_id,
                    false,
                    UNKNOWN_PROVIDER_NAME,
                );
                return;
            }

            let app_weak_for_thread = app_weak.clone();
            let provider_id_for_thread = provider_id.clone();
            let probe = probe.expect("provider id validated before spawning");

            std::thread::spawn(move || {
                let check = run_connection_probe(probe);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = app_weak_for_thread.upgrade() {
                        let conn_model = app.get_settings_connections();
                        match check {
                            Ok(message) => {
                                set_model_provider_connection_state(
                                    &conn_model,
                                    &provider_id_for_thread,
                                    true,
                                    message.as_str(),
                                );
                                if let Some(active_provider) =
                                    active_ai_provider_for_connection(&provider_id_for_thread)
                                {
                                    app.set_settings_active_ai_provider(active_provider.into());
                                }
                                app.set_settings_vault_status_text(
                                    format!("Connected: {message}").into(),
                                );
                            }
                            Err(message) => {
                                set_model_provider_connection_state(
                                    &conn_model,
                                    &provider_id_for_thread,
                                    false,
                                    &format!("{INVALID_CONFIGURATION_PREFIX}{message}"),
                                );
                                app.set_settings_vault_status_text(
                                    format!(
                                        "{} failed: {message}",
                                        provider_display_name(&provider_id_for_thread)
                                    )
                                    .into(),
                                );
                            }
                        }
                    }
                });
            });
        });
    }

    {
        let connections = connections.clone();
        let app_weak = app.as_weak();
        app.on_settings_disconnect_provider_requested(move |provider_id| {
            let provider_id = provider_id.to_string();
            set_provider_connection_state(&connections, &provider_id, false, DISCONNECTED_STATUS);
            if let Some(app) = app_weak.upgrade() {
                if let Some(active_provider) = active_ai_provider_for_connection(&provider_id) {
                    if app.get_settings_active_ai_provider() == active_provider {
                        app.set_settings_active_ai_provider(DEFAULT_AI_PROVIDER.into());
                    }
                }
                app.set_settings_vault_status_text(format!("Disconnected: {provider_id}").into());
            }
        });
    }

    app.on_settings_save_ai_keys_requested({
        let app_weak = app.as_weak();
        let connections = connections.clone();
        move || {
            if let Some(app) = app_weak.upgrade() {
                let mut configured: Vec<&'static str> = Vec::new();
                let mut errors: Vec<String> = Vec::new();

                for binding in AI_PROVIDER_CREDENTIALS {
                    match sync_ai_provider_credential(&app, &connections, binding) {
                        Ok(Some(label)) => configured.push(label),
                        Ok(None) => {}
                        Err(error) => {
                            set_provider_status(
                                &connections,
                                binding.connection_id,
                                KEY_REMOVAL_HINT,
                            );
                            errors.push(format!("{}: {error}", binding.label));
                        }
                    }
                }

                let status = if !errors.is_empty() {
                    format!("Failed to save AI credentials. {}", errors.join(" | "))
                } else if configured.is_empty() {
                    "No AI keys set; credentials unchanged.".to_string()
                } else {
                    format!("Saved keys for: {}", configured.join(", "))
                };
                app.set_settings_vault_status_text(status.into());
            }
        }
    });

    app.on_settings_close_requested({
        let app_weak = app.as_weak();
        move || {
            if let Some(app) = app_weak.upgrade() {
                app.set_active_surface(crate::WorkbenchSurface::Canvas);
            }
        }
    });
}

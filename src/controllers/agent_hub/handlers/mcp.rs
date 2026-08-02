// Rust guideline compliant 2026-02-21

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::controllers::ai::mcp_router;
use crate::{AppWindow, MCPServerItem};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use super::super::helpers::{append_log, logs_to_string, update_mcp_server_status};

fn update_mcp_server_status_on_ui_model(
    servers: &ModelRc<MCPServerItem>,
    server_id: &str,
    status: String,
    tool_count: Option<i32>,
) {
    for idx in 0..servers.row_count() {
        if let Some(mut server) = servers.row_data(idx) {
            if server.id == server_id {
                server.status = status.clone().into();
                if let Some(tool_count) = tool_count {
                    server.tool_count = tool_count;
                }
                servers.set_row_data(idx, server);
                return;
            }
        }
    }
}

fn append_log_to_ui_output(current: &str, message: &str) -> String {
    let mut entries = if current.is_empty() {
        Vec::new()
    } else {
        current.split('\n').map(str::to_owned).collect()
    };
    entries.push(message.to_owned());
    if entries.len() > 200 {
        let extra = entries.len() - 200;
        entries.drain(0..extra);
    }
    entries.join("\n")
}

pub(crate) fn register_add_mcp_server_requested(
    app: &AppWindow,
    mcp_servers: Rc<VecModel<MCPServerItem>>,
    mcp_server_endpoints: Rc<RefCell<HashMap<String, String>>>,
    logs: Rc<RefCell<Vec<String>>>,
) {
    let app_weak = app.as_weak();
    app.on_orchestrator_add_mcp_server_requested(move || {
        let next = mcp_servers.row_count() + 1;
        let id = format!("mcp-{next}");
        let fallback = mcp_router::fallback_mcp_endpoint().to_string();
        mcp_server_endpoints
            .borrow_mut()
            .insert(id.clone(), fallback.clone());
        let name = format!("Custom MCP {next}");
        mcp_servers.push(MCPServerItem {
            id: id.clone().into(),
            name: name.clone().into(),
            transport: "http".into(),
            tool_count: 0,
            status: "Ready".into(),
        });
        append_log(
            &logs,
            &format!("[mcp] added {name}; endpoint mapped to {fallback}"),
        );
        if let Some(app) = app_weak.upgrade() {
            app.set_orchestrator_mcp_servers(ModelRc::from(mcp_servers.clone()));
            app.set_orchestrator_active_log_output(logs_to_string(&logs).into());
        }
    });
}

pub(crate) fn register_ping_mcp_server_requested(
    app: &AppWindow,
    mcp_servers: Rc<VecModel<MCPServerItem>>,
    mcp_server_endpoints: Rc<RefCell<HashMap<String, String>>>,
    logs: Rc<RefCell<Vec<String>>>,
) {
    let app_weak = app.as_weak();
    app.on_orchestrator_ping_mcp_server_requested(move |server_id| {
        let server_id = server_id.to_string();
        let mut endpoint = {
            let map = mcp_server_endpoints.borrow();
            map.get(&server_id).cloned()
        };
        if endpoint.is_none() {
            if let Some(seed) = mcp_router::resolve_server_endpoint(&server_id) {
                endpoint = Some(seed.to_string());
                mcp_server_endpoints
                    .borrow_mut()
                    .insert(server_id.clone(), seed.to_string());
            }
        }

        if let Some(endpoint) = endpoint {
            update_mcp_server_status(&mcp_servers, &server_id, "Pinging".into(), None);
            append_log(&logs, &format!("[mcp] ping requested for {server_id}"));
            if let Some(app) = app_weak.upgrade() {
                app.set_orchestrator_mcp_servers(ModelRc::from(mcp_servers.clone()));
                app.set_orchestrator_active_log_output(logs_to_string(&logs).into());
            }

            let app_weak = app_weak.clone();
            std::thread::spawn(move || {
                let result = mcp_router::ping_mcp_server(&endpoint, &server_id);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(app) = app_weak.upgrade() else {
                        return;
                    };

                    // ModelRc and the log property are UI-thread-owned. Reacquire
                    // both through AppWindow only after the Send boundary has been
                    // crossed; the worker transports only the Send-owned ping result.
                    let mcp_servers = app.get_orchestrator_mcp_servers();
                    let log_message = match result {
                        Ok(success) => {
                            update_mcp_server_status_on_ui_model(
                                &mcp_servers,
                                &server_id,
                                success.status.clone(),
                                Some(success.tool_count),
                            );
                            format!(
                                "[mcp] {server_id} ping ok: {} ({} tools)",
                                success.status, success.tool_count
                            )
                        }
                        Err(error) => {
                            update_mcp_server_status_on_ui_model(
                                &mcp_servers,
                                &server_id,
                                "Error".into(),
                                None,
                            );
                            format!("[mcp] {server_id} ping failed: {error}")
                        }
                    };

                    let log_output = append_log_to_ui_output(
                        &app.get_orchestrator_active_log_output().to_string(),
                        &log_message,
                    );
                    app.set_orchestrator_active_log_output(log_output.into());
                });
            });
        } else {
            append_log(
                &logs,
                &format!("[mcp] {server_id} has no configured endpoint"),
            );
            update_mcp_server_status(&mcp_servers, &server_id, "Not configured".into(), None);
            if let Some(app) = app_weak.upgrade() {
                app.set_orchestrator_mcp_servers(ModelRc::from(mcp_servers.clone()));
                app.set_orchestrator_active_log_output(logs_to_string(&logs).into());
            }
        }
    });
}

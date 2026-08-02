// Rust guideline compliant 2026-02-21

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{cell::RefCell, env, rc::Rc};

use crate::AppWindow;
use agent_auth::resolve_api_key;
use slint::{Model, VecModel};

use crate::controllers::settings::{
    CLAUDE_PROVIDER_ID, GEMINI_PROVIDER_ID, LOCAL_CLI_PROVIDER_ID, OPENAI_PROVIDER_ID,
};
use crate::MCPServerItem;

pub(crate) fn now_stamp() -> String {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(elapsed) => format!("{}", elapsed.as_secs()),
        Err(error) => format!("error:{error}"),
    }
}

pub(crate) fn logs_to_string(logs: &Rc<RefCell<Vec<String>>>) -> String {
    logs.borrow().iter().cloned().collect::<Vec<_>>().join("\n")
}

pub(crate) fn append_log(logs: &Rc<RefCell<Vec<String>>>, message: &str) {
    let mut snapshot = logs.borrow_mut();
    snapshot.push(message.to_string());
    if snapshot.len() > 200 {
        let extra = snapshot.len() - 200;
        snapshot.drain(0..extra);
    }
}

pub(crate) fn update_mcp_server_status(
    servers: &Rc<VecModel<MCPServerItem>>,
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

pub(crate) fn active_provider_ready(app: &AppWindow) -> bool {
    let provider = app.get_settings_active_ai_provider().to_string();
    match provider.as_str() {
        CLAUDE_PROVIDER_ID => {
            app_provider_api_key_present(app.get_settings_claude_api_key().to_string().as_str())
        }
        OPENAI_PROVIDER_ID => {
            app_provider_api_key_present(app.get_settings_openai_api_key().to_string().as_str())
        }
        GEMINI_PROVIDER_ID => {
            app_provider_api_key_present(app.get_settings_gemini_api_key().to_string().as_str())
        }
        LOCAL_CLI_PROVIDER_ID => active_local_cli_available(),
        _ => false,
    }
}

pub(crate) fn app_provider_api_key_present(raw_api_key: &str) -> bool {
    match resolve_api_key(Some(raw_api_key)) {
        Ok(Some(value)) => !value.trim().is_empty(),
        Ok(None) => false,
        Err(_) => false,
    }
}

pub(crate) fn active_local_cli_available() -> bool {
    for env_name in ["AI_LOCAL_CLI", "RETROSPECT_LOCAL_AI_CLI", "LOCAL_AI_CLI"] {
        if let Ok(value) = env::var(env_name) {
            if !value.trim().is_empty() {
                return true;
            }
        }
    }
    ["retrospect-ai-cli", "local-ai-cli", "ollama"]
        .into_iter()
        .any(command_exists)
}

pub(crate) fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

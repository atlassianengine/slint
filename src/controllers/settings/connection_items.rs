// Rust guideline compliant 2026-02-21

use std::rc::Rc;

use crate::ConnectionItem;
use slint::{Model, VecModel};

use super::constants::{
    AI_CLAUDE_CONNECTION_ID, AI_GEMINI_CONNECTION_ID, AI_LOCAL_CLI_CONNECTION_ID,
    AI_OPENAI_CONNECTION_ID, API_KEY_MISSING_HINT, API_KEY_PRESENT_HINT, CLAUDE_PROVIDER_ID,
    CLAUDE_PROVIDER_NAME, DISCONNECTED_STATUS, GEMINI_PROVIDER_ID, GEMINI_PROVIDER_NAME,
    GITHUB_OAUTH_CONNECTION_ID, GITHUB_OAUTH_PROVIDER_ID, GITHUB_OAUTH_PROVIDER_NAME,
    GOOGLE_OAUTH_CONNECTION_ID, GOOGLE_OAUTH_PROVIDER_ID, GOOGLE_OAUTH_PROVIDER_NAME,
    LOCAL_CLI_PROVIDER_ID, LOCAL_CLI_PROVIDER_NAME, MICROSOFT_GRAPH_CONNECTION_ID,
    MICROSOFT_GRAPH_PROVIDER_NAME, MICROSOFT_OAUTH_PROVIDER_ID, OPENAI_PROVIDER_ID,
    OPENAI_PROVIDER_NAME, UNKNOWN_PROVIDER_NAME,
};

#[derive(Clone, Copy)]
struct ConnectionRowTemplate {
    id: &'static str,
    name: &'static str,
    provider: &'static str,
    status: &'static str,
    connected: bool,
}

const CONNECTION_ROWS: [ConnectionRowTemplate; 7] = [
    ConnectionRowTemplate {
        id: GOOGLE_OAUTH_CONNECTION_ID,
        name: GOOGLE_OAUTH_PROVIDER_NAME,
        provider: GOOGLE_OAUTH_PROVIDER_ID,
        status: DISCONNECTED_STATUS,
        connected: false,
    },
    ConnectionRowTemplate {
        id: GITHUB_OAUTH_CONNECTION_ID,
        name: GITHUB_OAUTH_PROVIDER_NAME,
        provider: GITHUB_OAUTH_PROVIDER_ID,
        status: DISCONNECTED_STATUS,
        connected: false,
    },
    ConnectionRowTemplate {
        id: MICROSOFT_GRAPH_CONNECTION_ID,
        name: MICROSOFT_GRAPH_PROVIDER_NAME,
        provider: MICROSOFT_OAUTH_PROVIDER_ID,
        status: DISCONNECTED_STATUS,
        connected: false,
    },
    ConnectionRowTemplate {
        id: AI_GEMINI_CONNECTION_ID,
        name: GEMINI_PROVIDER_NAME,
        provider: GEMINI_PROVIDER_ID,
        status: API_KEY_MISSING_HINT,
        connected: false,
    },
    ConnectionRowTemplate {
        id: AI_CLAUDE_CONNECTION_ID,
        name: CLAUDE_PROVIDER_NAME,
        provider: CLAUDE_PROVIDER_ID,
        status: API_KEY_MISSING_HINT,
        connected: false,
    },
    ConnectionRowTemplate {
        id: AI_OPENAI_CONNECTION_ID,
        name: OPENAI_PROVIDER_NAME,
        provider: OPENAI_PROVIDER_ID,
        status: API_KEY_MISSING_HINT,
        connected: false,
    },
    ConnectionRowTemplate {
        id: AI_LOCAL_CLI_CONNECTION_ID,
        name: LOCAL_CLI_PROVIDER_NAME,
        provider: LOCAL_CLI_PROVIDER_ID,
        status: DISCONNECTED_STATUS,
        connected: false,
    },
];

pub(crate) fn create_connection_items() -> Rc<VecModel<ConnectionItem>> {
    Rc::new(VecModel::from(
        CONNECTION_ROWS
            .iter()
            .map(|row| ConnectionItem {
                id: row.id.into(),
                name: row.name.into(),
                provider: row.provider.into(),
                status: row.status.into(),
                connected: row.connected,
            })
            .collect::<Vec<_>>(),
    ))
}

pub(crate) fn set_model_provider_connection_state(
    connections: &slint::ModelRc<ConnectionItem>,
    provider_id: &str,
    connected: bool,
    status: &str,
) {
    for idx in 0..connections.row_count() {
        if let Some(mut connection) = connections.row_data(idx) {
            if connection.id == provider_id {
                connection.connected = connected;
                connection.status = status.into();
                connections.set_row_data(idx, connection);
                break;
            }
        }
    }
}

pub(crate) fn set_provider_connection_state(
    connections: &Rc<VecModel<ConnectionItem>>,
    provider_id: &str,
    connected: bool,
    status: &str,
) {
    for idx in 0..connections.row_count() {
        if let Some(mut connection) = connections.row_data(idx) {
            if connection.id == provider_id {
                connection.connected = connected;
                connection.status = status.into();
                connections.set_row_data(idx, connection);
                break;
            }
        }
    }
}

pub(crate) fn set_provider_status(
    connections: &Rc<VecModel<ConnectionItem>>,
    provider_id: &str,
    status: &str,
) {
    for idx in 0..connections.row_count() {
        if let Some(mut connection) = connections.row_data(idx) {
            if connection.id == provider_id {
                connection.status = status.into();
                connections.set_row_data(idx, connection);
                break;
            }
        }
    }
}

pub(crate) fn refresh_ai_connection_key_hint(
    connections: &Rc<VecModel<ConnectionItem>>,
    provider_id: &str,
    has_key: bool,
) {
    set_provider_status(
        connections,
        provider_id,
        if has_key {
            API_KEY_PRESENT_HINT
        } else {
            API_KEY_MISSING_HINT
        },
    );
}

pub(crate) fn provider_display_name(provider_id: &str) -> &'static str {
    match provider_id {
        GOOGLE_OAUTH_CONNECTION_ID => GOOGLE_OAUTH_PROVIDER_NAME,
        GITHUB_OAUTH_CONNECTION_ID => GITHUB_OAUTH_PROVIDER_NAME,
        MICROSOFT_GRAPH_CONNECTION_ID => MICROSOFT_GRAPH_PROVIDER_NAME,
        AI_GEMINI_CONNECTION_ID => GEMINI_PROVIDER_NAME,
        AI_CLAUDE_CONNECTION_ID => CLAUDE_PROVIDER_NAME,
        AI_OPENAI_CONNECTION_ID => OPENAI_PROVIDER_NAME,
        AI_LOCAL_CLI_CONNECTION_ID => LOCAL_CLI_PROVIDER_NAME,
        _ => UNKNOWN_PROVIDER_NAME,
    }
}

pub(crate) fn active_ai_provider_for_connection(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        AI_GEMINI_CONNECTION_ID => Some(GEMINI_PROVIDER_ID),
        AI_CLAUDE_CONNECTION_ID => Some(CLAUDE_PROVIDER_ID),
        AI_OPENAI_CONNECTION_ID => Some(OPENAI_PROVIDER_ID),
        AI_LOCAL_CLI_CONNECTION_ID => Some(LOCAL_CLI_PROVIDER_ID),
        _ => None,
    }
}

// Rust guideline compliant 2026-02-21

// Settings controller constants and shared model used by AI credential wiring.
use agent_auth::{
    CLAUDE_API_KEY_CREDENTIAL_FIELD, CLAUDE_API_KEY_CREDENTIAL_REF,
    GEMINI_API_KEY_CREDENTIAL_FIELD, GEMINI_API_KEY_CREDENTIAL_REF,
    OPENAI_API_KEY_CREDENTIAL_FIELD, OPENAI_API_KEY_CREDENTIAL_REF,
};

pub(crate) const GOOGLE_OAUTH_CONNECTION_ID: &str = "google_oauth";
pub(crate) const GITHUB_OAUTH_CONNECTION_ID: &str = "github_oauth";
pub(crate) const MICROSOFT_GRAPH_CONNECTION_ID: &str = "microsoft_graph";
pub(crate) const AI_GEMINI_CONNECTION_ID: &str = "ai_gemini";
pub(crate) const AI_CLAUDE_CONNECTION_ID: &str = "ai_claude";
pub(crate) const AI_OPENAI_CONNECTION_ID: &str = "ai_openai";
pub(crate) const AI_LOCAL_CLI_CONNECTION_ID: &str = "ai_local_cli";

pub(crate) const GOOGLE_OAUTH_PROVIDER_ID: &str = "google";
pub(crate) const GITHUB_OAUTH_PROVIDER_ID: &str = "github";
pub(crate) const MICROSOFT_OAUTH_PROVIDER_ID: &str = "microsoft";
pub(crate) const GEMINI_PROVIDER_ID: &str = "gemini";
pub(crate) const CLAUDE_PROVIDER_ID: &str = "claude";
pub(crate) const OPENAI_PROVIDER_ID: &str = "openai";
pub(crate) const LOCAL_CLI_PROVIDER_ID: &str = "local_cli";

pub(crate) const GOOGLE_OAUTH_PROVIDER_NAME: &str = "Google OAuth";
pub(crate) const GITHUB_OAUTH_PROVIDER_NAME: &str = "GitHub OAuth";
pub(crate) const MICROSOFT_GRAPH_PROVIDER_NAME: &str = "Microsoft Graph";
pub(crate) const GEMINI_PROVIDER_NAME: &str = "Gemini API";
pub(crate) const CLAUDE_PROVIDER_NAME: &str = "Claude API";
pub(crate) const OPENAI_PROVIDER_NAME: &str = "OpenAI API";
pub(crate) const LOCAL_CLI_PROVIDER_NAME: &str = "Local AI CLI";
pub(crate) const UNKNOWN_PROVIDER_NAME: &str = "Unknown provider";

pub(crate) const DEFAULT_AI_PROVIDER: &str = GEMINI_PROVIDER_ID;
pub(crate) const GEMINI_API_KEY_FIELD_ID: &str = GEMINI_API_KEY_CREDENTIAL_FIELD;
pub(crate) const CLAUDE_API_KEY_FIELD_ID: &str = CLAUDE_API_KEY_CREDENTIAL_FIELD;
pub(crate) const OPENAI_API_KEY_FIELD_ID: &str = OPENAI_API_KEY_CREDENTIAL_FIELD;
pub(crate) const HTTP_TIMEOUT_SECONDS: u64 = 6;
pub(crate) const DISCONNECTED_STATUS: &str = "Disconnected";
pub(crate) const API_KEY_MISSING_HINT: &str = "No key saved";
pub(crate) const API_KEY_PRESENT_HINT: &str = "API key present (not connected)";
pub(crate) const NO_PROVIDER_CREDENTIALS_STATUS: &str = "No provider credentials configured.";
pub(crate) const CONNECTING_STATUS: &str = "Connecting...";
pub(crate) const INVALID_CONFIGURATION_PREFIX: &str = "Error: ";
pub(crate) const KEY_REMOVAL_HINT: &str = "Error: credential is not available";

#[derive(Clone, Copy)]
pub(crate) struct AiProviderCredentialBinding {
    pub(crate) field_id: &'static str,
    pub(crate) connection_id: &'static str,
    pub(crate) credential_ref: &'static str,
    pub(crate) label: &'static str,
}

pub(crate) const AI_PROVIDER_CREDENTIALS: [AiProviderCredentialBinding; 3] = [
    AiProviderCredentialBinding {
        field_id: GEMINI_API_KEY_FIELD_ID,
        connection_id: AI_GEMINI_CONNECTION_ID,
        credential_ref: GEMINI_API_KEY_CREDENTIAL_REF,
        label: "Gemini",
    },
    AiProviderCredentialBinding {
        field_id: CLAUDE_API_KEY_FIELD_ID,
        connection_id: AI_CLAUDE_CONNECTION_ID,
        credential_ref: CLAUDE_API_KEY_CREDENTIAL_REF,
        label: "Claude",
    },
    AiProviderCredentialBinding {
        field_id: OPENAI_API_KEY_FIELD_ID,
        connection_id: AI_OPENAI_CONNECTION_ID,
        credential_ref: OPENAI_API_KEY_CREDENTIAL_REF,
        label: "OpenAI",
    },
];

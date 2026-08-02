// Rust guideline compliant 2026-02-21

use std::env;
use std::io::ErrorKind;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::AppWindow;
use agent_auth::resolve_api_key;
use agent_execution_lane::{
    ProviderCredentialReadiness, ProviderReadinessObservation, RuntimeReadinessSnapshot,
};
use reqwest::blocking::Client;

use super::constants::{
    AI_CLAUDE_CONNECTION_ID, AI_GEMINI_CONNECTION_ID, AI_LOCAL_CLI_CONNECTION_ID,
    AI_OPENAI_CONNECTION_ID, CLAUDE_PROVIDER_ID, GEMINI_PROVIDER_ID, GITHUB_OAUTH_CONNECTION_ID,
    GOOGLE_OAUTH_CONNECTION_ID, HTTP_TIMEOUT_SECONDS, LOCAL_CLI_PROVIDER_ID,
    MICROSOFT_GRAPH_CONNECTION_ID, OPENAI_PROVIDER_ID,
};

#[derive(Debug)]
pub(crate) enum ConnectionProbe {
    OAuth,
    Gemini(String),
    Claude(String),
    OpenAI(String),
    LocalCli,
    InvalidConfiguration(String),
}

pub(crate) fn build_connection_probe(
    provider_id: &str,
    app: &AppWindow,
) -> Option<ConnectionProbe> {
    match provider_id {
        GOOGLE_OAUTH_CONNECTION_ID | GITHUB_OAUTH_CONNECTION_ID | MICROSOFT_GRAPH_CONNECTION_ID => {
            Some(ConnectionProbe::OAuth)
        }
        AI_GEMINI_CONNECTION_ID => {
            match build_ai_connection_probe(app.get_settings_gemini_api_key().to_string()) {
                Ok(api_key) => Some(ConnectionProbe::Gemini(api_key)),
                Err(error) => Some(ConnectionProbe::InvalidConfiguration(error)),
            }
        }
        AI_CLAUDE_CONNECTION_ID => {
            match build_ai_connection_probe(app.get_settings_claude_api_key().to_string()) {
                Ok(api_key) => Some(ConnectionProbe::Claude(api_key)),
                Err(error) => Some(ConnectionProbe::InvalidConfiguration(error)),
            }
        }
        AI_OPENAI_CONNECTION_ID => {
            match build_ai_connection_probe(app.get_settings_openai_api_key().to_string()) {
                Ok(api_key) => Some(ConnectionProbe::OpenAI(api_key)),
                Err(error) => Some(ConnectionProbe::InvalidConfiguration(error)),
            }
        }
        AI_LOCAL_CLI_CONNECTION_ID => Some(ConnectionProbe::LocalCli),
        _ => None,
    }
}

pub(crate) fn run_connection_probe(probe: ConnectionProbe) -> Result<String, String> {
    match probe {
        ConnectionProbe::OAuth => Ok("Connected".to_string()),
        ConnectionProbe::Gemini(api_key) => {
            verify_gemini_api_key(&api_key).map(|status| format!("Gemini API: {status}"))
        }
        ConnectionProbe::Claude(api_key) => {
            verify_claude_api_key(&api_key).map(|status| format!("Claude API: {status}"))
        }
        ConnectionProbe::OpenAI(api_key) => {
            verify_openai_api_key(&api_key).map(|status| format!("OpenAI API: {status}"))
        }
        ConnectionProbe::LocalCli => {
            verify_local_cli().map(|status| format!("Local CLI: {status}"))
        }
        ConnectionProbe::InvalidConfiguration(message) => Err(message),
    }
}

/// Builds fresh, non-secret provider evidence for the native execution lane.
///
/// The existing live provider probe is the authority for this observation;
/// only the shared readiness snapshot crosses into execution policy.
pub(crate) fn build_active_provider_readiness_snapshot(
    app: &AppWindow,
) -> RuntimeReadinessSnapshot {
    let provider_id = app.get_settings_active_ai_provider().to_string();
    let observed_at_ms = unix_time_ms();
    let connection_id = match provider_id.as_str() {
        GEMINI_PROVIDER_ID => AI_GEMINI_CONNECTION_ID,
        CLAUDE_PROVIDER_ID => AI_CLAUDE_CONNECTION_ID,
        OPENAI_PROVIDER_ID => AI_OPENAI_CONNECTION_ID,
        LOCAL_CLI_PROVIDER_ID => AI_LOCAL_CLI_CONNECTION_ID,
        _ => provider_id.as_str(),
    };
    let credential_readiness = match build_connection_probe(connection_id, app) {
        Some(probe) => {
            if run_connection_probe(probe).is_ok() {
                ProviderCredentialReadiness::Ready
            } else {
                ProviderCredentialReadiness::Unknown
            }
        }
        None => ProviderCredentialReadiness::Unknown,
    };

    RuntimeReadinessSnapshot::from_provider_observations(
        "retrospect-slint-provider-probe",
        1,
        observed_at_ms,
        observed_at_ms.saturating_add(30_000),
        [ProviderReadinessObservation {
            provider_id,
            credential_readiness,
        }],
    )
}

fn unix_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or_default()
}

fn build_ai_connection_probe(raw_api_key: String) -> Result<String, String> {
    resolve_api_key(Some(&raw_api_key)).map(|resolved| resolved.unwrap_or_default())
}

fn build_http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECONDS))
        .build()
        .map_err(|error| format!("Could not build HTTP client: {error}"))
}

fn verify_gemini_api_key(api_key: &str) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err("Gemini API key is empty.".to_string());
    }
    let client = build_http_client()?;
    let response = client
        .get("https://generativelanguage.googleapis.com/v1/models")
        .query(&[("key", api_key)])
        .send()
        .map_err(|error| format!("Gemini request failed: {error}"))?;
    let status = response.status();
    if status.is_success() {
        return Ok("reachable".to_string());
    }
    let reason = response
        .text()
        .map_err(|error| format!("Gemini response read failed: {error}"))?;
    Err(format!("status {status}: {}", reason.trim()))
}

fn verify_claude_api_key(api_key: &str) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err("Claude API key is empty.".to_string());
    }
    let client = build_http_client()?;
    let response = client
        .get("https://api.anthropic.com/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .map_err(|error| format!("Claude request failed: {error}"))?;
    let status = response.status();
    if status.is_success() {
        return Ok("reachable".to_string());
    }
    let reason = response
        .text()
        .map_err(|error| format!("Claude response read failed: {error}"))?;
    Err(format!("status {status}: {}", reason.trim()))
}

fn verify_openai_api_key(api_key: &str) -> Result<String, String> {
    if api_key.trim().is_empty() {
        return Err("OpenAI API key is empty.".to_string());
    }
    let client = build_http_client()?;
    let response = client
        .get("https://api.openai.com/v1/models")
        .bearer_auth(api_key)
        .send()
        .map_err(|error| format!("OpenAI request failed: {error}"))?;
    let status = response.status();
    if status.is_success() {
        return Ok("reachable".to_string());
    }
    let reason = response
        .text()
        .map_err(|error| format!("OpenAI response read failed: {error}"))?;
    Err(format!("status {status}: {}", reason.trim()))
}

fn local_cli_candidates() -> Vec<String> {
    let mut candidates = Vec::new();
    for env_name in ["AI_LOCAL_CLI", "RETROSPECT_LOCAL_AI_CLI", "LOCAL_AI_CLI"] {
        if let Ok(value) = env::var(env_name) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                candidates.push(trimmed.to_string());
            }
        }
    }
    candidates.extend([
        "retrospect-ai-cli".to_string(),
        "local-ai-cli".to_string(),
        "ollama".to_string(),
    ]);
    candidates
}

fn verify_local_cli() -> Result<String, String> {
    let candidates = local_cli_candidates();
    if candidates.is_empty() {
        return Err("No local CLI command configured.".to_string());
    }

    let mut last_error = String::new();
    for command in candidates {
        let result = Command::new(&command)
            .arg("--version")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output();
        match result {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !version.is_empty() {
                    return Ok(format!("{command}: {version}"));
                }
                return Ok(format!("{command}: reachable"));
            }
            Ok(output) => {
                last_error = format!("{command} returned non-zero exit status {}", output.status);
            }
            Err(error) => {
                if error.kind() == ErrorKind::NotFound {
                    last_error = format!("{command} not found");
                } else {
                    last_error = format!("{command} execution failed: {error}");
                }
            }
        }
    }

    Err(last_error)
}

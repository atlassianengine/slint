// Rust guideline compliant 2026-02-21

//! MCP Router controller delegating tool execution requests to MCP servers.

use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, CONTENT_TYPE, USER_AGENT};
use reqwest::StatusCode;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const MCP_USER_AGENT: &str = "retrospect-slint-ai-hub/0.1";
const MCP_TIMEOUT_SECONDS: u64 = 8;
const REQUEST_ID_MODULUS: u64 = 1_000_000;

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct McpPingResult {
    pub tool_count: i32,
    pub status: String,
    pub detail: String,
}

pub fn install(_app: &crate::AppWindow) {
    // MCP callbacks are wired from the orchestrator view.
}

pub fn fallback_mcp_endpoint() -> &'static str {
    "https://mcp.cloudflare.com/mcp"
}

pub fn resolve_server_endpoint(server_id: &str) -> Option<&'static str> {
    match server_id {
        "mcp-1" => Some("https://mcp.cloudflare.com/mcp"),
        "mcp-2" => Some("https://mcp.wrike.com/app/mcp/stream"),
        _ => None,
    }
}

pub fn ping_mcp_server(endpoint: &str, server_id: &str) -> Result<McpPingResult, String> {
    let request_url = resolve_mcp_url(endpoint)?;
    let payload = json!({
        "jsonrpc": "2.0",
        "id": next_request_id(),
        "method": "tools/list",
        "params": {}
    });

    let client = Client::builder()
        .timeout(Duration::from_secs(MCP_TIMEOUT_SECONDS))
        .build()
        .map_err(|error| format!("Could not build MCP client: {error}"))?;
    let response = client
        .post(request_url)
        .header(USER_AGENT, MCP_USER_AGENT)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json")
        .header("MCP-Protocol-Version", MCP_PROTOCOL_VERSION)
        .json(&payload)
        .send()
        .map_err(|error| format!("MCP request failed for {server_id}: {error}"))?;

    let status = response.status();
    let body = response
        .text()
        .map_err(|error| format!("Could not read MCP response for {server_id}: {error}"))?;

    if !status.is_success() {
        return Err(mcp_http_error_message(status, &body));
    }

    let parsed: Value = serde_json::from_str(&body)
        .map_err(|error| format!("Could not parse MCP response for {server_id}: {error}"))?;
    if let Some(error_block) = parsed.get("error") {
        return Err(format!(
            "MCP returned an error for {server_id}: {error_block}"
        ));
    }

    let result = parsed
        .get("result")
        .ok_or_else(|| format!("MCP response for {server_id} is missing result."))?;
    let tool_count = result
        .get("tools")
        .and_then(Value::as_array)
        .map(|arr| arr.len() as i32)
        .unwrap_or(0);
    Ok(McpPingResult {
        tool_count,
        status: if tool_count > 0 {
            "Reachable".to_string()
        } else {
            "Reachable (no tools)".to_string()
        },
        detail: if tool_count > 0 {
            format!("Discovered {tool_count} tool(s).")
        } else {
            "Server responded with no tools.".to_string()
        },
    })
}

fn resolve_mcp_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed =
        reqwest::Url::parse(url).map_err(|_| "MCP endpoint URL is invalid.".to_string())?;
    if parsed.scheme() != "https" {
        return Err("MCP endpoint must use https.".to_string());
    }
    if !parsed.path().contains("/mcp") {
        return Err("MCP endpoint URL must include an /mcp path segment.".to_string());
    }
    Ok(parsed)
}

fn mcp_http_error_message(status: StatusCode, body: &str) -> String {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            format!("MCP authentication rejected (HTTP {status}): {body}")
        }
        _ => format!("MCP response failed: HTTP {status}: {body}"),
    }
}

fn next_request_id() -> u64 {
    REQUEST_ID.fetch_add(1, Ordering::Relaxed) % REQUEST_ID_MODULUS
}

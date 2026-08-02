// Rust guideline compliant 2026-02-21

//! Agent Hub, Skills, MCP, and Orchestrator execution stream controller.

mod data;
mod handlers;
mod helpers;

use std::cell::RefCell;
use std::rc::Rc;

use crate::AppWindow;
use data::{build_mcp_server_endpoints, build_state};
use handlers::register_handlers;
use helpers::logs_to_string;
use slint::ModelRc;

/// Installs the Agent Hub and Orchestrator controllers onto the main Slint `AppWindow`.
pub fn install(app: &AppWindow) {
    let state = build_state();
    let logs = Rc::new(RefCell::new(vec![
        "[system] orchestrator hub ready".to_string(),
        "[system] no active executions".to_string(),
    ]));
    let selected_agent_id = Rc::new(RefCell::new("agent-1".to_string()));
    let selected_skill_id = Rc::new(RefCell::new("skill-1".to_string()));
    let run_count = Rc::new(RefCell::new(1usize));
    let mcp_server_endpoints = Rc::new(build_mcp_server_endpoints());

    app.set_orchestrator_envelopes(ModelRc::from(state.envelopes.clone()));
    app.set_orchestrator_active_agent_name("Developer Intelligence".into());
    app.set_orchestrator_execution_status("idle".into());
    app.set_orchestrator_run_progress(0.0);
    app.set_orchestrator_active_log_output(logs_to_string(&logs).into());
    app.set_orchestrator_agents(ModelRc::from(state.agents.clone()));
    app.set_orchestrator_selected_agent_id("agent-1".into());
    app.set_orchestrator_mcp_servers(ModelRc::from(state.mcp_servers.clone()));
    app.set_orchestrator_skills(ModelRc::from(state.skills.clone()));
    app.set_orchestrator_selected_skill_id("skill-1".into());
    app.set_orchestrator_active_skill_instructions(
        "SKILL.md instructions for: Codebase Search".into(),
    );

    register_handlers(
        app,
        state.envelopes.clone(),
        state.agents.clone(),
        state.mcp_servers.clone(),
        state.skills.clone(),
        logs.clone(),
        selected_agent_id.clone(),
        selected_skill_id.clone(),
        run_count.clone(),
        mcp_server_endpoints.clone(),
    );
}

// Rust guideline compliant 2026-02-21

mod agents;
mod execution;
mod mcp;
mod skills;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::{AgentProfileItem, AppWindow, ExecutionEnvelopeItem, MCPServerItem, SkillItem};
use slint::VecModel;

use agents::{register_agent_selected, register_create_agent_requested};
use execution::{
    register_ai_chat_requested, register_cancel_execution_requested, register_run_requested,
};
use mcp::{register_add_mcp_server_requested, register_ping_mcp_server_requested};
use skills::register_skill_selected;

pub(crate) fn register_handlers(
    app: &AppWindow,
    envelopes: Rc<VecModel<ExecutionEnvelopeItem>>,
    agents: Rc<VecModel<AgentProfileItem>>,
    mcp_servers: Rc<VecModel<MCPServerItem>>,
    skills: Rc<VecModel<SkillItem>>,
    logs: Rc<RefCell<Vec<String>>>,
    selected_agent_id: Rc<RefCell<String>>,
    selected_skill_id: Rc<RefCell<String>>,
    run_count: Rc<RefCell<usize>>,
    mcp_server_endpoints: Rc<RefCell<HashMap<String, String>>>,
) {
    register_run_requested(
        app,
        envelopes.clone(),
        agents.clone(),
        logs.clone(),
        run_count.clone(),
        selected_agent_id.clone(),
    );
    register_cancel_execution_requested(app, envelopes.clone(), logs.clone());
    register_agent_selected(app, agents.clone(), selected_agent_id.clone());
    register_create_agent_requested(app, agents.clone(), logs.clone());
    register_add_mcp_server_requested(
        app,
        mcp_servers.clone(),
        mcp_server_endpoints.clone(),
        logs.clone(),
    );
    register_ping_mcp_server_requested(app, mcp_servers, mcp_server_endpoints, logs.clone());
    register_skill_selected(app, skills, logs.clone(), selected_skill_id.clone());
    register_ai_chat_requested(app, envelopes, agents, logs, run_count, selected_agent_id);
}

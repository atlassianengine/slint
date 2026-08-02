// Rust guideline compliant 2026-02-21

use std::collections::HashMap;

use crate::{
    controllers::ai::mcp_router, AgentProfileItem, ExecutionEnvelopeItem, MCPServerItem, SkillItem,
};
use slint::VecModel;

use std::rc::Rc;

pub(crate) struct AgentHubState {
    pub(crate) envelopes: Rc<VecModel<ExecutionEnvelopeItem>>,
    pub(crate) agents: Rc<VecModel<AgentProfileItem>>,
    pub(crate) mcp_servers: Rc<VecModel<MCPServerItem>>,
    pub(crate) skills: Rc<VecModel<SkillItem>>,
}

pub(crate) fn build_state() -> AgentHubState {
    let envelopes = Rc::new(VecModel::from(vec![ExecutionEnvelopeItem {
        id: "seed-1".into(),
        agent_name: "Planner Agent".into(),
        task_description: "Booting orchestrator shell".into(),
        status: "Complete".into(),
        started_at: "boot".into(),
    }]));

    let agents = Rc::new(VecModel::from(vec![
        AgentProfileItem {
            id: "agent-1".into(),
            name: "Developer Intelligence".into(),
            role: "planner".into(),
            model: "gpt-4o".into(),
            status: "Idle".into(),
            active_task_count: 0,
        },
        AgentProfileItem {
            id: "agent-2".into(),
            name: "Documentation Reviewer".into(),
            role: "reviewer".into(),
            model: "claude-3-5-sonnet".into(),
            status: "Idle".into(),
            active_task_count: 0,
        },
    ]));

    let mcp_servers = Rc::new(VecModel::from(vec![
        MCPServerItem {
            id: "mcp-1".into(),
            name: "Filesystem MCP".into(),
            transport: "stdio".into(),
            tool_count: 12,
            status: "Ready".into(),
        },
        MCPServerItem {
            id: "mcp-2".into(),
            name: "Remote Graph MCP".into(),
            transport: "http".into(),
            tool_count: 6,
            status: "Ready".into(),
        },
    ]));

    let skills = Rc::new(VecModel::from(vec![
        SkillItem {
            id: "skill-1".into(),
            name: "Codebase Search".into(),
            description: "Searches indexed project artifacts for symbols and references.".into(),
            path: "skills/codebase_search".into(),
            enabled: true,
        },
        SkillItem {
            id: "skill-2".into(),
            name: "Document Summarizer".into(),
            description: "Summarizes document blocks and extracts action items.".into(),
            path: "skills/document_summary".into(),
            enabled: true,
        },
        SkillItem {
            id: "skill-3".into(),
            name: "Planner Assistant".into(),
            description: "Assists with planner surface item creation and scheduling logic.".into(),
            path: "skills/planner_assist".into(),
            enabled: false,
        },
    ]));

    AgentHubState {
        envelopes,
        agents,
        mcp_servers,
        skills,
    }
}

pub(crate) fn build_mcp_server_endpoints() -> std::cell::RefCell<HashMap<String, String>> {
    std::cell::RefCell::new(HashMap::from([
        (
            "mcp-1".to_string(),
            mcp_router::resolve_server_endpoint("mcp-1")
                .unwrap_or(mcp_router::fallback_mcp_endpoint())
                .to_string(),
        ),
        (
            "mcp-2".to_string(),
            mcp_router::resolve_server_endpoint("mcp-2")
                .unwrap_or(mcp_router::fallback_mcp_endpoint())
                .to_string(),
        ),
    ]))
}

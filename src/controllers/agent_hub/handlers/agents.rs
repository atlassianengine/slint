// Rust guideline compliant 2026-02-21

use std::cell::RefCell;
use std::rc::Rc;

use crate::{AgentProfileItem, AppWindow};
use retrospect_persistence_contracts::ExpertRecord;
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use super::super::helpers::append_log;

pub(crate) fn register_agent_selected(
    app: &AppWindow,
    agents: Rc<VecModel<AgentProfileItem>>,
    selected_agent_id: Rc<RefCell<String>>,
) {
    let app_weak = app.as_weak();
    app.on_orchestrator_agent_selected(move |agent_id| {
        let agent_id = agent_id.to_string();
        let selected_name =
            find_agent_name(&agents, &agent_id).unwrap_or_else(|| "Unknown".to_string());

        if let Some(app) = app_weak.upgrade() {
            *selected_agent_id.borrow_mut() = agent_id.clone();
            app.set_orchestrator_selected_agent_id(agent_id.into());
            app.set_orchestrator_active_agent_name(selected_name.into());
            app.set_orchestrator_execution_status("selected".into());
        }
    });
}

pub(crate) fn register_create_agent_requested(
    app: &AppWindow,
    agents: Rc<VecModel<AgentProfileItem>>,
    logs: Rc<RefCell<Vec<String>>>,
) {
    let app_weak = app.as_weak();
    app.on_orchestrator_create_agent_requested(move || {
        let next = agents.row_count() + 1;
        let id = format!("agent-{next}");
        let name = format!("Agent {next}");
        agents.push(AgentProfileItem {
            id: id.clone().into(),
            name: name.clone().into(),
            role: "custom".into(),
            model: "claude-mini".into(),
            status: "Idle".into(),
            active_task_count: 0,
        });
        append_log(&logs, &format!("[agent-manager] created {name}"));
        if let Some(app) = app_weak.upgrade() {
            app.set_orchestrator_agents(ModelRc::from(agents.clone()));
            app.set_orchestrator_selected_agent_id(id.clone().into());
            app.set_orchestrator_active_agent_name(name.into());
            app.set_orchestrator_active_log_output(
                super::super::helpers::logs_to_string(&logs).into(),
            );
        }
    });
}

fn find_agent_name(agents: &Rc<VecModel<AgentProfileItem>>, agent_id: &str) -> Option<String> {
    for idx in 0..agents.row_count() {
        if let Some(agent) = agents.row_data(idx) {
            if agent.id == agent_id {
                return Some(agent.name.to_string());
            }
        }
    }
    None
}

/// Projects a canonical ExpertRecord from the ExpertRegistry into the Slint Agent Hub.
pub(crate) fn register_expert_node_into_agent_hub(
    agents: &Rc<VecModel<AgentProfileItem>>,
    expert: &ExpertRecord,
) {
    for idx in 0..agents.row_count() {
        if let Some(agent) = agents.row_data(idx) {
            if agent.id == expert.id {
                return;
            }
        }
    }
    agents.push(AgentProfileItem {
        id: expert.id.clone().into(),
        name: expert.name.clone().into(),
        role: format!("expert:{}", expert.domain_tags.join(",")).into(),
        model: "expert_node".into(),
        status: "Library Expert".into(),
        active_task_count: expert.indexed_chunk_count as i32,
    });
}

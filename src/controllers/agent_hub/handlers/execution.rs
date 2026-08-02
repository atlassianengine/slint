// Rust guideline compliant 2026-02-21

use std::cell::RefCell;
use std::rc::Rc;

use crate::controllers::ai::agent_runner;
use crate::{AgentProfileItem, AppWindow, ExecutionEnvelopeItem};
use agent_execution_lane::{FactoryDomainPreset, RoomDebateSession, RoomDebateStatus};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use super::super::helpers::{append_log, logs_to_string, now_stamp};
use crate::controllers::settings::build_active_provider_readiness_snapshot;

pub(crate) fn register_run_requested(
    app: &AppWindow,
    envelopes: Rc<VecModel<ExecutionEnvelopeItem>>,
    agents: Rc<VecModel<AgentProfileItem>>,
    logs: Rc<RefCell<Vec<String>>>,
    run_count: Rc<RefCell<usize>>,
    selected_agent_id: Rc<RefCell<String>>,
) {
    let app_weak = app.as_weak();
    app.on_orchestrator_run_requested(move || {
        let active_agent = selected_agent_id.borrow().clone();
        let (agent_name, agent_role, agent_model, task_suffix, selected_agent_idx) =
            active_agent_snapshot(&agents, &active_agent);

        let execution_id = {
            let mut run_count = run_count.borrow_mut();
            let id = format!("run-{run_count}");
            *run_count += 1;
            id
        };

        let (policy_ok, policy_status_label, policy_reason) = if let Some(app) = app_weak.upgrade()
        {
            let provider_id = app.get_settings_active_ai_provider().to_string();
            let readiness_snapshot = build_active_provider_readiness_snapshot(&app);
            let decision = agent_runner::evaluate_orchestrator_policy(
                &agent_role,
                &provider_id,
                &readiness_snapshot,
                true,
                "workflow_step",
            );
            let _envelope = agent_runner::build_orchestrator_envelope(
                execution_id.clone(),
                &agent_name,
                &agent_model,
                decision.lane,
                &readiness_snapshot,
                true,
                &decision,
            );
            (
                decision.can_run,
                agent_runner::status_label(decision.status).to_string(),
                decision.reason,
            )
        } else {
            (
                false,
                "Blocked".to_string(),
                "UI unavailable to evaluate run policy.".to_string(),
            )
        };

        if let Some(idx) = selected_agent_idx {
            if let Some(mut agent) = agents.row_data(idx) {
                agent.status = if policy_ok {
                    "Running".into()
                } else {
                    "Blocked".into()
                };
                if policy_ok {
                    agent.active_task_count = i32::try_from(task_suffix).unwrap_or(i32::MAX);
                }
                agents.set_row_data(idx, agent);
            }
        }

        envelopes.push(ExecutionEnvelopeItem {
            id: execution_id.clone().into(),
            agent_name: agent_name.clone().into(),
            task_description: format!("Manual run #{task_suffix} from {agent_name}").into(),
            status: policy_status_label.clone().into(),
            started_at: now_stamp().into(),
        });

        if policy_ok {
            append_log(
                &logs,
                &format!(
                    "[orchestrator] started run {execution_id} for {agent_name} ({agent_model})"
                ),
            );
        } else {
            append_log(
                &logs,
                &format!(
                    "[orchestrator] blocked run {execution_id} for {agent_name}: {policy_reason}"
                ),
            );
        }

        if let Some(app) = app_weak.upgrade() {
            app.set_orchestrator_agents(ModelRc::from(agents.clone()));
            app.set_orchestrator_envelopes(ModelRc::from(envelopes.clone()));
            app.set_orchestrator_execution_status(
                if policy_ok { "running" } else { "blocked" }.into(),
            );
            app.set_orchestrator_run_progress(if policy_ok { 1.0 } else { 0.0 });
            app.set_orchestrator_active_log_output(logs_to_string(&logs).into());
            app.set_orchestrator_selected_agent_id(active_agent.into());
        }
    });
}

pub(crate) fn register_cancel_execution_requested(
    app: &AppWindow,
    envelopes: Rc<VecModel<ExecutionEnvelopeItem>>,
    logs: Rc<RefCell<Vec<String>>>,
) {
    let app_weak = app.as_weak();
    app.on_orchestrator_cancel_execution_requested(move |execution_id| {
        let mut removed = false;
        for idx in 0..envelopes.row_count() {
            if let Some(mut envelope) = envelopes.row_data(idx) {
                if envelope.id == execution_id && envelope.status != "Cancelled" {
                    envelope.status = "Cancelled".into();
                    envelopes.set_row_data(idx, envelope);
                    removed = true;
                    break;
                }
            }
        }
        if removed {
            append_log(
                &logs,
                &format!("[orchestrator] cancelled execution {execution_id}"),
            );
        }
        if let Some(app) = app_weak.upgrade() {
            if removed {
                app.set_orchestrator_execution_status("cancelled".into());
                app.set_orchestrator_run_progress(0.0);
                app.set_orchestrator_envelopes(ModelRc::from(envelopes.clone()));
                app.set_orchestrator_active_log_output(logs_to_string(&logs).into());
            }
        }
    });
}

pub(crate) fn register_ai_chat_requested(
    app: &AppWindow,
    envelopes: Rc<VecModel<ExecutionEnvelopeItem>>,
    agents: Rc<VecModel<AgentProfileItem>>,
    logs: Rc<RefCell<Vec<String>>>,
    run_count: Rc<RefCell<usize>>,
    selected_agent_id: Rc<RefCell<String>>,
) {
    let app_weak = app.as_weak();
    app.on_orchestrator_ai_chat_requested(move |message| {
        let message = message.trim().to_string();
        if message.is_empty() {
            return;
        }
        let selected = selected_agent_id.borrow().clone();
        let (agent_name, agent_role, active_agent_idx) = active_agent_for_chat(&agents, &selected);
        let run_id = {
            let mut run_count = run_count.borrow_mut();
            let id = format!("chat-{run_count}");
            *run_count += 1;
            id
        };
        let (provider_id, readiness_snapshot) = if let Some(app) = app_weak.upgrade() {
            (
                app.get_settings_active_ai_provider().to_string(),
                build_active_provider_readiness_snapshot(&app),
            )
        } else {
            (
                "unknown".to_string(),
                agent_execution_lane::RuntimeReadinessSnapshot::from_provider_observations(
                    "slint-ui-unavailable",
                    1,
                    0,
                    1,
                    std::iter::empty::<agent_execution_lane::ProviderReadinessObservation>(),
                ),
            )
        };
        let decision = agent_runner::evaluate_orchestrator_policy(
            &agent_role,
            &provider_id,
            &readiness_snapshot,
            false,
            "chat",
        );
        append_log(
            &logs,
            &format!(
                "[ai-chat] request {run_id} from {agent_name}: {message}",
                run_id = run_id,
                agent_name = agent_name,
            ),
        );
        append_log(
            &logs,
            &format!(
                "[ai-chat] policy {} — {}",
                agent_runner::status_label(decision.status),
                decision.reason
            ),
        );
        if let Some(idx) = active_agent_idx {
            if let Some(mut agent) = agents.row_data(idx) {
                agent.status = if decision.can_run {
                    "Active conversation".into()
                } else {
                    "Blocked".into()
                };
                agents.set_row_data(idx, agent);
            }
        }
        envelopes.push(ExecutionEnvelopeItem {
            id: run_id.clone().into(),
            agent_name: agent_name.clone().into(),
            task_description: format!("chat: {message}").into(),
            status: agent_runner::status_label(decision.status).into(),
            started_at: now_stamp().into(),
        });
        if let Some(app) = app_weak.upgrade() {
            app.set_orchestrator_agents(ModelRc::from(agents.clone()));
            app.set_orchestrator_envelopes(ModelRc::from(envelopes.clone()));
            app.set_orchestrator_execution_status(
                if decision.can_run {
                    "chatting"
                } else {
                    "blocked"
                }
                .into(),
            );
            app.set_orchestrator_run_progress(if decision.can_run { 1.0 } else { 0.0 });
            app.set_orchestrator_active_log_output(logs_to_string(&logs).into());
        }
    });
}

fn active_agent_snapshot(
    agents: &Rc<VecModel<AgentProfileItem>>,
    agent_id: &str,
) -> (String, String, String, u32, Option<usize>) {
    let mut agent_name = "Unknown".to_string();
    let mut agent_role = "draft".to_string();
    let mut agent_model = "default".to_string();
    let mut task_suffix = 1u32;
    let mut selected_idx = None;

    for idx in 0..agents.row_count() {
        if let Some(agent) = agents.row_data(idx) {
            if agent.id == agent_id {
                agent_name = agent.name.to_string();
                agent_role = agent.role.to_string();
                agent_model = agent.model.to_string();
                task_suffix = (agent.active_task_count.max(0) as u32).saturating_add(1);
                selected_idx = Some(idx);
                break;
            }
        }
    }
    (
        agent_name,
        agent_role,
        agent_model,
        task_suffix,
        selected_idx,
    )
}

fn active_agent_for_chat(
    agents: &Rc<VecModel<AgentProfileItem>>,
    agent_id: &str,
) -> (String, String, Option<usize>) {
    let (agent_name, agent_role, _, _, selected_idx) = active_agent_snapshot(agents, agent_id);
    (agent_name, agent_role, selected_idx)
}

/// Triggers a universal room debate session and streams turn progression to orchestrator logs.
pub(crate) fn trigger_room_debate(
    logs: &Rc<RefCell<Vec<String>>>,
    domain: FactoryDomainPreset,
    topic_title: &str,
    context_prompt: &str,
) -> RoomDebateSession {
    let session_id = format!("session-{}", now_stamp().replace(':', "-"));
    let mut session = RoomDebateSession::new(
        &session_id,
        "default-tenant",
        "default-workspace",
        domain,
        topic_title,
        context_prompt,
    );

    append_log(
        logs,
        &format!("[room-debate] Initialized debate session '{topic_title}' with 3 persona pills"),
    );

    session.status = RoomDebateStatus::Debating;
    let timestamp = now_stamp();

    for participant in &session.participants.clone() {
        let log_msg = format!(
            "[{}] ({}) evaluated prompt context.",
            participant.name, participant.role_title
        );
        session.record_turn(
            participant.id.clone(),
            log_msg.clone(),
            Vec::new(),
            timestamp.clone(),
        );
        append_log(logs, &format!("[room-debate] {}", log_msg));
    }

    session
}

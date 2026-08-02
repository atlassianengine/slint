// Rust guideline compliant 2026-02-21

//! Agent execution policy controller for the native orchestrator runtime.

use agent_execution_lane::{
    evaluate_egress_gate, evaluate_readiness_refs, readiness_block_reason,
    resolve_cycle_continuation, CycleMode, EgressClass, EgressGateResult, ExecutionLane,
    ExecutionLaneConfig, ProviderCredentialReadiness, ProviderReadinessObservation,
    ReadinessEvaluationContext, ReadinessEvaluationResult, ReadinessRef, ReadinessRefKind,
    RuntimeReadinessSnapshot,
};
use retrospect_runtime_contracts::{
    runtime_envelope, RuntimeExecutionEnvelope, RuntimeExecutionKind, RuntimeExecutionStatus,
};
use std::time::{SystemTime, UNIX_EPOCH};

/// Result from policy evaluation for one orchestrator run request.
#[derive(Debug)]
pub struct OrchestratorPolicyDecision {
    pub can_run: bool,
    pub status: RuntimeExecutionStatus,
    pub reason: String,
    pub continuation_reason: String,
    pub readiness: ReadinessEvaluationResult,
    pub tool_gate: EgressGateResult,
    pub lane: ExecutionLane,
}

pub fn install(_app: &crate::AppWindow) {
    // Native agent triggers are wired from `agent_hub`.
}

/// Map agent role to an execution lane.
pub fn lane_from_role(role: &str) -> ExecutionLane {
    if role.to_ascii_lowercase().contains("prod") {
        ExecutionLane::Production
    } else {
        ExecutionLane::Draft
    }
}

/// Compact label used by Slint rows and status pills.
pub fn status_label(status: RuntimeExecutionStatus) -> &'static str {
    match status {
        RuntimeExecutionStatus::Queued => "Queued",
        RuntimeExecutionStatus::Running => "Running",
        RuntimeExecutionStatus::Waiting => "Waiting",
        RuntimeExecutionStatus::Blocked => "Blocked",
        RuntimeExecutionStatus::Cancelled => "Cancelled",
        RuntimeExecutionStatus::Failed => "Failed",
        RuntimeExecutionStatus::Completed => "Completed",
    }
}

/// Builds a minimal contract envelope for log/tracing continuity.
pub fn build_orchestrator_envelope(
    run_id: impl Into<String>,
    agent_name: &str,
    model: &str,
    lane: ExecutionLane,
    readiness_snapshot: &RuntimeReadinessSnapshot,
    workspace_ready: bool,
    policy: &OrchestratorPolicyDecision,
) -> RuntimeExecutionEnvelope {
    let mut envelope = runtime_envelope(run_id, RuntimeExecutionKind::WorkflowRun, policy.status);
    envelope.title = Some(format!("{agent_name} orchestration run"));
    envelope.capability.model_id = Some(model.to_string());
    envelope.capability.provider_ready =
        Some(!readiness_snapshot.context.configured_credentials.is_empty());
    envelope.capability.trust_tier = Some(if lane == ExecutionLane::Production {
        "production".to_string()
    } else {
        "draft".to_string()
    });
    envelope.capability.requires_execution_workspace = matches!(lane, ExecutionLane::Production);
    envelope.capability.provider_status = Some(policy.reason.clone());
    if workspace_ready {
        envelope.scope.execution_workspace_id = Some("default-orchestrator-workspace".to_string());
    }
    envelope
}

/// Evaluate whether a requested run can proceed and under which gate.
pub fn evaluate_orchestrator_policy(
    role: &str,
    provider_id: &str,
    readiness_snapshot: &RuntimeReadinessSnapshot,
    workspace_ready: bool,
    action_hint: &str,
) -> OrchestratorPolicyDecision {
    let lane = lane_from_role(role);
    let requires_execution_workspace = lane == ExecutionLane::Production;
    let mut readiness_refs = vec![ReadinessRef {
        id: provider_id.to_string(),
        kind: ReadinessRefKind::Credential,
        label: Some("AI provider credentials".to_string()),
        required: true,
    }];

    if requires_execution_workspace {
        readiness_refs.push(ReadinessRef {
            id: "execution_workspace".to_string(),
            kind: ReadinessRefKind::Artifact,
            label: Some("Execution workspace selected".to_string()),
            required: true,
        });
    }

    let snapshot_is_fresh = is_fresh_snapshot(readiness_snapshot);
    let snapshot_context = if snapshot_is_fresh {
        readiness_snapshot.context.clone()
    } else {
        ReadinessEvaluationContext::default()
    };
    let context = ReadinessEvaluationContext {
        present_artifacts: if workspace_ready {
            vec!["execution_workspace".to_string()]
        } else {
            Vec::new()
        },
        ..snapshot_context
    };

    let readiness = evaluate_readiness_refs(&readiness_refs, &context);
    let egress = evaluate_egress_gate(
        lane,
        if requires_execution_workspace {
            EgressClass::Draft
        } else {
            EgressClass::None
        },
        action_hint,
    );

    let can_run = readiness.ready && egress.allowed;
    let status = if can_run {
        RuntimeExecutionStatus::Running
    } else {
        RuntimeExecutionStatus::Blocked
    };

    let mut reason = String::new();
    if !snapshot_is_fresh {
        reason.push_str("Provider readiness snapshot is stale.");
    }
    let readiness_reason = readiness_block_reason(&readiness);
    if !readiness_reason.is_empty() {
        reason.push_str(&readiness_reason);
    }
    if let Some(egress_reason) = egress.reason.clone() {
        if !reason.is_empty() {
            reason.push(' ');
        }
        reason.push_str(&egress_reason);
    }
    if reason.is_empty() {
        reason = "Run policy passed.".to_string();
    }

    let continuation_plan = resolve_cycle_continuation(
        &ExecutionLaneConfig {
            lane,
            egress_class: if requires_execution_workspace {
                EgressClass::Draft
            } else {
                EgressClass::None
            },
            readiness_refs,
            cycle_mode: Some(CycleMode::Manual),
            whats_next_agent_slug: None,
            auto_cycle_handoff: false,
        },
        can_run,
        false,
        &readiness,
    );

    OrchestratorPolicyDecision {
        can_run,
        status,
        reason,
        continuation_reason: continuation_plan.reason,
        readiness,
        tool_gate: egress,
        lane,
    }
}

#[expect(
    dead_code,
    reason = "Retained for legacy callers and future wiring tests"
)]
pub fn evaluate_lane_readiness(env: &RuntimeExecutionEnvelope) -> bool {
    let lane = if env.capability.trust_tier.as_deref() == Some("production") {
        ExecutionLane::Production
    } else {
        lane_from_role("draft")
    };
    let now_ms = unix_time_ms();
    let snapshot = RuntimeReadinessSnapshot::from_provider_observations(
        "legacy-runtime-envelope",
        1,
        now_ms,
        now_ms.saturating_add(30_000),
        [ProviderReadinessObservation {
            provider_id: "provider_api_key".to_string(),
            credential_readiness: if env.capability.provider_ready == Some(true) {
                ProviderCredentialReadiness::Ready
            } else {
                ProviderCredentialReadiness::Unknown
            },
        }],
    );
    let decision = evaluate_orchestrator_policy(
        if lane == ExecutionLane::Production {
            "production"
        } else {
            "draft"
        },
        "provider_api_key",
        &snapshot,
        env.scope.execution_workspace_id.is_some(),
        "workflow_step",
    );
    matches!(
        env.status,
        RuntimeExecutionStatus::Queued | RuntimeExecutionStatus::Waiting
    ) && decision.can_run
}

fn is_fresh_snapshot(snapshot: &RuntimeReadinessSnapshot) -> bool {
    let now_ms = unix_time_ms();
    !snapshot.authority_id.trim().is_empty()
        && snapshot.generation > 0
        && snapshot.expires_at_ms > snapshot.observed_at_ms
        && now_ms >= snapshot.observed_at_ms
        && now_ms <= snapshot.expires_at_ms
}

fn unix_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or_default()
}

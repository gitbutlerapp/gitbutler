//! Check command: read-only pre-edit coordination decision API.

use serde::Serialize;

use super::conflict::{self, CheckDecision, CheckReasonCode, RequiredAction};
use super::hook_common::{self, IdentitySource};
use super::stack_dependency::{self, CoordinationHints};
use crate::db::DbHandle;
use crate::types::validate_agent_id;

/// Ordered, machine-actionable steps for handling a `check` response.
pub type ActionPlan = Vec<ActionPlanStep>;

#[derive(Debug, Serialize)]
pub struct ActionPlanStep {
    pub action: RequiredAction,
    pub priority: u8,
    pub required: bool,
    pub why: String,
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinationMode {
    ExclusiveOwner,
    Blocked,
    Advisory,
    Clear,
}

/// Stable JSON response for `but-engineering check`.
#[derive(Debug, Serialize)]
pub struct CheckResponse {
    pub file_path: String,
    pub decision: CheckDecision,
    pub reason: Option<String>,
    pub blocking_agents: Vec<String>,
    pub warnings: Vec<String>,
    pub reason_code: CheckReasonCode,
    pub required_actions: Vec<RequiredAction>,
    pub coordination_hints: CoordinationHints,
    pub action_plan: ActionPlan,
    pub coordination_mode: CoordinationMode,
    pub announce_required: bool,
    pub retry_after_seconds: Option<u64>,
    pub lock_owner: Option<String>,
    pub self_agent_id: Option<String>,
    pub identity_source: IdentitySource,
}

/// Evaluate whether a file edit should proceed.
///
/// This command is intentionally read-only: it does not auto-claim files and
/// does not auto-post block messages.
pub fn execute(
    db: &DbHandle,
    file_path: String,
    agent_id: Option<String>,
    include_stack: bool,
    intent_branch: Option<String>,
) -> anyhow::Result<CheckResponse> {
    if file_path.trim().is_empty() {
        anyhow::bail!("file_path cannot be empty");
    }

    if let Some(id) = agent_id.as_deref() {
        validate_agent_id(id)?;
    }
    if let Some(branch) = intent_branch.as_deref()
        && branch.trim().is_empty()
    {
        anyhow::bail!("intent_branch cannot be empty");
    }

    let identity = hook_common::resolve_identity(db, agent_id.as_deref());
    let self_agent_id = identity.agent_id.clone();

    if matches!(identity.source, IdentitySource::Arg | IdentitySource::Env)
        && let Some(id) = self_agent_id.as_deref()
    {
        validate_agent_id(id)?;
    }

    let evaluation = conflict::evaluate_conflict(db, &file_path, self_agent_id.as_deref())?;
    let status_json_override = std::env::var("BUT_ENGINEERING_TEST_STATUS_JSON").ok();
    let coordination_hints = if include_stack {
        stack_dependency::evaluate(
            db,
            &evaluation.file_path,
            self_agent_id.as_deref(),
            intent_branch.as_deref(),
            evaluation.reason_code,
            status_json_override.as_deref(),
        )
    } else {
        CoordinationHints::empty()
    };

    let mut reason_code = evaluation.reason_code;
    let mut required_actions = evaluation.required_actions.clone();
    let lock_owner = evaluation.lock_owner.clone();
    if evaluation.decision == CheckDecision::Allow && coordination_hints.stack_dependency_detected {
        reason_code = CheckReasonCode::StackDependency;
        ensure_action(&mut required_actions, RequiredAction::ReadChannel);
        ensure_action(&mut required_actions, RequiredAction::PostCoordinationMessage);
        ensure_action(&mut required_actions, RequiredAction::ProceedWithEdit);
    }

    let (reason_code, required_actions) = reason_and_actions_with_identity(
        self_agent_id.as_deref(),
        reason_code,
        required_actions,
        evaluation.exclusive_self_claim,
    );

    let coordination_mode = derive_coordination_mode(
        evaluation.decision,
        evaluation.exclusive_self_claim,
        reason_code,
        &evaluation.warnings,
        &coordination_hints,
    );
    let announce_required = !matches!(coordination_mode, CoordinationMode::ExclusiveOwner);
    let retry_after_seconds = if matches!(coordination_mode, CoordinationMode::Blocked) {
        Some(5)
    } else {
        None
    };

    let action_plan = build_action_plan(
        &evaluation.file_path,
        reason_code,
        coordination_mode,
        self_agent_id.as_deref(),
        &evaluation.blocking_agents,
        &coordination_hints,
        include_stack,
        intent_branch.as_deref(),
    );

    Ok(CheckResponse {
        file_path: evaluation.file_path,
        decision: evaluation.decision,
        reason: evaluation.reason,
        blocking_agents: evaluation.blocking_agents,
        warnings: evaluation.warnings,
        reason_code,
        required_actions,
        coordination_hints,
        action_plan,
        coordination_mode,
        announce_required,
        retry_after_seconds,
        lock_owner,
        self_agent_id,
        identity_source: identity.source,
    })
}

fn ensure_action(actions: &mut Vec<RequiredAction>, action: RequiredAction) {
    if !actions.contains(&action) {
        actions.push(action);
    }
}

fn reason_and_actions_with_identity(
    self_agent_id: Option<&str>,
    reason_code: CheckReasonCode,
    required_actions: Vec<RequiredAction>,
    exclusive_self_claim: bool,
) -> (CheckReasonCode, Vec<RequiredAction>) {
    // Deny-on-claim should keep a stable reason for wrappers.
    if reason_code == CheckReasonCode::ClaimedByOther {
        return (reason_code, required_actions);
    }

    // Add identity_missing as an additive signal when no stronger advisory exists.
    if self_agent_id.is_none() && reason_code == CheckReasonCode::NoConflict {
        let mut actions = vec![
            RequiredAction::ReadChannel,
            RequiredAction::PostCoordinationMessage,
            RequiredAction::ProceedWithEdit,
        ];
        // Preserve any future additive actions while keeping canonical front ordering.
        for action in required_actions {
            if !actions.contains(&action) {
                actions.push(action);
            }
        }
        return (CheckReasonCode::IdentityMissing, actions);
    }

    // Even with no explicit conflict, require baseline announce/listen behavior.
    if reason_code == CheckReasonCode::NoConflict {
        if exclusive_self_claim {
            return (reason_code, vec![RequiredAction::ProceedWithEdit]);
        }
        let mut actions = required_actions;
        if !actions.contains(&RequiredAction::ReadChannel) {
            actions.insert(0, RequiredAction::ReadChannel);
        }
        if !actions.contains(&RequiredAction::PostCoordinationMessage) {
            actions.insert(1, RequiredAction::PostCoordinationMessage);
        }
        if !actions.contains(&RequiredAction::ProceedWithEdit) {
            actions.push(RequiredAction::ProceedWithEdit);
        }
        return (reason_code, actions);
    }

    // For advisory outcomes, require explicit announce/listen discipline.
    if matches!(
        reason_code,
        CheckReasonCode::MessageMention | CheckReasonCode::SemanticDependency
    ) {
        let mut actions = required_actions;
        if !actions.contains(&RequiredAction::ReadChannel) {
            actions.insert(0, RequiredAction::ReadChannel);
        }
        if !actions.contains(&RequiredAction::PostCoordinationMessage) {
            actions.insert(1, RequiredAction::PostCoordinationMessage);
        }
        if !actions.contains(&RequiredAction::ProceedWithEdit) {
            actions.push(RequiredAction::ProceedWithEdit);
        }
        return (reason_code, actions);
    }

    (reason_code, required_actions)
}

fn derive_coordination_mode(
    decision: CheckDecision,
    exclusive_self_claim: bool,
    reason_code: CheckReasonCode,
    warnings: &[String],
    coordination_hints: &CoordinationHints,
) -> CoordinationMode {
    if decision == CheckDecision::Deny {
        return CoordinationMode::Blocked;
    }
    if exclusive_self_claim {
        return CoordinationMode::ExclusiveOwner;
    }
    if reason_code != CheckReasonCode::NoConflict
        || !warnings.is_empty()
        || coordination_hints.stack_dependency_detected
    {
        return CoordinationMode::Advisory;
    }
    CoordinationMode::Clear
}

fn build_action_plan(
    file_path: &str,
    reason_code: CheckReasonCode,
    coordination_mode: CoordinationMode,
    self_agent_id: Option<&str>,
    blocking_agents: &[String],
    coordination_hints: &CoordinationHints,
    include_stack: bool,
    intent_branch: Option<&str>,
) -> ActionPlan {
    let agent = self_agent_id.unwrap_or("<id>");
    let retry_intent = intent_branch
        .filter(|v| !v.trim().is_empty())
        .or(coordination_hints.intent_branch.as_deref());

    match reason_code {
        CheckReasonCode::ClaimedByOther => {
            let blocker = blocking_agents
                .first()
                .map(String::as_str)
                .unwrap_or("<blocking-agent>");
            vec![
                step(
                    RequiredAction::PostCoordinationMessage,
                    1,
                    true,
                    format!("The file is currently claimed by {blocker}."),
                    vec![format!(
                        "but-engineering post \"@{blocker} I need {file_path}. Please release when possible.\" --agent-id {agent}"
                    )],
                ),
                step(
                    RequiredAction::WaitForRelease,
                    2,
                    true,
                    "Wait for the owner to respond or release their claim.".to_string(),
                    vec![format!("but-engineering read --agent-id {agent} --wait --timeout 5s")],
                ),
                step(
                    RequiredAction::RetryCheck,
                    3,
                    true,
                    "Retry the coordination check before editing.".to_string(),
                    vec![build_check_command(file_path, agent, include_stack, retry_intent)],
                ),
            ]
        }
        CheckReasonCode::MessageMention => vec![
            step(
                RequiredAction::ReadChannel,
                1,
                true,
                "Recent channel activity mentions this file.".to_string(),
                vec![format!("but-engineering read --agent-id {agent}")],
            ),
            step(
                RequiredAction::PostCoordinationMessage,
                2,
                true,
                "Announce your intent to reduce overlap before editing.".to_string(),
                vec![format!(
                    "but-engineering post \"I am editing {file_path}; please flag conflicts.\" --agent-id {agent}"
                )],
            ),
            step(
                RequiredAction::ProceedWithEdit,
                3,
                false,
                "Proceed if no conflicts are reported.".to_string(),
                vec![format!("but-engineering claim {file_path} --agent-id {agent}")],
            ),
        ],
        CheckReasonCode::SemanticDependency => vec![
            step(
                RequiredAction::ReadChannel,
                1,
                true,
                "Potential semantic dependency on another agent's active files.".to_string(),
                vec![format!("but-engineering read --agent-id {agent}")],
            ),
            step(
                RequiredAction::PostCoordinationMessage,
                2,
                true,
                "Notify dependent teammates before proceeding.".to_string(),
                vec![format!(
                    "but-engineering post \"I may affect dependencies around {file_path}; please flag conflicts.\" --agent-id {agent}"
                )],
            ),
            step(
                RequiredAction::ProceedWithEdit,
                3,
                false,
                "Proceed carefully after coordination.".to_string(),
                vec![format!("but-engineering claim {file_path} --agent-id {agent}")],
            ),
        ],
        CheckReasonCode::StackDependency => {
            let depends_on = if coordination_hints.depends_on_branches.is_empty() {
                "<depends_on_branches>".to_string()
            } else {
                coordination_hints.depends_on_branches.join(", ")
            };
            let base = coordination_hints
                .depends_on_branches
                .first()
                .map(String::as_str)
                .unwrap_or("<base>");
            vec![
                step(
                    RequiredAction::ReadChannel,
                    1,
                    true,
                    "Dependency branches are active for other agents.".to_string(),
                    vec![format!("but-engineering read --agent-id {agent}")],
                ),
                step(
                    RequiredAction::PostCoordinationMessage,
                    2,
                    true,
                    "Coordinate stack dependency work before editing.".to_string(),
                    vec![
                        "but status --json".to_string(),
                        format!("but-engineering post \"Coordinating dependency on {depends_on}\" --agent-id {agent}"),
                        format!("but branch new <child> -a {base}"),
                        "but commit <branch> -m \"<message>\" --json --status-after".to_string(),
                    ],
                ),
                step(
                    RequiredAction::ProceedWithEdit,
                    3,
                    false,
                    "Proceed once dependency coordination is acknowledged.".to_string(),
                    vec![format!("but-engineering claim {file_path} --agent-id {agent}")],
                ),
            ]
        }
        CheckReasonCode::IdentityMissing => vec![
            step(
                RequiredAction::ReadChannel,
                1,
                true,
                "Agent identity is missing; establish context first.".to_string(),
                vec!["but-engineering read --agent-id <id>".to_string()],
            ),
            step(
                RequiredAction::PostCoordinationMessage,
                2,
                true,
                "Post a start update once identity is set.".to_string(),
                vec!["but-engineering post \"Starting edits; please flag conflicts.\" --agent-id <id>".to_string()],
            ),
            step(
                RequiredAction::RetryCheck,
                3,
                true,
                "Retry with an explicit identity.".to_string(),
                vec![build_check_command(file_path, "<id>", include_stack, retry_intent)],
            ),
        ],
        CheckReasonCode::NoConflict => {
            if coordination_mode == CoordinationMode::ExclusiveOwner {
                vec![step(
                    RequiredAction::ProceedWithEdit,
                    1,
                    true,
                    "You already hold the active claim for this file. Proceed directly.".to_string(),
                    vec![format!("but-engineering claim {file_path} --agent-id {agent}")],
                )]
            } else {
                vec![
                    step(
                        RequiredAction::PostCoordinationMessage,
                        1,
                        true,
                        "Announce your start so teammates can react early.".to_string(),
                        vec![format!(
                            "but-engineering post \"Starting edits in {file_path}; please flag conflicts.\" --agent-id {agent}"
                        )],
                    ),
                    step(
                        RequiredAction::ReadChannel,
                        2,
                        true,
                        "Read channel updates before the first edit.".to_string(),
                        vec![format!("but-engineering read --agent-id {agent}")],
                    ),
                    step(
                        RequiredAction::ProceedWithEdit,
                        3,
                        true,
                        "No active coordination conflict detected.".to_string(),
                        vec![format!("but-engineering claim {file_path} --agent-id {agent}")],
                    ),
                ]
            }
        }
    }
}

fn step(action: RequiredAction, priority: u8, required: bool, why: String, commands: Vec<String>) -> ActionPlanStep {
    ActionPlanStep {
        action,
        priority,
        required,
        why,
        commands,
    }
}

fn build_check_command(file_path: &str, agent_id: &str, include_stack: bool, intent_branch: Option<&str>) -> String {
    let mut command = format!("but-engineering check {file_path} --agent-id {agent_id}");
    if include_stack {
        command.push_str(" --include-stack");
        if let Some(branch) = intent_branch
            && !branch.trim().is_empty()
        {
            command.push_str(" --intent-branch ");
            command.push_str(branch.trim());
        }
    }
    command
}

use std::str::FromStr;

use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_rules::{CreateRuleRequest, UpdateRuleRequest};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A simplified subset of a `but_rules::WorkspaceRule` representing a rule for assigning a Claude Code session to a stack.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClaudeSessionAssignmentRule {
    /// A unique identifier for the rule.
    pub id: String,
    /// The time when the rule was created, represented as a Unix timestamp in milliseconds.
    pub created_at: chrono::NaiveDateTime,
    /// Whether the rule is currently enabled or not.
    pub enabled: bool,
    /// The original Claude Code session id.
    pub session_id: Uuid,
    /// The Stack ID to which the session should be assigned.
    pub stack_id: StackId,
}

impl TryFrom<but_rules::WorkspaceRule> for ClaudeSessionAssignmentRule {
    type Error = anyhow::Error;

    fn try_from(rule: but_rules::WorkspaceRule) -> Result<Self, Self::Error> {
        let stack_id = rule
            .target_stack_id()
            .and_then(|id| StackId::from_str(&id).ok())
            .ok_or_else(|| anyhow::anyhow!("Rule does not have a target stack ID"))?;

        let session_id = rule
            .session_id()
            .and_then(|id| Uuid::from_str(&id).ok())
            .ok_or_else(|| anyhow::anyhow!("Rule does not have a session ID"))?;

        Ok(Self {
            id: rule.id(),
            created_at: rule.created_at(),
            enabled: rule.enabled(),
            session_id,
            stack_id,
        })
    }
}

/// Lists all Claude session assignment rules in the workspace.
pub(crate) fn list_claude_assignment_rules(
    ctx: &mut Context,
) -> anyhow::Result<Vec<ClaudeSessionAssignmentRule>> {
    let rules = but_rules::list_rules(ctx)?
        .iter()
        .map(|rule| ClaudeSessionAssignmentRule::try_from(rule.clone()))
        .filter_map(Result::ok)
        .collect();
    Ok(rules)
}

/// Updates the target stack ID of an existing Claude session assignment rule.
pub(crate) fn update_claude_assignment_rule_target(
    ctx: &mut Context,
    rule_id: String,
    stack_id: StackId,
) -> anyhow::Result<ClaudeSessionAssignmentRule> {
    let mut req: UpdateRuleRequest = but_rules::get_rule(ctx, &rule_id)?.into();
    req.action = req.action.and_then(|a| match a {
        but_rules::Action::Explicit(but_rules::Operation::Assign { target: _ }) => {
            Some(but_rules::Action::Explicit(but_rules::Operation::Assign {
                target: but_rules::StackTarget::StackId(stack_id.to_string()),
            }))
        }
        _ => None,
    });
    let rule = but_rules::update_rule(ctx, req)?;
    rule.try_into()
}

/// Creates a new Claude session assignment rule for a given session ID and stack ID.
/// Errors out if there is another rule with a ClaudeCodeHook trigger referencing the same stack ID in the action.
/// Errors out if there is another rule referencing the same session ID in a filter.
pub(crate) fn create_claude_assignment_rule(
    ctx: &mut Context,
    session_id: Uuid,
    stack_id: StackId,
) -> anyhow::Result<ClaudeSessionAssignmentRule> {
    let existing_rules = list_claude_assignment_rules(ctx)?;
    if existing_rules.iter().any(|rule| rule.stack_id == stack_id) {
        return Err(anyhow::anyhow!(
            "There is an existing WorkspaceRule triggered on ClaudeCodeHook which references stack_id: {}",
            stack_id
        ));
    }
    if existing_rules
        .iter()
        .any(|rule| rule.session_id == session_id)
    {
        return Err(anyhow::anyhow!(
            "These is an existing WorkspaceRule triggered on ClaudeCodeHook with filter on session_id: {}",
            session_id
        ));
    }

    let req = CreateRuleRequest {
        trigger: but_rules::Trigger::ClaudeCodeHook,
        filters: vec![but_rules::Filter::ClaudeCodeSessionId(
            session_id.to_string(),
        )],
        action: but_rules::Action::Explicit(but_rules::Operation::Assign {
            target: but_rules::StackTarget::StackId(stack_id.to_string()),
        }),
    };
    let rule = but_rules::create_rule(ctx, req)?;
    ClaudeSessionAssignmentRule::try_from(rule)
}

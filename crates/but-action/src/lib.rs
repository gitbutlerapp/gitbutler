//! This crate implements various automations that GitButler can perform.

use std::{
    collections::HashMap,
    fmt::{Debug, Display},
};

use but_workspace::{DiffSpec, VirtualBranchesTomlMetadata, ui::StackEntry};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::VirtualBranchesHandle;
use serde::{Deserialize, Serialize};

mod action;
mod simple;
pub use action::ActionListing;
pub use action::list_actions;
use strum::EnumString;

pub fn handle_changes(
    ctx: &mut CommandContext,
    change_summary: &str,
    external_prompt: Option<String>,
    handler: ActionHandler,
) -> anyhow::Result<Outcome> {
    match handler {
        ActionHandler::HandleChangesSimple => {
            simple::handle_changes(ctx, change_summary, external_prompt)
        }
    }
}

/// If there are multiple diffs spces where path and previous_path are the same, collapse them into one.
fn flatten_diff_specs(input: Vec<DiffSpec>) -> Vec<DiffSpec> {
    let mut output: HashMap<String, DiffSpec> = HashMap::new();
    for spec in input {
        let key = format!(
            "{}:{}",
            spec.path,
            spec.previous_path
                .clone()
                .map(|p| p.to_string())
                .unwrap_or_default()
        );
        output
            .entry(key)
            .and_modify(|e| e.hunk_headers.extend(spec.hunk_headers.clone()))
            .or_insert(spec);
    }
    output.into_values().collect()
}

/// Returns the currently applied stacks, creating one if none exists.
fn stacks_creating_if_none(
    ctx: &CommandContext,
    vb_state: &VirtualBranchesHandle,
    repo: &gix::Repository,
    perm: &mut WorktreeWritePermission,
) -> anyhow::Result<Vec<StackEntry>> {
    let meta = VirtualBranchesTomlMetadata::from_path(
        ctx.project().gb_dir().join("virtual_branches.toml"),
    )?;
    let stacks = but_workspace::stacks_v3(repo, &meta, but_workspace::StacksFilter::InWorkspace)?;
    if stacks.is_empty() {
        let template = gitbutler_stack::canned_branch_name(ctx.repo())?;
        let branch_name = gitbutler_stack::Stack::next_available_name(
            &ctx.gix_repo()?,
            vb_state,
            template,
            false,
        )?;
        let create_req = BranchCreateRequest {
            name: Some(branch_name),
            ownership: None,
            order: None,
            selected_for_changes: None,
        };
        let stack = gitbutler_branch_actions::create_virtual_branch(ctx, &create_req, perm)?;
        Ok(vec![stack])
    } else {
        Ok(stacks)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, EnumString, Default)]
#[serde(rename_all = "camelCase")]
pub enum ActionHandler {
    #[default]
    HandleChangesSimple,
}

impl Display for ActionHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    updated_branches: Vec<UpdatedBranch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedBranch {
    pub branch_name: String,
    pub new_commits: Vec<String>,
}

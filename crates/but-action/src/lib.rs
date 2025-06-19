//! This crate implements various automations that GitButler can perform.

use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use but_workspace::ui::StackEntry;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::{Target, VirtualBranchesHandle};
pub use openai::{CredentialsKind, OpenAiProvider};
use serde::{Deserialize, Serialize};

mod action;
mod gb_client;
mod generate;
mod openai;
pub mod reword;
mod serialize;
mod simple;
mod workflow;
pub use action::ActionListing;
pub use action::Source;
pub use action::list_actions;
use but_graph::VirtualBranchesTomlMetadata;
use strum::EnumString;
use uuid::Uuid;
pub use workflow::WorkflowList;
pub use workflow::list_workflows;

pub fn handle_changes(
    ctx: &mut CommandContext,
    openai: &Option<OpenAiProvider>,
    change_summary: &str,
    external_prompt: Option<String>,
    handler: ActionHandler,
    source: Source,
) -> anyhow::Result<(Uuid, Outcome)> {
    match handler {
        ActionHandler::HandleChangesSimple => {
            simple::handle_changes(ctx, openai, change_summary, external_prompt, source)
        }
    }
}

fn default_target_setting_if_none(
    ctx: &CommandContext,
    vb_state: &VirtualBranchesHandle,
) -> anyhow::Result<Target> {
    if let Ok(default_target) = vb_state.get_default_target() {
        return Ok(default_target);
    }
    // Lets do the equivalent of `git symbolic-ref refs/remotes/origin/HEAD --short` to guess the default target.

    let repo = ctx.gix_repo()?;
    let remote_name = repo
        .remote_default_name(gix::remote::Direction::Push)
        .ok_or_else(|| anyhow::anyhow!("No push remote set"))?
        .to_string();

    let mut head_ref = repo
        .find_reference(&format!("refs/remotes/{}/HEAD", remote_name))
        .map_err(|_| anyhow::anyhow!("No HEAD reference found for remote {}", remote_name))?;

    let head_commit = head_ref.peel_to_commit()?;

    let remote_refname =
        gitbutler_reference::RemoteRefname::from_str(&head_ref.name().as_bstr().to_string())?;

    let target = Target {
        branch: remote_refname,
        remote_url: "".to_string(),
        sha: head_commit.id.to_git2(),
        push_remote_name: None,
    };

    vb_state.set_default_target(target.clone())?;
    Ok(target)
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
    pub updated_branches: Vec<UpdatedBranch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedBranch {
    pub branch_name: String,
    pub new_commits: Vec<String>,
}

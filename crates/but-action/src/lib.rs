//! This crate implements various automations that GitButler can perform.

use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use but_core::TreeChange;
use but_workspace::ui::StackEntry;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::{Project, access::WorktreeWritePermission};
use gitbutler_stack::{Target, VirtualBranchesHandle};
pub use openai::{CredentialsKind, OpenAiProvider};
use serde::{Deserialize, Serialize};

mod action;
mod auto_commit;
mod branch_changes;
mod generate;
mod grouping;
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

pub(crate) const DIFF_CONTEXT_LINES: u32 = 3;

pub fn branch_changes(
    app_handle: &tauri::AppHandle,
    ctx: &mut CommandContext,
    openai: &OpenAiProvider,
    changes: Vec<TreeChange>,
) -> anyhow::Result<()> {
    branch_changes::branch_changes(app_handle, ctx, openai, changes)
}

pub fn auto_commit(
    app_handle: &tauri::AppHandle,
    ctx: &mut CommandContext,
    openai: &OpenAiProvider,
    changes: Vec<TreeChange>,
) -> anyhow::Result<()> {
    auto_commit::auto_commit(app_handle, ctx, openai, changes)
}

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

fn stacks(ctx: &CommandContext, repo: &gix::Repository) -> anyhow::Result<Vec<StackEntry>> {
    let project = ctx.project();
    if ctx.app_settings().feature_flags.ws3 {
        let meta = ref_metadata_toml(ctx.project())?;
        but_workspace::stacks_v3(repo, &meta, but_workspace::StacksFilter::InWorkspace)
    } else {
        but_workspace::stacks(
            ctx,
            &project.gb_dir(),
            repo,
            but_workspace::StacksFilter::InWorkspace,
        )
    }
}

fn ref_metadata_toml(project: &Project) -> anyhow::Result<VirtualBranchesTomlMetadata> {
    VirtualBranchesTomlMetadata::from_path(project.gb_dir().join("virtual_branches.toml"))
}

/// Returns the currently applied stacks, creating one if none exists.
fn stacks_creating_if_none(
    ctx: &CommandContext,
    vb_state: &VirtualBranchesHandle,
    repo: &gix::Repository,
    perm: &mut WorktreeWritePermission,
) -> anyhow::Result<Vec<StackEntry>> {
    let stacks = stacks(ctx, repo)?;
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

//! This crate implements various automations that GitButler can perform.

use std::{
    fmt::{Debug, Display},
    path::Path,
    str::FromStr,
};

use but_core::{TreeChange, sync::RepoExclusiveGuard};
use but_ctx::{Context, access::RepoExclusive};
use but_hunk_assignment::CommitAbsorption;
use but_oxidize::ObjectIdExt;
use but_workspace::legacy::ui::StackEntry;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_project::ProjectId;
use gitbutler_stack::{Target, VirtualBranchesHandle};
use serde::{Deserialize, Serialize};

mod action;
mod auto_commit;
mod branch_changes;
pub mod cli;
pub mod commit_format;
mod generate;
pub mod rename_branch;
pub mod reword;
mod simple;
mod workflow;
pub use action::{ActionListing, Source, list_actions};
use but_core::ref_metadata::StackId;
use strum::EnumString;
use uuid::Uuid;
pub use workflow::{WorkflowList, list_workflows};

pub fn branch_changes(
    ctx: &mut Context,
    llm: &but_llm::LLMProvider,
    changes: Vec<TreeChange>,
    model: String,
) -> anyhow::Result<()> {
    branch_changes::branch_changes(ctx, llm, changes, model)
}

#[allow(clippy::too_many_arguments)]
pub fn auto_commit(
    project_id: ProjectId,
    repo: &gix::Repository,
    project_data_dir: &Path,
    context_lines: u32,
    llm: Option<&but_llm::LLMProvider>,
    emitter: impl Fn(&str, serde_json::Value) + Send + Sync + 'static,
    absorption_plan: Vec<CommitAbsorption>,
    guard: &mut RepoExclusiveGuard,
) -> anyhow::Result<usize> {
    auto_commit::auto_commit(
        project_id,
        repo,
        project_data_dir,
        context_lines,
        llm,
        emitter,
        absorption_plan,
        guard,
    )
}

pub fn auto_commit_simple(
    repo: &gix::Repository,
    project_data_dir: &Path,
    context_lines: u32,
    llm: Option<&but_llm::LLMProvider>,
    absorption_plan: Vec<CommitAbsorption>,
    guard: &mut RepoExclusiveGuard,
) -> anyhow::Result<usize> {
    auto_commit::auto_commit_simple(repo, project_data_dir, context_lines, llm, absorption_plan, guard)
}

pub fn handle_changes(
    ctx: &mut Context,
    change_summary: &str,
    external_prompt: Option<String>,
    handler: ActionHandler,
    source: Source,
    exclusive_stack: Option<StackId>,
) -> anyhow::Result<(Uuid, Outcome)> {
    match handler {
        ActionHandler::HandleChangesSimple => {
            simple::handle_changes(ctx, change_summary, external_prompt, source, exclusive_stack)
        }
    }
}

fn default_target_setting_if_none(ctx: &Context, vb_state: &VirtualBranchesHandle) -> anyhow::Result<Target> {
    if let Ok(default_target) = vb_state.get_default_target() {
        return Ok(default_target);
    }
    // Lets do the equivalent of `git symbolic-ref refs/remotes/origin/HEAD --short` to guess the default target.

    let repo = ctx.repo.get()?;
    let remote_name = repo
        .remote_default_name(gix::remote::Direction::Push)
        .ok_or_else(|| anyhow::anyhow!("No push remote set or more than one remote"))?
        .to_string();

    let mut head_ref = repo
        .find_reference(&format!("refs/remotes/{remote_name}/HEAD"))
        .map_err(|_| anyhow::anyhow!("No HEAD reference found for remote {remote_name}"))?;

    let head_commit = head_ref.peel_to_commit()?;

    let remote_refname = gitbutler_reference::RemoteRefname::from_str(&head_ref.name().as_bstr().to_string())?;

    let target = Target {
        branch: remote_refname,
        remote_url: "".to_string(),
        sha: head_commit.id.to_git2(),
        push_remote_name: None,
    };

    vb_state.set_default_target(target.clone())?;
    Ok(target)
}

fn stacks(ctx: &Context, repo: &gix::Repository) -> anyhow::Result<Vec<StackEntry>> {
    let meta = ctx.legacy_meta()?;
    but_workspace::legacy::stacks_v3(repo, &meta, but_workspace::legacy::StacksFilter::InWorkspace, None)
}

/// Returns the currently applied stacks, creating one if none exists.
fn stacks_creating_if_none(ctx: &Context, perm: &mut RepoExclusive) -> anyhow::Result<Vec<StackEntry>> {
    let repo = &*ctx.repo.get()?;
    let stacks = stacks(ctx, repo)?;
    if stacks.is_empty() {
        let template = but_core::branch::canned_refname(repo)?;
        let branch_name = but_core::branch::find_unique_refname(repo, template.as_ref())?;
        let create_req = BranchCreateRequest {
            name: Some(branch_name.shorten().to_string()),
            order: None,
        };
        let stack = gitbutler_branch_actions::create_virtual_branch(ctx, &create_req, perm)?;
        Ok(vec![stack.into()])
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
    pub stack_id: StackId,
    pub branch_name: String,
    pub new_commits: Vec<String>,
}

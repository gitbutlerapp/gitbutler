//! This crate implements various automations that GitButler can perform.

use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use but_core::{TreeChange, UnifiedDiff};
use but_workspace::ui::StackEntry;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::access::WorktreeWritePermission;
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
    let meta = VirtualBranchesTomlMetadata::from_path(
        ctx.project().gb_dir().join("virtual_branches.toml"),
    )?;
    but_workspace::stacks_v3(repo, &meta, but_workspace::StacksFilter::InWorkspace)
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RichHunk {
    /// The diff string.
    pub diff: String,
    /// The stack ID this hunk is assigned to, if any.
    pub assigned_to_stack: Option<but_workspace::StackId>,
    /// The locks this hunk has, if any.
    pub dependency_locks: Vec<but_hunk_dependency::ui::HunkLock>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    /// The path of the file that has changed.
    pub path: String,
    /// The file change status
    pub status: String,
    /// The hunk changes in the file.
    pub hunks: Vec<RichHunk>,
}

/// Represents the status of a project, including applied stacks and file changes.
///
/// The shape of this struct is designed to be serializable and as simple as possible for use in LLM context.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatus {
    /// List of stacks applied to the project's workspace
    stacks: Vec<but_workspace::ui::StackEntry>,
    /// Unified diff changes that could be committed.
    file_changes: Vec<FileChange>,
}

pub fn get_project_status(
    ctx: &mut CommandContext,
    repo: &gix::Repository,
    filter_changes: Option<Vec<but_core::TreeChange>>,
) -> anyhow::Result<ProjectStatus> {
    let stacks = crate::stacks(ctx, repo)?;

    let worktree = but_core::diff::worktree_changes(repo)?;
    let changes = if let Some(filter) = filter_changes {
        worktree
            .changes
            .into_iter()
            .filter(|change| filter.iter().any(|f| f.path == change.path))
            .collect::<Vec<_>>()
    } else {
        worktree.changes.clone()
    };
    let diff = unified_diff_for_changes(repo, changes, crate::DIFF_CONTEXT_LINES)?;
    // Get any assignments that may have been made, which also includes any hunk locks. Assignments should be updated according to locks where applicable.
    let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
        ctx,
        true,
        None::<Vec<but_core::TreeChange>>,
        None,
    )
    .map_err(|err| serde_error::Error::new(&*err))?;

    let file_changes = get_file_changes(&diff, assignments.clone());

    Ok(ProjectStatus {
        stacks,
        file_changes,
    })
}

fn get_file_changes(
    changes: &[(TreeChange, UnifiedDiff)],
    assingments: Vec<but_hunk_assignment::HunkAssignment>,
) -> Vec<FileChange> {
    let mut file_changes = vec![];
    for (change, unified_diff) in changes.iter() {
        match unified_diff {
            but_core::UnifiedDiff::Patch { hunks, .. } => {
                let path = change.path.to_string();
                let status = match &change.status {
                    but_core::TreeStatus::Addition { .. } => "added".to_string(),
                    but_core::TreeStatus::Deletion { .. } => "deleted".to_string(),
                    but_core::TreeStatus::Modification { .. } => "modified".to_string(),
                    but_core::TreeStatus::Rename { previous_path, .. } => {
                        format!("renamed from {}", previous_path)
                    }
                };

                let hunks = hunks
                    .iter()
                    .map(|hunk| {
                        let diff = hunk.diff.to_string();
                        let assignment = assingments
                            .iter()
                            .find(|a| {
                                a.path_bytes == change.path && a.hunk_header == Some(hunk.into())
                            })
                            .map(|a| (a.stack_id, a.hunk_locks.clone()));

                        let (assigned_to_stack, dependency_locks) =
                            if let Some((stack_id, locks)) = assignment {
                                let locks = locks.unwrap_or_default();
                                (stack_id, locks)
                            } else {
                                (None, vec![])
                            };

                        RichHunk {
                            diff,
                            assigned_to_stack,
                            dependency_locks,
                        }
                    })
                    .collect::<Vec<_>>();

                file_changes.push(FileChange {
                    path,
                    status,
                    hunks,
                });
            }
            _ => continue,
        }
    }

    file_changes
}

fn unified_diff_for_changes(
    repo: &gix::Repository,
    changes: Vec<but_core::TreeChange>,
    context_lines: u32,
) -> anyhow::Result<Vec<(but_core::TreeChange, but_core::UnifiedDiff)>> {
    changes
        .into_iter()
        .map(|tree_change| {
            tree_change
                .unified_diff(repo, context_lines)
                .map(|diff| (tree_change, diff.expect("no submodule")))
        })
        .collect::<Result<Vec<_>, _>>()
}

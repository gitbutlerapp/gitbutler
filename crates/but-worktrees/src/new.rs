use std::path::PathBuf;

use anyhow::{Result, bail};
use gitbutler_command_context::CommandContext;
use gitbutler_project::{Project, access::WorktreeReadPermission};
use serde::Serialize;

use crate::{Worktree, WorktreeId, WorktreeMeta, db::save_worktree_meta, git::git_worktree_add};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub struct NewWorktreeOutcome {
    pub created: Worktree,
    /// The git branch name created for this worktree (e.g., "gitbutler/worktree/name-a")
    pub branch_name: String,
    /// The commit message (first line) of the base commit
    pub base_commit_message: Option<String>,
}

/// Generates a unique branch name by appending letters (-a, -b, -c, etc.) if needed.
///
/// Returns the deduplicated name without the "gitbutler/worktree/" prefix.
fn deduplicate_branch_name(repo: &gix::Repository, base_name: &str) -> Result<String> {
    let mut name = base_name.to_string();

    // Check if the base name already exists
    let full_ref = format!("refs/heads/gitbutler/worktree/{}", name);
    if gix::refs::FullName::try_from(full_ref.clone())
        .ok()
        .and_then(|r| repo.find_reference(&r).ok())
        .is_none()
    {
        return Ok(name);
    }

    // Try appending letters a-z
    for c in 'a'..='z' {
        name = format!("{}-{}", base_name, c);
        let full_ref = format!("refs/heads/gitbutler/worktree/{}", name);
        if gix::refs::FullName::try_from(full_ref.clone())
            .ok()
            .and_then(|r| repo.find_reference(&r).ok())
            .is_none()
        {
            return Ok(name);
        }
    }

    // If we exhausted all letters, fall back to UUID
    Ok(WorktreeId::new().as_str().to_string())
}

/// Creates a new worktree off of a branches given name.
///
/// # Parameters
/// - `name`: Optional custom name for the worktree branch. If None, generates a UUID.
pub fn worktree_new(
    ctx: &mut CommandContext,
    perm: &WorktreeReadPermission,
    refname: &gix::refs::FullNameRef,
    name: Option<String>,
) -> Result<NewWorktreeOutcome> {
    let repo = ctx.gix_repo_for_merging()?;

    let (repo, _, graph) = ctx.graph_and_meta(repo, perm)?;
    let ws = graph.to_workspace()?;
    if ws.find_segment_and_stack_by_refname(refname).is_none() {
        bail!("Branch not found in workspace");
    }

    let to_checkout = repo.find_reference(refname)?.id();

    // Determine the branch name to use
    let base_name = if let Some(custom_name) = name {
        custom_name
    } else {
        refname.shorten().to_string()
    };

    // Deduplicate the branch name
    let deduplicated_name = deduplicate_branch_name(&repo, &base_name)?;

    // Generate a new worktree ID from the deduplicated name
    let id = WorktreeId::from_bstr(deduplicated_name.clone());

    let path = worktree_path(ctx.project(), &id);
    let branch_name =
        gix::refs::PartialName::try_from(format!("gitbutler/worktree/{}", deduplicated_name))?;

    git_worktree_add(
        &ctx.project().common_git_dir()?,
        &path,
        branch_name.as_ref(),
        to_checkout.detach(),
    )?;

    let path = path.canonicalize()?;

    // Get the commit message for the base commit
    let base_commit_message = repo
        .find_object(to_checkout.detach())
        .ok()
        .and_then(|obj| obj.try_into_commit().ok())
        .and_then(|commit| commit.message().ok().map(|msg| msg.title.to_string()));

    let meta = WorktreeMeta {
        id: id.clone(),
        created_from_ref: Some(refname.to_owned()),
        base: to_checkout.detach(),
    };

    save_worktree_meta(&repo, meta)?;

    Ok(NewWorktreeOutcome {
        created: Worktree {
            id,
            created_from_ref: Some(refname.to_owned()),
            path,
            base: Some(to_checkout.detach()),
        },
        branch_name: branch_name.as_ref().to_string(),
        base_commit_message,
    })
}

fn worktree_path(project: &Project, id: &WorktreeId) -> PathBuf {
    project.gb_dir().join("worktrees").join(id.as_str())
}

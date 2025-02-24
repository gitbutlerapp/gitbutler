use std::path::PathBuf;

use anyhow::{Result, bail};
use but_ctx::{Context, access::WorktreeReadPermission};
use gitbutler_project::Project;
use serde::Serialize;

use crate::{Worktree, WorktreeId, WorktreeMeta, db::save_worktree_meta, git::git_worktree_add};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub struct NewWorktreeOutcome {
    pub created: Worktree,
}

/// Creates a new worktree off of a branches given name.
pub fn worktree_new(
    ctx: &mut Context,
    perm: &WorktreeReadPermission,
    refname: &gix::refs::FullNameRef,
) -> Result<NewWorktreeOutcome> {
    let repo = ctx.open_repo_for_merging()?;

    let (repo, _, graph) = ctx.graph_and_meta_and_repo_from_head(repo, perm)?;
    let ws = graph.to_workspace()?;
    if ws.find_segment_and_stack_by_refname(refname).is_none() {
        bail!("Branch not found in workspace");
    }

    let to_checkout = repo.find_reference(refname)?.id();

    // Generate a new worktree ID
    let id = WorktreeId::new();

    let path = worktree_path(&ctx.legacy_project, &id);
    let branch_name =
        gix::refs::PartialName::try_from(format!("gitbutler/worktree/{}", id.as_str()))?;

    git_worktree_add(
        &ctx.legacy_project.common_git_dir()?,
        &path,
        branch_name.as_ref(),
        to_checkout.detach(),
    )?;

    let path = path.canonicalize()?;

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
    })
}

fn worktree_path(project: &Project, id: &WorktreeId) -> PathBuf {
    project.gb_dir().join("worktrees").join(id.as_str())
}

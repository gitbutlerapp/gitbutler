use std::path::PathBuf;

use anyhow::{Context, Result};
use but_graph::VirtualBranchesTomlMetadata;
use but_workspace::{StacksFilter, stacks_v3};
use gitbutler_command_context::CommandContext;
use gitbutler_project::{Project, access::WorktreeReadPermission};
use serde::{Deserialize, Serialize};

use crate::{Worktree, WorktreeSource, db::save_worktree, git::git_worktree_add};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub struct NewWorktreeOutcome {
    pub created: Worktree,
}

/// Creates a new worktree off of a branches given name.
pub fn worktree_new(
    ctx: &mut CommandContext,
    _perm: &WorktreeReadPermission,
    refname: &gix::refs::PartialNameRef,
) -> Result<NewWorktreeOutcome> {
    let repo = ctx.gix_repo_for_merging()?;
    let meta = VirtualBranchesTomlMetadata::from_path(
        ctx.project().gb_dir().join("virtual_branches.toml"),
    )?;
    let stacks = stacks_v3(&repo, &meta, StacksFilter::InWorkspace, None)?;
    let head = stacks
        .into_iter()
        .flat_map(|s| s.heads)
        .find(|h| {
            gix::refs::PartialName::try_from(h.name.clone())
                .map(|n| n.as_ref() == refname)
                .unwrap_or(false)
        })
        .context("Failed to find matching head")?;

    // Used as a method of generating the path & refrence name.
    let id = uuid::Uuid::new_v4();

    let path = worktree_path(ctx.project(), id);
    let branch_name = gix::refs::PartialName::try_from(format!("gitbutler/worktree/{}", id))?;

    git_worktree_add(&ctx.project().path, &path, branch_name.as_ref(), head.tip)?;

    let worktree = Worktree {
        path: path.canonicalize()?,
        reference: gix::refs::FullName::try_from(format!("refs/heads/gitbutler/worktree/{}", id))?,
        base: head.tip,
        source: WorktreeSource::Branch(refname.to_owned()),
    };
    save_worktree(ctx, worktree.clone())?;

    Ok(NewWorktreeOutcome { created: worktree })
}

fn worktree_path(project: &Project, id: uuid::Uuid) -> PathBuf {
    project.gb_dir().join("worktrees").join(id.to_string())
}

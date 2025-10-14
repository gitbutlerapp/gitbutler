use std::path::PathBuf;

use anyhow::{Result, bail};
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
    perm: &WorktreeReadPermission,
    refname: &gix::refs::PartialNameRef,
) -> Result<NewWorktreeOutcome> {
    let repo = ctx.gix_repo_for_merging()?;

    let (repo, _, graph) = ctx.graph_and_meta(repo, perm)?;
    let ws = graph.to_workspace()?;
    let mut ws_segment_names = ws
        .stacks
        .into_iter()
        .flat_map(|s| s.segments)
        .filter_map(|s| {
            s.ref_name
                .and_then(|n| gix::refs::PartialName::try_from(n.shorten().to_owned()).ok())
        });

    if !ws_segment_names.any(|n| n.as_ref() == refname) {
        bail!("Branch not found in workspace");
    }

    let to_checkout = repo.find_reference(refname)?.id();

    // Used as a method of generating the path & refrence name.
    let id = uuid::Uuid::new_v4();

    let path = worktree_path(ctx.project(), id);
    let branch_name = gix::refs::PartialName::try_from(format!("gitbutler/worktree/{}", id))?;

    git_worktree_add(
        &ctx.project().path,
        &path,
        branch_name.as_ref(),
        to_checkout.detach(),
    )?;

    let worktree = Worktree {
        path: path.canonicalize()?,
        reference: gix::refs::FullName::try_from(format!("refs/heads/gitbutler/worktree/{}", id))?,
        base: to_checkout.detach(),
        source: WorktreeSource::Branch(refname.to_owned()),
    };
    save_worktree(ctx, worktree.clone())?;

    Ok(NewWorktreeOutcome { created: worktree })
}

fn worktree_path(project: &Project, id: uuid::Uuid) -> PathBuf {
    project.gb_dir().join("worktrees").join(id.to_string())
}

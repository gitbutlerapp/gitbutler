use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use but_ctx::{Context, access::RepoShared};
use serde::Serialize;

use crate::{Worktree, WorktreeId, WorktreeMeta, db::save_worktree_meta, git::git_worktree_add};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub struct NewWorktreeOutcome {
    pub created: Worktree,
}

/// Creates a new worktree off of workspace branch with given `refname`.
// TODO: make this plumbing to take the `but_graph::projection::Workspace` directly.
pub fn worktree_new(
    ctx: &mut Context,
    perm: &RepoShared,
    refname: &gix::refs::FullNameRef,
) -> Result<NewWorktreeOutcome> {
    let (repo, ws, _) = ctx.workspace_and_db_with_perm(perm)?;
    if !ws.refname_is_segment(refname) {
        bail!("Branch not found in workspace");
    }

    let to_checkout = repo.find_reference(refname)?.id();

    // Generate a new worktree ID
    let id = WorktreeId::generate();

    let path = worktree_workdir(&ctx.project_data_dir(), &id);
    let branch_name =
        gix::refs::PartialName::try_from(format!("gitbutler/worktree/{}", id.as_bstr()))?;

    git_worktree_add(
        repo.common_dir(),
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

/// The path at which the linked worktree should be checked out to.
fn worktree_workdir(data_dir: &Path, id: &WorktreeId) -> PathBuf {
    data_dir.join("worktrees").join(id.to_os_str())
}

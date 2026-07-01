use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use serde::Serialize;

use crate::{Worktree, WorktreeId, WorktreeMeta, db::save_worktree_meta, git::git_worktree_add};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// This gets used as a public API in the CLI so be careful when modifying.
pub struct NewWorktreeOutcome {
    pub created: Worktree,
}

/// Creates a new worktree off of workspace branch with given `refname`,
/// checked out under `data_dir`.
pub fn worktree_new(
    repo: &gix::Repository,
    ws: &but_graph::Workspace,
    data_dir: &Path,
    refname: &gix::refs::FullNameRef,
) -> Result<NewWorktreeOutcome> {
    if !ws.refname_is_segment(refname) {
        bail!("Branch not found in workspace");
    }

    let to_checkout = repo.find_reference(refname)?.id();

    // Generate a new worktree ID
    let id = WorktreeId::generate();

    let path = worktree_workdir(data_dir, &id);
    git_worktree_add(repo.common_dir(), &path, to_checkout.detach())?;

    let path = path.canonicalize()?;

    let meta = WorktreeMeta {
        id: id.clone(),
        created_from_ref: Some(refname.to_owned()),
        base: to_checkout.detach(),
    };

    save_worktree_meta(repo, meta)?;

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

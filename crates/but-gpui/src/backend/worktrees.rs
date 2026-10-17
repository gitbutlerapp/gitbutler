//! Linked worktrees, like the ones agents work in, each with its uncommitted changes. They
//! are listed when GitButler's experimental worktree support is on; archived ones are hidden.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use bstr::ByteSlice;
use but_ctx::Context;

use super::{Workspace, Worktree, open, worktree_files};

/// The active linked worktrees with their uncommitted changes, and the names of the
/// archived ones.
pub(super) fn linked(ctx: &Context) -> Result<(Vec<Worktree>, Vec<Arc<str>>)> {
    if !ctx.settings.feature_flags.worktree_manipulation {
        return Ok(Default::default());
    }
    let listing = {
        let guard = ctx.shared_worktree_access();
        but_api::worktrees::worktrees_list_with_perm(ctx, guard.read_permission())?
    };
    let repo = ctx.repo.get()?;
    let active = (listing.active.into_iter())
        .filter_map(|listed| {
            let name: Arc<str> = listed.name.to_str_lossy().into();
            let uncommitted = files_of(&repo, &name)?;
            Some(Worktree {
                name,
                path: listed.path,
                branch: listed.ref_name.map(|name| name.shorten().to_string()),
                uncommitted,
            })
        })
        .collect();
    let archived = (listing.archived.iter())
        .map(|listed| listed.name.to_str_lossy().into())
        .collect();
    Ok((active, archived))
}

/// `shown` with its uncommitted files read again, and those of its linked worktrees, for
/// when only files changed.
pub fn reload_uncommitted(path: &Path, shown: Workspace) -> Result<Workspace> {
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let worktrees = (shown.worktrees.into_iter())
        .filter_map(|mut worktree| {
            worktree.uncommitted = files_of(&repo, &worktree.name)?;
            Some(worktree)
        })
        .collect();
    Ok(Workspace {
        uncommitted: worktree_files(&repo)?,
        worktrees,
        ..shown
    })
}

/// The uncommitted files of worktree `name`, or nothing when it can't be read, as when an
/// agent just removed it: the next full load lists the worktrees as they are.
fn files_of(repo: &gix::Repository, name: &str) -> Option<Vec<super::FileChange>> {
    let files = but_workspace::worktrees::open_worktree_repo(repo, name.as_bytes().as_bstr())
        .and_then(|checkout| worktree_files(&checkout));
    files
        .inspect_err(|err| tracing::warn!("couldn't read worktree {name}: {err:#}"))
        .ok()
}

/// Hide worktree `name` from the workspace, or with `archived` false show it again. It
/// stays on disk either way.
pub fn set_archived(path: &Path, name: &str, archived: bool) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::worktrees::worktree_set_archived(&mut ctx, name.to_owned(), archived)
}

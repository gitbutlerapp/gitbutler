//! Linked worktrees, like the ones agents work in, each with its uncommitted changes. They
//! are listed when GitButler's experimental worktree support is on; archived ones are hidden.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use bstr::ByteSlice;
use but_ctx::Context;

use super::{
    FileChange, KeptWorktree, Workspace, Worktree, git_operation, head_tree, open, worktree_files,
};

/// The active linked worktrees with their uncommitted changes, and the names of the
/// archived ones.
type Listing = (Vec<Worktree>, Vec<Arc<str>>);

/// The project's [`Listing`]; `None` with GitButler's worktree support off. A worktree's
/// files in `kept` are shown again rather than read, while its `HEAD` has the tree they
/// were read against.
pub(super) fn linked(
    ctx: &Context,
    kept: &mut HashMap<Arc<str>, KeptWorktree>,
) -> Result<Option<Listing>> {
    if !ctx.settings.feature_flags.worktree_manipulation {
        return Ok(None);
    }
    let listing = {
        let guard = ctx.shared_worktree_access();
        but_api::worktrees::worktrees_list_with_perm(ctx, guard.read_permission())?
    };
    let repo = ctx.repo.get()?;
    let active = (listing.active.into_iter())
        .filter_map(|listed| {
            let name: Arc<str> = listed.name.to_str_lossy().into();
            let checkout = checkout(&repo, &name)?;
            let tree = head_tree(&checkout);
            let (uncommitted, git_operation) = match kept.remove(&name) {
                Some((files, operation, read_against)) if read_against == tree => {
                    (files, operation)
                }
                _ => read(&checkout, &name)?,
            };
            Some(Worktree {
                name,
                path: listed.path,
                branch: listed.ref_name.map(|name| name.shorten().to_string()),
                branches: Vec::new(),
                uncommitted,
                git_operation,
                tree,
                updated_at: listed.updated_at_ms.map(|ms| ms / 1000),
            })
        })
        .collect();
    let archived = (listing.archived.iter())
        .map(|listed| listed.name.to_str_lossy().into())
        .collect();
    Ok(Some((active, archived)))
}

/// `shown` with the uncommitted files read again of the main checkout, with `main`, and of
/// the `linked` worktrees, for when only files changed.
pub fn reload_uncommitted(
    path: &Path,
    mut shown: Workspace,
    main: bool,
    linked: &HashSet<Arc<str>>,
) -> Result<Workspace> {
    let started = std::time::Instant::now();
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    if main {
        shown.uncommitted = worktree_files(&repo)?;
        shown.tree = head_tree(&repo);
    }
    shown.worktrees.retain_mut(|worktree| {
        if !linked.contains(&worktree.name) {
            return true;
        }
        let Some(checkout) = checkout(&repo, &worktree.name) else {
            return false;
        };
        match read(&checkout, &worktree.name) {
            Some((files, operation)) => {
                worktree.uncommitted = files;
                worktree.git_operation = operation;
                worktree.tree = head_tree(&checkout);
                true
            }
            None => false,
        }
    });
    tracing::debug!(elapsed = ?started.elapsed(), main, ?linked, "reloaded uncommitted files");
    Ok(shown)
}

/// The uncommitted files of worktree `name` and the Git command it's partway through, or
/// nothing when it can't be read, as when an agent just removed it: the next full load lists
/// the worktrees as they are.
fn read(checkout: &gix::Repository, name: &str) -> Option<(Vec<FileChange>, Option<&'static str>)> {
    (worktree_files(checkout))
        .inspect_err(|err| tracing::warn!("couldn't read worktree {name}: {err:#}"))
        .ok()
        .map(|files| (files, git_operation(checkout)))
}

/// Worktree `name`'s own repository, to read its checkout from, or nothing when it can't be
/// opened, as [`read`] says.
fn checkout(repo: &gix::Repository, name: &str) -> Option<gix::Repository> {
    (but_workspace::worktrees::open_worktree_repo(repo, name.as_bytes().as_bstr()))
        .inspect_err(|err| tracing::warn!("couldn't open worktree {name}: {err:#}"))
        .ok()
}

/// Where each linked worktree's `HEAD` points, to tell whether an operation moved one.
pub fn heads(path: &Path) -> Vec<Option<gix::ObjectId>> {
    let Ok(repo) = gix::open(path) else {
        return Vec::new();
    };
    (repo.worktrees().into_iter().flatten())
        .map(|wt| Some(wt.into_repo().ok()?.head_id().ok()?.detach()))
        .collect()
}

/// Remove worktree `name` from disk as `git worktree remove` does, which refuses one with
/// uncommitted changes unless `force`. Its branch stays.
pub fn remove(path: &Path, name: &str, force: bool) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::worktrees::worktree_remove(&mut ctx, name.to_owned(), force)
}

/// Hide worktree `name` from the workspace, or with `archived` false show it again. It
/// stays on disk either way.
pub fn set_archived(path: &Path, name: &str, archived: bool) -> Result<()> {
    let mut ctx = open(path)?;
    but_api::worktrees::worktree_set_archived(&mut ctx, name.to_owned(), archived)
}

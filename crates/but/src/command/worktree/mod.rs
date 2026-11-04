use std::path::Path;

use bstr::BString;
use but_core::sync::RepoShared;
use but_ctx::Context;

use crate::{
    CliResult, IdMap,
    args::atoms::{BranchArg, CliIdArg},
    bad_input,
};

pub mod archive;
pub mod list;
pub mod new;
pub mod remove;

/// The refusal `E` from gix, if that is what stopped the operation behind `err`.
fn gix_refusal<E: std::error::Error + 'static>(err: &anyhow::Error) -> Option<&E> {
    err.downcast_ref::<gix::Error>()?.downcast_any_ref()
}

/// The stable name of the worktree `arg` refers to: its exact name, then its checkout path, as
/// archived worktrees have no CLI ID, then the branch it has checked out, then the CLI ID of an
/// active one. This finds worktrees whose checkout is gone too, as Git still knows those.
///
/// The name comes first because `git worktree move` keeps it, so it may also be the relative
/// path of another worktree's checkout. Like `git worktree remove`, a relative path is taken
/// from `current_dir`.
fn resolve(
    ctx: &Context,
    current_dir: &Path,
    arg: &CliIdArg,
    perm: &RepoShared,
) -> CliResult<BString> {
    let repo = ctx.repo.get()?;
    let worktrees = repo.worktrees()?;
    // A path that is gone can't be canonicalized, but it can be resolved as far as it goes.
    let real_path = |path: &Path| {
        std::fs::canonicalize(path)
            .ok()
            .or_else(|| gix::path::realpath(path).ok())
    };
    let by_path = || {
        let path = real_path(&current_dir.join(&arg.0))?;
        worktrees.iter().find(|worktree| {
            worktree
                .base()
                .ok()
                .and_then(|base| real_path(&base))
                .as_ref()
                == Some(&path)
        })
    };
    let by_branch = || {
        let branch = BranchArg(arg.0.clone()).resolve_local_branch_name().ok()?;
        worktrees.iter().find(|worktree| {
            worktree
                .id()
                .ok()
                .and_then(|name| ctx.worktree_head(name).ok().flatten())
                .and_then(|head| head.ref_name)
                .as_ref()
                == Some(&branch)
        })
    };
    if let Some(worktree) = worktrees
        .iter()
        .find(|worktree| worktree.id().is_ok_and(|id| id == arg.0.as_str()))
        .or_else(by_path)
        .or_else(by_branch)
    {
        return Ok(worktree.id()?.to_owned());
    }

    let id_map = IdMap::new_from_context(ctx, perm, None)?;
    if let Some(name) = arg.try_resolve_worktree(&repo, &id_map)? {
        return Ok(name);
    }
    Err(bad_input(format!("Could not find worktree: '{arg}'"))
        .hint("Run `but worktree list` for the worktrees and their IDs.")
        .into())
}

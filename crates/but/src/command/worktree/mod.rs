use std::path::Path;

use bstr::BString;
use but_core::sync::RepoShared;
use but_ctx::Context;

use crate::{CliResult, IdMap, args::atoms::CliIdArg, bad_input};

pub mod archive;
pub mod list;
pub mod new;
pub mod remove;

/// The stable name of the worktree `arg` refers to: its exact name, then its checkout path, as
/// archived worktrees have no CLI ID, then the CLI ID of an active one.
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
    let worktrees = ctx.worktrees_with_state()?;
    let by_path = || {
        let path = std::fs::canonicalize(current_dir.join(&arg.0)).ok()?;
        worktrees
            .iter()
            .find(|worktree| worktree.path.canonicalize().ok().as_ref() == Some(&path))
    };
    if let Some(worktree) = worktrees
        .iter()
        .find(|worktree| worktree.name == arg.0.as_bytes())
        .or_else(by_path)
    {
        return Ok(worktree.name.clone());
    }

    let repo = ctx.repo.get()?;
    let id_map = IdMap::new_from_context(ctx, perm, None)?;
    if let Some(name) = arg.try_resolve_worktree(&repo, &id_map)? {
        return Ok(name);
    }
    Err(bad_input(format!("Could not find worktree: '{arg}'"))
        .hint("Run `but worktree list` for the worktrees and their IDs.")
        .into())
}

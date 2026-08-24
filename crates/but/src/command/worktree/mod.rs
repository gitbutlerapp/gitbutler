use bstr::BString;
use but_core::sync::RepoShared;
use but_ctx::Context;

use crate::{CliResult, IdMap, args::atoms::CliIdArg, bad_input};

pub mod archive;
pub mod list;
pub mod remove;

/// The stable name of the worktree `arg` refers to: its exact name first, as archived
/// worktrees have no CLI ID, then the CLI ID of an active one.
fn resolve(ctx: &Context, arg: &CliIdArg, perm: &RepoShared) -> CliResult<BString> {
    but_api::worktrees::ensure_worktree_manipulation_enabled(ctx)?;
    if let Some(worktree) = ctx
        .worktrees_with_state()?
        .into_iter()
        .find(|worktree| worktree.name == arg.0.as_bytes())
    {
        return Ok(worktree.name);
    }

    let repo = ctx.repo.get()?;
    let id_map = IdMap::new_from_context(ctx, perm)?;
    if let Some(name) = arg.try_resolve_worktree(&repo, &id_map)? {
        return Ok(name);
    }
    Err(bad_input(format!("Could not find worktree: '{arg}'"))
        .hint("Run `but worktree list` for the worktrees and their IDs.")
        .into())
}

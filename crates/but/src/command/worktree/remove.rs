use anyhow::Result;
use bstr::BString;
use but_core::sync::{RepoExclusive, RepoShared};
use but_ctx::Context;
use serde::Serialize;

use crate::{
    CliResult,
    args::atoms::CliIdArg,
    command::worktree::resolve,
    theme::Theme,
    utils::{CliOutput, CliOutputHuman, WriteWithUtils},
};

pub fn remove(ctx: &mut Context, worktree: &CliIdArg, force: bool) -> CliResult<RemoveOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let op = RemoveOperation::resolve(ctx, guard.read_permission(), worktree, force)?;
    Ok(run(ctx, guard.write_permission(), op)?)
}

pub(crate) struct RemoveOperation {
    pub worktree: BString,
    pub force: bool,
}

impl RemoveOperation {
    pub(crate) fn resolve(
        ctx: &Context,
        perm: &RepoShared,
        worktree: &CliIdArg,
        force: bool,
    ) -> CliResult<Self> {
        Ok(Self {
            worktree: resolve(ctx, worktree, perm)?,
            force,
        })
    }
}

pub fn run(ctx: &Context, perm: &mut RepoExclusive, op: RemoveOperation) -> Result<RemoveOutcome> {
    but_api::worktrees::worktree_remove_with_perm(ctx, op.worktree.as_ref(), op.force, perm)?;
    Ok(RemoveOutcome { name: op.worktree })
}

#[must_use]
pub struct RemoveOutcome {
    name: BString,
}

impl CliOutputHuman for RemoveOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        writeln!(out, "Removed worktree {}", self.name)?;
        Ok(())
    }
}

impl CliOutput for RemoveOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        struct Output {
            name: String,
        }
        Output {
            name: self.name.to_string(),
        }
    }
}

use anyhow::Result;
use but_api::worktrees::{NewWorktree, RecycledWorktree};
use but_core::sync::RepoExclusive;
use but_ctx::Context;
use serde::Serialize;

use crate::{
    CliResult,
    theme::{self, Theme},
    utils::{CliOutput, CliOutputHuman, WriteWithUtils},
};

pub fn recycle(ctx: &mut Context) -> CliResult<RecycleOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    Ok(run(ctx, guard.write_permission())?)
}

pub fn run(ctx: &Context, perm: &mut RepoExclusive) -> Result<RecycleOutcome> {
    let recycled = but_api::worktrees::worktree_recycle_with_perm(ctx, perm)?;
    Ok(RecycleOutcome { recycled })
}

#[must_use]
pub struct RecycleOutcome {
    pub recycled: RecycledWorktree,
}

impl CliOutputHuman for RecycleOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        let (done, worktree) = match self.recycled {
            RecycledWorktree::Unarchived(worktree) => ("Recycled", worktree),
            RecycledWorktree::Created(worktree) => ("Created", worktree),
        };
        let NewWorktree {
            name,
            path,
            ref_name,
            base,
        } = worktree;
        writeln!(
            out,
            "{done} worktree {name} on {} from {} at {}",
            theme::Branch(ref_name),
            theme::Commit(base),
            path.display()
        )?;
        Ok(())
    }
}

impl CliOutput for RecycleOutcome {
    fn on_json(self) -> impl Serialize {
        self.recycled
    }
}

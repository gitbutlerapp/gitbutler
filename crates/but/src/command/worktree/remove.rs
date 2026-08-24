use bstr::BString;
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
    let name = resolve(ctx, worktree, guard.read_permission())?;
    but_api::worktrees::worktree_remove_with_perm(
        ctx,
        name.as_ref(),
        force,
        guard.write_permission(),
    )?;
    Ok(RemoveOutcome { name })
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

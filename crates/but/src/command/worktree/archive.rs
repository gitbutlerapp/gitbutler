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

pub fn set_archived(
    ctx: &Context,
    worktree: &CliIdArg,
    archived: bool,
) -> CliResult<ArchiveOutcome> {
    let guard = ctx.shared_worktree_access();
    let name = resolve(ctx, worktree, guard.read_permission())?;
    but_api::worktrees::worktree_set_archived_with_perm(
        ctx,
        name.as_ref(),
        archived,
        guard.read_permission(),
    )?;
    Ok(ArchiveOutcome { name, archived })
}

#[must_use]
pub struct ArchiveOutcome {
    name: BString,
    archived: bool,
}

impl CliOutputHuman for ArchiveOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        let verb = if self.archived {
            "archived"
        } else {
            "unarchived"
        };
        writeln!(out, "Successfully {verb} {}", self.name)?;
        Ok(())
    }
}

impl CliOutput for ArchiveOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        struct Output {
            name: String,
            archived: bool,
        }
        Output {
            name: self.name.to_string(),
            archived: self.archived,
        }
    }
}

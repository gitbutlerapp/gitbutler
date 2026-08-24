use anyhow::Result;
use bstr::BString;
use but_core::sync::RepoShared;
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
    let perm = guard.read_permission();
    let op = ArchivalOperation::resolve(ctx, perm, worktree, archived)?;
    Ok(run(ctx, perm, op)?)
}

pub(crate) struct ArchivalOperation {
    pub worktree: BString,
    /// The state of archival that the worktree should take on.
    pub archived: bool,
}

impl ArchivalOperation {
    pub(crate) fn resolve(
        ctx: &Context,
        perm: &RepoShared,
        worktree: &CliIdArg,
        archived: bool,
    ) -> CliResult<Self> {
        Ok(Self {
            worktree: resolve(ctx, worktree, perm)?,
            archived,
        })
    }
}

pub fn run(ctx: &Context, perm: &RepoShared, op: ArchivalOperation) -> Result<ArchiveOutcome> {
    but_api::worktrees::worktree_set_archived_with_perm(
        ctx,
        op.worktree.as_ref(),
        op.archived,
        perm,
    )?;
    Ok(ArchiveOutcome {
        name: op.worktree,
        archived: op.archived,
    })
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

use anyhow::Result;
use but_api::worktrees::NewWorktree;
use but_core::sync::{RepoExclusive, RepoShared};
use but_ctx::Context;
use gix::refs::FullName;
use serde::Serialize;

use crate::{
    CliResult,
    args::atoms::BranchArg,
    theme::{self, Theme},
    utils::{CliOutput, CliOutputHuman, WriteWithUtils},
};

pub fn new(ctx: &mut Context, name: Option<&BranchArg>) -> CliResult<NewOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let op = NewOperation::resolve(ctx, guard.read_permission(), name)?;
    Ok(run(ctx, guard.write_permission(), op)?)
}

pub(crate) struct NewOperation {
    /// The branch to create, or `None` for a canned name.
    pub ref_name: Option<FullName>,
}

impl NewOperation {
    pub(crate) fn resolve(
        ctx: &Context,
        perm: &RepoShared,
        name: Option<&BranchArg>,
    ) -> CliResult<Self> {
        but_api::worktrees::ensure_worktree_manipulation_enabled(ctx)?;
        let ref_name = match name {
            Some(name) => {
                let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm)?;
                Some(name.resolve_for_creation(&repo, &ws)?)
            }
            None => None,
        };
        Ok(Self { ref_name })
    }
}

pub fn run(ctx: &Context, perm: &mut RepoExclusive, op: NewOperation) -> Result<NewOutcome> {
    let created = but_api::worktrees::worktree_new_with_perm(ctx, op.ref_name, perm)?;
    Ok(NewOutcome { created })
}

#[must_use]
pub struct NewOutcome {
    created: NewWorktree,
}

impl CliOutputHuman for NewOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        let NewWorktree {
            name,
            path,
            ref_name,
            base,
        } = self.created;
        writeln!(
            out,
            "Created worktree {name} on {} from {} at {}",
            theme::Branch(ref_name),
            theme::Commit(base),
            path.display()
        )?;
        Ok(())
    }
}

impl CliOutput for NewOutcome {
    fn on_json(self) -> impl Serialize {
        self.created
    }
}

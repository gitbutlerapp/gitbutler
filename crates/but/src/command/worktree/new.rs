use anyhow::Result;
use but_api::worktrees::{NewWorktree, WorktreeCreationMode};
use but_core::sync::{RepoExclusive, RepoShared};
use but_ctx::Context;
use gix::{ObjectId, prelude::ObjectIdExt as _, refs::FullName};
use serde::Serialize;

use crate::{
    CliResult, IdMap,
    args::atoms::{BranchArg, CliIdArg},
    bad_input,
    theme::{self, Theme},
    utils::{CliOutput, CliOutputHuman, WriteWithUtils},
};

pub fn new(
    ctx: &mut Context,
    name: Option<&BranchArg>,
    above: Option<&CliIdArg>,
    mode: WorktreeCreationMode,
) -> CliResult<NewOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let op = NewOperation::resolve(ctx, guard.read_permission(), name, above, mode)?;
    Ok(run(ctx, guard.write_permission(), op)?)
}

pub(crate) struct NewOperation {
    /// The branch to create, or `None` for a canned name.
    pub ref_name: Option<FullName>,
    pub base: Option<ObjectId>,
    pub mode: WorktreeCreationMode,
}

impl NewOperation {
    pub(crate) fn resolve(
        ctx: &Context,
        perm: &RepoShared,
        name: Option<&BranchArg>,
        above: Option<&CliIdArg>,
        mode: WorktreeCreationMode,
    ) -> CliResult<Self> {
        but_api::worktrees::ensure_worktree_manipulation_enabled(ctx)?;
        let ref_name = match name {
            Some(name) => {
                let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm)?;
                Some(name.resolve_for_creation(&repo, &ws)?)
            }
            None => None,
        };
        let base = match above {
            Some(above) => {
                let repo = ctx.repo.get()?;
                let id_map = IdMap::new_from_context(ctx, perm)?;
                let base = above.resolve_commit_in_workspace(&repo, &id_map)?;
                if but_core::Commit::from_id(base.attach(&repo))?.is_conflicted() {
                    return Err(
                        bad_input("Cannot create a worktree from a conflicted commit")
                            .arg_name("--above")
                            .arg_value(&above.0)
                            .into(),
                    );
                }
                Some(base)
            }
            None => None,
        };
        Ok(Self {
            ref_name,
            base,
            mode,
        })
    }
}

pub fn run(ctx: &Context, perm: &mut RepoExclusive, op: NewOperation) -> Result<NewOutcome> {
    let created = if let Some(base) = op.base {
        but_api::worktrees::worktree_new_at_base_with_perm(ctx, op.ref_name, base, op.mode, perm)?
    } else {
        but_api::worktrees::worktree_new_with_perm(ctx, op.ref_name, op.mode, perm)?
    };
    Ok(NewOutcome { created })
}

#[must_use]
pub struct NewOutcome {
    pub created: NewWorktree,
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

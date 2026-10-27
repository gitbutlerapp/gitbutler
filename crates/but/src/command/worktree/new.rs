use anyhow::Result;
use but_api::worktrees::NewWorktree;
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
) -> CliResult<NewOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let op = NewOperation::resolve(ctx, guard.read_permission(), name, above)?;
    Ok(run(ctx, guard.write_permission(), op)?)
}

pub(crate) enum NewOperation {
    NewBranch {
        /// The branch to create, or `None` for a canned name.
        ref_name: Option<FullName>,
        base: Option<ObjectId>,
    },
    ExistingBranch(FullName),
}

impl NewOperation {
    pub(crate) fn resolve(
        ctx: &Context,
        perm: &RepoShared,
        name: Option<&BranchArg>,
        above: Option<&CliIdArg>,
    ) -> CliResult<Self> {
        if let Some(name) = name {
            let existing = name.resolve_local_branch_name()?;
            if ctx.repo.get()?.try_find_reference(&existing)?.is_some() {
                if let Some(above) = above {
                    return Err(bad_input(format!(
                        "Cannot place the existing branch '{name}' above a commit"
                    ))
                    .arg_name("--above")
                    .arg_value(&above.0)
                    .into());
                }
                return Ok(NewOperation::ExistingBranch(existing));
            }
        }
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
                let id_map = IdMap::new_from_context(ctx, perm, None)?;
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
        Ok(NewOperation::NewBranch { ref_name, base })
    }
}

pub fn run(ctx: &Context, perm: &mut RepoExclusive, op: NewOperation) -> Result<NewOutcome> {
    let created = match op {
        NewOperation::NewBranch {
            ref_name,
            base: Some(base),
        } => but_api::worktrees::worktree_new_at_base_with_perm(ctx, ref_name, base, perm)?,
        NewOperation::NewBranch {
            ref_name,
            base: None,
        } => but_api::worktrees::worktree_new_with_perm(ctx, ref_name, perm)?,
        NewOperation::ExistingBranch(ref_name) => {
            but_api::worktrees::worktree_new_on_branch_with_perm(ctx, ref_name, perm)?
        }
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

use anyhow::Result;
use but_api::worktrees::NewWorktree;
use but_core::sync::{RepoExclusive, RepoShared};
use but_ctx::Context;
use but_graph::workspace::commit::is_managed_workspace_by_message;
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
                // A worktree whose post-checkout hook failed is kept, so a retry lands here.
                let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(perm)?;
                if let Some(worktree) = ws
                    .worktrees
                    .iter()
                    .find(|worktree| worktree.ref_name.as_ref() == Some(&existing))
                {
                    return Err(bad_input(format!(
                        "Branch '{name}' is already checked out in worktree '{}'",
                        worktree.name
                    ))
                    .arg_name("<NAME>")
                    .arg_value(&name.0)
                    .hint("Run `but worktree list` for its path, or pass a new name")
                    .into());
                }
                // A branch in the workspace is refused with or without --above.
                if above.is_some() && !ws.refname_is_segment(existing.as_ref()) {
                    return Err(bad_input(format!("Branch '{name}' already exists"))
                        .arg_name("<NAME>")
                        .arg_value(&name.0)
                        .hint("Omit --above to check out the existing branch, or pass a new name")
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
                let base = above.resolve_commit(&repo, &id_map)?;
                let commit = but_core::Commit::from_id(base.attach(&repo))?;
                if commit.is_conflicted() {
                    return Err(
                        bad_input("Cannot create a worktree from a conflicted commit")
                            .arg_name("--above")
                            .arg_value(&above.0)
                            .into(),
                    );
                }
                if is_managed_workspace_by_message(commit.message.as_ref()) {
                    return Err(bad_input(
                        "Cannot create a worktree from the GitButler workspace commit",
                    )
                    .arg_name("--above")
                    .arg_value(&above.0)
                    .hint("Pass a branch or commit instead, or omit --above to start at the workspace base")
                    .into());
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

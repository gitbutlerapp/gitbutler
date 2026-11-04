use std::path::Path;

use anyhow::Result;
use bstr::{BString, ByteSlice as _};
use but_core::sync::{RepoExclusive, RepoShared};
use but_ctx::Context;
use serde::Serialize;

use crate::{
    CliResult,
    args::atoms::CliIdArg,
    bad_input,
    command::worktree::{gix_refusal, resolve},
    theme::Theme,
    utils::{CliOutput, CliOutputHuman, WriteWithUtils},
};

pub fn remove(
    ctx: &mut Context,
    current_dir: &Path,
    worktree: &CliIdArg,
    force: bool,
) -> CliResult<RemoveOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let op = RemoveOperation::resolve(ctx, guard.read_permission(), current_dir, worktree, force)?;
    run_with_hint(ctx, guard.write_permission(), op)
}

pub(crate) struct RemoveOperation {
    pub worktree: BString,
    pub force: bool,
}

impl RemoveOperation {
    pub(crate) fn resolve(
        ctx: &Context,
        perm: &RepoShared,
        current_dir: &Path,
        worktree: &CliIdArg,
        force: bool,
    ) -> CliResult<Self> {
        Ok(Self {
            worktree: resolve(ctx, current_dir, worktree, perm)?,
            force,
        })
    }
}

pub fn run(ctx: &Context, perm: &mut RepoExclusive, op: RemoveOperation) -> Result<RemoveOutcome> {
    but_api::worktrees::worktree_remove_with_perm(ctx, op.worktree.as_ref(), op.force, perm)?;
    Ok(RemoveOutcome { name: op.worktree })
}

/// Like [`run()`], but a refusal to lose what is in the checkout says what to do about it.
pub(crate) fn run_with_hint(
    ctx: &Context,
    perm: &mut RepoExclusive,
    op: RemoveOperation,
) -> CliResult<RemoveOutcome> {
    use gix::worktree::remove::Error;
    let name = op.worktree.clone();
    run(ctx, perm, op).map_err(|err| {
        let force = format!(
            "but worktree remove {} --force",
            shell_words::quote(&name.to_str_lossy())
        );
        match gix_refusal(&err) {
            Some(Error::Dirty { path }) => {
                // A detached worktree has no branch to commit to until `-b` names one.
                let detached = ctx
                    .worktree_head(name.as_ref())
                    .ok()
                    .flatten()
                    .is_some_and(|head| head.ref_name.is_none());
                let new_branch = if detached { " -b <new-branch>" } else { "" };
                bad_input(format!("Worktree {name} has uncommitted changes"))
                    .hint(format!(
                        "Run `but -C {} commit{new_branch} -m \"<message>\"` to keep them, or `{force}` to discard them",
                        shell_words::quote(&path.to_string_lossy())
                    ))
                    .into()
            }
            Some(Error::ContainsSubmodule { .. }) => {
                bad_input(format!("Worktree {name} contains an initialized submodule"))
                    .hint(format!("Run `{force}` to remove it anyway"))
                    .into()
            }
            _ => err.into(),
        }
    })
}

#[must_use]
pub struct RemoveOutcome {
    pub(crate) name: BString,
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

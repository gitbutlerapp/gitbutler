//! Implementation of the `but _publish` and `but _pull` commands.

use std::path::Path;

use but_api::hosted::{HostedBranch, LocalHome, OnConflict, SyncOutcome};
use but_ctx::Context;

use crate::{
    CliResult,
    args::publish::{Conflict, Platform, PullPlatform},
    error::bad_input,
    theme::Theme,
    utils::{
        CliOutput, CliOutputHuman, Confirm, ConfirmDefault, IntermediateChannel, WriteWithUtils,
    },
};

pub fn publish(
    ctx: &mut Context,
    out: IntermediateChannel<'_>,
    args: Platform,
    current_dir: &Path,
) -> CliResult<Outcome> {
    let Platform {
        branch,
        include_uncommitted,
        conflict,
    } = args;
    let branch = match branch {
        Some(branch) => branch,
        // The branch checked out where this runs, where the context always means the main worktree.
        None => match gix::discover(current_dir)?.head_name()? {
            Some(head) if head.as_bstr() != but_core::WORKSPACE_REF_NAME => {
                head.shorten().to_string()
            }
            _ => {
                return Err(bad_input("Name the branch to publish").into());
            }
        },
    };
    resolve_choice(out, conflict, |choice| {
        but_api::hosted::hosted_branch_publish(ctx, branch.clone(), include_uncommitted, choice)
    })
}

pub fn pull(
    ctx: &mut Context,
    out: IntermediateChannel<'_>,
    args: PullPlatform,
) -> CliResult<Outcome> {
    let PullPlatform {
        branch,
        into_workspace,
        conflict,
    } = args;
    Ok(match branch {
        None => Outcome::Listed(but_api::hosted::hosted_branches(ctx)?),
        Some(branch) => resolve_choice(out, conflict, |choice| {
            but_api::hosted::hosted_branch_pull(ctx, branch.clone(), into_workspace, choice)
        })?,
    })
}

/// Run `sync`, and when it would replace work, ask, or use the choice from the flags.
fn resolve_choice(
    mut out: IntermediateChannel<'_>,
    conflict: Conflict,
    mut sync: impl FnMut(Option<OnConflict>) -> anyhow::Result<SyncOutcome>,
) -> CliResult<Outcome> {
    let flagged = match (conflict.overwrite, conflict.keep) {
        (true, _) => Some(OnConflict::Overwrite),
        (_, true) => Some(OnConflict::Keep),
        _ => None,
    };
    let reason = match sync(flagged)? {
        SyncOutcome::Done(done) => return Ok(Outcome::Done(done)),
        SyncOutcome::NeedsChoice(reason) => reason,
    };
    let Some(mut input) = out.prepare_for_terminal_input() else {
        return Err(bad_input(reason).hint("Pass --overwrite or --keep").into());
    };
    let choice = match input.confirm(format!("{reason} Overwrite?"), ConfirmDefault::No)? {
        Confirm::Yes => OnConflict::Overwrite,
        Confirm::No => OnConflict::Keep,
    };
    match sync(Some(choice))? {
        SyncOutcome::Done(done) | SyncOutcome::NeedsChoice(done) => Ok(Outcome::Done(done)),
    }
}

/// The published branches, or what publishing or pulling did, in a sentence.
#[must_use]
pub enum Outcome {
    Listed(Vec<HostedBranch>),
    Done(String),
}

impl CliOutputHuman for Outcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        match self {
            Outcome::Listed(branches) if branches.is_empty() => {
                writeln!(out, "Nothing is published for this project")?;
            }
            Outcome::Listed(branches) => {
                for HostedBranch {
                    branch,
                    uncommitted,
                    local,
                } in branches
                {
                    let uncommitted = if uncommitted {
                        ", with uncommitted changes"
                    } else {
                        ""
                    };
                    let local = match local {
                        LocalHome::None => "",
                        LocalHome::Worktree(_) => " (local: in a worktree)",
                        LocalHome::Workspace => " (local: in the workspace)",
                        LocalHome::Branch => " (local: a branch)",
                    };
                    writeln!(out, "{branch}{uncommitted}{local}")?;
                }
            }
            Outcome::Done(done) => writeln!(out, "{done}")?,
        }
        Ok(())
    }
}

impl CliOutput for Outcome {
    fn on_json(self) -> impl serde::Serialize {
        match self {
            Outcome::Listed(branches) => serde_json::json!({ "branches": branches }),
            Outcome::Done(done) => serde_json::json!({ "done": done }),
        }
    }
}

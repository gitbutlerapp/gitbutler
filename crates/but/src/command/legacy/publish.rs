//! Implementation of the `but _publish` and `but _pull` commands.

use std::path::Path;

use but_api::hosted::{HostedMachine, LocalHome, MachineBranch, OnConflict, SyncOutcome};
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

pub fn publish(ctx: &mut Context, args: Platform, current_dir: &Path) -> CliResult<Outcome> {
    let Platform {
        branch,
        include_uncommitted,
        to,
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
    Ok(Outcome::Done(match to {
        Some(to) => but_api::hosted::hosted_branch_send(ctx, branch, to, include_uncommitted)?,
        None => but_api::hosted::hosted_branch_publish(ctx, branch, include_uncommitted)?,
    }))
}

pub fn pull(
    ctx: &mut Context,
    out: IntermediateChannel<'_>,
    args: PullPlatform,
) -> CliResult<Outcome> {
    let PullPlatform {
        branch,
        from,
        into_workspace,
        conflict,
    } = args;
    let machines = but_api::hosted::hosted_machines(ctx)?.machines;
    let Some(branch) = branch else {
        return Ok(Outcome::Listed(machines));
    };
    let mut publishers = machines
        .iter()
        .filter(|machine| from.as_ref().is_none_or(|from| *from == machine.name))
        .filter(|machine| machine.branches.iter().any(|b| b.branch == branch))
        .map(|machine| machine.name.clone());
    let machine = match (publishers.next(), publishers.next()) {
        (Some(machine), None) => machine,
        (None, _) => match from {
            Some(from) => return Err(bad_input(format!("{from} hasn't published {branch}")).into()),
            None => return Err(bad_input(format!("No other machine published {branch}")).into()),
        },
        (Some(_), Some(_)) => {
            return Err(
                bad_input(format!("More than one machine published {branch}"))
                    .hint("Pass --from with the machine to pull it from")
                    .into(),
            );
        }
    };
    resolve_choice(out, conflict, |choice| {
        but_api::hosted::hosted_branch_pull(
            ctx,
            machine.clone(),
            branch.clone(),
            into_workspace,
            choice,
        )
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

/// The other machines and what they published, or what publishing or pulling did, in a sentence.
#[must_use]
pub enum Outcome {
    Listed(Vec<HostedMachine>),
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
            Outcome::Listed(machines) if machines.is_empty() => {
                writeln!(out, "No other machine published to this project")?;
            }
            Outcome::Listed(machines) => {
                for HostedMachine { name, branches, .. } in machines {
                    writeln!(out, "{name}")?;
                    for MachineBranch {
                        branch,
                        uncommitted,
                        local,
                        sent,
                        ..
                    } in branches
                    {
                        let uncommitted = if uncommitted.is_some() {
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
                        let sent = if sent { " (sent to you)" } else { "" };
                        writeln!(out, "  {branch}{sent}{uncommitted}{local}")?;
                    }
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
            Outcome::Listed(machines) => serde_json::json!({ "machines": machines }),
            Outcome::Done(done) => serde_json::json!({ "done": done }),
        }
    }
}

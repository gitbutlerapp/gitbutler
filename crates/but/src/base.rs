use std::io::Write;

use crate::LegacyProject;
use colored::Colorize;
use gitbutler_branch_actions::upstream_integration::{
    BranchStatus::{Conflicted, Empty, Integrated, SaflyUpdatable},
    Resolution, ResolutionApproach,
    StackStatuses::{UpToDate, UpdatesRequired},
};

#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Subcommands,
}
#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Fetches remotes from the remote and checks the mergeability of the branches in the workspace.
    Check,
    /// Updates the workspace (with all applied branches) to include the latest changes from the base branch.
    Update,
}

pub fn handle(cmd: Subcommands, project: &LegacyProject, json: bool) -> anyhow::Result<()> {
    let mut stdout = std::io::stdout();
    match cmd {
        Subcommands::Check => {
            if !json {
                writeln!(stdout, "🔍 Checking base branch status...").ok();
            }
            let base_branch = but_api::virtual_branches::fetch_from_remotes(
                project.id,
                Some("auto".to_string()),
            )?;
            writeln!(stdout, "\n📍 Base branch:\t\t{}", base_branch.branch_name).ok();
            writeln!(
                stdout,
                "⏫ Upstream commits:\t{} new commits on {}\n",
                base_branch.behind, base_branch.branch_name
            )
            .ok();
            let commits = base_branch.recent_commits.iter().take(3);
            for commit in commits {
                writeln!(
                    stdout,
                    "\t{} {}",
                    &commit.id[..7],
                    &commit
                        .description
                        .to_string()
                        .replace('\n', " ")
                        .chars()
                        .take(72)
                        .collect::<String>()
                )
                .ok();
            }
            let hidden_commits = base_branch.behind.saturating_sub(3);
            if hidden_commits > 0 {
                writeln!(
                    stdout,
                    "\t... ({hidden_commits} more - run `but base check --all` to see all)"
                )
                .ok();
            }

            let status =
                but_api::virtual_branches::upstream_integration_statuses(project.id, None)?;

            match status {
                UpToDate => _ = writeln!(stdout, "\n✅ Everything is up to date").ok(),
                UpdatesRequired {
                    worktree_conflicts,
                    statuses,
                } => {
                    if !worktree_conflicts.is_empty() {
                        writeln!(stdout,
                            "\n❗️ There are uncommitted changes in the worktree that may conflict with the updates."
                        ).ok();
                    }
                    if !statuses.is_empty() {
                        writeln!(stdout, "\n{}", "Active Branch Status".bold()).ok();
                        for (_id, status) in statuses {
                            for bs in status.branch_statuses {
                                let status_icon = match bs.status {
                                    SaflyUpdatable => "✅".to_string(),
                                    Integrated => "🔄".to_string(),
                                    Conflicted { rebasable } => {
                                        if rebasable {
                                            "⚠️".to_string()
                                        } else {
                                            "❗️".to_string()
                                        }
                                    }
                                    Empty => "✅".to_string(),
                                };
                                let status_text = match bs.status {
                                    SaflyUpdatable => "Updatable".green(),
                                    Integrated => "Integrated".blue(),
                                    Conflicted { rebasable } => {
                                        if rebasable {
                                            "Conflicted (Rebasable)".yellow()
                                        } else {
                                            "Conflicted (Not Rebasable)".red()
                                        }
                                    }
                                    Empty => "Nothing to do".normal(),
                                };
                                writeln!(stdout, "\n{} {} ({})", status_icon, bs.name, status_text)
                                    .ok();
                            }
                        }
                    }
                }
            }
            writeln!(stdout, "\nRun `but base update` to update your branches").ok();
            Ok(())
        }
        Subcommands::Update => {
            let status =
                but_api::virtual_branches::upstream_integration_statuses(project.id, None)?;
            let resolutions = match status {
                UpToDate => {
                    writeln!(stdout, "✅ Everything is up to date").ok();
                    None
                }
                UpdatesRequired {
                    worktree_conflicts,
                    statuses,
                } => {
                    if !worktree_conflicts.is_empty() {
                        writeln!(stdout,
                            "❗️ There are uncommitted changes in the worktree that may conflict with
                            the updates. Please commit or stash them and try again."
                        )
                        .ok();
                        None
                    } else {
                        writeln!(stdout, "🔄 Updating branches...").ok();
                        let mut resolutions = vec![];
                        for (maybe_stack_id, status) in statuses {
                            let Some(stack_id) = maybe_stack_id else {
                                writeln!(
                                    stdout,
                                    "No stack ID, assuming we're on single-branch mode...",
                                )
                                .ok();
                                continue;
                            };
                            let approach = if status
                                .branch_statuses
                                .iter()
                                .all(|s| s.status == gitbutler_branch_actions::upstream_integration::BranchStatus::Integrated)
                            && status.tree_status != gitbutler_branch_actions::upstream_integration::TreeStatus::Conflicted
                            {
                                    ResolutionApproach::Delete
                                } else {
                                    ResolutionApproach::Rebase
                                };
                            let resolution = Resolution {
                                stack_id,
                                approach,
                                delete_integrated_branches: true,
                                force_integrated_branches: vec![],
                            };
                            resolutions.push(resolution);
                        }
                        Some(resolutions)
                    }
                }
            };

            if let Some(resolutions) = resolutions {
                but_api::virtual_branches::integrate_upstream(project.id, resolutions, None)?;
            }
            Ok(())
        }
    }
}

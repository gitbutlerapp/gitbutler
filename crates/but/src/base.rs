use colored::Colorize;
use gitbutler_branch_actions::upstream_integration::{
    BranchStatus::{Conflicted, Empty, Integrated, SaflyUpdatable},
    Resolution, ResolutionApproach,
    StackStatuses::{UpToDate, UpdatesRequired},
};
use gitbutler_project::Project;

#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Subcommands,
}
#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Fetches remotes from the remote and checks the mergeability of the branches in the workspace.
    Check {
        /// Show all upstream commits instead of just the first 3
        #[clap(long)]
        all: bool,
    },
    /// Updates the workspace (with all applied branches) to include the latest changes from the base branch.
    Update,
}

pub fn handle(cmd: &Subcommands, project: &Project, json: bool) -> anyhow::Result<()> {
    match cmd {
        Subcommands::Check { all } => {
            if !json {
                println!("🔍 Checking base branch status...");
            }
            let base_branch = but_api::virtual_branches::fetch_from_remotes(
                project.id,
                Some("auto".to_string()),
            )?;
            println!("\n📍 Base branch:\t\t{}", base_branch.branch_name);
            println!(
                "⏫ Upstream commits:\t{} new commits on {}\n",
                base_branch.behind, base_branch.branch_name
            );

            let total_upstream_commits = base_branch.upstream_commits.len();
            let commits_to_show = if *all {
                total_upstream_commits
            } else {
                total_upstream_commits.min(3)
            };

            for commit in base_branch.upstream_commits.iter().take(commits_to_show) {
                println!(
                    "\t{} {}",
                    &commit.id[..7],
                    &commit
                        .description
                        .to_string()
                        .replace('\n', " ")
                        .chars()
                        .take(72)
                        .collect::<String>()
                );
            }
            let hidden_commits = total_upstream_commits.saturating_sub(3);
            if hidden_commits > 0 && !*all {
                println!("\t... ({hidden_commits} more - run `but base check --all` to see all)");
            }

            let status =
                but_api::virtual_branches::upstream_integration_statuses(project.id, None)?;

            match status {
                UpToDate => println!("\n✅ Everything is up to date"),
                UpdatesRequired {
                    worktree_conflicts,
                    statuses,
                } => {
                    if !worktree_conflicts.is_empty() {
                        println!(
                            "\n❗️ There are uncommitted changes in the worktree that may conflict with the updates."
                        );
                    }
                    if !statuses.is_empty() {
                        println!("\n{}", "Active Branch Status".bold());
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
                                println!("\n{} {} ({})", status_icon, bs.name, status_text);
                            }
                        }
                    }
                }
            }
            println!("\nRun `but base update` to update your branches");
            Ok(())
        }
        Subcommands::Update => {
            let status =
                but_api::virtual_branches::upstream_integration_statuses(project.id, None)?;
            let resolutions = match status {
                UpToDate => {
                    println!("✅ Everything is up to date");
                    None
                }
                UpdatesRequired {
                    worktree_conflicts,
                    statuses,
                } => {
                    if !worktree_conflicts.is_empty() {
                        println!(
                            "❗️ There are uncommitted changes in the worktree that may conflict with
                            the updates. Please commit or stash them and try again."
                        );
                        None
                    } else {
                        println!("🔄 Updating branches...");
                        let mut resolutions = vec![];
                        for (maybe_stack_id, status) in statuses {
                            let Some(stack_id) = maybe_stack_id else {
                                println!("No stack ID, assuming we're on single-branch mode...",);
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

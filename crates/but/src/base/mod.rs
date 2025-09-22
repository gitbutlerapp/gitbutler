use std::path::Path;

use colored::Colorize;
use gitbutler_branch_actions::upstream_integration::{
    BranchStatus::{Conflicted, Empty, Integrated, SaflyUpdatable},
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
    Check,
    /// Updates the worspace (with all applied branches) to include the latest changes from the base branch.
    Update,
}

pub fn handle(cmd: &Subcommands, repo_path: &Path, json: bool) -> anyhow::Result<()> {
    match cmd {
        Subcommands::Check => {
            let project = Project::find_by_path(repo_path)?;
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
            let commits = base_branch.recent_commits.iter().take(3);
            for commit in commits {
                println!(
                    "\t{} {}",
                    &commit.id[..7],
                    &commit.description.to_string().replace('\n', " ")[..72]
                );
            }
            let hidden_commits = base_branch.behind.saturating_sub(3);
            if hidden_commits > 0 {
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
            // metrics_if_configured(app_settings, CommandName::Log, p).ok();
            Ok(())
        }
    }
}

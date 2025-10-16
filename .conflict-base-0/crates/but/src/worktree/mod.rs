use anyhow::{Context, Result};
use but_api::worktree::IntegrationStatus;
use std::path::PathBuf;

#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Create a new worktree from a reference
    New {
        /// The reference (branch, commit, etc.) to create the worktree from
        reference: String,
    },
    /// List all worktrees
    List,
    /// Integrate a worktree
    Integrate {
        /// The path to the worktree to integrate
        path: String,
        /// The target reference to integrate into (defaults to the reference the worktree was created from)
        #[clap(long)]
        target: Option<String>,
        /// Perform a dry run without making changes
        #[clap(long)]
        dry: bool,
    },
}
pub fn handle(cmd: &Subcommands, project: &gitbutler_project::Project, json: bool) -> Result<()> {
    match handle_inner(cmd, project, json) {
        Ok(_) => Ok(()),
        Err(e) => {
            eprintln!("{:?}", e);
            Err(e)
        }
    }
}

pub fn handle_inner(
    cmd: &Subcommands,
    project: &gitbutler_project::Project,
    json: bool,
) -> Result<()> {
    match cmd {
        Subcommands::New { reference } => {
            // Naivly append refs/heads/ if it's not present to always have a
            // full reference.
            let reference = if reference.starts_with("refs/heads/") {
                gix::refs::FullName::try_from(reference.clone())?
            } else {
                gix::refs::FullName::try_from(format!("refs/heads/{}", reference))?
            };
            let output = but_api::worktree::worktree_new(project.id, reference)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("Created worktree at: {}", output.created.path.display());
                if let Some(reference) = output.created.created_from_ref {
                    println!("Reference: {}", reference);
                }
            }
            Ok(())
        }
        Subcommands::List => {
            let output = but_api::worktree::worktree_list(project.id)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else if output.entries.is_empty() {
                println!("No worktrees found");
            } else {
                for entry in &output.entries {
                    println!("Path: {}", entry.path.display());
                    if let Some(reference) = &entry.created_from_ref {
                        println!("Reference: {}", reference);
                    }
                    if let Some(base) = entry.base {
                        println!("Base: {}", base);
                    }
                    println!();
                }
            }
            Ok(())
        }
        Subcommands::Integrate { path, target, dry } => {
            let path = PathBuf::from(path);

            // Determine the target reference
            let target_ref = if let Some(target_str) = target {
                // User specified a target - parse it
                if target_str.starts_with("refs/") {
                    gix::refs::FullName::try_from(target_str.clone())?
                } else {
                    // Assume it's a branch name and prepend refs/heads/
                    gix::refs::FullName::try_from(format!("refs/heads/{}", target_str))?
                }
            } else {
                // No target specified - get it from the worktree metadata
                // First, we need to get the worktree metadata to find what reference it was created from
                let worktree_list = but_api::worktree::worktree_list(project.id)?;
                let worktree_entry = worktree_list
                    .entries
                    .iter()
                    .find(|e| e.path == path)
                    .context("Worktree not found - path does not match any known worktree")?;

                worktree_entry.created_from_ref.clone().context(
                    "Worktree does not have a created_from_ref - please specify --target",
                )?
            };

            if *dry {
                // Dry run - check integration status
                let status = but_api::worktree::worktree_integration_status(
                    project.id,
                    path.clone(),
                    target_ref.clone(),
                )?;

                if json {
                    println!("{}", serde_json::to_string_pretty(&status)?);
                } else {
                    println!("Integration status for worktree at: {}", path.display());
                    println!("Target: {}", target_ref);
                    match status {
                        IntegrationStatus::NoMergeBaseFound => {
                            println!("Status: Cannot integrate - no merge base found");
                        }
                        IntegrationStatus::WorktreeIsBare => {
                            println!("Status: Cannot integrate - worktree is bare");
                        }
                        IntegrationStatus::CausesWorkspaceConflicts => {
                            println!("Status: Cannot integrate - would cause workspace conflicts");
                        }
                        IntegrationStatus::Integratable {
                            cherry_pick_conflicts,
                            commits_above_conflict,
                            working_dir_conflicts,
                        } => {
                            println!("Status: Integratable");
                            if cherry_pick_conflicts {
                                println!("  Warning: Cherry-pick will have conflicts");
                            }
                            if commits_above_conflict {
                                println!("  Warning: Commits above will have conflicts");
                            }
                            if working_dir_conflicts {
                                println!("  Warning: Working directory will have conflicts");
                            }
                            if !cherry_pick_conflicts
                                && !commits_above_conflict
                                && !working_dir_conflicts
                            {
                                println!("  No conflicts expected");
                            }
                        }
                    }
                }
            } else {
                // Actual integration
                but_api::worktree::worktree_integrate(
                    project.id,
                    path.clone(),
                    target_ref.clone(),
                )?;

                if json {
                    println!("{{\"status\": \"success\"}}");
                } else {
                    println!("Successfully integrated worktree at: {}", path.display());
                    println!("Target: {}", target_ref);
                }
            }

            Ok(())
        }
    }
}

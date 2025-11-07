use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};
use but_api::worktree::IntegrationStatus;
use but_worktrees::WorktreeId;

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
        /// The path or name of the worktree to integrate
        path: String,
        /// The target reference to integrate into (defaults to the reference the worktree was created from)
        #[clap(long)]
        target: Option<String>,
        /// Perform a dry run without making changes
        #[clap(long)]
        dry: bool,
    },
    /// Destroy worktree(s)
    Destroy {
        /// The path to the worktree to destroy, or a reference to destroy all worktrees created from it
        target: String,
        /// Treat the target as a reference instead of a path
        #[clap(long)]
        reference: bool,
    },
}
/// Parse a worktree identifier which can be either:
/// - A full path to the worktree
/// - Just the worktree name
///
/// Returns the WorktreeId.
fn parse_worktree_identifier(
    input: &str,
    _project: &gitbutler_project::Project,
) -> Result<WorktreeId> {
    // If it's an absolute path or looks like a full path, extract the name from it
    let input_path = PathBuf::from(input);
    if input_path.is_absolute() || input_path.components().count() > 1 {
        return WorktreeId::from_path(&input_path);
    }

    // Otherwise treat it as just the worktree name
    Ok(WorktreeId::from_bstr(input))
}

pub fn handle(cmd: Subcommands, project: &gitbutler_project::Project, json: bool) -> Result<()> {
    let mut stderr = std::io::stderr();
    match handle_inner(cmd, project, json) {
        Ok(_) => Ok(()),
        Err(e) => {
            writeln!(stderr, "{:?}", e).ok();
            Err(e)
        }
    }
}

pub fn handle_inner(
    cmd: Subcommands,
    project: &gitbutler_project::Project,
    json: bool,
) -> Result<()> {
    let mut stdout = std::io::stdout();
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
                writeln!(stdout, "{}", serde_json::to_string_pretty(&output)?)?;
            } else {
                writeln!(
                    stdout,
                    "Created worktree at: {}",
                    output.created.path.display()
                )
                .ok();
                if let Some(reference) = output.created.created_from_ref {
                    writeln!(stdout, "Reference: {}", reference).ok();
                }
            }
            Ok(())
        }
        Subcommands::List => {
            let output = but_api::worktree::worktree_list(project.id)?;
            if json {
                writeln!(stdout, "{}", serde_json::to_string_pretty(&output)?)?;
            } else if output.entries.is_empty() {
                writeln!(stdout, "No worktrees found").ok();
            } else {
                for entry in &output.entries {
                    writeln!(stdout, "Path: {}", entry.path.display()).ok();
                    if let Some(reference) = &entry.created_from_ref {
                        writeln!(stdout, "Reference: {}", reference).ok();
                    }
                    if let Some(base) = entry.base {
                        writeln!(stdout, "Base: {}", base).ok();
                    }
                    writeln!(stdout,).ok();
                }
            }
            Ok(())
        }
        Subcommands::Integrate { path, target, dry } => {
            let id = parse_worktree_identifier(&path, project)?;

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
                    .find(|e| e.id == id)
                    .context("Worktree not found - ID does not match any known worktree")?;

                worktree_entry.created_from_ref.clone().context(
                    "Worktree does not have a created_from_ref - please specify --target",
                )?
            };

            if dry {
                // Dry run - check integration status
                let status = but_api::worktree::worktree_integration_status(
                    project.id,
                    id.clone(),
                    target_ref.clone(),
                )?;

                if json {
                    writeln!(stdout, "{}", serde_json::to_string_pretty(&status)?)?;
                } else {
                    writeln!(stdout, "Integration status for worktree: {}", id.as_str()).ok();
                    writeln!(stdout, "Target: {}", target_ref).ok();
                    match status {
                        IntegrationStatus::NoMergeBaseFound => {
                            writeln!(stdout, "Status: Cannot integrate - no merge base found").ok();
                        }
                        IntegrationStatus::WorktreeIsBare => {
                            writeln!(stdout, "Status: Cannot integrate - worktree is bare").ok();
                        }
                        IntegrationStatus::CausesWorkspaceConflicts => {
                            writeln!(
                                stdout,
                                "Status: Cannot integrate - would cause workspace conflicts"
                            )
                            .ok();
                        }
                        IntegrationStatus::Integratable {
                            cherry_pick_conflicts,
                            commits_above_conflict,
                            working_dir_conflicts,
                        } => {
                            writeln!(stdout, "Status: Integratable").ok();
                            if cherry_pick_conflicts {
                                writeln!(stdout, "  Warning: Cherry-pick will have conflicts").ok();
                            }
                            if commits_above_conflict {
                                writeln!(stdout, "  Warning: Commits above will have conflicts")
                                    .ok();
                            }
                            if working_dir_conflicts {
                                writeln!(
                                    stdout,
                                    "  Warning: Working directory will have conflicts"
                                )
                                .ok();
                            }
                            if !cherry_pick_conflicts
                                && !commits_above_conflict
                                && !working_dir_conflicts
                            {
                                writeln!(stdout, "  No conflicts expected").ok();
                            }
                        }
                    }
                }
            } else {
                // Actual integration
                but_api::worktree::worktree_integrate(project.id, id.clone(), target_ref.clone())?;

                if json {
                    writeln!(stdout, "{{\"status\": \"success\"}}")?;
                } else {
                    writeln!(stdout, "Successfully integrated worktree: {}", id.as_str()).ok();
                    writeln!(stdout, "Target: {}", target_ref).ok();
                }
            }

            Ok(())
        }
        Subcommands::Destroy { target, reference } => {
            if reference {
                // Treat target as a reference - parse it
                let reference = if target.starts_with("refs/") {
                    gix::refs::FullName::try_from(target.clone())?
                } else {
                    // Assume it's a branch name and prepend refs/heads/
                    gix::refs::FullName::try_from(format!("refs/heads/{}", target))?
                };

                let output = but_api::worktree::worktree_destroy_by_reference(
                    project.id,
                    reference.clone(),
                )?;

                if json {
                    writeln!(stdout, "{}", serde_json::to_string_pretty(&output)?)?;
                } else if output.destroyed_ids.is_empty() {
                    writeln!(stdout, "No worktrees found for reference: {}", reference).ok();
                } else {
                    writeln!(
                        stdout,
                        "Destroyed {} worktree(s) for reference: {}",
                        output.destroyed_ids.len(),
                        reference
                    )
                    .ok();
                    for id in &output.destroyed_ids {
                        writeln!(stdout, "  - {}", id.as_str()).ok();
                    }
                }
            } else {
                // Treat target as a path or worktree name
                let id = parse_worktree_identifier(&target, project)?;
                let output = but_api::worktree::worktree_destroy_by_id(project.id, id.clone())?;

                if json {
                    writeln!(stdout, "{}", serde_json::to_string_pretty(&output)?)?;
                } else {
                    writeln!(stdout, "Destroyed worktree: {}", id.as_str()).ok();
                }
            }

            Ok(())
        }
    }
}

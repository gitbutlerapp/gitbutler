use std::io::{self, Write};

use but_settings::AppSettings;
use but_workspace::{StackId, ui::StackEntry};
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;

mod list;

#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Creates a new branch in the workspace
    New {
        /// Name of the new branch
        branch_name: Option<String>,
        /// Anchor point - either a commit ID or branch name to create the new branch from
        #[clap(long, short = 'a')]
        anchor: Option<String>,
    },
    /// Deletes a branch from the workspace
    #[clap(short_flag = 'd')]
    Delete {
        /// Name of the branch to delete
        branch_name: String,
        /// Force deletion without confirmation
        #[clap(long, short = 'f')]
        force: bool,
    },
    /// List the branches in the repository
    List {
        /// Show only local branches
        #[clap(long, short = 'l')]
        local: bool,
    },
    /// Unapply a branch from the workspace
    Unapply {
        /// Name of the branch to unapply
        branch_name: String,
        /// Force unapply without confirmation
        #[clap(long, short = 'f')]
        force: bool,
    },
}

pub fn handle(cmd: &Subcommands, project: &Project, _json: bool) -> anyhow::Result<()> {
    match cmd {
        Subcommands::New {
            branch_name,
            anchor,
        } => {
            let ctx =
                CommandContext::open(project, AppSettings::load_from_default_path_creating()?)?;
            // Get branch name or use canned name
            let branch_name = if let Some(name) = branch_name {
                let repo = ctx.gix_repo()?;
                if repo.try_find_reference(name)?.is_some() {
                    println!("Branch '{name}' already exists");
                    return Ok(());
                }
                name.clone()
            } else {
                but_api::workspace::canned_branch_name(project.id)?
            };
            let anchor = if let Some(anchor_str) = anchor {
                // Use the new create_reference API when anchor is provided
                let mut ctx = ctx; // Make mutable for CliId resolution

                // Resolve the anchor string to a CliId
                let anchor_ids = crate::id::CliId::from_str(&mut ctx, anchor_str)?;
                if anchor_ids.is_empty() {
                    return Err(anyhow::anyhow!("Could not find anchor: {}", anchor_str));
                }
                if anchor_ids.len() > 1 {
                    return Err(anyhow::anyhow!(
                        "Ambiguous anchor '{}', matches multiple items",
                        anchor_str
                    ));
                }
                let anchor_id = &anchor_ids[0];

                // Create the anchor for create_reference
                // as dependent branch
                match anchor_id {
                    crate::id::CliId::Commit { oid } => {
                        Some(but_api::stack::create_reference::Anchor::AtCommit {
                            commit_id: (*oid).into(),
                            position: but_workspace::branch::create_reference::Position::Above,
                        })
                    }
                    crate::id::CliId::Branch { name } => {
                        Some(but_api::stack::create_reference::Anchor::AtReference {
                            short_name: name.clone(),
                            position: but_workspace::branch::create_reference::Position::Above,
                        })
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Invalid anchor type: {}, expected commit or branch",
                            anchor_id.kind()
                        ));
                    }
                }
            } else {
                // Create an independent branch
                None
            };
            but_api::stack::create_reference(
                project.id,
                but_api::stack::create_reference::Request {
                    new_name: branch_name.clone(),
                    anchor,
                },
            )?;
            println!("Created branch {branch_name}");
            Ok(())
        }
        Subcommands::Delete { branch_name, force } => {
            let stacks = but_api::workspace::stacks(
                project.id,
                Some(but_workspace::StacksFilter::InWorkspace),
            )?;

            // Find which stack this branch belongs to
            for stack_entry in &stacks {
                if stack_entry.heads.iter().all(|b| b.name != *branch_name) {
                    // Not found in this stack,
                    continue;
                }

                if let Some(sid) = stack_entry.id {
                    return confirm_branch_deletion(project, sid, branch_name, force);
                }
            }

            println!("Branch '{}' not found in any stack", branch_name);
            Ok(())
        }
        Subcommands::List { local } => list::list(project, *local),
        Subcommands::Unapply { branch_name, force } => {
            let stacks = but_api::workspace::stacks(
                project.id,
                Some(but_workspace::StacksFilter::InWorkspace),
            )?;

            // Find which stack this branch belongs to
            for stack_entry in &stacks {
                if stack_entry.heads.iter().all(|b| b.name != *branch_name) {
                    // Not found in this stack,
                    continue;
                }

                if let Some(sid) = stack_entry.id {
                    return confirm_unapply_stack(project, sid, stack_entry, force);
                }
            }

            println!("Branch '{}' not found in any stack", branch_name);
            Ok(())
        }
    }
}

fn confirm_unapply_stack(
    project: &Project,
    sid: StackId,
    stack_entry: &StackEntry,
    force: &bool,
) -> Result<(), anyhow::Error> {
    let branches = stack_entry
        .heads
        .iter()
        .map(|head| head.name.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    if !force {
        println!(
            "Are you sure you want to unapply stack with branches '{}'? [y/N]:",
            branches
        );

        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            println!("Aborted unapply operation.");
            return Ok(());
        }
    }

    but_api::virtual_branches::unapply_stack(project.id, sid)?;
    println!(
        "Unapplied stack with branches '{}' from workspace",
        branches
    );
    Ok(())
}

fn confirm_branch_deletion(
    project: &Project,
    sid: StackId,
    branch_name: &String,
    force: &bool,
) -> Result<(), anyhow::Error> {
    if !force {
        println!(
            "Are you sure you want to delete branch '{}'? [y/N]:",
            branch_name
        );

        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim().to_lowercase();
        if input != "y" && input != "yes" {
            println!("Aborted branch deletion.");
            return Ok(());
        }
    }

    but_api::stack::remove_branch(project.id, sid, branch_name.clone())?;
    println!("Deleted branch {branch_name}");
    Ok(())
}

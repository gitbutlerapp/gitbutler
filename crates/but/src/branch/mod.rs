use std::path::Path;

use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;

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
}

pub fn handle(cmd: &Subcommands, repo_path: &Path, _json: bool) -> anyhow::Result<()> {
    let project = Project::find_by_path(repo_path)?;
    match cmd {
        Subcommands::New {
            branch_name,
            anchor,
        } => {
            if let Some(anchor_str) = anchor {
                // Use the new create_reference API when anchor is provided
                let ctx = CommandContext::open(
                    &project,
                    AppSettings::load_from_default_path_creating()?,
                )?;
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

                // Get branch name or use canned name
                let branch_name = if let Some(name) = branch_name {
                    name.clone()
                } else {
                    but_api::workspace::canned_branch_name(project.id)?
                };

                // Create the anchor for create_reference
                let anchor = match anchor_id {
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
                };

                // Use create_reference API
                let request = but_api::stack::create_reference::Request {
                    new_name: branch_name.clone(),
                    anchor,
                };
                but_api::stack::create_reference(project.id, request)?;
                println!("Created branch {branch_name}");
                Ok(())
            } else {
                // Create an independent branch
                let req = gitbutler_branch::BranchCreateRequest {
                    name: branch_name.clone(),
                    ownership: None,
                    order: None,
                    selected_for_changes: None,
                };
                but_api::virtual_branches::create_virtual_branch(project.id, req)?;
                Ok(())
            }
        }
    }
}

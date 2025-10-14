use anyhow::Result;

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
        /// The worktree ID to integrate
        worktree_id: String,
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
            let output = but_api::worktree::worktree_new(project.id, reference.clone())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("Created worktree at: {}", output.created.path.display());
                println!("Reference: {}", output.created.reference);
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
                    println!("Path: {}", entry.worktree.path.display());
                    println!("  Reference: {}", entry.worktree.reference);
                    println!("  Status: {:?}", entry.status);
                    println!();
                }
            }
            Ok(())
        }
        Subcommands::Integrate { worktree_id, dry } => {
            if *dry {
                todo!("Dry run integrate worktree: {}", worktree_id)
            } else {
                todo!("Integrate worktree: {}", worktree_id)
            }
        }
    }
}

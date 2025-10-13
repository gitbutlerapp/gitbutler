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

pub fn handle(cmd: &Subcommands, _project: &gitbutler_project::Project, _json: bool) -> Result<()> {
    match cmd {
        Subcommands::New { reference } => {
            todo!("Create new worktree from reference: {}", reference)
        }
        Subcommands::List => {
            todo!("List all worktrees")
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

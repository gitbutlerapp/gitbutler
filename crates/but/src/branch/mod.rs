use std::path::Path;

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
    },
}

pub fn handle(cmd: &Subcommands, repo_path: &Path, _json: bool) -> anyhow::Result<()> {
    let project = Project::find_by_path(repo_path)?;
    match cmd {
        Subcommands::New { branch_name } => {
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

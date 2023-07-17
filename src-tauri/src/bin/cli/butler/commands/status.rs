use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;

use git_butler_tauri::virtual_branches;

use crate::cli::butler::app::App;

#[derive(Debug, Args)]
pub struct Status {}

impl super::RunCommand for Status {
    fn run(self) -> Result<()> {
        let app = App::new().context("Failed to create app")?;

        let statuses =
            virtual_branches::get_status_by_branch(app.gb_repository(), &app.project_repository())
                .context("failed to get status by branch")?;

        for (branch, files) in statuses {
            if branch.applied {
                println!(" branch: {}", branch.name.blue());
                println!("   head: {}", branch.head.to_string().green());
                println!("   tree: {}", branch.tree.to_string().green());
                println!("     id: {}", branch.id.green());
                println!("applied: {}", branch.applied.to_string().green());
                println!(" files:");
                for file in files {
                    println!("        {}", file.path.display().to_string().yellow());
                    for hunk in file.hunks {
                        println!("          {}", hunk.id);
                    }
                }
            } else {
                println!(" branch: {}", branch.name.blue());
                println!("applied: {}", branch.applied.to_string().green());
            }
            println!();
        }
        Ok(())
    }
}

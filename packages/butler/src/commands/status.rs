use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;

use gblib::virtual_branches;

use crate::app::App;

#[derive(Debug, Args)]
pub struct Status;

impl super::RunCommand for Status {
    fn run(self) -> Result<()> {
        let app = App::new().context("Failed to create app")?;

        let statuses =
            virtual_branches::get_status_by_branch(&app.gb_repository(), &app.project_repository())
                .context("failed to get status by branch")?;

        for (branch, files) in statuses {
            if branch.applied {
                println!(" branch: {}", branch.name.blue());
                println!("   head: {}", branch.head.to_string().green());
                println!("   tree: {}", branch.tree.to_string().green());
                println!("     id: {}", branch.id.to_string().green());
                println!("applied: {}", branch.applied.to_string().green());
                println!(" files:");
                for (filepath, hunks) in files {
                    println!("        {}", filepath.display().to_string().yellow());
                    for hunk in hunks {
                        println!(
                            "          {}-{}",
                            hunk.new_lines,
                            hunk.new_start + hunk.new_lines
                        );
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

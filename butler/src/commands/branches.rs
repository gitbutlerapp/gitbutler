use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;

use git_butler_tauri::virtual_branches;

use crate::app::App;

#[derive(Debug, Args)]
pub struct Branches {}

impl super::RunCommand for Branches {
    fn run(self) -> Result<()> {
        let app = App::new().context("Failed to create app")?;

        let branches =
            virtual_branches::list_virtual_branches(app.gb_repository(), &app.project_repository())
                .context("failed to list branches")?;

        for branch in branches {
            println!("{}", branch.id.red());
            println!("{}", branch.name.red());
            for file in branch.files {
                println!("  {}", file.path.display().to_string().blue());
                for hunk in file.hunks {
                    println!("--");
                    println!("    {}", hunk.diff.green());
                    println!("--");
                }
            }
        }

        Ok(())
    }
}

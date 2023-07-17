use anyhow::{Context, Result};
use clap::Args;

use git_butler_tauri::virtual_branches;

use crate::cli::butler::app::App;

#[derive(Debug, Args)]
pub struct Remotes {}

impl super::RunCommand for Remotes {
    fn run(self) -> Result<()> {
        let app = App::new().context("Failed to create app")?;
        let branches =
            virtual_branches::remote_branches(app.gb_repository(), &app.project_repository())
                .context("failed to get remote branches")?;
        for branch in branches {
            println!("{}", branch.name);
        }
        Ok(())
    }
}

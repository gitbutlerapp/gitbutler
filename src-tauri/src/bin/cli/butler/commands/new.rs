use anyhow::{Context, Result};
use clap::Args;
use dialoguer::{theme::ColorfulTheme, Input};

use git_butler_tauri::virtual_branches;

use crate::cli::butler::app::App;

#[derive(Debug, Args)]
pub struct New {}

impl super::RunCommand for New {
    fn run(self) -> Result<()> {
        let app = App::new().context("Failed to create app")?;

        let input: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("New branch name")
            .interact_text()
            .context("failed to get branch name")?;

        virtual_branches::create_virtual_branch(
            app.gb_repository(),
            &virtual_branches::branch::BranchCreateRequest {
                name: Some(input),
                ..Default::default()
            },
        )
        .context("failed to create virtual branch")?;

        Ok(())
    }
}

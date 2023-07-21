use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use dialoguer::{console::Term, theme::ColorfulTheme, Select};

use crate::app::App;

#[derive(Debug, Args)]
pub struct Setup {}

impl super::RunCommand for Setup {
    fn run(self) -> Result<()> {
        let app = App::new().context("Failed to create app")?;

        println!(
            "  HEAD: {}",
            app.project_repository()
                .get_head()
                .context("failed to get head")?
                .name()
                .context("failed to get head name")?
                .blue()
        );
        let items = app
            .project_repository()
            .git_remote_branches()
            .context("failed to get remote branches")?;

        let selection = Select::with_theme(&ColorfulTheme::default())
            .items(&items)
            .default(0)
            .interact_on_opt(&Term::stderr())
            .context("failed to get selection")?;

        match selection {
            Some(index) => {
                println!("Setting target to: {}", items[index].red());
                app.gb_repository()
                    .set_base_branch(&app.project_repository(), &items[index])
                    .context("failed to set target branch")?;
            }
            None => println!("User did not select anything"),
        };

        Ok(())
    }
}

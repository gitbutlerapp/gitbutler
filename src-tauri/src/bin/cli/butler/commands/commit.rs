use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use dialoguer::{console::Term, theme::ColorfulTheme, Input, Select};

use git_butler_tauri::{reader, sessions, virtual_branches};

use crate::cli::butler::app::App;

#[derive(Debug, Args)]
pub struct Commit {}

impl super::RunCommand for Commit {
    fn run(self) -> Result<()> {
        let app = App::new().context("Failed to create app")?;

        // get the branch to commit
        let current_session = app
            .gb_repository()
            .get_or_create_current_session()
            .context("failed to get or create currnt session")?;

        let current_session_reader = sessions::Reader::open(app.gb_repository(), &current_session)
            .context("failed to open current session reader")?;

        let virtual_branches = virtual_branches::Iterator::new(&current_session_reader)
            .context("failed to read virtual branches")?
            .collect::<Result<Vec<virtual_branches::branch::Branch>, reader::Error>>()
            .context("failed to read virtual branches")?
            .into_iter()
            .collect::<Vec<_>>();

        let mut ids = Vec::new();
        let mut names = Vec::new();
        for branch in virtual_branches {
            ids.push(branch.id);
            names.push(branch.name);
        }
        let selection = match Select::with_theme(&ColorfulTheme::default())
            .items(&names)
            .default(0)
            .interact_on_opt(&Term::stderr())
            .context("failed to get selection")?
        {
            Some(selection) => selection,
            None => return Ok(()),
        };

        let commit_branch = ids[selection].clone();
        println!("Committing virtual branch {}", commit_branch.red());

        // get the commit message
        let message: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Commit message")
            .interact_text()
            .context("failed to get commit message")?;

        virtual_branches::commit(
            app.gb_repository(),
            &app.project_repository(),
            &commit_branch,
            &message,
        )
        .context("failed to commit")?;

        Ok(())
    }
}

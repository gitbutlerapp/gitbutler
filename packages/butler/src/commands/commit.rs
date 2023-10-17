use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use dialoguer::{console::Term, theme::ColorfulTheme, Input, Select};

use gitbutler::{
    reader, sessions,
    virtual_branches::{self, Branch, BranchId},
};

use crate::app::App;

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

        let gb_repository = app.gb_repository();
        let current_session_reader = sessions::Reader::open(&gb_repository, &current_session)
            .context("failed to open current session reader")?;

        let (ids, names): (Vec<BranchId>, Vec<String>) =
            virtual_branches::Iterator::new(&current_session_reader)
                .context("failed to read virtual branches")?
                .collect::<Result<Vec<virtual_branches::branch::Branch>, reader::Error>>()
                .context("failed to read virtual branches")?
                .into_iter()
                .map(|b| (b.id, b.name))
                .unzip();

        let selection = match Select::with_theme(&ColorfulTheme::default())
            .items(&names)
            .default(0)
            .interact_on_opt(&Term::stderr())
            .context("failed to get selection")?
        {
            Some(selection) => selection,
            None => return Ok(()),
        };

        let commit_branch = ids[selection];
        println!(
            "Committing virtual branch {}",
            commit_branch.to_string().red()
        );

        // get the commit message
        let message: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Commit message")
            .interact_text()
            .context("failed to get commit message")?;

        virtual_branches::commit(
            &gb_repository,
            &app.project_repository(),
            &commit_branch,
            &message,
            None,
            None,
            app.user(),
        )
        .context("failed to commit")?;

        Ok(())
    }
}

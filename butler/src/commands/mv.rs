use anyhow::{Context, Result};
use clap::Args;
use dialoguer::{console::Term, theme::ColorfulTheme, MultiSelect, Select};

use git_butler_tauri::{reader, sessions, virtual_branches};

use crate::app::App;

#[derive(Debug, Args)]
pub struct Move {}

impl super::RunCommand for Move {
    fn run(self) -> Result<()> {
        let app = App::new().context("Failed to create app")?;

        let all_hunks =
            virtual_branches::get_status_by_branch(app.gb_repository(), &app.project_repository())
                .context("failed to get status files")?
                .into_iter()
                .flat_map(|(_branch, files)| {
                    files
                        .into_iter()
                        .flat_map(|file| {
                            file.hunks
                                .into_iter()
                                .map(|hunk| hunk.id)
                                .collect::<Vec<_>>()
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();

        let selected_files: Vec<String> = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt("Which hunks do you want to move?")
            .items(&all_hunks)
            .interact()
            .context("failed to get selections")?
            .iter()
            .map(|i| all_hunks[*i].clone())
            .collect::<Vec<_>>();

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

        let selection = match Select::with_theme(&ColorfulTheme::default())
            .items(
                &virtual_branches
                    .iter()
                    .map(|b| b.name.clone())
                    .collect::<Vec<_>>(),
            )
            .default(0)
            .interact_on_opt(&Term::stderr())
            .context("failed to get selection")?
        {
            Some(selection) => selection,
            None => return Ok(()),
        };

        let target_branch = virtual_branches[selection].clone();
        let mut ownership = target_branch.ownership.clone();
        ownership.put(
            &selected_files
                .join("\n")
                .try_into()
                .context("failed to convert to ownership")?,
        );

        virtual_branches::update_branch(
            app.gb_repository(),
            virtual_branches::branch::BranchUpdateRequest {
                id: target_branch.id,
                ownership: Some(ownership),
                ..Default::default()
            },
        )
        .context("failed to update branch")?;

        Ok(())
    }
}

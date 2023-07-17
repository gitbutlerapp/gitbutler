use anyhow::{Context, Result};
use clap::Args;

use git_butler_tauri::{reader, sessions, virtual_branches};

use crate::cli::butler::app::App;

#[derive(Debug, Args)]
pub struct Reset {}

impl super::RunCommand for Reset {
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

        let writer = virtual_branches::branch::Writer::new(app.gb_repository());
        for mut branch in virtual_branches {
            println!("resetting {}", branch.name);
            branch.applied = false;
            writer.write(&branch).context("failed to write branch")?;
        }

        Ok(())
    }
}

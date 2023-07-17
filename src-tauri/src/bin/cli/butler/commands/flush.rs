use anyhow::{Context, Result};
use clap::Args;

use crate::cli::butler::app::App;

#[derive(Debug, Args)]
pub struct Flush {}

impl super::RunCommand for Flush {
    fn run(self) -> Result<()> {
        let app = App::new().context("Failed to create app")?;
        println!("Flushing sessions");
        app.gb_repository()
            .flush()
            .context("failed to flush sessions")?;
        Ok(())
    }
}

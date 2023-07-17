use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;

use git_butler_tauri::{reader, sessions, virtual_branches};

use crate::cli::butler::app::App;

#[derive(Debug, Args)]
pub struct Info {}

impl super::RunCommand for Info {
    fn run(self) -> Result<()> {
        let app = App::new().context("Failed to create app")?;

        // just print information for the project
        println!("path: {}", app.path().yellow());
        println!("data_dir: {}", app.local_data_dir().yellow());

        // find the project in project storage that matches the cwd
        println!("{}", "project:".to_string().red());
        println!("  id: {}", app.project().id.blue());
        println!("  title: {}", app.project().title.blue());
        println!(
            "  description: {}",
            app.project()
                .description
                .clone()
                .unwrap_or("none".to_string())
                .blue()
        );
        println!(
            "  project_data_last_fetched: {:?}",
            app.project().project_data_last_fetched
        );
        println!(
            "  project_gitbutler_data_last_fetched: {:?}",
            app.project().gitbutler_data_last_fetched
        );
        println!("  path: {}", app.project().path.blue());

        if let Some(api) = app.project().api.as_ref() {
            println!("  {}:", "api".to_string().red());
            println!("   api name: {}", api.name.blue());
            println!("   repo id: {}", api.repository_id.blue());
            println!("   git url: {}", api.git_url.blue());
            println!("   created: {}", api.created_at.blue());
            println!("   updated: {}", api.updated_at.blue());
        }

        println!("{}", "project repo:".to_string().red());
        println!(
            "  HEAD: {}",
            app.project_repository()
                .get_head()
                .context("failed to get head")?
                .name()
                .context("failed to get head name")?
                .blue()
        );

        println!("{}", "sessions:".to_string().red());
        // sessions storage
        let sessions = app
            .sessions_db()
            .list_by_project_id(&app.project().id, None)
            .unwrap();
        //list the sessions
        for session in &sessions {
            println!("  id: {}", session.id.blue());
        }

        // gitbutler repo stuff
        // read default target

        let current_session = app
            .gb_repository()
            .get_or_create_current_session()
            .context("failed to get or create currnt session")?;
        let current_session_reader = sessions::Reader::open(app.gb_repository(), &current_session)
            .context("failed to open current session reader")?;

        let target_reader = virtual_branches::target::Reader::new(&current_session_reader);
        match target_reader.read_default() {
            Ok(target) => {
                println!("{}", "target:".to_string().red());
                println!("  base sha: {}", target.sha.to_string().blue());
            }
            Err(reader::Error::NotFound) => {}
            Err(e) => panic!("failed to read default target: {}", e),
        };

        println!("{}", "virtual branches:".to_string().red());
        virtual_branches::Iterator::new(&current_session_reader)
            .context("failed to read virtual branches")?
            .collect::<Result<Vec<virtual_branches::branch::Branch>, reader::Error>>()
            .context("failed to read virtual branches")?
            .into_iter()
            .for_each(|branch| {
                println!("  {}", branch.name);
                for file in branch.ownership.to_string().lines() {
                    println!("    {}", file);
                }
            });

        Ok(())
    }
}

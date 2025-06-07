use std::path::Path;

use super::print;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;

pub fn status(repo_path: &Path, json: bool) -> anyhow::Result<()> {
    let repo_path = repo_path.to_owned();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new().map(|r| r.block_on(status_inner(&repo_path, json)))
    })
    .join()
    .unwrap()??;

    Ok(())
}

async fn status_inner(repo_path: &Path, json: bool) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path).expect("Failed to create project from path");
    let ctx = CommandContext::open(&project, AppSettings::default())?;

    let outcome = but_inspection::index_stats(&ctx).await?;

    print(&outcome, json)
}

pub fn generate(repo_path: &Path, json: bool) -> anyhow::Result<()> {
    let repo_path = repo_path.to_owned();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new().map(|r| r.block_on(generate_inner(&repo_path, json)))
    })
    .join()
    .unwrap()??;

    Ok(())
}

async fn generate_inner(repo_path: &Path, json: bool) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path).expect("Failed to create project from path");
    let ctx = CommandContext::open(&project, AppSettings::default())?;

    let outcome = but_inspection::generate_embeddings(&ctx).await?;

    print(&outcome, json)
}

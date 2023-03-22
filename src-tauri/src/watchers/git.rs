use crate::{events, projects};
use anyhow::{Context, Result};
use std::path::Path;

pub async fn on_git_file_change<P: AsRef<Path>>(
    sender: &tokio::sync::mpsc::Sender<events::Event>,
    project: &projects::Project,
    path: P,
) -> Result<()> {
    if path.as_ref().ne(Path::new(".git/log/HEAD")) {
        return Ok(());
    }
    let event = events::Event::git(&project);
    sender.send(event).await.with_context(|| {
        format!(
            "{}: failed to send git event for \"{}\"",
            project.id,
            path.as_ref().to_str().unwrap()
        )
    })?;
    Ok(())
}

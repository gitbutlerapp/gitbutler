use crate::{events, repositories};
use anyhow::{Context, Result};
use std::path::Path;

pub async fn on_git_file_change<P: AsRef<Path>>(
    sender: &tokio::sync::mpsc::Sender<events::Event>,
    repository: &repositories::Repository,
    path: P,
) -> Result<()> {
    let event = if path.as_ref().eq(Path::new(".git/logs/HEAD")) {
        events::Event::git_activity(&repository.project)
    } else if path.as_ref().eq(Path::new(".git/HEAD")) {
        events::Event::git_head(&repository.project, &repository.head()?)
    } else if path.as_ref().eq(Path::new(".git/index")) {
        events::Event::git_index(&repository.project)
    } else {
        return Ok(());
    };

    sender.send(event).await.with_context(|| {
        format!(
            "{}: failed to send git event for \"{}\"",
            repository.project.id,
            path.as_ref().to_str().unwrap()
        )
    })?;

    Ok(())
}

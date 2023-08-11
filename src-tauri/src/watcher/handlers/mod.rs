mod check_current_session;
mod fetch_gitbutler_data;
mod fetch_project_data;
mod flush_session;
mod git_file_change;
mod index_handler;
mod project_file_change;

use anyhow::{Context, Result};
use tauri::AppHandle;
use tokio::time::{timeout, Duration};
use tracing::instrument;

use crate::events as app_events;

use super::events;

#[derive(Clone)]
pub struct Handler {
    project_file_handler: project_file_change::Handler,
    git_file_change_handler: git_file_change::Handler,
    check_current_session_handler: check_current_session::Handler,
    flush_session_handler: flush_session::Handler,
    fetch_project_handler: fetch_project_data::Handler,
    fetch_gitbutler_handler: fetch_gitbutler_data::Handler,
    index_handler: index_handler::Handler,

    events_sender: app_events::Sender,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;
    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(Self {
            events_sender: app_events::Sender::from(value),
            project_file_handler: project_file_change::Handler::try_from(value)?,
            check_current_session_handler: check_current_session::Handler::try_from(value)?,
            git_file_change_handler: git_file_change::Handler::try_from(value)?,
            flush_session_handler: flush_session::Handler::try_from(value)?,
            fetch_project_handler: fetch_project_data::Handler::try_from(value)?,
            fetch_gitbutler_handler: fetch_gitbutler_data::Handler::try_from(value)?,
            index_handler: index_handler::Handler::try_from(value)?,
        })
    }
}

impl<'handler> Handler {
    #[instrument(name = "handle", skip(self), fields(event = %event))]
    pub async fn handle(&self, event: &events::Event) -> Result<Vec<events::Event>> {
        self.handle_with_timeout(Duration::from_secs(30), event).await
    }

    async fn handle_with_timeout(&self, duration: Duration, event: &events::Event) -> Result<Vec<events::Event>> {
        match timeout(duration, self.handle_event(event)).await {
            Ok(events) => events,
            Err(_) => 
                Err(anyhow::anyhow!("handler timed out after {} sec", duration.as_secs()))?
        }
    }

    async fn handle_event(&self, event: &events::Event) -> Result<Vec<events::Event>> {
        match event {
            events::Event::ProjectFileChange(project_id, path) => self
                .project_file_handler
                .handle(path, project_id)
                .context(format!(
                    "failed to handle project file change event: {:?}",
                    path.display()
                )),

            events::Event::GitFileChange(project_id, path) => self
                .git_file_change_handler
                .handle(path, project_id)
                .context("failed to handle git file change event"),

            events::Event::FetchGitbutlerData(project_id, tick) => self
                .fetch_gitbutler_handler
                .handle(project_id, tick)
                .context("failed to fetch gitbutler data"),

            events::Event::Tick(project_id, tick) => {
                let one = match self.check_current_session_handler.handle(project_id, tick) {
                    Ok(events) => events,
                    Err(err) => {
                        tracing::error!(
                            "{}: failed to check current session: {:#}",
                            project_id,
                            err
                        );
                        vec![]
                    }
                };

                let two = match self.fetch_project_handler.handle(project_id, tick) {
                    Ok(events) => events,
                    Err(err) => {
                        tracing::error!("{}: failed to fetch project data: {:#}", project_id, err);
                        vec![]
                    }
                };

                let three = match self.fetch_gitbutler_handler.handle(project_id, tick) {
                    Ok(events) => events,
                    Err(err) => {
                        tracing::error!(
                            "{}: failed to fetch gitbutler data: {:#}",
                            project_id,
                            err
                        );
                        vec![]
                    }
                };

                Ok(one
                    .into_iter()
                    .chain(two.into_iter())
                    .chain(three.into_iter())
                    .collect())
            }

            events::Event::Flush(project_id, session) => self
                .flush_session_handler
                .handle(project_id, session)
                .context("failed to handle flush session event"),

            events::Event::SessionFile((project_id, session_id, file_path, contents)) => {
                let mut events = self
                    .index_handler
                    .index_file(
                        project_id,
                        session_id,
                        file_path.to_str().unwrap(),
                        contents,
                    )
                    .context("failed to index file")?;
                events.push(events::Event::Emit(app_events::Event::file(
                    project_id,
                    session_id,
                    &file_path.display().to_string(),
                    contents,
                )));
                Ok(events)
            }

            events::Event::Session(project_id, session) => self
                .index_handler
                .index_session(project_id, session)
                .context("failed to index session"),

            events::Event::SessionDelta((project_id, session_id, path, delta)) => {
                let mut events = self
                    .index_handler
                    .index_deltas(
                        project_id,
                        session_id,
                        path.to_str().unwrap(),
                        &vec![delta.clone()],
                    )
                    .context("failed to index deltas")?;

                events.push(events::Event::Emit(app_events::Event::deltas(
                    project_id,
                    session_id,
                    &vec![delta.clone()],
                    path,
                )));

                Ok(events)
            }

            events::Event::Bookmark(bookmark) => self
                .index_handler
                .index_bookmark(&bookmark.project_id, bookmark)
                .context("failed to index bookmark"),

            events::Event::IndexAll(project_id) => self.index_handler.reindex(project_id),

            events::Event::Emit(event) => {
                self.events_sender
                    .send(event.clone())
                    .context("failed to send event")?;
                Ok(vec![])
            }
        }
    }
}

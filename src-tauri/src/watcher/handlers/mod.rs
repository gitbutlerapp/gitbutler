mod fetch_gitbutler_data;
mod fetch_project_data;
mod flush_session;
mod git_file_change;
mod index_handler;
mod project_file_change;
mod push_gitbutler_data;
mod tick_handler;

use anyhow::{Context, Result};
use tauri::AppHandle;
use tracing::instrument;

use crate::events as app_events;

use super::events;

#[derive(Clone)]
pub struct Handler {
    project_file_handler: project_file_change::Handler,
    git_file_change_handler: git_file_change::Handler,
    tick_handler: tick_handler::Handler,
    flush_session_handler: flush_session::Handler,
    fetch_project_handler: fetch_project_data::Handler,
    fetch_gitbutler_handler: fetch_gitbutler_data::Handler,
    push_gitbutler_handler: push_gitbutler_data::Handler,
    index_handler: index_handler::Handler,

    events_sender: app_events::Sender,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;
    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(Self {
            events_sender: app_events::Sender::from(value),
            project_file_handler: project_file_change::Handler::try_from(value)?,
            tick_handler: tick_handler::Handler::try_from(value)?,
            git_file_change_handler: git_file_change::Handler::try_from(value)?,
            flush_session_handler: flush_session::Handler::try_from(value)?,
            push_gitbutler_handler: push_gitbutler_data::Handler::try_from(value)?,
            fetch_project_handler: fetch_project_data::Handler::try_from(value)?,
            fetch_gitbutler_handler: fetch_gitbutler_data::Handler::try_from(value)?,
            index_handler: index_handler::Handler::try_from(value)?,
        })
    }
}

impl Handler {
    #[instrument(name = "handle", skip(self), fields(event = %event))]
    pub fn handle(&self, event: &events::Event) -> Result<Vec<events::Event>> {
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

            events::Event::PushGitbutlerData(project_id) => self
                .push_gitbutler_handler
                .handle(project_id)
                .context("failed to push gitbutler data"),

            events::Event::FetchGitbutlerData(project_id, tick) => self
                .fetch_gitbutler_handler
                .handle(project_id, tick)
                .context("failed to fetch gitbutler data"),

            events::Event::FetchProjectData(project_id, tick) => self
                .fetch_project_handler
                .handle(project_id, tick)
                .context("failed to fetch project data"),

            events::Event::Tick(project_id, tick) => self
                .tick_handler
                .handle(project_id, tick)
                .context("failed to handle tick"),

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

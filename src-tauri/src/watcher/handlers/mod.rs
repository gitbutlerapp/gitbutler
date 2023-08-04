mod check_current_session;
mod fetch_gitbutler_data;
mod fetch_project_data;
mod flush_session;
mod git_file_change;
mod index_handler;
mod project_file_change;

use std::path;

use anyhow::{Context, Result};

use crate::{
    bookmarks, deltas, events as app_events, files, keys, projects, search, sessions, users,
};

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

impl<'handler> Handler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        local_data_dir: &path::Path,
        project_store: &projects::Storage,
        user_store: &users::Storage,
        searcher: &search::Searcher,
        events_sender: &app_events::Sender,
        sessions_database: &sessions::Database,
        deltas_database: &deltas::Database,
        files_database: &files::Database,
        bookmarks_database: &bookmarks::Database,
        keys_controller: &keys::Controller,
    ) -> Self {
        Self {
            events_sender: events_sender.clone(),

            project_file_handler: project_file_change::Handler::new(
                local_data_dir,
                project_store,
                user_store,
            ),
            check_current_session_handler: check_current_session::Handler::new(
                local_data_dir,
                project_store,
                user_store,
            ),
            git_file_change_handler: git_file_change::Handler::new(project_store),
            flush_session_handler: flush_session::Handler::new(
                local_data_dir,
                project_store,
                user_store,
            ),
            fetch_project_handler: fetch_project_data::Handler::new(
                project_store,
                local_data_dir,
                user_store,
                keys_controller,
            ),
            fetch_gitbutler_handler: fetch_gitbutler_data::Handler::new(
                local_data_dir,
                project_store,
                user_store,
            ),
            index_handler: index_handler::Handler::new(
                local_data_dir,
                project_store,
                user_store,
                searcher,
                files_database,
                sessions_database,
                deltas_database,
                bookmarks_database,
            ),
        }
    }

    pub async fn handle(&self, event: events::Event) -> Result<Vec<events::Event>> {
        match event {
            events::Event::ProjectFileChange(project_id, path) => self
                .project_file_handler
                .handle(&path, &project_id)
                .context(format!(
                    "failed to handle project file change event: {:?}",
                    path.display()
                )),

            events::Event::GitFileChange(project_id, path) => self
                .git_file_change_handler
                .handle(path, &project_id)
                .context("failed to handle git file change event"),

            events::Event::FetchGitbutlerData(project_id, tick) => self
                .fetch_gitbutler_handler
                .handle(&project_id, tick)
                .context("failed to fetch gitbutler data"),

            events::Event::Tick(project_id, tick) => {
                let one = match self.check_current_session_handler.handle(&project_id, tick) {
                    Ok(events) => events,
                    Err(err) => {
                        log::error!("{}: failed to check current session: {:#}", project_id, err);
                        vec![]
                    }
                };

                let two = match self.fetch_project_handler.handle(&project_id, tick) {
                    Ok(events) => events,
                    Err(err) => {
                        log::error!("{}: failed to fetch project data: {:#}", project_id, err);
                        vec![]
                    }
                };

                let three = match self.fetch_gitbutler_handler.handle(&project_id, tick) {
                    Ok(events) => events,
                    Err(err) => {
                        log::error!("{}: failed to fetch gitbutler data: {:#}", project_id, err);
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
                .handle(&project_id, &session)
                .context("failed to handle flush session event"),

            events::Event::SessionFile((project_id, session_id, file_path, contents)) => {
                let mut events = self
                    .index_handler
                    .index_file(
                        &project_id,
                        &session_id,
                        file_path.to_str().unwrap(),
                        &contents,
                    )
                    .context("failed to index file")?;
                events.push(events::Event::Emit(app_events::Event::file(
                    &project_id,
                    &session_id,
                    &file_path.display().to_string(),
                    &contents,
                )));
                Ok(events)
            }

            events::Event::Session(project_id, session) => self
                .index_handler
                .index_session(&project_id, &session)
                .context("failed to index session"),

            events::Event::SessionDelta((project_id, session_id, path, delta)) => {
                let mut events = self
                    .index_handler
                    .index_deltas(
                        &project_id,
                        &session_id,
                        path.to_str().unwrap(),
                        &vec![delta.clone()],
                    )
                    .context("failed to index deltas")?;

                events.push(events::Event::Emit(app_events::Event::deltas(
                    &project_id,
                    &session_id,
                    &vec![delta],
                    &path,
                )));

                Ok(events)
            }

            events::Event::Bookmark(bookmark) => self
                .index_handler
                .index_bookmark(&bookmark.project_id, &bookmark)
                .context("failed to index bookmark"),

            events::Event::IndexAll(project_id) => self.index_handler.reindex(&project_id),

            events::Event::Emit(event) => {
                self.events_sender
                    .send(event)
                    .context("failed to send event")?;
                Ok(vec![])
            }
        }
    }
}

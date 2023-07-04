mod check_current_session;
mod fetch_gitbutler_data;
mod fetch_project_data;
mod file_change;
mod flush_session;
mod git_file_change;
mod index_handler;
mod project_file_change;

#[cfg(test)]
mod check_current_session_tests;
#[cfg(test)]
mod project_file_change_tests;

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::{bookmarks, deltas, events as app_events, files, projects, search, sessions, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    project_id: String,

    file_change_handler: file_change::Handler,
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
        local_data_dir: PathBuf,
        project_id: String,
        project_store: projects::Storage,
        user_store: users::Storage,
        searcher: search::Searcher,
        events_sender: app_events::Sender,
        sessions_database: sessions::Database,
        deltas_database: deltas::Database,
        files_database: files::Database,
        bookmarks_database: bookmarks::Database,
    ) -> Self {
        Self {
            project_id: project_id.clone(),
            events_sender: events_sender.clone(),

            file_change_handler: file_change::Handler::new(),
            project_file_handler: project_file_change::Handler::new(
                local_data_dir.clone(),
                project_id.clone(),
                project_store.clone(),
                user_store.clone(),
            ),
            check_current_session_handler: check_current_session::Handler::new(
                local_data_dir.clone(),
                project_id.clone(),
                project_store.clone(),
                user_store.clone(),
            ),
            git_file_change_handler: git_file_change::Handler::new(
                project_id.clone(),
                project_store.clone(),
            ),
            flush_session_handler: flush_session::Handler::new(
                local_data_dir.clone(),
                project_id.clone(),
                project_store.clone(),
                user_store.clone(),
            ),
            fetch_project_handler: fetch_project_data::Handler::new(
                project_id.clone(),
                project_store.clone(),
            ),
            fetch_gitbutler_handler: fetch_gitbutler_data::Handler::new(
                local_data_dir.clone(),
                project_id.clone(),
                project_store.clone(),
                user_store.clone(),
            ),
            index_handler: index_handler::Handler::new(
                local_data_dir,
                project_id,
                project_store,
                user_store,
                searcher,
                files_database,
                sessions_database,
                deltas_database,
                bookmarks_database,
                events_sender,
            ),
        }
    }

    pub async fn handle(&self, event: events::Event) -> Result<Vec<events::Event>> {
        // its's noisy for development
        #[cfg(not(debug_assertions))]
        log::info!("{}: handling event: {}", self.project_id, event);

        match event {
            events::Event::FileChange(path) => self
                .file_change_handler
                .handle(path.clone())
                .with_context(|| format!("failed to handle file change event: {:?}", path)),
            events::Event::ProjectFileChange(path) => self
                .project_file_handler
                .handle(path.clone())
                .with_context(|| format!("failed to handle project file change event: {:?}", path)),
            events::Event::GitFileChange(path) => self
                .git_file_change_handler
                .handle(path)
                .context("failed to handle git file change event"),
            events::Event::GitActivity => {
                self.events_sender
                    .send(app_events::Event::git_activity(&self.project_id))
                    .context("failed to send git activity event")?;
                Ok(vec![])
            }
            events::Event::GitHeadChange(head) => {
                self.events_sender
                    .send(app_events::Event::git_head(&self.project_id, &head))
                    .context("failed to send git head event")?;
                Ok(vec![])
            }
            events::Event::GitFetch => {
                self.events_sender
                    .send(app_events::Event::git_fetch(&self.project_id))
                    .context("failed to send git fetch event")?;
                Ok(vec![])
            }
            events::Event::GitIndexChange => {
                self.events_sender
                    .send(app_events::Event::git_index(&self.project_id))
                    .context("failed to send git index event")?;
                Ok(vec![])
            }
            events::Event::FetchGitbutlerData(tick) => self
                .fetch_gitbutler_handler
                .handle(tick)
                .context("failed to fetch gitbutler data"),
            events::Event::Tick(tick) => {
                let one = match self.check_current_session_handler.handle(tick) {
                    Ok(events) => events,
                    Err(err) => {
                        log::error!(
                            "{}: failed to check current session: {:#?}",
                            self.project_id,
                            err
                        );
                        vec![]
                    }
                };

                let two = match self.fetch_project_handler.handle(tick) {
                    Ok(events) => events,
                    Err(err) => {
                        log::error!(
                            "{}: failed to fetch project data: {:#?}",
                            self.project_id,
                            err
                        );
                        vec![]
                    }
                };

                let three = match self.fetch_gitbutler_handler.handle(tick) {
                    Ok(events) => events,
                    Err(err) => {
                        log::error!(
                            "{}: failed to fetch gitbutler data: {:#?}",
                            self.project_id,
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
            events::Event::Flush(session) => self
                .flush_session_handler
                .handle(&session)
                .context("failed to handle flush session event"),

            events::Event::SessionFile((session_id, file_path, contents)) => {
                let file_events = self
                    .index_handler
                    .index_file(&session_id, file_path.to_str().unwrap(), &contents)
                    .context("failed to index file")?;
                self.events_sender
                    .send(app_events::Event::file(
                        &self.project_id,
                        &session_id,
                        file_path.to_str().unwrap(),
                        &contents,
                    ))
                    .context("failed to send file event")?;
                Ok(file_events)
            }
            events::Event::Session(session) => {
                let session_events = self
                    .index_handler
                    .index_session(&session)
                    .context("failed to index session")?;
                self.events_sender
                    .send(app_events::Event::session(&self.project_id, &session))
                    .context("failed to send session event")?;
                Ok(session_events)
            }
            events::Event::SessionDelta((session_id, path, delta)) => {
                let deltas = vec![delta];
                let delta_events = self
                    .index_handler
                    .index_deltas(&session_id, path.to_str().unwrap(), &deltas)
                    .context("failed to index deltas")?;
                self.events_sender
                    .send(app_events::Event::deltas(
                        &self.project_id,
                        &session_id,
                        &deltas,
                        &path,
                    ))
                    .context("failed to send deltas event")?;
                Ok(delta_events)
            }
            events::Event::Bookmark(bookmark) => {
                let bookmark_events = self
                    .index_handler
                    .index_bookmark(&bookmark)
                    .context("failed to index bookmark")?;
                self.events_sender
                    .send(app_events::Event::bookmark(&self.project_id, &bookmark))
                    .context("failed to send bookmark event")?;
                Ok(bookmark_events)
            }

            events::Event::IndexAll => self.index_handler.reindex(),
        }
    }
}

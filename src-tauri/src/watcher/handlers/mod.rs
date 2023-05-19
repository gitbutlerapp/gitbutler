mod check_current_session;
mod check_fetch_project;
mod fetch_project;
mod file_change;
mod flush_session;
mod git_file_change;
mod index_handler;
mod project_file_change;

#[cfg(test)]
mod check_current_session_tests;
#[cfg(test)]
mod project_file_change_tests;

use anyhow::{Context, Result};

use crate::{
    bookmarks, deltas, events as app_events, files, gb_repository, projects, search, sessions,
};

use super::events;

pub struct Handler<'handler> {
    project_id: String,

    file_change_handler: file_change::Handler,
    project_file_handler: project_file_change::Handler<'handler>,
    git_file_change_handler: git_file_change::Handler,
    check_current_session_handler: check_current_session::Handler<'handler>,
    flush_session_handler: flush_session::Handler<'handler>,
    fetch_project_handler: fetch_project::Handler<'handler>,
    chech_fetch_project_handler: check_fetch_project::Handler,
    index_handler: index_handler::Handler<'handler>,

    events_sender: app_events::Sender,
}

impl<'handler> Handler<'handler> {
    pub fn new(
        project_id: String,
        project_store: projects::Storage,
        gb_repository: &'handler gb_repository::Repository,
        searcher: search::Deltas,
        events_sender: app_events::Sender,
        sessions_database: sessions::Database,
        deltas_database: deltas::Database,
        files_database: files::Database,
        bookmarks_database: bookmarks::Database,
    ) -> Self {
        Self {
            project_id: project_id.clone(),
            events_sender,

            file_change_handler: file_change::Handler::new(),
            project_file_handler: project_file_change::Handler::new(
                project_id.clone(),
                project_store.clone(),
                gb_repository,
            ),
            check_current_session_handler: check_current_session::Handler::new(gb_repository),
            git_file_change_handler: git_file_change::Handler::new(
                project_id.clone(),
                project_store.clone(),
            ),
            flush_session_handler: flush_session::Handler::new(
                project_id.clone(),
                project_store.clone(),
                gb_repository,
            ),
            fetch_project_handler: fetch_project::Handler::new(
                project_id.clone(),
                project_store.clone(),
                gb_repository,
            ),
            chech_fetch_project_handler: check_fetch_project::Handler::new(
                project_id.clone(),
                project_store,
            ),
            index_handler: index_handler::Handler::new(
                project_id,
                searcher,
                gb_repository,
                files_database,
                sessions_database,
                deltas_database,
                bookmarks_database,
            ),
        }
    }

    pub fn handle(&self, event: events::Event) -> Result<Vec<events::Event>> {
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
            events::Event::GitIndexChange => {
                self.events_sender
                    .send(app_events::Event::git_index(&self.project_id))
                    .context("failed to send git index event")?;
                Ok(vec![])
            }
            events::Event::Tick(tick) => {
                let one = self
                    .check_current_session_handler
                    .handle(tick)
                    .context("failed to handle tick event")?;
                let two = self
                    .chech_fetch_project_handler
                    .handle(tick)
                    .context("failed to handle tick event")?;
                Ok(one.into_iter().chain(two.into_iter()).collect())
            }
            events::Event::Flush(session) => self
                .flush_session_handler
                .handle(&session)
                .context("failed to handle flush session event"),
            events::Event::SessionFlushed(session) => self.index_handler.index_session(&session),
            events::Event::Fetch => self.fetch_project_handler.handle(),

            events::Event::File((session_id, file_path, contents)) => {
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
            events::Event::Deltas((session_id, path, deltas)) => {
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
            events::Event::Bookmark(bookmark) => self
                .index_handler
                .index_bookmark(&bookmark)
                .context("failed to index bookmark"),
        }
    }
}

mod check_current_session;
mod check_fetch_project;
mod fetch_project;
mod file_change;
mod flush_session;
mod git_file_change;
mod project_file_change;

#[cfg(test)]
mod check_current_session_tests;
#[cfg(test)]
mod project_file_change_tests;

use anyhow::{Context, Result};

use crate::{app::gb_repository, events as app_events, projects, search};

use super::events;

pub struct Handler<'handler> {
    gb_repository: &'handler gb_repository::Repository,

    file_change_handler: file_change::Handler,
    project_file_handler: project_file_change::Handler<'handler>,
    git_file_change_handler: git_file_change::Handler,
    check_current_session_handler: check_current_session::Handler<'handler>,
    flush_session_handler: flush_session::Handler<'handler>,
    fetch_project_handler: fetch_project::Handler<'handler>,
    chech_fetch_project_handler: check_fetch_project::Handler,

    searcher: search::Deltas,
    events_sender: app_events::Sender,
}

impl<'handler> Handler<'handler> {
    pub fn new(
        project_id: String,
        project_store: projects::Storage,
        gb_repository: &'handler gb_repository::Repository,
        searcher: search::Deltas,
        events_sender: app_events::Sender,
    ) -> Self {
        Self {
            events_sender,
            gb_repository,

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
                project_store.clone(),
                gb_repository,
            ),
            chech_fetch_project_handler: check_fetch_project::Handler::new(
                project_id,
                project_store,
            ),

            searcher,
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
            events::Event::Session((project, session)) => {
                self.events_sender
                    .send(app_events::Event::session(&project, &session))
                    .context("failed to send session event")?;
                Ok(vec![])
            }
            events::Event::Deltas((project, session, path, deltas)) => {
                self.events_sender
                    .send(app_events::Event::detlas(
                        &project, &session, &deltas, &path,
                    ))
                    .context("failed to send deltas event")?;
                Ok(vec![])
            }
            events::Event::GitFileChange(path) => self
                .git_file_change_handler
                .handle(path)
                .context("failed to handle git file change event"),
            events::Event::GitActivity(project) => {
                self.events_sender
                    .send(app_events::Event::git_activity(&project))
                    .context("failed to send git activity event")?;
                Ok(vec![])
            }
            events::Event::GitHeadChange((project, head)) => {
                self.events_sender
                    .send(app_events::Event::git_head(&project, &head))
                    .context("failed to send git head event")?;
                Ok(vec![])
            }
            events::Event::GitIndexChange(project) => {
                self.events_sender
                    .send(app_events::Event::git_index(&project))
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
            events::Event::FlushSession(session) => self
                .flush_session_handler
                .handle(&session)
                .context("failed to handle flush session event"),
            events::Event::SessionFlushed(session) => {
                self.searcher
                    .index_session(self.gb_repository, &session)
                    .context("failed to index session")?;
                Ok(vec![])
            }
            events::Event::FetchProject(project) => self.fetch_project_handler.handle(&project),
        }
    }
}

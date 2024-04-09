mod analytics_handler;
pub mod calculate_deltas_handler;
mod caltulate_virtual_branches_handler;
pub mod fetch_gitbutler_data;
mod filter_ignored_files;
mod flush_session;
pub mod git_file_change;
mod index_handler;
mod push_gitbutler_data;
pub mod push_project_to_gitbutler;

use std::time;

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};
use tracing::instrument;

use super::events;
use crate::events as app_events;

#[derive(Clone)]
pub struct Handler {
    git_file_change_handler: git_file_change::Handler,
    flush_session_handler: flush_session::Handler,
    fetch_gitbutler_handler: fetch_gitbutler_data::Handler,
    push_gitbutler_handler: push_gitbutler_data::Handler,
    analytics_handler: analytics_handler::Handler,
    index_handler: index_handler::Handler,
    push_project_to_gitbutler: push_project_to_gitbutler::Handler,
    calculate_vbranches_handler: caltulate_virtual_branches_handler::Handler,
    calculate_deltas_handler: calculate_deltas_handler::Handler,
    filter_ignored_files_handler: filter_ignored_files::Handler,

    events_sender: app_events::Sender,
}

impl Handler {
    pub fn from_app(app: &AppHandle) -> Result<Self, anyhow::Error> {
        if let Some(handler) = app.try_state::<Handler>() {
            // TODO(ST): figure out of this protections are necessary - is this happening?
            //           `.manage()` can deal with duplication, but maybe there is side-effects?
            Ok(handler.inner().clone())
        } else {
            let handler = Handler {
                git_file_change_handler: git_file_change::Handler::from_app(app)?,
                flush_session_handler: flush_session::Handler::from_app(app)?,
                fetch_gitbutler_handler: fetch_gitbutler_data::Handler::from_app(app)?,
                push_gitbutler_handler: push_gitbutler_data::Handler::from_app(app)?,
                analytics_handler: analytics_handler::Handler::from_app(app)?,
                index_handler: index_handler::Handler::from_app(app)?,

                push_project_to_gitbutler: push_project_to_gitbutler::Handler::from_app(app)?,
                calculate_vbranches_handler: caltulate_virtual_branches_handler::Handler::from_app(
                    app,
                )?,
                calculate_deltas_handler: calculate_deltas_handler::Handler::from_app(app)?,
                filter_ignored_files_handler: filter_ignored_files::Handler::from_app(app)?,
                events_sender: app_events::Sender::from_app(app)?,
            };

            app.manage(handler.clone());
            Ok(handler)
        }
    }
}

impl Handler {
    #[instrument(skip(self), fields(event = %event), level = "debug")]
    pub async fn handle(
        &self,
        event: &events::Event,
        now: time::SystemTime,
    ) -> Result<Vec<events::Event>> {
        match event {
            events::Event::ProjectFileChange(project_id, path) => {
                Ok(vec![events::Event::FilterIgnoredFiles(
                    *project_id,
                    path.clone(),
                )])
            }

            events::Event::FilterIgnoredFiles(project_id, path) => self
                .filter_ignored_files_handler
                .handle(path, project_id)
                .context("failed to handle filter ignored files event"),

            events::Event::GitFileChange(project_id, path) => self
                .git_file_change_handler
                .handle(path, project_id)
                .context("failed to handle git file change event"),

            events::Event::PushGitbutlerData(project_id) => self
                .push_gitbutler_handler
                .handle(project_id)
                .context("failed to push gitbutler data"),

            events::Event::PushProjectToGitbutler(project_id) => self
                .push_project_to_gitbutler
                .handle(project_id)
                .await
                .context("failed to push project to gitbutler"),

            events::Event::FetchGitbutlerData(project_id) => self
                .fetch_gitbutler_handler
                .handle(project_id, &now)
                .await
                .context("failed to fetch gitbutler data"),

            events::Event::Flush(project_id, session) => self
                .flush_session_handler
                .handle(project_id, session)
                .context("failed to handle flush session event"),

            events::Event::SessionFile((project_id, session_id, file_path, contents)) => {
                Ok(vec![events::Event::Emit(app_events::Event::file(
                    project_id,
                    session_id,
                    &file_path.display().to_string(),
                    contents.as_ref(),
                ))])
            }

            events::Event::SessionDelta((project_id, session_id, path, delta)) => {
                self.index_handler
                    .index_deltas(project_id, session_id, path, &vec![delta.clone()])
                    .context("failed to index deltas")?;

                Ok(vec![events::Event::Emit(app_events::Event::deltas(
                    project_id,
                    session_id,
                    &vec![delta.clone()],
                    path,
                ))])
            }

            events::Event::CalculateVirtualBranches(project_id) => self
                .calculate_vbranches_handler
                .handle(project_id)
                .await
                .context("failed to handle virtual branch event"),

            events::Event::CalculateDeltas(project_id, path) => self
                .calculate_deltas_handler
                .handle(path, project_id)
                .context(format!(
                    "failed to handle session processing event: {:?}",
                    path.display()
                )),

            events::Event::Emit(event) => {
                self.events_sender
                    .send(event)
                    .context("failed to send event")?;
                Ok(vec![])
            }

            events::Event::Analytics(event) => self
                .analytics_handler
                .handle(event)
                .await
                .context("failed to handle analytics event"),

            events::Event::Session(project_id, session) => self
                .index_handler
                .index_session(project_id, session)
                .context("failed to index session"),

            events::Event::IndexAll(project_id) => self.index_handler.reindex(project_id),
        }
    }
}

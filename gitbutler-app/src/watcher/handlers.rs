mod analytics_handler;
mod calculate_deltas_handler;
mod caltulate_virtual_branches_handler;
mod fetch_gitbutler_data;
mod fetch_project_data;
mod flush_session;
mod git_file_change;
mod index_handler;
mod push_gitbutler_data;
mod push_project_to_gitbutler;
mod tick_handler;

use std::time;

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};
use tracing::instrument;

use crate::events as app_events;

use super::events;

#[derive(Clone)]
pub struct Handler {
    git_file_change_handler: git_file_change::Handler,
    tick_handler: tick_handler::Handler,
    flush_session_handler: flush_session::Handler,
    fetch_project_handler: fetch_project_data::Handler,
    fetch_gitbutler_handler: fetch_gitbutler_data::Handler,
    push_gitbutler_handler: push_gitbutler_data::Handler,
    analytics_handler: analytics_handler::Handler,
    index_handler: index_handler::Handler,
    push_project_to_gitbutler: push_project_to_gitbutler::Handler,
    calculate_vbranches_handler: caltulate_virtual_branches_handler::Handler,
    calculate_deltas_handler: calculate_deltas_handler::Handler,

    events_sender: app_events::Sender,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        if let Some(handler) = value.try_state::<Handler>() {
            Ok(handler.inner().clone())
        } else {
            let handler = Handler::new(
                git_file_change::Handler::try_from(value)?,
                tick_handler::Handler::try_from(value)?,
                flush_session::Handler::try_from(value)?,
                fetch_project_data::Handler::try_from(value)?,
                fetch_gitbutler_data::Handler::try_from(value)?,
                push_gitbutler_data::Handler::try_from(value)?,
                analytics_handler::Handler::try_from(value)?,
                index_handler::Handler::try_from(value)?,
                push_project_to_gitbutler::Handler::try_from(value)?,
                caltulate_virtual_branches_handler::Handler::try_from(value)?,
                calculate_deltas_handler::Handler::try_from(value)?,
                app_events::Sender::try_from(value)?,
            );
            value.manage(handler.clone());
            Ok(handler)
        }
    }
}

impl Handler {
    #[allow(clippy::too_many_arguments)]
    fn new(
        git_file_change_handler: git_file_change::Handler,
        tick_handler: tick_handler::Handler,
        flush_session_handler: flush_session::Handler,
        fetch_project_handler: fetch_project_data::Handler,
        fetch_gitbutler_handler: fetch_gitbutler_data::Handler,
        push_gitbutler_handler: push_gitbutler_data::Handler,
        analytics_handler: analytics_handler::Handler,
        index_handler: index_handler::Handler,
        push_project_to_gitbutler: push_project_to_gitbutler::Handler,
        calculate_vbranches_handler: caltulate_virtual_branches_handler::Handler,
        calculate_deltas_handler: calculate_deltas_handler::Handler,
        events_sender: app_events::Sender,
    ) -> Self {
        Self {
            git_file_change_handler,
            tick_handler,
            flush_session_handler,
            fetch_project_handler,
            fetch_gitbutler_handler,
            push_gitbutler_handler,
            analytics_handler,
            index_handler,
            push_project_to_gitbutler,
            calculate_vbranches_handler,
            calculate_deltas_handler,
            events_sender,
        }
    }

    #[instrument(skip(self), fields(event = %event), level = "debug")]
    pub async fn handle(
        &self,
        event: &events::Event,
        now: time::SystemTime,
    ) -> Result<Vec<events::Event>> {
        match event {
            events::Event::ProjectFileChange(project_id, path) => Ok(vec![
                events::Event::CalculateDeltas(*project_id, path.clone()),
                events::Event::CalculateVirtualBranches(*project_id),
            ]),

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

            events::Event::FetchProjectData(project_id) => self
                .fetch_project_handler
                .handle(project_id)
                .await
                .context("failed to fetch project data"),

            events::Event::Tick(project_id) => self
                .tick_handler
                .handle(project_id, &now)
                .context("failed to handle tick"),

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

#[cfg(test)]
fn test_remote_repository() -> Result<git2::Repository> {
    let path = tempfile::tempdir()?.path().to_str().unwrap().to_string();
    let repo_a = git2::Repository::init_bare(path)?;

    Ok(repo_a)
}

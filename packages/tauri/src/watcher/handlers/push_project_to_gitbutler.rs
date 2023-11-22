use std::{sync::Arc, time};

use anyhow::{Context, Result};
use tauri::AppHandle;
use tokio::sync::Mutex;

use crate::{
    gb_repository,
    paths::DataDir,
    project_repository,
    projects::{self, CodePushState, ProjectId},
    users,
};

use super::events;

#[derive(Clone)]
pub struct Handler {
    inner: Arc<Mutex<HandlerInner>>,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        let inner = HandlerInner::try_from(value)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }
}

impl Handler {
    pub async fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        if let Ok(inner) = self.inner.try_lock() {
            inner.handle(project_id).await
        } else {
            Ok(vec![])
        }
    }
}

pub struct HandlerInner {
    local_data_dir: DataDir,
    project_store: projects::Controller,
    users: users::Controller,
}

impl TryFrom<&AppHandle> for HandlerInner {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            local_data_dir: DataDir::try_from(value)?,
            project_store: projects::Controller::try_from(value)?,
            users: users::Controller::from(value),
        })
    }
}

impl HandlerInner {
    pub async fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        let project = self
            .project_store
            .get(project_id)
            .context("failed to get project")?;

        if !project.is_sync_enabled() || !project.has_code_url() {
            return Ok(vec![]);
        }

        let user = self.users.get_user()?;
        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;

        let gb_code_last_commit = project
            .gitbutler_code_push_state
            .as_ref()
            .map(|state| &state.id)
            .copied();

        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )?;
        let default_target = gb_repository
            .default_target()
            .context("failed to open gb repo")?
            .context("failed to get default target")?;

        let ids = project_repository.l(
            default_target.sha,
            project_repository::LogUntil::EveryNth {
                n: 1000,
                until_id: gb_code_last_commit,
            },
        )?;

        tracing::debug!(
            %project_id,
            batches=%ids.len(),
            "batches collected",
        );

        let id_count = ids.len();

        for (idx, id) in ids.iter().enumerate().rev() {
            let refspec = format!("+{}:refs/push-tmp/{}", id, project_id);

            match project_repository.push_to_gitbutler_server(user.as_ref(), &[&refspec]) {
                Ok(()) => {}
                Err(project_repository::RemoteError::Network) => return Ok(vec![]),
                Err(err) => return Err(err).context("failed to push"),
            };

            self.project_store
                .update(&projects::UpdateRequest {
                    id: *project_id,
                    gitbutler_code_push_state: Some(CodePushState {
                        id: *id,
                        timestamp: time::SystemTime::now(),
                    }),
                    ..Default::default()
                })
                .await
                .context("failed to update last push")?;

            tracing::info!(
                %project_id,
                "project batch pushed: {}/{}",id_count.saturating_sub(idx),id_count,
            );
        }

        // push refs/{project_id}
        match project_repository.push_to_gitbutler_server(
            user.as_ref(),
            &[&format!("+{}:refs/{}", default_target.sha, project_id)],
        ) {
            Ok(()) => {}
            Err(project_repository::RemoteError::Network) => return Ok(vec![]),
            Err(err) => return Err(err).context("failed to push"),
        };

        let refs = gb_refs(&project_repository)?;

        let all_refs = refs
            .iter()
            .map(|r| format!("+{}:{}", r, r))
            .collect::<Vec<_>>();

        let all_refs = all_refs.iter().map(String::as_str).collect::<Vec<_>>();

        // push all gitbutler refs
        project_repository
            .push_to_gitbutler_server(user.as_ref(), all_refs.as_slice())
            .context("failed to push project (all refs) to gitbutler")?;

        //TODO: remove push-tmp ref

        tracing::info!(
            %project_id,
            "project fully pushed",
        );

        Ok(vec![])
    }
}

fn gb_refs(project_repository: &project_repository::Repository) -> anyhow::Result<Vec<String>> {
    Ok(project_repository
        .git_repository
        .references_glob("refs/*")?
        .flatten()
        .filter_map(|r| r.name().map(|name| name.to_string()))
        .collect::<Vec<_>>())
}

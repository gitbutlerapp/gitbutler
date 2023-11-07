use std::sync::{Arc, Mutex, TryLockError};

use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{
    project_repository,
    projects::{self, ProjectId},
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
    pub fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        match self.inner.try_lock() {
            Ok(inner) => inner.handle(project_id),
            Err(TryLockError::Poisoned(_)) => Err(anyhow::anyhow!("mutex poisoned")),
            Err(TryLockError::WouldBlock) => Ok(vec![]),
        }
    }
}

pub struct HandlerInner {
    project_store: projects::Controller,
    users: users::Controller,
}

impl TryFrom<&AppHandle> for HandlerInner {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            project_store: projects::Controller::try_from(value)?,
            users: users::Controller::from(value),
        })
    }
}

impl HandlerInner {
    pub fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        tracing::info!(
            %project_id,
            "push_project_to_gb::handle",
        );

        let project = self
            .project_store
            .get(project_id)
            .context("failed to get project")?;

        let user = self.users.get_user()?;
        let project_repository = project_repository::Repository::try_from(&project)
            .context("failed to open repository")?;

        if project_repository.project().is_sync_enabled()
            && project_repository.project().has_code_url()
        {
            let head_id = project_repository.get_head()?.peel_to_commit()?.id();
            let gb_code_last_commit = project.gitbutler_code_push.as_ref().copied();

            let ids = project_repository.l(
                head_id,
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

                project_repository
                    .push_to_gitbutler_server(user.as_ref(), &[&refspec])
                    .context("failed to push project to gitbutler")?;

                self.project_store
                    .update(&projects::UpdateRequest {
                        id: *project_id,
                        gitbutler_code_push: Some(*id),
                        ..Default::default()
                    })
                    .context("failed to update last push")?;

                tracing::debug!(
                    %project_id,
                    "project batch pushed: {}/{}",id_count.saturating_sub(idx),id_count,
                );
            }

            // push refs/{project_id}
            project_repository
                .push_to_gitbutler_server(
                    user.as_ref(),
                    &[&format!("+{}:refs/{}", head_id, project_id)],
                )
                .context("failed to push project (head) to gitbutler")?;

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

            tracing::debug!(
                %project_id,
                "project fully pushed",
            );
        } else {
            tracing::debug!(
                %project_id,
                "cannot push code to gb",
            );
        }

        Ok(vec![])
    }
}

fn gb_refs(project_repository: &project_repository::Repository) -> anyhow::Result<Vec<String>> {
    Ok(project_repository
        .git_repository
        .references_glob("refs/gitbutler/*")?
        .flatten()
        .filter_map(|r| r.name().map(ToString::to_string))
        .collect::<Vec<_>>())
}

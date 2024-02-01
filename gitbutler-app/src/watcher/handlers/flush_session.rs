use std::{path, sync::Arc};

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::{
    app,
    appstate::{self, AppState},
    gb_repository, project_repository, projects,
    projects::ProjectId,
    sessions, users,
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
    pub async fn handle(
        &self,
        project_id: &ProjectId,
        session: &sessions::Session,
    ) -> Result<Vec<events::Event>> {
        if let Ok(inner) = self.inner.try_lock() {
            inner.handle(project_id, session).await
        } else {
            Ok(vec![])
        }
    }
}

struct HandlerInner {
    local_data_dir: path::PathBuf,
    project_store: projects::Controller,
    users_controller: Arc<Mutex<users::Controller>>,
}

impl TryFrom<&AppHandle> for HandlerInner {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        let path = value
            .path_resolver()
            .app_data_dir()
            .context("failed to get app data dir")?;
        Ok(Self {
            local_data_dir: path,
            project_store: projects::Controller::try_from(value)?,
            users_controller: Arc::clone(&value.state::<appstate::AppState>().users_controller),
        })
    }
}

impl HandlerInner {
    pub async fn handle(
        &self,
        project_id: &ProjectId,
        session: &sessions::Session,
    ) -> Result<Vec<events::Event>> {
        let project = self
            .project_store
            .get(project_id)
            .context("failed to get project")?;

        let user = async {
            let users_controller = self.users_controller.lock().await;
            users_controller.get_user()
        }
        .await?;
        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        let session = gb_repo
            .flush_session(&project_repository, session, user.as_ref())
            .context(format!("failed to flush session {}", session.id))?;

        Ok(vec![
            events::Event::Session(*project_id, session),
            events::Event::PushGitbutlerData(*project_id),
            events::Event::PushProjectToGitbutler(*project_id),
        ])
    }
}

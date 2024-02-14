use std::{path, sync::Arc};

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::{gb_repository, project_repository, projects, projects::ProjectId, sessions, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    inner: Arc<Mutex<HandlerInner>>,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        if let Some(handler) = value.try_state::<Handler>() {
            Ok(handler.inner().clone())
        } else if let Some(app_data_dir) = value.path_resolver().app_data_dir() {
            let projects = projects::Controller::try_from(value)?;
            let users = users::Controller::try_from(value)?;
            let inner = HandlerInner::new(app_data_dir, projects, users);

            let handler = Handler::new(inner);
            value.manage(handler.clone());
            Ok(handler)
        } else {
            Err(anyhow::anyhow!("failed to get app data dir"))
        }
    }
}

impl Handler {
    fn new(inner: HandlerInner) -> Handler {
        Handler {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub fn handle(
        &self,
        project_id: &ProjectId,
        session: &sessions::Session,
    ) -> Result<Vec<events::Event>> {
        if let Ok(inner) = self.inner.try_lock() {
            inner.handle(project_id, session)
        } else {
            Ok(vec![])
        }
    }
}

struct HandlerInner {
    local_data_dir: path::PathBuf,
    project_store: projects::Controller,
    users: users::Controller,
}

impl HandlerInner {
    fn new(
        local_data_dir: path::PathBuf,
        project_store: projects::Controller,
        users: users::Controller,
    ) -> HandlerInner {
        HandlerInner {
            local_data_dir,
            project_store,
            users,
        }
    }

    pub fn handle(
        &self,
        project_id: &ProjectId,
        session: &sessions::Session,
    ) -> Result<Vec<events::Event>> {
        let project = self
            .project_store
            .get(project_id)
            .context("failed to get project")?;

        let user = self.users.get_user()?;
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

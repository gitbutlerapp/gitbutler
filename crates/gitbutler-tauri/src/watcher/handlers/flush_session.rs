use std::path::PathBuf;
use std::{path, sync::Arc};

use anyhow::{Context, Result};
use gitbutler_core::{
    gb_repository, project_repository, projects, projects::ProjectId, sessions, users,
};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use super::events;

#[derive(Clone)]
pub struct Handler {
    inner: Arc<Mutex<HandlerInner>>,
}

impl Handler {
    pub fn from_app(app: &AppHandle, app_data_dir: impl Into<PathBuf>) -> Self {
        let projects = app.state::<projects::Controller>().inner().clone();
        let users = app.state::<users::Controller>().inner().clone();
        // TODO(ST): see if one day this can be more self-contained so all this nesting isn't required
        let inner = HandlerInner {
            local_data_dir: app_data_dir.into(),
            project_store: projects,
            users,
        };
        Handler {
            inner: Arc::new(inner.into()),
        }
    }
}

impl Handler {
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

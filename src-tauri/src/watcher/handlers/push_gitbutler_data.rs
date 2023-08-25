use std::path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{gb_repository, projects, users};

use super::events;

#[derive(Clone)]
pub struct Handler {
    inner: Arc<HandlerInner>,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            inner: Arc::new(HandlerInner::try_from(value)?),
        })
    }
}

impl Handler {
    pub fn handle(&self, project_id: &str) -> Result<Vec<events::Event>> {
        self.inner.handle(project_id)
    }
}

struct HandlerInner {
    local_data_dir: path::PathBuf,
    project_storage: projects::Storage,
    user_storage: users::Storage,

    // it's ok to use mutex here, because even though project_id is a paramenter, we create
    // and use a handler per project.
    // if that changes, we'll need to use a more granular locking mechanism
    mutex: Mutex<()>,
}

impl TryFrom<&AppHandle> for HandlerInner {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        let local_data_dir = value
            .path_resolver()
            .app_local_data_dir()
            .context("failed to get local data dir")?;
        Ok(Self {
            local_data_dir: local_data_dir.to_path_buf(),
            project_storage: projects::Storage::try_from(value)?,
            user_storage: users::Storage::try_from(value)?,
            mutex: Mutex::new(()),
        })
    }
}

impl HandlerInner {
    pub fn handle(&self, project_id: &str) -> Result<Vec<events::Event>> {
        let _lock = self.mutex.lock().unwrap();

        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            project_id,
            self.project_storage.clone(),
            self.user_storage.clone(),
        )
        .context("failed to open repository")?;

        gb_repo.push().context("failed to push")?;

        Ok(vec![])
    }
}

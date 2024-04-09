use std::{sync::Arc, time::Duration, vec};

use anyhow::{Context, Result};
use gitbutler_core::{
    project_repository,
    projects::{self, ProjectId},
};
use governor::{
    clock::QuantaClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use super::events;

#[derive(Clone)]
pub struct Handler {
    inner: Arc<Mutex<InnerHandler>>,
    limit: Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock>>,
}

impl Handler {
    pub fn from_app(app: &AppHandle) -> std::result::Result<Self, anyhow::Error> {
        if let Some(handler) = app.try_state::<Handler>() {
            Ok(handler.inner().clone())
        } else {
            let projects = app.state::<projects::Controller>().inner().clone();
            let inner = InnerHandler { projects };
            let handler = Handler::new(inner);
            app.manage(handler.clone());
            Ok(handler)
        }
    }
}

impl Handler {
    fn new(inner: InnerHandler) -> Self {
        // There could be an application (e.g an IDE) which is constantly writing, so the threshold cant be too high
        let quota = Quota::with_period(Duration::from_millis(5)).expect("valid quota");
        Self {
            inner: Arc::new(Mutex::new(inner)),
            limit: Arc::new(RateLimiter::direct(quota)),
        }
    }

    pub fn handle<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        project_id: &ProjectId,
    ) -> Result<Vec<events::Event>> {
        if self.limit.check().is_err() {
            Ok(vec![])
        } else if let Ok(handler) = self.inner.try_lock() {
            handler.handle(path, project_id)
        } else {
            Ok(vec![])
        }
    }
}

struct InnerHandler {
    projects: projects::Controller,
}
impl InnerHandler {
    pub fn handle<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        project_id: &ProjectId,
    ) -> Result<Vec<events::Event>> {
        let project = self
            .projects
            .get(project_id)
            .context("failed to get project")?;
        let project_repository = project_repository::Repository::open(&project)
            .with_context(|| "failed to open project repository for project")?;

        if project_repository
            .is_path_ignored(path.as_ref())
            .unwrap_or(false)
        {
            Ok(vec![])
        } else {
            Ok(vec![
                events::Event::CalculateDeltas(*project_id, path.as_ref().to_path_buf()),
                events::Event::CalculateVirtualBranches(*project_id),
            ])
        }
    }
}

use std::sync::Arc;

use anyhow::{Context, Result};
use futures::executor::block_on;
use gitbutler_core::projects::{self, Project, ProjectId};
use gitbutler_core::{assets, deltas, sessions, users, virtual_branches};
use tauri::{AppHandle, Manager};
use tracing::instrument;

mod event {
    use anyhow::{Context, Result};
    use gitbutler_core::projects::ProjectId;
    use gitbutler_core::serde::path::json_escape;
    use gitbutler_watcher::Change;
    use tauri::Manager;

    /// An change we want to inform the frontend about.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(super) struct ChangeForFrontend {
        name: String,
        payload: serde_json::Value,
        project_id: ProjectId,
    }

    impl From<Change> for ChangeForFrontend {
        fn from(value: Change) -> Self {
            match value {
                Change::GitIndex(project_id) => ChangeForFrontend {
                    name: format!("project://{}/git/index", project_id),
                    payload: serde_json::json!({}),
                    project_id,
                },
                Change::GitFetch(project_id) => ChangeForFrontend {
                    name: format!("project://{}/git/fetch", project_id),
                    payload: serde_json::json!({}),
                    project_id,
                },
                Change::GitHead { project_id, head } => ChangeForFrontend {
                    name: format!("project://{}/git/head", project_id),
                    payload: serde_json::json!({ "head": head }),
                    project_id,
                },
                Change::GitActivity(project_id) => ChangeForFrontend {
                    name: format!("project://{}/git/activity", project_id),
                    payload: serde_json::json!({}),
                    project_id,
                },
                Change::File {
                    project_id,
                    session_id,
                    file_path,
                    contents,
                } => ChangeForFrontend {
                    name: format!("project://{}/sessions/{}/files", project_id, session_id),
                    payload: serde_json::json!({
                        "filePath": json_escape(&file_path),
                        "contents": contents,
                    }),
                    project_id,
                },
                Change::Session {
                    project_id,
                    session,
                } => ChangeForFrontend {
                    name: format!("project://{}/sessions", project_id),
                    payload: serde_json::to_value(session).unwrap(),
                    project_id,
                },
                Change::Deltas {
                    project_id,
                    session_id,
                    deltas,
                    relative_file_path,
                } => ChangeForFrontend {
                    name: format!("project://{}/sessions/{}/deltas", project_id, session_id),
                    payload: serde_json::json!({
                        "deltas": deltas,
                        "filePath": relative_file_path,
                    }),
                    project_id,
                },
                Change::VirtualBranches {
                    project_id,
                    virtual_branches,
                } => ChangeForFrontend {
                    name: format!("project://{}/virtual-branches", project_id),
                    payload: serde_json::json!(virtual_branches),
                    project_id,
                },
            }
        }
    }

    impl ChangeForFrontend {
        pub(super) fn send(&self, app_handle: &tauri::AppHandle) -> Result<()> {
            app_handle
                .emit_all(&self.name, Some(&self.payload))
                .context("emit event")?;
            tracing::trace!(event_name = self.name);
            Ok(())
        }
    }
}
use event::ChangeForFrontend;

/// Note that this type is managed in Tauri and thus needs to be send and sync.
#[derive(Clone)]
pub struct Watchers {
    /// NOTE: This handle is required for this type to be self-contained as it's used by `core` through a trait.
    app_handle: AppHandle,
    /// The watcher of the currently active project.
    /// NOTE: This is a `tokio` mutex as this needs to lock the inner option from within async.
    watcher: Arc<tokio::sync::Mutex<Option<gitbutler_watcher::WatcherHandle>>>,
}

fn handler_from_app(app: &AppHandle) -> anyhow::Result<gitbutler_watcher::Handler> {
    let app_data_dir = app
        .path_resolver()
        .app_data_dir()
        .context("failed to get app data dir")?;
    let analytics = app
        .try_state::<gitbutler_analytics::Client>()
        .map_or(gitbutler_analytics::Client::default(), |client| {
            client.inner().clone()
        });
    let users = app.state::<users::Controller>().inner().clone();
    let projects = app.state::<projects::Controller>().inner().clone();
    let vbranches = app.state::<virtual_branches::Controller>().inner().clone();
    let assets_proxy = app.state::<assets::Proxy>().inner().clone();
    let sessions_db = app.state::<sessions::Database>().inner().clone();
    let deltas_db = app.state::<deltas::Database>().inner().clone();

    Ok(gitbutler_watcher::Handler::new(
        app_data_dir.clone(),
        analytics,
        users,
        projects,
        vbranches,
        assets_proxy,
        sessions_db,
        deltas_db,
        {
            let app = app.clone();
            move |change| ChangeForFrontend::from(change).send(&app)
        },
    ))
}

impl Watchers {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            watcher: Default::default(),
        }
    }

    #[instrument(skip(self, project), err(Debug))]
    pub fn watch(&self, project: &projects::Project) -> Result<()> {
        let handler = handler_from_app(&self.app_handle)?;

        let project_id = project.id;
        let project_path = project.path.clone();

        let handle = gitbutler_watcher::watch_in_background(handler, project_path, project_id)?;
        block_on(self.watcher.lock()).replace(handle);
        Ok(())
    }

    pub async fn post(&self, action: gitbutler_watcher::Action) -> Result<()> {
        let watcher = self.watcher.lock().await;
        if let Some(handle) = watcher
            .as_ref()
            .filter(|watcher| watcher.project_id() == action.project_id())
        {
            handle.post(action).await.context("failed to post event")
        } else {
            Err(anyhow::anyhow!("watcher not found",))
        }
    }

    pub async fn stop(&self, project_id: ProjectId) {
        let mut handle = self.watcher.lock().await;
        if handle
            .as_ref()
            .map_or(false, |handle| handle.project_id() == project_id)
        {
            handle.take();
        }
    }
}

#[async_trait::async_trait]
impl gitbutler_core::projects::Watchers for Watchers {
    fn watch(&self, project: &Project) -> Result<()> {
        Watchers::watch(self, project)
    }

    async fn stop(&self, id: ProjectId) {
        Watchers::stop(self, id).await
    }

    async fn fetch_gb_data(&self, id: ProjectId) -> Result<()> {
        self.post(gitbutler_watcher::Action::FetchGitbutlerData(id))
            .await
    }

    async fn push_gb_data(&self, id: ProjectId) -> Result<()> {
        self.post(gitbutler_watcher::Action::PushGitbutlerData(id))
            .await
    }
}

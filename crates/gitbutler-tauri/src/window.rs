use std::sync::Arc;

use anyhow::{Context, Result};
use futures::executor::block_on;
use gitbutler_project as projects;
use gitbutler_project::{Project, ProjectId};
use gitbutler_user as users;
use tauri::{AppHandle, Manager};
use tracing::instrument;

mod event {
    use anyhow::{Context, Result};
    use gitbutler_project::ProjectId;
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

/// The name of the lock file to signal exclusive access to other windows.
const WINDOW_LOCK_FILE: &str = "window.lock";

struct State {
    /// The id of the project displayed by the window.
    project_id: ProjectId,
    /// The watcher of the currently active project.
    watcher: gitbutler_watcher::WatcherHandle,
    /// An active lock to signal that the entire project is locked for the Window this state belongs to.
    exclusive_access: fslock::LockFile,
}

impl Drop for State {
    fn drop(&mut self) {
        // We only do this to display an error if it fails - `LockFile` also implements `Drop`.
        if let Err(err) = self.exclusive_access.unlock() {
            tracing::error!(err = ?err, "Failed to release the project-wide lock");
        }
    }
}

/// State associated to windows
/// Note that this type is managed in Tauri and thus needs to be send and sync.
#[derive(Clone)]
pub struct WindowState {
    /// NOTE: This handle is required for this type to be self-contained as it's used by `core` through a trait.
    app_handle: AppHandle,
    /// The state for the main window.
    /// NOTE: This is a `tokio` mutex as this needs to lock the inner option from within async.
    state: Arc<tokio::sync::Mutex<Option<State>>>,
}

fn handler_from_app(app: &AppHandle) -> anyhow::Result<gitbutler_watcher::Handler> {
    let projects = app.state::<projects::Controller>().inner().clone();
    let users = app.state::<users::Controller>().inner().clone();
    let vbranches = gitbutler_branch_actions::VirtualBranchActions::default();

    Ok(gitbutler_watcher::Handler::new(
        projects,
        users,
        vbranches,
        {
            let app = app.clone();
            move |change| ChangeForFrontend::from(change).send(&app)
        },
    ))
}

impl WindowState {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            app_handle,
            state: Default::default(),
        }
    }

    /// Watch the project and assure no other instance can access it.
    #[instrument(skip(self, project), err(Debug))]
    pub fn watch(&self, project: &projects::Project) -> Result<()> {
        let mut lock_file =
            fslock::LockFile::open(project.gb_dir().join(WINDOW_LOCK_FILE).as_os_str())?;
        lock_file
            .lock()
            .context("Another GitButler Window already has the project opened")?;

        let handler = handler_from_app(&self.app_handle)?;
        let worktree_dir = project.path.clone();
        let project_id = project.id;
        let watcher = gitbutler_watcher::watch_in_background(handler, worktree_dir, project_id)?;
        block_on(self.state.lock()).replace(State {
            project_id,
            watcher,
            exclusive_access: lock_file,
        });
        Ok(())
    }

    pub async fn post(&self, action: gitbutler_watcher::Action) -> Result<()> {
        let state = self.state.lock().await;
        if let Some(state) = state
            .as_ref()
            .filter(|state| state.project_id == action.project_id())
        {
            state
                .watcher
                .post(action)
                .await
                .context("failed to post event")
        } else {
            Err(anyhow::anyhow!(
                "matching watcher to post event not found, wanted {wanted}, got {actual:?}",
                wanted = action.project_id(),
                actual = state.as_ref().map(|s| s.project_id)
            ))
        }
    }

    pub async fn flush(&self) -> Result<()> {
        let state = self.state.lock().await;
        if let Some(state) = state.as_ref() {
            state.watcher.flush()?;
        }

        Ok(())
    }

    pub async fn stop(&self, project_id: ProjectId) {
        let mut state = self.state.lock().await;
        if state
            .as_ref()
            .map_or(false, |state| state.project_id == project_id)
        {
            state.take();
        }
    }
}

#[async_trait::async_trait]
impl projects::Watchers for WindowState {
    fn watch(&self, project: &Project) -> Result<()> {
        WindowState::watch(self, project)
    }

    async fn stop(&self, id: ProjectId) {
        WindowState::stop(self, id).await
    }
}

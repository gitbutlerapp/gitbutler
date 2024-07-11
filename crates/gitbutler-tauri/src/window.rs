pub(super) mod state {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use anyhow::{bail, Context, Result};
    use futures::executor::block_on;
    use gitbutler_project as projects;
    use gitbutler_project::ProjectId;
    use gitbutler_user as users;
    use tauri::{AppHandle, Manager};
    use tracing::instrument;

    mod event {
        use anyhow::{Context, Result};
        use gitbutler_project::ProjectId;
        use gitbutler_watcher::Change;
        use tauri::Manager;

        /// A change we want to inform the frontend about.
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

    type WindowLabel = String;
    pub(super) type WindowLabelRef = str;

    /// State associated to windows
    /// Note that this type is managed in Tauri and thus needs to be `Send` and `Sync`.
    #[derive(Clone)]
    pub struct WindowState {
        /// NOTE: This handle is required for this type to be self-contained as it's used by `core` through a trait.
        app_handle: AppHandle,
        /// The state for every open application window.
        /// NOTE: This is a `tokio` mutex as this needs to lock the inner option from within async.
        state: Arc<tokio::sync::Mutex<BTreeMap<WindowLabel, State>>>,
    }

    fn handler_from_app(app: &AppHandle) -> Result<gitbutler_watcher::Handler> {
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

        /// Watch the `project`, assure no other instance can access it, and associate it with the window
        /// uniquely identified by `window`.
        ///
        /// Previous state will be removed and its resources cleaned up.
        #[instrument(skip(self, project), err(Debug))]
        pub fn set_project_to_window(
            &self,
            window: &WindowLabelRef,
            project: &projects::Project,
        ) -> Result<()> {
            let mut state_by_label = block_on(self.state.lock());
            if let Some(state) = state_by_label.get(window) {
                if state.project_id == project.id {
                    return Ok(());
                }
            }
            let mut lock_file =
                fslock::LockFile::open(project.gb_dir().join(WINDOW_LOCK_FILE).as_os_str())?;
            let got_lock = lock_file
                .try_lock()
                .context("Failed to check if lock is taken")?;
            if !got_lock {
                bail!(
                    "Project '{}' is already opened in another window",
                    project.title
                );
            }

            let handler = handler_from_app(&self.app_handle)?;
            let worktree_dir = project.path.clone();
            let project_id = project.id;
            let watcher =
                gitbutler_watcher::watch_in_background(handler, worktree_dir, project_id)?;
            state_by_label.insert(
                window.to_owned(),
                State {
                    project_id,
                    watcher,
                    exclusive_access: lock_file,
                },
            );
            tracing::debug!("Maintaining {} Windows", state_by_label.len());
            Ok(())
        }

        pub async fn post(&self, action: gitbutler_watcher::Action) -> Result<()> {
            let mut state_by_label = self.state.lock().await;
            let state = state_by_label
                .values_mut()
                .find(|state| state.project_id == action.project_id());
            if let Some(state) = state {
                state
                    .watcher
                    .post(action)
                    .await
                    .context("failed to post event")
            } else {
                Err(anyhow::anyhow!(
                    "matching watcher to post event not found, wanted {wanted}",
                    wanted = action.project_id(),
                ))
            }
        }

        /// Flush file-monitor watcher events once the windows regains focus for it to respond instantly
        /// instead of according to the tick-rate.
        pub fn flush(&self, window: &WindowLabelRef) -> Result<()> {
            let state_by_label = block_on(self.state.lock());
            if let Some(state) = state_by_label.get(window) {
                state.watcher.flush()?;
            }

            Ok(())
        }

        /// Remove the state associated with `window`, typically upon its destruction.
        pub fn remove(&self, window: &WindowLabelRef) {
            let mut state_by_label = block_on(self.state.lock());
            state_by_label.remove(window);
        }

        /// Return the list of project ids that are currently open.
        pub fn open_projects(&self) -> Vec<ProjectId> {
            let state_by_label = block_on(self.state.lock());
            state_by_label
                .values()
                .map(|state| state.project_id)
                .collect()
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn create(
    handle: &tauri::AppHandle,
    label: &state::WindowLabelRef,
    window_relative_url: String,
) -> tauri::Result<tauri::Window> {
    tracing::info!("creating window '{label}' created at '{window_relative_url}'");
    let window = tauri::WindowBuilder::new(
        handle,
        label,
        tauri::WindowUrl::App(window_relative_url.into()),
    )
    .resizable(true)
    .title(handle.package_info().name.clone())
    .disable_file_drop_handler()
    .min_inner_size(800.0, 600.0)
    .inner_size(1160.0, 720.0)
    .build()?;
    Ok(window)
}

#[cfg(target_os = "macos")]
pub fn create(
    handle: &tauri::AppHandle,
    label: &state::WindowLabelRef,
    window_relative_url: String,
) -> tauri::Result<tauri::Window> {
    tracing::info!("creating window '{label}' created at '{window_relative_url}'");
    let window = tauri::WindowBuilder::new(
        handle,
        label,
        tauri::WindowUrl::App(window_relative_url.into()),
    )
    .resizable(true)
    .title(handle.package_info().name.clone())
    .min_inner_size(800.0, 600.0)
    .inner_size(1160.0, 720.0)
    .hidden_title(true)
    .disable_file_drop_handler()
    .title_bar_style(tauri::TitleBarStyle::Overlay)
    .build()?;
    Ok(window)
}

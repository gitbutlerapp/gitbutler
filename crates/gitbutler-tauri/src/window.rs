pub(crate) mod state {
    use std::{collections::BTreeMap, sync::Arc};

    use anyhow::Result;
    use but_settings::AppSettingsWithDiskSync;
    use gitbutler_command_context::CommandContext;
    use gitbutler_project as projects;
    use gitbutler_project::ProjectId;
    use tauri::AppHandle;
    use tracing::instrument;

    pub(crate) mod event {
        use anyhow::{Context, Result};
        use but_db::poll::ItemKind;
        use but_settings::AppSettings;
        use gitbutler_project::ProjectId;
        use gitbutler_watcher::Change;
        use tauri::Emitter;

        /// A change we want to inform the frontend about.
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct ChangeForFrontend {
            name: String,
            payload: serde_json::Value,
            project_id: ProjectId,
        }

        impl From<Change> for ChangeForFrontend {
            fn from(value: Change) -> Self {
                match value {
                    Change::GitFetch(project_id) => ChangeForFrontend {
                        name: format!("project://{project_id}/git/fetch"),
                        payload: serde_json::json!({}),
                        project_id,
                    },
                    Change::GitHead {
                        project_id,
                        head,
                        operating_mode,
                    } => ChangeForFrontend {
                        name: format!("project://{project_id}/git/head"),
                        payload: serde_json::json!({ "head": head, "operatingMode": operating_mode }),
                        project_id,
                    },
                    Change::GitActivity(project_id) => ChangeForFrontend {
                        name: format!("project://{project_id}/git/activity"),
                        payload: serde_json::json!({}),
                        project_id,
                    },
                    Change::WorktreeChanges {
                        project_id,
                        changes,
                    } => ChangeForFrontend {
                        name: format!("project://{project_id}/worktree_changes"),
                        payload: serde_json::json!(&changes),
                        project_id,
                    },
                }
            }
        }

        impl From<AppSettings> for ChangeForFrontend {
            fn from(settings: AppSettings) -> Self {
                ChangeForFrontend {
                    name: "settings://update".to_string(),
                    payload: serde_json::json!(settings),
                    // TODO: remove dummy project id
                    project_id: ProjectId::default(),
                }
            }
        }

        impl From<(ProjectId, ItemKind)> for ChangeForFrontend {
            fn from(project_item: (ProjectId, ItemKind)) -> Self {
                let (project_id, item) = project_item;
                // Use the shared conversion function from but_broadcaster
                let event = but_broadcaster::FrontendEvent::from_db_item(project_id, item);
                ChangeForFrontend {
                    name: event.name,
                    payload: event.payload,
                    project_id,
                }
            }
        }

        impl ChangeForFrontend {
            pub fn send(&self, app_handle: &tauri::AppHandle) -> Result<()> {
                app_handle
                    .emit(&self.name, Some(&self.payload))
                    .context("emit event")?;
                tracing::trace!(event_name = self.name);
                Ok(())
            }
        }
    }
    use event::ChangeForFrontend;

    struct State {
        /// The id of the project displayed by the window.
        project_id: ProjectId,
        /// The watcher of the currently active project.
        watcher: gitbutler_watcher::WatcherHandle,
        /// An active lock to signal that the entire project is locked for the Window this state belongs to.
        /// Let's make it optional while it's only in our own way, while aiming for making that reasonably well working.
        exclusive_access: Option<but_core::sync::LockFile>,
        // Database watcher handle.
        #[expect(dead_code)]
        db_watcher: but_db::poll::DBWatcherHandle,
    }

    impl Drop for State {
        fn drop(&mut self) {
            // We only do this to display an error if it fails - `LockFile` also implements `Drop`.
            if let Some(Err(err)) = self.exclusive_access.take().map(|mut lock| lock.unlock()) {
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
        state: Arc<parking_lot::Mutex<BTreeMap<WindowLabel, State>>>,
    }

    fn handler_from_app(app_handle: &AppHandle) -> Result<gitbutler_watcher::Handler> {
        Ok(gitbutler_watcher::Handler::new({
            let app = app_handle.clone();
            move |change| ChangeForFrontend::from(change).send(&app)
        }))
    }

    #[derive(Debug)]
    pub enum ProjectAccessMode {
        // This is the first window to look at a project.
        First,
        // This is not the first Window to look at the project.
        Shared,
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
        /// The previous state will be removed and its resources cleaned up.
        #[instrument(skip(self, project, app_settings, ctx), err(Debug))]
        pub fn set_project_to_window(
            &self,
            window: &WindowLabelRef,
            project: &projects::Project,
            app_settings: &AppSettingsWithDiskSync,
            ctx: &mut CommandContext,
        ) -> Result<ProjectAccessMode> {
            let mut state_by_label = self.state.lock();
            if let Some(state) = state_by_label.get(window)
                && state.project_id == project.id
            {
                return Ok(state
                    .exclusive_access
                    .as_ref()
                    .map(|_| ProjectAccessMode::First)
                    .unwrap_or(ProjectAccessMode::Shared));
            }
            let exclusive_access = project.try_exclusive_access().ok();
            let handler = handler_from_app(&self.app_handle)?;
            let worktree_dir = project.worktree_dir()?;
            let project_id = project.id;
            let watcher = gitbutler_watcher::watch_in_background(
                handler,
                worktree_dir,
                project_id,
                app_settings.clone(),
            )?;

            let db = ctx.db()?;
            let db_watcher = but_db::poll::watch_in_background(db, {
                let app_handle = self.app_handle.clone();
                move |item| ChangeForFrontend::from((project_id, item)).send(&app_handle)
            })?;

            let has_exclusive_access = exclusive_access.is_some();
            state_by_label.insert(
                window.to_owned(),
                State {
                    project_id,
                    watcher,
                    exclusive_access,
                    db_watcher,
                },
            );
            tracing::debug!("Maintaining {} Windows", state_by_label.len());
            Ok(if has_exclusive_access {
                ProjectAccessMode::First
            } else {
                ProjectAccessMode::Shared
            })
        }

        /// Flush file-monitor watcher events once the windows regains focus for it to respond instantly
        /// instead of according to the tick-rate.
        pub fn flush(&self, window: &WindowLabelRef) -> Result<()> {
            let state_by_label = self.state.lock();
            if let Some(state) = state_by_label.get(window) {
                state.watcher.flush()?;
            }

            Ok(())
        }

        /// Remove the state associated with `window`, typically upon its destruction.
        pub fn remove(&self, window: &WindowLabelRef) {
            let mut state_by_label = self.state.lock();
            state_by_label.remove(window);
        }

        /// Return the list of project ids that are currently open.
        pub fn open_projects(&self) -> Vec<ProjectId> {
            let state_by_label = self.state.lock();
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
) -> tauri::Result<tauri::WebviewWindow> {
    tracing::info!("creating window '{label}' created at '{window_relative_url}'");
    let window = tauri::WebviewWindowBuilder::new(
        handle,
        label,
        tauri::WebviewUrl::App(window_relative_url.into()),
    )
    .resizable(true)
    .title(handle.package_info().name.clone())
    .disable_drag_drop_handler()
    .min_inner_size(1000.0, 600.0)
    .inner_size(1160.0, 720.0)
    .build()?;
    Ok(window)
}

#[cfg(target_os = "macos")]
pub fn create(
    handle: &tauri::AppHandle,
    label: &state::WindowLabelRef,
    window_relative_url: String,
) -> tauri::Result<tauri::WebviewWindow> {
    tracing::info!("creating window '{label}' created at '{window_relative_url}'");

    let use_native_title_bar = but_settings::AppSettings::load_from_default_path_creating()
        .ok()
        .map(|settings| settings.ui.use_native_title_bar)
        .unwrap_or(false);

    let window = tauri::WebviewWindowBuilder::new(
        handle,
        label,
        tauri::WebviewUrl::App(window_relative_url.into()),
    )
    .resizable(true)
    .title(handle.package_info().name.clone())
    .min_inner_size(1000.0, 600.0)
    .inner_size(1160.0, 720.0)
    .disable_drag_drop_handler()
    .hidden_title(!use_native_title_bar)
    .title_bar_style(if use_native_title_bar {
        tauri::TitleBarStyle::Visible
    } else {
        tauri::TitleBarStyle::Overlay
    })
    .build()?;

    if !use_native_title_bar {
        use tauri::{LogicalPosition, Manager};
        use tauri_plugin_trafficlights_positioner::WindowExt;
        if let Some(window) = window.get_window(label) {
            // Note that these lights get reset when the Window label is changed!
            // See https://github.com/tauri-apps/tauri/issues/13044 .
            // TODO: stop doing this entirely and work with the defaults, my preference,
            //       create a build script that makes it clear which MacOS version we are using
            //       to conditionally compile the right value.
            let y_offset_for_small_dots_in_pre_tahoe = 25.0;
            window.setup_traffic_lights_inset(LogicalPosition::new(
                16.0,
                y_offset_for_small_dots_in_pre_tahoe,
            ))?;
        }
    }

    Ok(window)
}

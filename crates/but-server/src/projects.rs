use std::collections::HashMap;

use anyhow::{Context as _, Result};
use but_api::json::ToJsonError;
use but_claude::{Claude, broadcaster::FrontendEvent};
use but_db::poll::DBWatcherHandle;
use but_settings::AppSettingsWithDiskSync;
use gitbutler_command_context::CommandContext;
use gitbutler_project::{Project, ProjectId};
use gitbutler_watcher::{Change, WatcherHandle};
use serde::Deserialize;
use serde_json::json;

use crate::Extra;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetProjectActiveParams {
    id: ProjectId,
}

struct ProjectHandles {
    // Watchers are kept alive, drop handles cleanup.
    _file_watcher: WatcherHandle,
    // Watchers are kept alive, drop handles cleanup.
    _db_watcher: DBWatcherHandle,
}

pub struct ActiveProjects {
    projects: HashMap<ProjectId, ProjectHandles>,
}

impl ActiveProjects {
    pub fn new() -> Self {
        Self {
            projects: HashMap::new(),
        }
    }

    pub fn set_active(
        &mut self,
        project: &Project,
        ctx: &Claude,
        app_settings_sync: AppSettingsWithDiskSync,
    ) -> Result<()> {
        if self.projects.contains_key(&project.id) {
            return Ok(());
        }

        // Set up file watcher for worktree changes
        let handler = gitbutler_watcher::Handler::new({
            let broadcaster = ctx.broadcaster.clone();

            move |value| {
                let frontend_event = match value {
                    Change::GitFetch(project_id) => FrontendEvent {
                        name: format!("project://{project_id}/git/fetch"),
                        payload: serde_json::json!({}),
                    },
                    Change::GitHead {
                        project_id,
                        head,
                        operating_mode,
                    } => FrontendEvent {
                        name: format!("project://{project_id}/git/head"),
                        payload: serde_json::json!({ "head": head, "operatingMode": operating_mode }),
                    },
                    Change::GitActivity(project_id) => FrontendEvent {
                        name: format!("project://{project_id}/git/activity"),
                        payload: serde_json::json!({}),
                    },
                    Change::WorktreeChanges {
                        project_id,
                        changes,
                    } => FrontendEvent {
                        name: format!("project://{project_id}/worktree_changes"),
                        payload: serde_json::json!(&changes),
                    },
                };

                println!("Sending event");
                broadcaster.blocking_lock().send(frontend_event);
                println!("Sent event");
                Ok(())
            }
        });

        let file_watcher = gitbutler_watcher::watch_in_background(
            handler,
            project.worktree_dir()?,
            project.id,
            app_settings_sync.clone(),
        )?;

        // Set up database watcher for database changes
        let settings = app_settings_sync.get()?.clone();
        let mut command_ctx = CommandContext::open(project, settings)?;
        let db = command_ctx.db()?;
        let db_watcher = but_db::poll::watch_in_background(db, {
            let broadcaster = ctx.broadcaster.clone();
            let project_id = project.id;
            move |item| {
                let event = FrontendEvent::from_db_item(project_id, item);
                let broadcaster = broadcaster.clone();
                tokio::task::spawn(async move {
                    broadcaster.lock().await.send(event);
                });
                Ok(())
            }
        })?;

        self.projects.insert(
            project.id,
            ProjectHandles {
                _file_watcher: file_watcher,
                _db_watcher: db_watcher,
            },
        );
        Ok(())
    }
}

/// Additional information to help the user interface communicate what happened with the project.
#[derive(Debug, serde::Serialize)]
pub struct ProjectInfo {
    /// `true` if the window is the first one to open the project.
    is_exclusive: bool,
    /// The display version of the error that communicates what went wrong while opening the database.
    db_error: Option<String>,
    /// Provide information about the project just opened.
    headsup: Option<String>,
}

pub async fn list_projects(extra: &Extra) -> anyhow::Result<serde_json::Value> {
    let active_projects = extra.active_projects.lock().await;
    let project_ids: Vec<ProjectId> = active_projects.projects.keys().copied().collect();
    let projects_for_frontend = but_api::legacy::projects::list_projects(project_ids)?;
    Ok(json!(projects_for_frontend))
}

pub async fn set_project_active(
    ctx: &Claude,
    extra: &Extra,
    app_settings_sync: AppSettingsWithDiskSync,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let params: SetProjectActiveParams = serde_json::from_value(params).to_json_error()?;
    let project = gitbutler_project::get_validated(params.id).context("project not found")?;

    // TODO: Adding projects to a list of active projects requires some more
    // knowledge around how many unique tabs are looking at it

    let mut active_projects = extra.active_projects.lock().await;
    active_projects.set_active(&project, ctx, app_settings_sync)?;

    // let is_exclusive = !active_projects.projects.contains(&params.id);

    Ok(json!(ProjectInfo {
        is_exclusive: true,
        db_error: None,
        headsup: None
    }))
}

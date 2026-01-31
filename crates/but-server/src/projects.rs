use std::collections::HashMap;

use anyhow::Result;
use but_api::json::ToJsonError;
use but_claude::{Claude, broadcaster::FrontendEvent};
use but_ctx::Context;
use but_db::poll::DBWatcherHandle;
use but_settings::AppSettingsWithDiskSync;
use gitbutler_project::ProjectId;
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
        ctx: &mut Context,
        claude: &Claude,
        app_settings_sync: AppSettingsWithDiskSync,
    ) -> Result<()> {
        if self.projects.contains_key(&ctx.legacy_project.id) {
            return Ok(());
        }

        // Set up file watcher for worktree changes
        let handler = gitbutler_watcher::Handler::new({
            let broadcaster = claude.broadcaster.clone();

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
                    Change::GitActivity {
                        project_id,
                        head_sha,
                    } => FrontendEvent {
                        name: format!("project://{project_id}/git/activity"),
                        payload: serde_json::json!({
                            "headSha": head_sha,
                        }),
                    },
                    Change::WorktreeChanges {
                        project_id,
                        changes,
                    } => FrontendEvent {
                        name: format!("project://{project_id}/worktree_changes"),
                        payload: serde_json::json!(&changes),
                    },
                };

                broadcaster.blocking_lock().send(frontend_event);
                Ok(())
            }
        });

        let watch_mode = gitbutler_watcher::WatchMode::from_env_or_settings(
            &app_settings_sync.get()?.feature_flags.watch_mode,
        );
        let file_watcher = gitbutler_watcher::watch_in_background(
            handler,
            ctx.workdir_or_fail()?,
            ctx.legacy_project.id,
            app_settings_sync.clone(),
            watch_mode,
        )?;

        // Set up database watcher for database changes
        let db_watcher = {
            let db = &mut *ctx.db.get_mut()?;
            but_db::poll::watch_in_background(db, {
                let broadcaster = claude.broadcaster.clone();
                let project_id = ctx.legacy_project.id;
                move |item| {
                    let event = FrontendEvent::from_db_item(project_id, item);
                    let broadcaster = broadcaster.clone();
                    tokio::task::spawn(async move {
                        broadcaster.lock().await.send(event);
                    });
                    Ok(())
                }
            })?
        };

        self.projects.insert(
            ctx.legacy_project.id,
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
    claude: &Claude,
    extra: &Extra,
    app_settings_sync: AppSettingsWithDiskSync,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let params: SetProjectActiveParams = serde_json::from_value(params).to_json_error()?;

    // TODO(ctx): Adding projects to a list of active projects requires some more
    //            knowledge around how many unique tabs are looking at it

    let mut active_projects = extra.active_projects.lock().await;
    let mut ctx = Context::new_from_legacy_project_id(params.id)?;
    active_projects.set_active(&mut ctx, claude, app_settings_sync)?;

    // let is_exclusive = !active_projects.projects.contains(&params.id);

    Ok(json!(ProjectInfo {
        is_exclusive: true,
        db_error: None,
        headsup: None
    }))
}

use anyhow::{Context as _, Result};
use gitbutler_project::{self as projects, Project, ProjectId};
use gitbutler_watcher::{Change, WatcherHandle};
use serde::Deserialize;
use serde_json::json;
use std::{collections::HashMap, path::Path};

use crate::{FrontendEvent, RequestContext};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProjectParams {
    project: projects::UpdateRequest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddProjectParams {
    path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetProjectParams {
    project_id: ProjectId,
    no_validation: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetProjectActiveParams {
    id: ProjectId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteProjectParams {
    project_id: ProjectId,
}

pub struct ActiveProjects {
    projects: HashMap<ProjectId, WatcherHandle>,
}

impl ActiveProjects {
    pub fn new() -> Self {
        Self {
            projects: HashMap::new(),
        }
    }

    pub fn set_active(&mut self, project: &Project, ctx: &RequestContext) -> Result<()> {
        if self.projects.contains_key(&project.id) {
            return Ok(());
        }

        let handler = gitbutler_watcher::Handler::new(
            (*ctx.project_controller).clone(),
            (*ctx.user_controller).clone(),
            {
                let broadcaster = ctx.broadcaster.clone();

                move |value| {
                    let frontend_event = match value {
                        Change::GitFetch(project_id) => FrontendEvent {
                            name: format!("project://{}/git/fetch", project_id),
                            payload: serde_json::json!({}),
                        },
                        Change::GitHead {
                            project_id,
                            head,
                            operating_mode,
                        } => FrontendEvent {
                            name: format!("project://{}/git/head", project_id),
                            payload: serde_json::json!({ "head": head, "operatingMode": operating_mode }),
                        },
                        Change::GitActivity(project_id) => FrontendEvent {
                            name: format!("project://{}/git/activity", project_id),
                            payload: serde_json::json!({}),
                        },
                        Change::WorktreeChanges {
                            project_id,
                            changes,
                        } => FrontendEvent {
                            name: format!("project://{}/worktree_changes", project_id),
                            payload: serde_json::json!(&changes),
                        },
                    };

                    println!("Sending event");
                    broadcaster.blocking_lock().send(frontend_event);
                    println!("Sent event");
                    Ok(())
                }
            },
        );

        let watcher = gitbutler_watcher::watch_in_background(
            handler,
            project.worktree_path(),
            project.id,
            (*ctx.app_settings).clone(),
        )?;

        self.projects.insert(project.id, watcher);
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

pub fn update_project(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let params: UpdateProjectParams = serde_json::from_value(params)?;
    let updated_project = ctx.project_controller.update(&params.project)?;
    Ok(serde_json::to_value(updated_project)?)
}

pub fn add_project(ctx: &RequestContext, params: serde_json::Value) -> Result<serde_json::Value> {
    let params: AddProjectParams = serde_json::from_value(params)?;
    let path = Path::new(&params.path);

    let user = ctx.user_controller.get_user()?;
    let name = user.as_ref().and_then(|u| u.name.clone());
    let email = user.as_ref().and_then(|u| u.email.clone());

    let project = ctx.project_controller.add(path, name, email)?;
    Ok(serde_json::to_value(project)?)
}

pub fn get_project(ctx: &RequestContext, params: serde_json::Value) -> Result<serde_json::Value> {
    let params: GetProjectParams = serde_json::from_value(params)?;
    let no_validation = params.no_validation.unwrap_or(false);

    let project = if no_validation {
        ctx.project_controller.get_raw(params.project_id)?
    } else {
        ctx.project_controller.get_validated(params.project_id)?
    };

    Ok(serde_json::to_value(project)?)
}

pub async fn list_projects(ctx: &RequestContext) -> Result<serde_json::Value> {
    let active_projects = ctx.active_projects.lock().await;
    // For server implementation, we don't have window state, so all projects are marked as not open
    let projects_for_frontend = ctx
        .project_controller
        .assure_app_can_startup_or_fix_it(ctx.project_controller.list())?
        .into_iter()
        .map(|project| ProjectForFrontend {
            is_open: active_projects.projects.contains_key(&project.id),
            inner: project,
        })
        .collect::<Vec<_>>();

    Ok(json!(projects_for_frontend))
}

pub async fn set_project_active(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let params: SetProjectActiveParams = serde_json::from_value(params)?;
    let project = ctx
        .project_controller
        .get_validated(params.id)
        .context("project not found")?;

    // TODO: Adding projects to a list of active projects requires some more
    // knowledge around how many unique tabs are looking at it

    let mut active_projects = ctx.active_projects.lock().await;
    active_projects.set_active(&project, ctx)?;

    // let is_exclusive = !active_projects.projects.contains(&params.id);

    // TODO: Migrate DB, start watcher

    Ok(json!(ProjectInfo {
        is_exclusive: true,
        db_error: None,
        headsup: None
    }))
}

pub fn delete_project(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let params: DeleteProjectParams = serde_json::from_value(params)?;
    ctx.project_controller.delete(params.project_id)?;
    Ok(json!({}))
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ProjectForFrontend {
    #[serde(flatten)]
    pub inner: Project,
    /// Tell if the project is known to be open in a Window in the frontend.
    pub is_open: bool,
}

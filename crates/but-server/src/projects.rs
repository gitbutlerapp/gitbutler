use anyhow::Result;
use gitbutler_project::{self as projects, Project, ProjectId};
use serde_json::json;
use std::path::Path;

use crate::RequestContext;

pub fn update_project(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let project: projects::UpdateRequest = serde_json::from_value(params["project"].clone())?;
    let updated_project = ctx.project_controller.update(&project)?;
    Ok(serde_json::to_value(updated_project)?)
}

pub fn add_project(ctx: &RequestContext, params: serde_json::Value) -> Result<serde_json::Value> {
    let path = params["path"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("path is required"))?;
    let path = Path::new(path);

    let user = ctx.user_controller.get_user()?;
    let name = user.as_ref().and_then(|u| u.name.clone());
    let email = user.as_ref().and_then(|u| u.email.clone());

    let project = ctx.project_controller.add(path, name, email)?;
    Ok(serde_json::to_value(project)?)
}

pub fn get_project(ctx: &RequestContext, params: serde_json::Value) -> Result<serde_json::Value> {
    let id: ProjectId = serde_json::from_value(params["projectId"].clone())?;
    let no_validation = params["no_validation"].as_bool().unwrap_or(false);

    let project = if no_validation {
        ctx.project_controller.get_raw(id)?
    } else {
        ctx.project_controller.get_validated(id)?
    };

    Ok(serde_json::to_value(project)?)
}

pub fn list_projects(
    ctx: &RequestContext,
    _params: serde_json::Value,
) -> Result<serde_json::Value> {
    // For server implementation, we don't have window state, so all projects are marked as not open
    let projects_for_frontend = ctx
        .project_controller
        .assure_app_can_startup_or_fix_it(ctx.project_controller.list())?
        .into_iter()
        .map(|project| ProjectForFrontend {
            is_open: false,
            inner: project,
        })
        .collect::<Vec<_>>();

    Ok(json!(projects_for_frontend))
}

pub fn delete_project(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let id: ProjectId = serde_json::from_value(params["projectId"].clone())?;
    ctx.project_controller.delete(id)?;
    Ok(json!({}))
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ProjectForFrontend {
    #[serde(flatten)]
    pub inner: Project,
    /// Tell if the project is known to be open in a Window in the frontend.
    pub is_open: bool,
}

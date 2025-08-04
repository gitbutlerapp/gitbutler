use crate::{App, error::Error};
use gitbutler_project::{self as projects, ProjectId};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectParams {
    pub project: projects::UpdateRequest,
}

pub fn update_project(_app: &App, params: UpdateProjectParams) -> Result<projects::Project, Error> {
    Ok(gitbutler_project::update(&params.project)?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddProjectParams {
    pub path: PathBuf,
}

pub fn add_project(_app: &App, params: AddProjectParams) -> Result<projects::Project, Error> {
    let user = gitbutler_user::get_user()?;
    let name = user.as_ref().and_then(|u| u.name.clone());
    let email = user.as_ref().and_then(|u| u.email.clone());
    Ok(gitbutler_project::add(&params.path, name, email)?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetProjectParams {
    pub project_id: ProjectId,
    pub no_validation: Option<bool>,
}

pub fn get_project(_app: &App, params: GetProjectParams) -> Result<projects::Project, Error> {
    if params.no_validation.unwrap_or(false) {
        Ok(gitbutler_project::get_raw(params.project_id)?)
    } else {
        Ok(gitbutler_project::get_validated(params.project_id)?)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteProjectParams {
    pub project_id: ProjectId,
}

pub fn delete_project(_app: &App, params: DeleteProjectParams) -> Result<(), Error> {
    gitbutler_project::delete(params.project_id).map_err(Into::into)
}

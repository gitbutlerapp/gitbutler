use crate::RequestContext;
use gitbutler_project::ProjectId;
use gitbutler_repo::RepoCommands;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectParams {
    project_id: ProjectId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddRemoteParams {
    project_id: ProjectId,
    name: String,
    url: String,
}

pub fn list_remotes(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: ProjectParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    let remotes = project.remotes()?;
    Ok(serde_json::to_value(remotes)?)
}

pub fn add_remote(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: AddRemoteParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    project.add_remote(&params.name, &params.url)?;
    Ok(serde_json::Value::Null)
}

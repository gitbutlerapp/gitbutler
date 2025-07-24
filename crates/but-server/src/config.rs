use crate::RequestContext;
use but_core::{RepositoryExt, settings::git::ui::GitConfigSettings};
use gitbutler_project::ProjectId;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectParams {
    project_id: ProjectId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SetConfigParams {
    project_id: ProjectId,
    config: GitConfigSettings,
}

pub fn get_gb_config(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: ProjectParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    let config: GitConfigSettings = but_core::open_repo(project.path)?.git_settings()?.into();
    Ok(serde_json::to_value(config)?)
}

pub fn set_gb_config(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: SetConfigParams = serde_json::from_value(params)?;
    let project = ctx.project_controller.get(params.project_id)?;
    but_core::open_repo(project.path)?.set_git_settings(&params.config.into())?;
    Ok(serde_json::Value::Null)
}

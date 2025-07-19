use crate::RequestContext;
use anyhow::Context;
use gitbutler_error::{error, error::Code};
use gitbutler_feedback::Archival;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetProjectArchivePathParams {
    project_id: String,
}

pub fn get_project_archive_path(
    ctx: &RequestContext,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let params: GetProjectArchivePathParams = serde_json::from_value(params)?;

    let project_id = params
        .project_id
        .parse()
        .context(error::Context::new_static(
            Code::Validation,
            "Malformed project id",
        ))?;

    // Create archival instance - similar to how it's managed in Tauri
    let cache_dir = dirs::cache_dir()
        .expect("missing cache dir")
        .join("gitbutler-server");
    let logs_dir = dirs::cache_dir()
        .expect("missing cache dir")
        .join("gitbutler-server")
        .join("logs");

    let archival = Archival {
        cache_dir,
        logs_dir,
        projects_controller: (*ctx.project_controller).clone(),
    };

    let path = archival.archive(project_id)?;
    Ok(serde_json::to_value(path)?)
}

pub fn get_logs_archive_path(
    _ctx: &RequestContext,
    _params: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    // Create archival instance - similar to how it's managed in Tauri
    let cache_dir = dirs::cache_dir()
        .expect("missing cache dir")
        .join("gitbutler-server");
    let logs_dir = dirs::cache_dir()
        .expect("missing cache dir")
        .join("gitbutler-server")
        .join("logs");

    let archival = Archival {
        cache_dir,
        logs_dir,
        projects_controller: (*_ctx.project_controller).clone(),
    };

    let path = archival.logs_archive()?;
    Ok(serde_json::to_value(path)?)
}

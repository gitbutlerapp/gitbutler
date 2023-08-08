use anyhow::Context;
use tauri::{AppHandle, Manager};
use timed::timed;

use crate::error::Error;

use super::controller::Controller;

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
pub async fn commit_virtual_branch(
    handle: AppHandle,
    project_id: &str,
    branch: &str,
    message: &str,
) -> Result<(), Error> {
    handle
        .state::<Controller>()
        .create_commit(project_id, branch, message)
        .await
        .context("failed to create commit")?;
    Ok(())
}

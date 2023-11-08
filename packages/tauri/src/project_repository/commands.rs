use tauri::Manager;
use tracing::instrument;

use crate::error::{Code, Error};

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn flush_project(handle: tauri::AppHandle, id: &str) -> Result<(), Error> {
    let id = id.parse().map_err(|_| Error::UserError {
        code: Code::Validation,
        message: "Malformed project id".into(),
    })?;

    let controller = handle.state::<crate::project_repository::Controller>();

    controller.flush(&id).map_err(Into::into)
}

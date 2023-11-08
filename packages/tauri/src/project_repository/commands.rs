use std::path;

use tauri::Manager;
use tracing::instrument;

use crate::{app, error::Error};

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn flush_project(handle: tauri::AppHandle, id: &str) -> Result<(), Error> {
    let app = handle.state::<app::App>();

    Ok(())
}

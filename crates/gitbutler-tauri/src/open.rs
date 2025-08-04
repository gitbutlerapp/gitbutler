use but_api::{commands::open, App};
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn open_url(app: State<'_, App>, url: String) -> Result<(), Error> {
    open::open_url(&app, open::OpenUrlParams { url })
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn show_in_finder(app: State<'_, App>, path: String) -> Result<(), Error> {
    open::show_in_finder(&app, open::ShowInFinderParams { path })
}

use but_api::commands::open;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn open_url(url: String) -> Result<(), Error> {
    open::open_url(open::OpenUrlParams { url })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn show_in_finder(path: String) -> Result<(), Error> {
    open::show_in_finder(open::ShowInFinderParams { path })
}

use but_api::commands::open;

use but_api::error::Error;

#[tauri::command(async)]
pub fn open_url(url: String) -> Result<(), Error> {
    open::open_url(url)
}

#[tauri::command(async)]
pub fn show_in_finder(path: String) -> Result<(), Error> {
    open::show_in_finder(path)
}

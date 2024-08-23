use crate::error::Error;
use tracing::instrument;

pub fn open_that(path: String) {
    // `open` required to run in separate thread, to avoid blocking on some
    // platforms (eg Fedora38 blocks)
    std::thread::spawn(|| {
        for mut cmd in open::commands(path) {
            // required to set path to good one to use `open` on Ubuntu 22
            // (otherwise can be permission error)
            cmd.current_dir(std::env::temp_dir());
            if cmd.status().is_ok() {
                break;
            };
        }
    });
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn open_url(url: String) -> Result<(), Error> {
    open_that(url);
    Ok(())
}

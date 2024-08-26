use crate::error::Error;
use anyhow::anyhow;
use regex::Regex;
use tracing::instrument;

pub fn open_that(path: String) -> Result<(), Error> {
    let re = Regex::new(r"^((https://)|(http://)|(mailto:)|(vscode://)|(vscodium://)).+").unwrap();
    if !re.is_match(&path) {
        return Err(anyhow!("Invalid path format").into());
    }

    std::thread::spawn(|| {
        for mut cmd in open::commands(path) {
            cmd.current_dir(std::env::temp_dir());
            if cmd.status().is_ok() {
                break;
            };
        }
    });
    Ok(())
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn open_url(url: String) -> Result<(), Error> {
    open_that(url).unwrap();
    Ok(())
}

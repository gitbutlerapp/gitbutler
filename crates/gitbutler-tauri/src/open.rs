use crate::error::Error;
use anyhow::anyhow;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use tracing::instrument;

pub fn open_that(path: &str) -> Result<(), Error> {
    let re = Regex::new(r"^((https://)|(http://)|(mailto:)|(vscode://)|(vscodium://)).+").unwrap();
    if !re.is_match(&path) {
        return Err(anyhow!("Invalid path format").into());
    }

    let filtered_env: HashMap<String, String> = env::vars()
        .filter(|&(ref k, _)| {
            k == "BROWSER"
                || k == "NIXOS_XDG_OPEN_USE_PORTAL"
                || k == "LANG"
                || k == "PATH"
                || k == "DISPLAY"
                || k == "WAYLAND_DISPLAY"
                || k == "DBUS_SESSION_BUS_ADDRESS"
        })
        .collect();

    std::thread::spawn({
        let path = path.to_string();
        move || {
            for mut cmd in open::commands(&path) {
                cmd.env_clear();
                cmd.envs(&filtered_env);
                cmd.current_dir(env::temp_dir());
                if cmd.status().is_ok() {
                    break;
                };
            }
        }
    });
    Ok(())
}

#[tauri::command()]
#[instrument(err(Debug))]
pub fn open_url(url: &str) -> Result<(), Error> {
    open_that(url).unwrap();
    Ok(())
}

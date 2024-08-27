use crate::error::Error;
use anyhow::anyhow;
use regex::Regex;
use tracing::instrument;

// fix_env_variables
use std::ffi::{OsStr, OsString};
use std::io;
use std::os::unix::prelude::OsStrExt;
use std::process::{Command, Stdio};
use std::sync::Arc;

use arc_swap::ArcSwapOption;
use nix::libc::uname;
use tauri::Manager;

pub fn open_that(path: &str) -> Result<(), Error> {
    let re = Regex::new(r"^((https://)|(http://)|(mailto:)|(vscode://)|(vscodium://)).+").unwrap();
    if !re.is_match(&path) {
        return Err(anyhow!("Invalid path format").into());
    }

    std::thread::spawn({
        let path = path.to_string();
        move || {
            for mut cmd in open::commands(&path) {
                fix_env_variables(&mut cmd);
                cmd.current_dir(std::env::temp_dir());
                if cmd.status().is_ok() {
                    break;
                };
            }
        }
    });
    Ok(())
}

static APPDIR: ArcSwapOption<OsString> = ArcSwapOption::const_empty();

pub fn initialize(app_handle: tauri::AppHandle) {
    APPDIR.store(app_handle.env().appdir.clone().map(Arc::from));
}

pub(super) fn fix_env_variables(command: &mut Command) {
    // NOTE: this does not handle env_clear correctly

    let appdir = APPDIR.load();
    let Some(appdir) = appdir.as_ref() else {
        return;
    };
    let appdir = appdir.as_ref();
    let appdir_bytes = appdir.as_bytes();

    if appdir_bytes.contains(&b':') || appdir_bytes.is_empty() {
        // if the appdir contains ':',
        // the PATH variables become wired (broken) and impossible to fix so keep as-is
        return;
    }

    // process path-like variables (remove appdir-related paths)
    // see https://github.com/AppImage/AppImageKit/blob/e8dadbb09fed3ae3c3d5a5a9ba2c47a072f71c40/src/AppRun.c#L171-L194
    // LD_LIBRARY_PATH is necessary for xdg-open to work correctly
    for var_name in [
        "PATH",
        "LD_LIBRARY_PATH",
        "PYTHONPATH",
        "XDG_DATA_DIRS",
        "PERLLIB",
        "GSETTINGS_SCHEMA_DIR",
        "QT_PLUGIN_PATH",
        // it looks incorrectly handled in AppRun.c
        // "GST_PLUGIN_SYSTEM_PATH"
        // "GST_PLUGIN_SYSTEM_PATH_1_0"
    ] {
        if command.get_envs().any(|(name, _)| name == var_name) {
            continue; // do not change manually specified variables
        }
        // in this case, the variable will be inherited from the environment

        let Some(current) = std::env::var_os(var_name) else {
            continue; // the variable is not set; nothing to do
        };

        let current_bytes = current.as_bytes();

        if current_bytes.starts_with(appdir_bytes) {
            // in this case, the variable was modified to start with appdir
            // therefore, we need to remove the appdir-related path

            let mut current_bytes = current_bytes;
            while current_bytes.starts_with(appdir_bytes) {
                if let Some(colon_pos) = current_bytes.iter().position(|&x| x == b':') {
                    current_bytes = &current_bytes[colon_pos + 1..];
                } else {
                    current_bytes = b"";
                }
            }

            if current_bytes != b"" {
                // We successfully removed paths relates to the AppDir so set that value
                command.env(var_name, OsStr::from_bytes(current_bytes));
            } else {
                // In this case, the env variable was not defined and defined only by AppRun so
                // remove the variable
                command.env_remove(var_name);
            }
        }
    }

    // remove other related variables
    command.env_remove("ARGV0");
    command.env_remove("APPIMAGE");
    command.env_remove("APPDIR");

    // it was set by AppRun
    command.env_remove("PYTHONHOME");
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn open_url(url: &str) -> Result<(), Error> {
    open_that(url).unwrap();
    Ok(())
}

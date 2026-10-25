//! The `but` CLI on macOS, the one platform the app installs it on: where a terminal finds it,
//! and linking this app's where one does.

use std::path::{Path, PathBuf};

use anyhow::Result;
use but_action::cli::ExistingSymlinkPolicy;
use but_api::legacy::cli;

/// The `but` a terminal runs: the first on the `PATH`, which the app takes from the user's
/// login shell.
pub fn installed() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("but"))
        .find(|program| program.is_file())
}

/// Link this app's `but` at `/usr/local/bin/but`, replacing a link there, as the desktop app
/// does. It asks for an administrator's password when the folder needs one; `false` when the
/// user cancelled that.
pub fn install() -> Result<bool> {
    let path = cli::cli_path()?;
    // Run from its disk image, or from where macOS moves a downloaded app to run it, the app
    // isn't there for long, and the link would break once it's gone. Lite refuses too.
    let home_apps = std::env::var_os("HOME").map(|home| Path::new(&home).join("Applications"));
    let in_applications = Path::new(&path).starts_with("/Applications")
        || home_apps.is_some_and(|apps| Path::new(&path).starts_with(apps));
    anyhow::ensure!(
        in_applications,
        "Move GitButler to Applications before installing the CLI"
    );
    cli::install_cli_v2(path, ExistingSymlinkPolicy::Replace)
}

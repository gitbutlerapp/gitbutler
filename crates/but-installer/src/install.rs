//! Platform-agnostic installation logic

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Returns the path to the `but` CLI binary for the given home directory.
pub(crate) fn but_binary_path(home_dir: &Path) -> PathBuf {
    home_dir.join(".local/bin/but")
}

/// Validate that but can be executed
pub(crate) fn validate_installed_binary(path: &Path) -> bool {
    Command::new(path)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

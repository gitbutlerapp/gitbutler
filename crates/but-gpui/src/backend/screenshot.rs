//! A picture of the window, saved as macOS saves screenshots, for a developer to show how a
//! tile they were dragging landed.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context as _, Result, bail};

/// Save the window AppKit numbers `number` where macOS saves screenshots, named as it names
/// them, and give back where and the PNG.
pub fn window(number: isize) -> Result<(PathBuf, Vec<u8>)> {
    let stamp = chrono::Local::now().format("%Y-%m-%d at %H.%M.%S");
    let path = folder()?.join(format!("GitButler {stamp}.png"));
    // Without its shadow or the camera's sound.
    let status = Command::new("/usr/sbin/screencapture")
        .args(["-x", "-o", "-l", &number.to_string()])
        .arg(&path)
        .status()
        .context("Couldn't run screencapture")?;
    if !status.success() {
        bail!("screencapture failed: {status}");
    }
    let png = std::fs::read(&path).with_context(|| format!("Couldn't read {}", path.display()))?;
    Ok((path, png))
}

/// Where macOS saves screenshots: where the Screenshot app was told to, else the desktop.
fn folder() -> Result<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME").context("No home folder")?);
    let chosen = Command::new("/usr/bin/defaults")
        .args(["read", "com.apple.screencapture", "location"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|location| match location.trim().strip_prefix("~/") {
            Some(under) => home.join(under),
            None => PathBuf::from(location.trim()),
        })
        .filter(|location| location.is_dir());
    Ok(chosen.unwrap_or_else(|| home.join("Desktop")))
}

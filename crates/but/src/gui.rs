use anyhow::{Context, Result, bail};
use std::process::Stdio;

/// Open the GitButler GUI application for `possibly_project_dir`.
///
/// This function attempts to launch the GitButler desktop application, and only works
/// if this binary is either co-located with `gitbutler-tauri` or *is* `gitbutler-tauri`
/// itself.
///
/// Then we open the GUI application and provide the path to figure out how to open it.
pub fn open(possibly_project_dir: &std::path::Path) -> Result<()> {
    let exe_path = std::env::current_exe()?.canonicalize()?;
    let stem = exe_path
        .file_stem()
        .context("Need file-stem of current executable")?;
    let gui_exe_path = if stem != "gitbutler-tauri" {
        exe_path.with_file_name(if cfg!(windows) {
            "gitbutler-tauri.exe"
        } else {
            "gitbutler-tauri"
        })
    } else {
        exe_path
    };

    if !gui_exe_path.is_file() {
        bail!(
            "Couldn't find GUI executable at '{}'",
            gui_exe_path.display()
        );
    }

    // We launch like this to inherit the environment, and the Env var is used to be explicit about the
    // project override - otherwise the UI should open with the recently opened project.
    std::process::Command::new(gui_exe_path)
        .env("GITBUTLER_PROJECT_DIR", possibly_project_dir)
        .stderr(Stdio::inherit())
        .stdout(Stdio::null())
        .stdin(Stdio::null())
        .spawn()?;
    Ok(())
}

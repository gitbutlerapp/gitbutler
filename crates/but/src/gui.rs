use anyhow::{Context, Result};
use std::path::Path;

/// Open the GitButler GUI application for the given project directory.
///
/// This function attempts to launch the GitButler desktop application
/// and pass the project path to it. The exact mechanism depends on the
/// platform and whether the application is installed.
pub fn open_gui(project_dir: &Path) -> Result<()> {
    let absolute_path = std::fs::canonicalize(project_dir)
        .with_context(|| format!("Failed to resolve path: {}", project_dir.display()))?;

    // Try to open the project directory using the system's default handler.
    // This will work if GitButler is properly registered as a handler for the
    // gitbutler:// URL scheme or if we can construct a proper URL.

    // For now, we'll attempt to launch the GitButler application directly.
    // The application should be able to detect and open the project.

    #[cfg(target_os = "macos")]
    {
        open_gui_macos(&absolute_path)
    }

    #[cfg(target_os = "linux")]
    {
        open_gui_linux(&absolute_path)
    }

    #[cfg(target_os = "windows")]
    {
        open_gui_windows(&absolute_path)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        anyhow::bail!(
            "Opening the GUI is not supported on this platform.\n\
            Please manually open GitButler and navigate to: {}",
            absolute_path.display()
        )
    }
}

#[cfg(target_os = "macos")]
fn open_gui_macos(project_dir: &Path) -> Result<()> {
    // On macOS, the app is typically installed as GitButler.app
    // We can use the `open` command to launch it

    // First, try to find the application
    let standard_paths = [
        "/Applications/GitButler.app",
        "/Applications/GitButler Dev.app",
    ];

    // Build user-specific paths if HOME is set
    let user_paths: Vec<String> = std::env::var("HOME")
        .ok()
        .map(|home| {
            vec![
                format!("{}/Applications/GitButler.app", home),
                format!("{}/Applications/GitButler Dev.app", home),
            ]
        })
        .unwrap_or_default();

    // Combine all paths
    let all_paths: Vec<&str> = standard_paths
        .iter()
        .copied()
        .chain(user_paths.iter().map(|s| s.as_str()))
        .collect();

    for app_path in all_paths {
        if Path::new(app_path).exists() {
            // Use the `open` command to open a new instance if needed
            // Note: GitButler doesn't currently accept project path as a command line arg
            let status = std::process::Command::new("open")
                .arg("-a")
                .arg(app_path)
                .status()
                .context("Failed to launch GitButler")?;

            if status.success() {
                println!("GitButler GUI launched. Please open the project from the application.");
                println!("Project location: {}", project_dir.display());
                return Ok(());
            }
        }
    }

    anyhow::bail!(
        "GitButler application not found in /Applications.\n\
        Please install GitButler from https://gitbutler.com\n\
        Project location: {}",
        project_dir.display()
    )
}

#[cfg(target_os = "linux")]
fn open_gui_linux(project_dir: &Path) -> Result<()> {
    // On Linux, try to find the GitButler executable
    // It could be installed via AppImage, deb, or flatpak

    // Try various possible executable names
    let possible_executables = ["gitbutler", "git-butler", "GitButler"];

    for exe in &possible_executables {
        if let Ok(path) = which::which(exe) {
            // Launch the executable
            // GitButler doesn't currently accept project path as command line arg,
            // so we just launch it and the user will need to select the project
            let status = std::process::Command::new(path)
                .status()
                .context("Failed to launch GitButler")?;

            if status.success() {
                println!("GitButler GUI launched. Please open the project from the application.");
                println!("Project location: {}", project_dir.display());
                return Ok(());
            }
        }
    }

    // If we can't find it in PATH, provide helpful error message
    anyhow::bail!(
        "GitButler executable not found in PATH.\n\
        Please install GitButler from https://gitbutler.com\n\
        Project location: {}",
        project_dir.display()
    )
}

#[cfg(target_os = "windows")]
fn open_gui_windows(project_dir: &Path) -> Result<()> {
    // On Windows, the app is typically installed in Program Files
    let standard_paths = [
        "C:\\Program Files\\GitButler\\GitButler.exe",
        "C:\\Program Files (x86)\\GitButler\\GitButler.exe",
    ];

    // Build user-specific path if USERPROFILE is set
    let user_paths: Vec<String> = std::env::var("USERPROFILE")
        .ok()
        .map(|user_profile| {
            vec![format!(
                "{}\\AppData\\Local\\Programs\\GitButler\\GitButler.exe",
                user_profile
            )]
        })
        .unwrap_or_default();

    // Combine all paths
    let all_paths: Vec<&str> = standard_paths
        .iter()
        .copied()
        .chain(user_paths.iter().map(|s| s.as_str()))
        .collect();

    for app_path in all_paths {
        if Path::new(app_path).exists() {
            // Launch GitButler
            // Note: GitButler doesn't currently accept project path as a command line arg
            let status = std::process::Command::new(app_path)
                .status()
                .context("Failed to launch GitButler")?;

            if status.success() {
                println!("GitButler GUI launched. Please open the project from the application.");
                println!("Project location: {}", project_dir.display());
                return Ok(());
            }
        }
    }

    anyhow::bail!(
        "GitButler application not found in Program Files.\n\
        Please install GitButler from https://gitbutler.com\n\
        Project location: {}",
        project_dir.display()
    )
}

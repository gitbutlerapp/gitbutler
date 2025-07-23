use std::path::PathBuf;

use anyhow::{bail, Context};
use gitbutler_project::Project;

pub fn project_from_path(path: PathBuf) -> anyhow::Result<Project> {
    Project::from_path(&path)
}

pub fn project_controller(
    app_suffix: Option<String>,
    app_data_dir: Option<PathBuf>,
) -> anyhow::Result<gitbutler_project::Controller> {
    let path = if let Some(dir) = app_data_dir {
        std::fs::create_dir_all(&dir).context("Failed to assure the designated data-dir exists")?;
        dir
    } else {
        dirs_next::data_dir()
            .map(|dir| {
                dir.join(format!(
                    "com.gitbutler.app{}",
                    app_suffix
                        .map(|mut suffix| {
                            suffix.insert(0, '.');
                            suffix
                        })
                        .unwrap_or_default()
                ))
            })
            .context("no data-directory available on this platform")?
    };
    if !path.is_dir() {
        bail!("Path '{}' must be a valid directory", path.display());
    }
    eprintln!("Using projects from '{}'", path.display());
    Ok(gitbutler_project::Controller::from_path(path))
}

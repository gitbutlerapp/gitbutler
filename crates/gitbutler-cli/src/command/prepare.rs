use std::path::PathBuf;

use gitbutler_project::Project;

pub fn project_from_path(path: PathBuf) -> anyhow::Result<Project> {
    Project::from_path(&path)
}

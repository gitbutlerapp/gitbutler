pub mod access;
mod controller;
mod default_true;
mod project;
mod storage;

use std::path::Path;

use controller::Controller;
pub use project::{
    AddProjectOutcome, ApiProject, AuthKey, CodePushState, FetchResult, Project, ProjectId,
};
pub use storage::UpdateRequest;

/// A utility to be used from applications to optimize `git2` configuration.
/// See comments for details.
pub fn configure_git2() {
    // Do not re-hash each decoded objects for quite a significant performance gain.
    // This delegates object validation to `git fsck`, which seems fair.
    git2::opts::strict_hash_verification(false);
    // Thus far, no broken object was created, and if that would be the case, tests should catch it.
    // These settings are only changed from `main` of applications.
    git2::opts::strict_object_creation(false);
}

/// The maximum size of files to automatically start tracking, i.e. untracked files we pick up for tree-creation.
/// **Inactive for now** while it's hard to tell if it's safe *not* to pick up everything.
pub const AUTO_TRACK_LIMIT_BYTES: u64 = 0;

pub fn get(id: ProjectId) -> anyhow::Result<Project> {
    let controller = Controller::from_path(but_path::app_data_dir()?);
    controller.get(id)
}

/// Testing purpose only.
pub fn get_with_path<P: AsRef<Path>>(data_dir: P, id: ProjectId) -> anyhow::Result<Project> {
    let controller = Controller::from_path(data_dir.as_ref());
    controller.get(id)
}

pub fn get_validated(id: ProjectId) -> anyhow::Result<Project> {
    let controller = Controller::from_path(but_path::app_data_dir()?);
    controller.get_validated(id)
}

pub fn get_raw(id: ProjectId) -> anyhow::Result<Project> {
    let controller = Controller::from_path(but_path::app_data_dir()?);
    controller.get_raw(id)
}

pub fn update(project: &UpdateRequest) -> anyhow::Result<Project> {
    let controller = Controller::from_path(but_path::app_data_dir()?);
    controller.update(project)
}

/// Testing purpose only.
pub fn update_with_path<P: AsRef<Path>>(
    data_dir: P,
    project: &UpdateRequest,
) -> anyhow::Result<Project> {
    let controller = Controller::from_path(data_dir.as_ref());
    controller.update(project)
}

pub fn add<P: AsRef<Path>>(path: P) -> anyhow::Result<AddProjectOutcome> {
    let controller = Controller::from_path(but_path::app_data_dir()?);
    controller.add(path)
}

/// Testing purpose only.
pub fn add_with_path(
    data_dir: impl AsRef<Path>,
    path: impl AsRef<Path>,
) -> anyhow::Result<AddProjectOutcome> {
    let controller = Controller::from_path(data_dir.as_ref());
    controller.add(path)
}

pub fn list() -> anyhow::Result<Vec<Project>> {
    let controller = Controller::from_path(but_path::app_data_dir()?);
    controller.list()
}

pub fn delete(id: ProjectId) -> anyhow::Result<()> {
    let controller = Controller::from_path(but_path::app_data_dir()?);
    controller.delete(id)
}

/// Testing purpose only.
pub fn delete_with_path<P: AsRef<Path>>(data_dir: P, id: ProjectId) -> anyhow::Result<()> {
    let controller = Controller::from_path(data_dir.as_ref());
    controller.delete(id)
}

pub fn assure_app_can_startup_or_fix_it(
    projects: anyhow::Result<Vec<Project>>,
) -> anyhow::Result<Vec<Project>> {
    let controller = Controller::from_path(but_path::app_data_dir()?);
    controller.assure_app_can_startup_or_fix_it(projects)
}

use std::{fs, path, path::PathBuf, str::FromStr};

use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::GITBUTLER_WORKSPACE_COMMIT_TITLE;
use gitbutler_error::error::Marker;
use gitbutler_project::{self as projects, Project, ProjectId};
use gitbutler_reference::Refname;
use gitbutler_testsupport::{paths, TestProject, VAR_NO_CLEANUP};
use tempfile::TempDir;

struct Test {
    repository: TestProject,
    project_id: ProjectId,
    project: Project,
    projects: projects::Controller,
    data_dir: Option<TempDir>,
}

impl Drop for Test {
    fn drop(&mut self) {
        if std::env::var_os(VAR_NO_CLEANUP).is_some() {
            let _ = self.data_dir.take().unwrap().into_path();
        }
    }
}

impl Default for Test {
    fn default() -> Self {
        let data_dir = paths::data_dir();
        let projects = projects::Controller::from_path(data_dir.path());

        let test_project = TestProject::default();
        let project = projects
            .add(test_project.path())
            .expect("failed to add project");

        Self {
            repository: test_project,
            project_id: project.id,
            projects,
            project,
            data_dir: Some(data_dir),
        }
    }
}

impl Test {
    /// Consume this instance and keep the temp directory that held the local repository, returning it.
    /// Best used inside a `dbg!(test.debug_local_repo())`
    #[allow(dead_code)]
    pub fn debug_local_repo(&mut self) -> Option<PathBuf> {
        self.repository.debug_local_repo()
    }
}

mod amend;
mod apply_virtual_branch;
mod create_commit;
mod create_virtual_branch_from_branch;
mod init;
mod insert_blank_commit;
mod list;
mod list_details;
mod locking;
mod move_commit_file;
mod move_commit_to_vbranch;
mod oplog;
mod references;
mod reset_virtual_branch;
mod save_and_unapply_virtual_branch;
mod selected_for_changes;
mod set_base_branch;
mod squash;
mod unapply_ownership;
mod unapply_without_saving_virtual_branch;
mod undo_commit;
mod update_commit_message;
mod upstream;
mod verify_branch;
mod workspace_migration;

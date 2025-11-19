use std::{fs, path, path::PathBuf, str::FromStr};

use but_ctx::Context;
use but_error::Marker;
use but_settings::AppSettings;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::GITBUTLER_WORKSPACE_COMMIT_TITLE;
use gitbutler_project::{self as projects, Project, ProjectId};
use gitbutler_reference::Refname;
use gitbutler_testsupport::{TestProject, VAR_NO_CLEANUP, paths};
use tempfile::TempDir;

struct Test {
    repo: TestProject,
    project_id: ProjectId,
    project: Project,
    data_dir: Option<TempDir>,
    ctx: Context,
}

impl Test {
    pub fn new_with_settings(change_settings: fn(&mut AppSettings)) -> Self {
        let data_dir = paths::data_dir();

        let test_project = TestProject::default();
        let outcome = gitbutler_project::add_with_path(data_dir.as_ref(), test_project.path())
            .expect("failed to add project");
        let project = outcome.unwrap_project();
        let mut settings = AppSettings::default();
        change_settings(&mut settings);
        let ctx = Context::new_from_legacy_project_and_settings(&project, settings);

        Self {
            repo: test_project,
            project_id: project.id,
            project,
            data_dir: Some(data_dir),
            ctx,
        }
    }
}

impl Drop for Test {
    fn drop(&mut self) {
        if std::env::var_os(VAR_NO_CLEANUP).is_some() {
            let _ = self.data_dir.take().unwrap().keep();
        }
    }
}

impl Default for Test {
    fn default() -> Self {
        Self::new_with_settings(|_settings| {})
    }
}

impl Test {
    /// Consume this instance and keep the temp directory that held the local repository, returning it.
    /// Best used inside a `dbg!(test.debug_local_repo())`
    #[expect(dead_code)]
    pub fn debug_local_repo(&mut self) -> Option<PathBuf> {
        self.repo.debug_local_repo()
    }
}

mod amend;
mod apply_virtual_branch;
mod create_virtual_branch_from_branch;
mod init;
mod insert_blank_commit;
mod list;
mod list_details;
mod move_commit_to_vbranch;
mod oplog;
mod save_and_unapply_virtual_branch;
mod set_base_branch;
mod unapply_without_saving_virtual_branch;
mod undo_commit;
mod update_commit_message;
mod workspace_migration;

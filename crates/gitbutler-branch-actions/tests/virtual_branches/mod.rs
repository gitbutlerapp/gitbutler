use std::{fs, path, path::PathBuf, str::FromStr};

use but_settings::AppSettings;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::GITBUTLER_WORKSPACE_COMMIT_TITLE;
use gitbutler_command_context::CommandContext;
use gitbutler_error::error::Marker;
use gitbutler_project::{self as projects, Project, ProjectId};
use gitbutler_reference::Refname;
use gitbutler_testsupport::{paths, TestProject, VAR_NO_CLEANUP};
use tempfile::TempDir;

struct Test {
    repo: TestProject,
    project_id: ProjectId,
    project: Project,
    data_dir: Option<TempDir>,
    ctx: CommandContext,
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
        let data_dir = paths::data_dir();

        let test_project = TestProject::default();
        let project =
            gitbutler_project::add_with_path(data_dir.as_ref(), test_project.path(), None, None)
                .expect("failed to add project");
        let ctx = CommandContext::open(&project, AppSettings::default()).unwrap();

        Self {
            repo: test_project,
            project_id: project.id,
            project,
            data_dir: Some(data_dir),
            ctx,
        }
    }
}

impl Test {
    /// Consume this instance and keep the temp directory that held the local repository, returning it.
    /// Best used inside a `dbg!(test.debug_local_repo())`
    #[allow(dead_code)]
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

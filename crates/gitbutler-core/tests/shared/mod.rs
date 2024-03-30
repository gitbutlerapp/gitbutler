pub const VAR_NO_CLEANUP: &str = "GITBUTLER_TESTS_NO_CLEANUP";

mod test_project;
pub use test_project::TestProject;

mod suite;
pub use suite::*;

pub mod paths {
    use tempfile::TempDir;

    use super::temp_dir;

    pub fn data_dir() -> TempDir {
        temp_dir()
    }
}

pub mod virtual_branches {
    use gitbutler_core::{
        gb_repository, project_repository,
        virtual_branches::{self, VirtualBranchesHandle},
    };

    use crate::shared::empty_bare_repository;

    pub fn set_test_target(
        gb_repo: &gb_repository::Repository,
        project_repository: &project_repository::Repository,
    ) -> anyhow::Result<()> {
        let (remote_repo, _tmp) = empty_bare_repository();
        let mut remote = project_repository
            .git_repository
            .remote(
                "origin",
                &remote_repo.path().to_str().unwrap().parse().unwrap(),
            )
            .expect("failed to add remote");
        remote.push(&["refs/heads/master:refs/heads/master"], None)?;

        virtual_branches::target::Writer::new(
            gb_repo,
            VirtualBranchesHandle::new(&project_repository.project().gb_dir()),
        )?
        .write_default(&virtual_branches::target::Target {
            branch: "refs/remotes/origin/master".parse().unwrap(),
            remote_url: remote_repo.path().to_str().unwrap().parse().unwrap(),
            sha: remote_repo.head().unwrap().target().unwrap(),
        })
        .expect("failed to write target");

        virtual_branches::integration::update_gitbutler_integration(gb_repo, project_repository)
            .expect("failed to update integration");

        Ok(())
    }
}

pub fn init_opts() -> git2::RepositoryInitOptions {
    let mut opts = git2::RepositoryInitOptions::new();
    opts.initial_head("master");
    opts
}

pub fn init_opts_bare() -> git2::RepositoryInitOptions {
    let mut opts = init_opts();
    opts.bare(true);
    opts
}

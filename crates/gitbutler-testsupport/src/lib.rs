#![forbid(rust_2018_idioms)]
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
    use gitbutler_core::{project_repository, virtual_branches};

    use crate::empty_bare_repository;

    pub fn set_test_target(
        project_repository: &project_repository::Repository,
    ) -> anyhow::Result<()> {
        let vb_state = project_repository.project().virtual_branches();
        let (remote_repo, _tmp) = empty_bare_repository();
        let mut remote = project_repository
            .repo()
            .remote("origin", remote_repo.path().to_str().unwrap())
            .expect("failed to add remote");
        remote.push(&["refs/heads/master:refs/heads/master"], None)?;

        vb_state
            .set_default_target(virtual_branches::target::Target {
                branch: "refs/remotes/origin/master".parse().unwrap(),
                remote_url: remote_repo.path().to_str().unwrap().parse().unwrap(),
                sha: remote_repo.head().unwrap().target().unwrap(),
                push_remote_name: None,
            })
            .expect("failed to write target");

        virtual_branches::integration::update_gitbutler_integration(&vb_state, project_repository)
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

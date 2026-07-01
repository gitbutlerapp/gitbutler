use but_testsupport::{
    gix_testtools::{Creation, scripted_fixture_writable_with_args},
    open_repo,
};
use tempfile::TempDir;

pub mod testing_repository;

pub fn repository(name: &str) -> (gix::Repository, TempDir) {
    let tmp = scripted_fixture_writable_with_args(
        format!("scenario/{name}.sh"),
        None::<String>,
        Creation::CopyFromReadOnly,
    )
    .map_err(anyhow::Error::from_boxed)
    .expect("fixture should materialize");
    let repo = open_repo(tmp.path()).expect("fixture should be a valid repository");
    (repo, tmp)
}

pub fn test_repository() -> (gix::Repository, TempDir) {
    repository("basic")
}

pub struct RepoWithOrigin {
    pub local_repo: gix::Repository,
    _fixture_dir: TempDir,
}

impl Default for RepoWithOrigin {
    fn default() -> Self {
        let tmp = scripted_fixture_writable_with_args(
            "scenario/repo-with-origin.sh",
            None::<String>,
            Creation::CopyFromReadOnly,
        )
        .map_err(anyhow::Error::from_boxed)
        .expect("fixture should materialize");
        let local_repo = open_repo(&tmp.path().join("local")).expect("local repo opens");

        Self {
            local_repo,
            _fixture_dir: tmp,
        }
    }
}

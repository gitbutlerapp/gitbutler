use std::path::{Path, PathBuf};

use but_core::RepositoryExt as _;
use but_testsupport::gix_testtools::{scripted_fixture_writable, tempfile::TempDir};

pub fn data_dir() -> TempDir {
    tempfile::tempdir().expect("tempdir can be created")
}

pub struct TestProject {
    _tmp: TempDir,
    local_path: PathBuf,
}

impl Default for TestProject {
    fn default() -> Self {
        let tmp = scripted_fixture_writable("repo-with-origin.sh").expect("fixture is valid");
        let local_path = tmp.path().join("local");
        let mut repo = but_testsupport::open_repo(&local_path).expect("fixture opens");
        let key = but_project_handle::storage_path_config_key();
        repo.config_snapshot_mut()
            .set_raw_value(key, "gitbutler")
            .expect("in-memory config is writable");
        let (_config, lock) = repo
            .local_common_config_for_editing()
            .expect("config lock can be acquired");
        repo.write_locked_config(&repo.config_snapshot(), lock)
            .expect("config can be persisted");
        let local_path = local_path
            .canonicalize()
            .expect("local fixture path is valid");
        Self {
            _tmp: tmp,
            local_path,
        }
    }
}

impl TestProject {
    pub fn path(&self) -> &Path {
        &self.local_path
    }
}

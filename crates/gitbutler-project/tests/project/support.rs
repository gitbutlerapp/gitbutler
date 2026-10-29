use std::path::{Path, PathBuf};

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
        let repo = but_testsupport::open_repo(&local_path).expect("valid git repo in fixture");
        but_core::git_config::edit_repo_config(&repo, gix::config::Source::Local, |config| {
            let key = but_project_handle::storage_path_config_key();
            config.set_raw_value(key, "gitbutler")?;
            Ok(())
        })
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

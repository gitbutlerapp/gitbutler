use std::{fs, path::Path};

use gix_testtools::bstr::ByteSlice as _;
use tempfile::{tempdir, TempDir};

pub struct TestingRepository {
    pub repository: git2::Repository,
    pub tempdir: TempDir,
}

impl TestingRepository {
    pub fn open() -> Self {
        let tempdir = tempdir().unwrap();
        let repository = git2::Repository::init(tempdir.path()).unwrap();

        let config = repository.config().unwrap();
        match config.open_level(git2::ConfigLevel::Local) {
            Ok(mut local) => {
                local.set_str("commit.gpgsign", "false").unwrap();
                local.set_str("user.name", "gitbutler-test").unwrap();
                local
                    .set_str("user.email", "gitbutler-test@example.com")
                    .unwrap();
            }
            Err(err) => panic!("{}", err),
        }

        Self {
            tempdir,
            repository,
        }
    }

    pub fn commit_tree<'a>(
        &'a self,
        parent: Option<&git2::Commit<'a>>,
        files: &[(&str, &str)],
    ) -> git2::Commit<'a> {
        // Remove everything other than the .git folder
        for entry in fs::read_dir(self.tempdir.path()).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name() != ".git" {
                let path = entry.path();
                if path.is_dir() {
                    fs::remove_dir_all(path).unwrap();
                } else {
                    fs::remove_file(path).unwrap();
                }
            }
        }
        // Write any files
        for (file_name, contents) in files {
            fs::write(self.tempdir.path().join(file_name), contents).unwrap();
        }

        // Update the index
        let mut index = self.repository.index().unwrap();
        // Make sure we're not having weird cached state
        index.read(true).unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();

        let signature = git2::Signature::now("Caleb", "caleb@gitbutler.com").unwrap();
        let commit = self
            .repository
            .commit(
                None,
                &signature,
                &signature,
                "Committee",
                &self
                    .repository
                    .find_tree(index.write_tree().unwrap())
                    .unwrap(),
                parent.map(|c| vec![c]).unwrap_or_default().as_slice(),
            )
            .unwrap();

        self.repository.find_commit(commit).unwrap()
    }
}

pub fn assert_tree_matches<'a>(
    repository: &'a git2::Repository,
    commit: &git2::Commit<'a>,
    files: &[(&str, &[u8])],
) {
    let tree = commit.tree().unwrap();

    for (path, content) in files {
        let blob = tree.get_path(Path::new(path)).unwrap().id();
        let blob: git2::Blob<'a> = repository.find_blob(blob).unwrap();
        assert_eq!(
            blob.content(),
            *content,
            "{}: expect {} == {}",
            path,
            blob.content().to_str_lossy(),
            content.to_str_lossy()
        );
    }
}

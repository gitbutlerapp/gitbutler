use std::fs;

use gitbutler_commit::commit_headers::CommitHeadersV2;
use gitbutler_oxidize::git2_to_gix_object_id;
use gitbutler_repo::RepositoryExt;
use gix_testtools::bstr::ByteSlice as _;
use tempfile::{TempDir, tempdir};
use uuid::Uuid;

use crate::init_opts;

pub struct TestingRepository {
    pub repository: git2::Repository,
    pub tempdir: TempDir,
}

impl TestingRepository {
    pub fn open() -> Self {
        let tempdir = tempdir().unwrap();
        let repository = git2::Repository::init_opts(tempdir.path(), &init_opts()).unwrap();
        // TODO(ST): remove this once `gix::Repository::index_or_load_from_tree_or_empty()`
        //           is available and used to get merge/diff resource caches. Also: name this
        //          `open_unborn()` to make it clear.
        // For now we need a resemblance of an initialized repo.
        let signature = git2::Signature::now("Caleb", "caleb@gitbutler.com").unwrap();
        let empty_tree_id = repository.treebuilder(None).unwrap().write().unwrap();
        repository
            .commit(
                Some("refs/heads/master"),
                &signature,
                &signature,
                "init to prevent load index failure",
                &repository.find_tree(empty_tree_id).unwrap(),
                &[],
            )
            .unwrap();

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

    pub fn gix_repository(&self) -> gix::Repository {
        gix::open(self.repository.path()).unwrap()
    }

    pub fn open_with_initial_commit(files: &[(&str, &str)]) -> Self {
        let tempdir = tempdir().unwrap();
        let repository = git2::Repository::init_opts(tempdir.path(), &init_opts()).unwrap();

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

        let repository = Self {
            tempdir,
            repository,
        };
        {
            let commit = repository.commit_tree(None, files);
            repository
                .repository
                .branch("master", &commit, true)
                .unwrap();
        }
        repository
    }

    pub fn commit_tree_with_change_id<'a>(
        &'a self,
        parent: Option<&git2::Commit<'_>>,
        change_id: &str,
        files: &[(&str, &str)],
    ) -> git2::Commit<'a> {
        self.commit_tree_inner(parent, &Uuid::new_v4().to_string(), files, Some(change_id))
    }

    pub fn commit_tree<'a>(
        &'a self,
        parent: Option<&git2::Commit<'_>>,
        files: &[(&str, &str)],
    ) -> git2::Commit<'a> {
        self.commit_tree_inner(parent, &Uuid::new_v4().to_string(), files, None)
    }

    pub fn commit_tree_with_message<'a>(
        &'a self,
        parent: Option<&git2::Commit<'_>>,
        message: &str,
        files: &[(&str, &str)],
    ) -> git2::Commit<'a> {
        self.commit_tree_inner(parent, message, files, None)
    }

    pub fn commit_tree_inner<'a>(
        &'a self,
        parent: Option<&git2::Commit<'_>>,
        message: &str,
        files: &[(&str, &str)],
        change_id: Option<&str>,
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
            let path = self.tempdir.path().join(file_name);
            let parent = path.parent().unwrap();

            if !parent.exists() {
                fs::create_dir_all(parent).unwrap();
            }

            fs::write(path, contents).unwrap();
        }

        // Update the index
        let mut index = self.repository.index().unwrap();
        // Make sure we're not having weird cached state
        index.read(true).unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();

        let signature = git2::Signature::now("Caleb", "caleb@gitbutler.com").unwrap();
        let commit_headers =
            change_id.map_or(CommitHeadersV2::new(), |change_id| CommitHeadersV2 {
                change_id: change_id.to_string(),
                conflicted: None,
            });

        let commit = self
            .repository
            .commit_with_signature(
                None,
                &signature,
                &signature,
                message,
                &self
                    .repository
                    .find_tree(index.write_tree().unwrap())
                    .unwrap(),
                parent.map(|c| vec![c]).unwrap_or_default().as_slice(),
                Some(commit_headers),
            )
            .unwrap();

        self.repository.find_commit(commit).unwrap()
    }

    pub fn persist(self) {
        let path = self.tempdir.keep();

        println!("Persisting test repository at {path:?}");
    }
}

pub fn assert_commit_tree_matches<'a>(
    repository: &'a git2::Repository,
    commit: &git2::Commit<'a>,
    files: &[(&str, &[u8])],
) {
    assert_tree_matches(repository, &commit.tree().unwrap(), files);
}

pub fn assert_tree_matches<'a>(
    repository: &'a git2::Repository,
    tree: &git2::Tree<'a>,
    files: &[(&str, &[u8])],
) {
    assert_tree_matches_with_mode(
        repository,
        tree.id(),
        &files
            .iter()
            .map(|(path, content)| (*path, *content, &[] as &[EntryAttribute]))
            .collect::<Vec<_>>(),
    );
}

#[derive(Debug, Copy, Clone)]
pub enum EntryAttribute {
    Tree,
    Commit,
    Link,
    Blob,
    Executable,
}

pub fn assert_tree_matches_with_mode(
    repository: &git2::Repository,
    tree: git2::Oid,
    files: &[(&str, &[u8], &[EntryAttribute])],
) {
    let gix_repository = gix::open(repository.path()).unwrap();

    let tree = gix_repository
        .find_tree(git2_to_gix_object_id(tree))
        .unwrap();

    for (path, content, entry_attributes) in files {
        let tree_entry = tree.lookup_entry_by_path(path).unwrap().unwrap();
        let object = gix_repository.find_object(tree_entry.id()).unwrap();
        assert_eq!(
            object.data,
            *content,
            "{}: expect {} == {}",
            path,
            object.data.to_str_lossy(),
            content.to_str_lossy()
        );

        for entry_attribute in *entry_attributes {
            match entry_attribute {
                EntryAttribute::Tree => assert!(tree_entry.mode().is_tree(), "{path} is a tree"),
                EntryAttribute::Commit => {
                    assert!(tree_entry.mode().is_commit(), "{path} is a commit")
                }
                EntryAttribute::Link => assert!(tree_entry.mode().is_link(), "{path} is a link"),
                EntryAttribute::Blob => assert!(tree_entry.mode().is_blob(), "{path} is a blob"),
                EntryAttribute::Executable => {
                    assert!(tree_entry.mode().is_executable(), "{path} is executable")
                }
            }
        }
    }
}

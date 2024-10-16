use std::fs;

use crate::init_opts;
use gitbutler_oxidize::git2_to_gix_object_id;
use gix_testtools::bstr::ByteSlice as _;
use tempfile::{tempdir, TempDir};
use uuid::Uuid;

pub struct TestingRepository {
    pub repository: git2::Repository,
    pub tempdir: TempDir,
}

impl TestingRepository {
    pub fn open() -> Self {
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
        self.commit_tree_with_message(parent, &Uuid::new_v4().to_string(), files)
    }

    pub fn commit_tree_with_message<'a>(
        &'a self,
        parent: Option<&git2::Commit<'a>>,
        message: &str,
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

        let signature = git2::Signature::now("Caleb", "caleb@gitbutler.com").unwrap();
        let commit = self
            .repository
            .commit(
                None,
                &signature,
                &signature,
                message,
                &self
                    .repository
                    .find_tree(index.write_tree().unwrap())
                    .unwrap(),
                parent.map(|c| vec![c]).unwrap_or_default().as_slice(),
            )
            .unwrap();

        self.repository.find_commit(commit).unwrap()
    }

    pub fn persist(self) {
        let path = self.tempdir.into_path();

        println!("Persisting test repository at {:?}", path);
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
                EntryAttribute::Tree => assert!(tree_entry.mode().is_tree(), "{} is a tree", path),
                EntryAttribute::Commit => {
                    assert!(tree_entry.mode().is_commit(), "{} is a commit", path)
                }
                EntryAttribute::Link => assert!(tree_entry.mode().is_link(), "{} is a link", path),
                EntryAttribute::Blob => assert!(tree_entry.mode().is_blob(), "{} is a blob", path),
                EntryAttribute::Executable => {
                    assert!(tree_entry.mode().is_executable(), "{} is executable", path)
                }
            }
        }
    }
}

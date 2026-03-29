use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

use but_oxidize::OidExt as _;
use gix::bstr::ByteSlice as _;
use tempfile::{TempDir, tempdir};

pub struct TestingRepository {
    pub repository: git2::Repository,
    pub tempdir: TempDir,
}

static NEXT_COMMIT_ID: AtomicU64 = AtomicU64::new(1);

impl TestingRepository {
    pub fn open() -> Self {
        let tempdir = tempdir().expect("tempdir exists");
        let repo = git2::Repository::init_opts(tempdir.path(), &super::init_opts())
            .expect("repository initializes");
        super::setup_config(&repo.config().expect("config exists")).expect("config is writable");

        {
            let empty_tree_id = repo
                .treebuilder(None)
                .expect("treebuilder exists")
                .write()
                .expect("tree writes");
            let tree = repo.find_tree(empty_tree_id).expect("tree exists");
            let sig = super::signature();
            repo.commit(
                Some("refs/heads/master"),
                &sig,
                &sig,
                "init to prevent load index failure",
                &tree,
                &[],
            )
            .expect("initial commit succeeds");
        }

        Self {
            tempdir,
            repository: repo,
        }
    }

    pub fn gix_repository(&self) -> gix::Repository {
        gix::open(self.repository.path()).expect("gix repo opens")
    }

    pub fn open_with_initial_commit(files: &[(&str, &str)]) -> Self {
        let tempdir = tempdir().expect("tempdir exists");
        let repository = git2::Repository::init_opts(tempdir.path(), &super::init_opts())
            .expect("repository initializes");
        super::setup_config(&repository.config().expect("config exists"))
            .expect("config is writable");

        let repo = Self {
            repository,
            tempdir,
        };
        {
            let commit = repo.commit_tree(None, files);
            repo.repository
                .branch("master", &commit, true)
                .expect("branch can be written");
        }
        repo
    }

    pub fn commit_tree<'a>(
        &'a self,
        parent: Option<&git2::Commit<'_>>,
        files: &[(&str, &str)],
    ) -> git2::Commit<'a> {
        let message = format!(
            "test commit {}",
            NEXT_COMMIT_ID.fetch_add(1, Ordering::Relaxed)
        );
        self.commit_tree_inner(parent, &message, files)
    }

    fn commit_tree_inner<'a>(
        &'a self,
        parent: Option<&git2::Commit<'_>>,
        message: &str,
        files: &[(&str, &str)],
    ) -> git2::Commit<'a> {
        for entry in fs::read_dir(self.tempdir.path()).expect("dir can be read") {
            let entry = entry.expect("entry exists");
            if entry.file_name() != ".git" {
                let path = entry.path();
                if path.is_dir() {
                    fs::remove_dir_all(path).expect("directory can be removed");
                } else {
                    fs::remove_file(path).expect("file can be removed");
                }
            }
        }

        for (file_name, contents) in files {
            let path = self.tempdir.path().join(file_name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent directories can be created");
            }
            fs::write(path, contents).expect("file contents can be written");
        }

        let mut index = self.repository.index().expect("index exists");
        index.read(true).expect("index can be read");
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .expect("files can be added");
        index.write().expect("index can be written");

        let sig = super::signature();
        let tree_id = index.write_tree().expect("tree can be written");
        let tree = self.repository.find_tree(tree_id).expect("tree exists");
        let oid = self
            .repository
            .commit(
                None,
                &sig,
                &sig,
                message,
                &tree,
                parent.map(|c| vec![c]).unwrap_or_default().as_slice(),
            )
            .expect("commit succeeds");

        self.repository.find_commit(oid).expect("commit exists")
    }
}

pub fn assert_commit_tree_matches<'a>(
    repository: &'a git2::Repository,
    commit: &git2::Commit<'a>,
    files: &[(&str, &[u8])],
) {
    assert_tree_matches(repository, &commit.tree().expect("tree exists"), files);
}

pub fn assert_tree_matches<'a>(
    repository: &'a git2::Repository,
    tree: &git2::Tree<'a>,
    files: &[(&str, &[u8])],
) {
    let gix_repository = gix::open(repository.path()).expect("gix repo opens");
    let tree = gix_repository
        .find_tree(tree.id().to_gix())
        .expect("tree exists");

    for (path, content) in files {
        let tree_entry = tree
            .lookup_entry_by_path(path)
            .expect("lookup succeeds")
            .unwrap_or_else(|| panic!("expected {path} in tree"));
        let object = gix_repository
            .find_object(tree_entry.id())
            .expect("object exists");
        assert_eq!(
            object.data,
            *content,
            "{}: expect {} == {}",
            path,
            object.data.to_str_lossy(),
            content.to_str_lossy()
        );
    }
}

const VAR_NO_CLEANUP: &str = "GITBUTLER_TESTS_NO_CLEANUP";

pub(crate) mod common;
mod suite {
    mod gb_repository;
    mod projects;
    mod virtual_branches;
}

mod database;
mod deltas;
mod gb_repository;
mod git;
mod keys;
mod lock;
mod reader;
mod sessions;
mod types;
pub mod virtual_branches;
mod watcher;
mod zip;

use std::path::{Path, PathBuf};
use std::{collections::HashMap, fs};

use tempfile::{tempdir, TempDir};

pub struct Suite {
    pub local_app_data: Option<TempDir>,
    pub storage: gitbutler_app::storage::Storage,
    pub users: gitbutler_app::users::Controller,
    pub projects: gitbutler_app::projects::Controller,
    pub keys: gitbutler_app::keys::Controller,
}

impl Drop for Suite {
    fn drop(&mut self) {
        if std::env::var_os(VAR_NO_CLEANUP).is_some() {
            let _ = self.local_app_data.take().unwrap().into_path();
        }
    }
}

impl Default for Suite {
    fn default() -> Self {
        let local_app_data = temp_dir();
        let storage = gitbutler_app::storage::Storage::new(&local_app_data);
        let users = gitbutler_app::users::Controller::from_path(&local_app_data);
        let projects = gitbutler_app::projects::Controller::from_path(&local_app_data);
        let keys = gitbutler_app::keys::Controller::from_path(&local_app_data);
        Self {
            storage,
            local_app_data: Some(local_app_data),
            users,
            projects,
            keys,
        }
    }
}

impl Suite {
    pub fn local_app_data(&self) -> &Path {
        self.local_app_data.as_ref().unwrap().path()
    }
    pub fn sign_in(&self) -> gitbutler_app::users::User {
        let user = gitbutler_app::users::User {
            name: Some("test".to_string()),
            email: "test@email.com".to_string(),
            access_token: "token".to_string(),
            ..Default::default()
        };
        self.users.set_user(&user).expect("failed to add user");
        user
    }

    fn project(&self, fs: HashMap<PathBuf, &str>) -> (gitbutler_app::projects::Project, TempDir) {
        let (repository, tmp) = test_repository();
        for (path, contents) in fs {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(repository.path().parent().unwrap().join(parent))
                    .expect("failed to create dir");
            }
            fs::write(
                repository.path().parent().unwrap().join(&path),
                contents.as_bytes(),
            )
            .expect("failed to write file");
        }
        commit_all(&repository);

        (
            self.projects
                .add(repository.path().parent().unwrap())
                .expect("failed to add project"),
            tmp,
        )
    }

    pub fn new_case_with_files(&self, fs: HashMap<PathBuf, &str>) -> Case {
        let (project, project_tmp) = self.project(fs);
        Case::new(self, project, project_tmp)
    }

    pub fn new_case(&self) -> Case {
        self.new_case_with_files(HashMap::new())
    }
}

pub struct Case<'a> {
    suite: &'a Suite,
    pub project: gitbutler_app::projects::Project,
    pub project_repository: gitbutler_app::project_repository::Repository,
    pub gb_repository: gitbutler_app::gb_repository::Repository,
    pub credentials: gitbutler_app::git::credentials::Helper,
    /// The directory containing the `project_repository`
    project_tmp: Option<TempDir>,
}

impl Drop for Case<'_> {
    fn drop(&mut self) {
        if let Some(tmp) = self
            .project_tmp
            .take()
            .filter(|_| std::env::var_os(VAR_NO_CLEANUP).is_some())
        {
            let _ = tmp.into_path();
        }
    }
}

impl<'a> Case<'a> {
    fn new(
        suite: &'a Suite,
        project: gitbutler_app::projects::Project,
        project_tmp: TempDir,
    ) -> Case<'a> {
        let project_repository = gitbutler_app::project_repository::Repository::open(&project)
            .expect("failed to create project repository");
        let gb_repository = gitbutler_app::gb_repository::Repository::open(
            suite.local_app_data(),
            &project_repository,
            None,
        )
        .expect("failed to open gb repository");
        let credentials =
            gitbutler_app::git::credentials::Helper::from_path(suite.local_app_data());
        Case {
            suite,
            project,
            gb_repository,
            project_repository,
            project_tmp: Some(project_tmp),
            credentials,
        }
    }

    pub fn refresh(mut self) -> Self {
        let project = self
            .suite
            .projects
            .get(&self.project.id)
            .expect("failed to get project");
        let project_repository = gitbutler_app::project_repository::Repository::open(&project)
            .expect("failed to create project repository");
        let user = self.suite.users.get_user().expect("failed to get user");
        let credentials =
            gitbutler_app::git::credentials::Helper::from_path(self.suite.local_app_data());
        Self {
            suite: self.suite,
            gb_repository: gitbutler_app::gb_repository::Repository::open(
                self.suite.local_app_data(),
                &project_repository,
                user.as_ref(),
            )
            .expect("failed to open gb repository"),
            credentials,
            project_repository,
            project,
            project_tmp: self.project_tmp.take(),
        }
    }
}

pub fn test_database() -> (gitbutler_app::database::Database, TempDir) {
    let tmp = temp_dir();
    let db = gitbutler_app::database::Database::open_in_directory(&tmp).unwrap();
    (db, tmp)
}

pub fn temp_dir() -> TempDir {
    tempdir().unwrap()
}

pub fn empty_bare_repository() -> (gitbutler_app::git::Repository, TempDir) {
    let tmp = temp_dir();
    (
        gitbutler_app::git::Repository::init_opts(&tmp, &init_opts_bare())
            .expect("failed to init repository"),
        tmp,
    )
}

pub fn test_repository() -> (gitbutler_app::git::Repository, TempDir) {
    let tmp = temp_dir();
    let repository = gitbutler_app::git::Repository::init_opts(&tmp, &init_opts())
        .expect("failed to init repository");
    let mut index = repository.index().expect("failed to get index");
    let oid = index.write_tree().expect("failed to write tree");
    let signature = gitbutler_app::git::Signature::now("test", "test@email.com").unwrap();
    repository
        .commit(
            Some(&"refs/heads/master".parse().unwrap()),
            &signature,
            &signature,
            "Initial commit",
            &repository.find_tree(oid).expect("failed to find tree"),
            &[],
        )
        .expect("failed to commit");
    (repository, tmp)
}

pub fn commit_all(repository: &gitbutler_app::git::Repository) -> gitbutler_app::git::Oid {
    let mut index = repository.index().expect("failed to get index");
    index
        .add_all(["."], git2::IndexAddOption::DEFAULT, None)
        .expect("failed to add all");
    index.write().expect("failed to write index");
    let oid = index.write_tree().expect("failed to write tree");
    let signature = gitbutler_app::git::Signature::now("test", "test@email.com").unwrap();
    let head = repository.head().expect("failed to get head");
    let commit_oid = repository
        .commit(
            Some(&head.name().unwrap()),
            &signature,
            &signature,
            "some commit",
            &repository.find_tree(oid).expect("failed to find tree"),
            &[&repository
                .find_commit(
                    repository
                        .refname_to_id("HEAD")
                        .expect("failed to get head"),
                )
                .expect("failed to find commit")],
        )
        .expect("failed to commit");
    commit_oid
}

fn init_opts() -> git2::RepositoryInitOptions {
    let mut opts = git2::RepositoryInitOptions::new();
    opts.initial_head("master");
    opts
}

pub fn init_opts_bare() -> git2::RepositoryInitOptions {
    let mut opts = init_opts();
    opts.bare(true);
    opts
}

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use gitbutler_core::types::Sensitive;
use gitbutler_core::{git::RepositoryExt, project_repository};
use tempfile::{tempdir, TempDir};

use crate::{init_opts, init_opts_bare, VAR_NO_CLEANUP};

pub struct Suite {
    pub local_app_data: Option<TempDir>,
    pub storage: gitbutler_core::storage::Storage,
    pub users: gitbutler_core::users::Controller,
    pub projects: gitbutler_core::projects::Controller,
    pub keys: gitbutler_core::keys::Controller,
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
        let storage = gitbutler_core::storage::Storage::new(local_app_data.path());
        let users = gitbutler_core::users::Controller::from_path(local_app_data.path());
        let projects = gitbutler_core::projects::Controller::from_path(local_app_data.path());
        let keys = gitbutler_core::keys::Controller::from_path(local_app_data.path());
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
    pub fn sign_in(&self) -> gitbutler_core::users::User {
        let user = gitbutler_core::users::User {
            name: Some("test".to_string()),
            email: "test@email.com".to_string(),
            access_token: Sensitive("token".to_string()),
            ..Default::default()
        };
        self.users.set_user(&user).expect("failed to add user");
        user
    }

    fn project(&self, fs: HashMap<PathBuf, &str>) -> (gitbutler_core::projects::Project, TempDir) {
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

pub struct Case {
    pub project: gitbutler_core::projects::Project,
    pub project_repository: gitbutler_core::project_repository::Repository,
    pub credentials: gitbutler_core::git::credentials::Helper,
    /// The directory containing the `project_repository`
    project_tmp: Option<TempDir>,
}

impl Drop for Case {
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

impl Case {
    fn new(
        suite: &Suite,
        project: gitbutler_core::projects::Project,
        project_tmp: TempDir,
    ) -> Case {
        let project_repository = gitbutler_core::project_repository::Repository::open(&project)
            .expect("failed to create project repository");
        let credentials =
            gitbutler_core::git::credentials::Helper::from_path(suite.local_app_data());
        Case {
            project,
            project_repository,
            project_tmp: Some(project_tmp),
            credentials,
        }
    }

    pub fn refresh(mut self, suite: &Suite) -> Self {
        let project = suite
            .projects
            .get(self.project.id)
            .expect("failed to get project");
        let project_repository = gitbutler_core::project_repository::Repository::open(&project)
            .expect("failed to create project repository");
        let credentials =
            gitbutler_core::git::credentials::Helper::from_path(suite.local_app_data());
        Self {
            credentials,
            project_repository,
            project,
            project_tmp: self.project_tmp.take(),
        }
    }
}

pub fn temp_dir() -> TempDir {
    tempdir().unwrap()
}

pub fn empty_bare_repository() -> (git2::Repository, TempDir) {
    let tmp = temp_dir();
    (
        git2::Repository::init_opts(&tmp, &init_opts_bare()).expect("failed to init repository"),
        tmp,
    )
}

pub fn test_repository() -> (git2::Repository, TempDir) {
    let tmp = temp_dir();
    let repository =
        git2::Repository::init_opts(&tmp, &init_opts()).expect("failed to init repository");
    project_repository::Config::from(&repository)
        .set_local("commit.gpgsign", "false")
        .unwrap();
    let mut index = repository.index().expect("failed to get index");
    let oid = index.write_tree().expect("failed to write tree");
    let signature = git2::Signature::now("test", "test@email.com").unwrap();
    let repo: &git2::Repository = &repository;
    repo.commit_with_signature(
        Some(&"refs/heads/master".parse().unwrap()),
        &signature,
        &signature,
        "Initial commit",
        &repository.find_tree(oid).expect("failed to find tree"),
        &[],
        None,
    )
    .expect("failed to commit");
    (repository, tmp)
}

pub fn commit_all(repository: &git2::Repository) -> git2::Oid {
    let mut index = repository.index().expect("failed to get index");
    index
        .add_all(["."], git2::IndexAddOption::DEFAULT, None)
        .expect("failed to add all");
    index.write().expect("failed to write index");
    let oid = index.write_tree().expect("failed to write tree");
    let signature = git2::Signature::now("test", "test@email.com").unwrap();
    let head = repository.head().expect("failed to get head");
    let repo: &git2::Repository = repository;
    let commit_oid = repo
        .commit_with_signature(
            Some(&head.name().map(|name| name.parse().unwrap()).unwrap()),
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
            None,
        )
        .expect("failed to commit");
    commit_oid
}

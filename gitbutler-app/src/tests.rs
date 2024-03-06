pub(crate) mod common;
mod suite {
    mod gb_repository;
    mod projects;
    mod virtual_branches;
}

use std::{collections::HashMap, fs, path};

use tempfile::tempdir;

use crate::{database, gb_repository, git, keys, project_repository, projects, storage, users};

pub struct Suite {
    pub local_app_data: path::PathBuf,
    pub storage: storage::Storage,
    pub users: users::Controller,
    pub projects: projects::Controller,
    pub keys: keys::Controller,
}

impl Default for Suite {
    fn default() -> Self {
        let local_app_data = temp_dir();
        let storage = storage::Storage::new(&local_app_data);
        let users = users::Controller::from_path(&local_app_data);
        let projects = projects::Controller::from_path(&local_app_data);
        let keys = keys::Controller::from_path(&local_app_data);
        Self {
            storage,
            local_app_data,
            users,
            projects,
            keys,
        }
    }
}

impl Suite {
    pub fn sign_in(&self) -> users::User {
        let user = users::User {
            name: Some("test".to_string()),
            email: "test@email.com".to_string(),
            access_token: "token".to_string(),
            ..Default::default()
        };
        self.users.set_user(&user).expect("failed to add user");
        user
    }

    fn project(&self, fs: HashMap<path::PathBuf, &str>) -> projects::Project {
        let repository = test_repository();
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

        self.projects
            .add(repository.path().parent().unwrap())
            .expect("failed to add project")
    }

    pub fn new_case_with_files(&self, fs: HashMap<path::PathBuf, &str>) -> Case {
        let project = self.project(fs);
        Case::new(self, project)
    }

    pub fn new_case(&self) -> Case {
        self.new_case_with_files(HashMap::new())
    }
}

pub struct Case<'a> {
    suite: &'a Suite,
    pub project: projects::Project,
    pub project_repository: project_repository::Repository,
    pub gb_repository: gb_repository::Repository,
    pub credentials: git::credentials::Helper,
}

impl<'a> Case<'a> {
    fn new(suite: &'a Suite, project: projects::Project) -> Case<'a> {
        let project_repository = project_repository::Repository::open(&project)
            .expect("failed to create project repository");
        let gb_repository =
            gb_repository::Repository::open(&suite.local_app_data, &project_repository, None)
                .expect("failed to open gb repository");
        let credentials = git::credentials::Helper::try_from(&suite.local_app_data)
            .expect("failed to get credentials");
        Case {
            suite,
            project,
            gb_repository,
            project_repository,
            credentials,
        }
    }

    pub fn refresh(&self) -> Self {
        let project = self
            .suite
            .projects
            .get(&self.project.id)
            .expect("failed to get project");
        let project_repository = project_repository::Repository::open(&project)
            .expect("failed to create project repository");
        let user = self.suite.users.get_user().expect("failed to get user");
        let credentials = git::credentials::Helper::try_from(&self.suite.local_app_data)
            .expect("failed to get credentials");
        Self {
            suite: self.suite,
            gb_repository: gb_repository::Repository::open(
                &self.suite.local_app_data,
                &project_repository,
                user.as_ref(),
            )
            .expect("failed to open gb repository"),
            credentials,
            project_repository,
            project,
        }
    }
}

pub fn test_database() -> database::Database {
    database::Database::try_from(&temp_dir()).unwrap()
}

pub fn temp_dir() -> path::PathBuf {
    let path = tempdir().unwrap().path().to_path_buf();
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn empty_bare_repository() -> git::Repository {
    let path = temp_dir();
    git::Repository::init_bare(path).expect("failed to init repository")
}

pub fn test_repository() -> git::Repository {
    let path = temp_dir();
    let repository = git::Repository::init(path).expect("failed to init repository");
    let mut index = repository.index().expect("failed to get index");
    let oid = index.write_tree().expect("failed to write tree");
    let signature = git::Signature::now("test", "test@email.com").unwrap();
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
    repository
}

pub fn commit_all(repository: &git::Repository) -> git::Oid {
    let mut index = repository.index().expect("failed to get index");
    index
        .add_all(["."], git2::IndexAddOption::DEFAULT, None)
        .expect("failed to add all");
    index.write().expect("failed to write index");
    let oid = index.write_tree().expect("failed to write tree");
    let signature = git::Signature::now("test", "test@email.com").unwrap();
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

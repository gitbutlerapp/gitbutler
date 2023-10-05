use std::{collections::HashMap, fs, path};

use tempfile::tempdir;

use crate::{database, gb_repository, git, keys, project_repository, projects, storage, users};

pub struct Suite {
    pub local_app_data: path::PathBuf,
    pub user_storage: users::Storage,
    pub projects_storage: projects::Storage,
    pub keys_storage: keys::Storage,
}

impl Default for Suite {
    fn default() -> Self {
        let local_app_data = temp_dir();
        let storage = storage::Storage::from(&local_app_data);
        let user_storage = users::Storage::from(&storage);
        let projects_storage = projects::Storage::from(&storage);
        let keys_storage = keys::Storage::from(&storage);
        Self {
            local_app_data,
            user_storage,
            projects_storage,
            keys_storage,
        }
    }
}

impl Suite {
    pub fn sign_in(&self) -> users::User {
        let user = users::User {
            name: "test".to_string(),
            email: "test@email.com".to_string(),
            ..Default::default()
        };
        self.user_storage.set(&user).expect("failed to add user");
        user
    }

    pub fn new_case_with_files(&self, fs: HashMap<path::PathBuf, &str>) -> Case {
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
        self.case_from_repository(repository)
    }

    pub fn new_case(&self) -> Case {
        self.new_case_with_files(HashMap::new())
    }

    fn case_from_repository(&self, repository: git::Repository) -> Case {
        let project = projects::Project {
            id: uuid::Uuid::new_v4().to_string(),
            title: repository
                .path()
                .parent()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
            path: repository.path().parent().unwrap().to_path_buf(),
            ..Default::default()
        };

        self.projects_storage
            .add(&project)
            .expect("failed to add project");
        let project_repository = project_repository::Repository::open(&project)
            .expect("failed to create project repository");
        let gb_repository = gb_repository::Repository::open(&self.local_app_data, &project, None)
            .expect("failed to open gb repository");
        Case {
            suite: self,
            project_repository,
            project,
            gb_repository,
        }
    }
}

pub struct Case<'a> {
    suite: &'a Suite,
    pub project_repository: project_repository::Repository,
    pub gb_repository: gb_repository::Repository,
    pub project: projects::Project,
}

impl Case<'_> {
    pub fn refresh(&mut self) {
        self.project = self
            .suite
            .projects_storage
            .get(&self.project.id)
            .expect("failed to get project");
        self.project_repository = project_repository::Repository::open(&self.project)
            .expect("failed to create project repository");
        self.gb_repository =
            gb_repository::Repository::open(&self.suite.local_app_data, &self.project, None)
                .expect("failed to open gb repository");
    }
}

pub fn test_database() -> database::Database {
    let path = temp_dir().join("test.db");
    database::Database::try_from(&path).unwrap()
}

pub fn temp_dir() -> path::PathBuf {
    let path = tempdir().unwrap().path().to_path_buf();
    fs::create_dir_all(&path).unwrap();
    path
}

pub fn test_repository() -> git::Repository {
    let path = temp_dir();
    let repository = git::Repository::init(path).expect("failed to init repository");
    let mut index = repository.index().expect("failed to get index");
    let oid = index.write_tree().expect("failed to write tree");
    let signature = git::Signature::now("test", "test@email.com").unwrap();
    repository
        .commit(
            Some("HEAD"),
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
    let commit_oid = repository
        .commit(
            Some("HEAD"),
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

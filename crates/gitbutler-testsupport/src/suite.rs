use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use but_ctx::Context;
use but_settings::AppSettings;
use gitbutler_repo::RepositoryExt;
use tempfile::{TempDir, tempdir};

use crate::{VAR_NO_CLEANUP, init_opts, init_opts_bare, test_project::setup_config};

pub struct Suite {
    pub local_app_data: Option<TempDir>,
    pub storage: but_fs::Storage,
}

impl Drop for Suite {
    fn drop(&mut self) {
        if std::env::var_os(VAR_NO_CLEANUP).is_some() {
            let _ = self.local_app_data.take().unwrap().keep();
        }
    }
}

impl Default for Suite {
    fn default() -> Self {
        let local_app_data = temp_dir();
        let storage = but_fs::Storage::new(local_app_data.path());
        Self {
            storage,
            local_app_data: Some(local_app_data),
        }
    }
}

impl Suite {
    pub fn local_app_data(&self) -> &Path {
        self.local_app_data.as_ref().unwrap().path()
    }
    pub fn sign_in(&self) -> gitbutler_user::User {
        crate::secrets::setup_blackhole_store();
        let user: gitbutler_user::User =
            serde_json::from_str(include_str!("fixtures/user/minimal.v1"))
                .expect("valid v1 user file");
        gitbutler_user::set_user(&user).expect("failed to add user");
        user
    }

    fn project(&self, fs: HashMap<PathBuf, &str>) -> (gitbutler_project::Project, TempDir) {
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

        let outcome = gitbutler_project::add_with_path(
            self.local_app_data(),
            repository.path().parent().unwrap(),
        );

        let project = outcome.expect("failed to add project").unwrap_project();

        (project, tmp)
    }

    pub fn new_case_with_files(&self, fs: HashMap<PathBuf, &str>) -> Case {
        let (project, project_tmp) = self.project(fs);
        Case::new(project, project_tmp)
    }

    pub fn new_case(&self) -> Case {
        self.new_case_with_files(HashMap::new())
    }
}

pub struct Case {
    pub project: gitbutler_project::Project,
    pub ctx: Context,
    /// The directory containing the `ctx`
    pub project_tmp: Option<TempDir>,
}

impl Drop for Case {
    fn drop(&mut self) {
        if let Some(tmp) = self
            .project_tmp
            .take()
            .filter(|_| std::env::var_os(VAR_NO_CLEANUP).is_some())
        {
            let _ = tmp.keep();
        }
    }
}

impl Case {
    fn new(project: gitbutler_project::Project, project_tmp: TempDir) -> Case {
        let ctx = Context::new_from_legacy_project_and_settings(&project, AppSettings::default());
        Case {
            project,
            ctx,
            project_tmp: Some(project_tmp),
        }
    }

    pub fn refresh(mut self, _suite: &Suite) -> Self {
        let project = gitbutler_project::get(self.project.id).expect("failed to get project");
        let ctx = Context::new_from_legacy_project_and_settings(&project, AppSettings::default());
        Self {
            ctx,
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
    setup_config(&repository.config().unwrap()).unwrap();
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

    repo.commit_with_signature(
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
    .expect("failed to commit")
}

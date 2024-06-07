use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;

use super::{storage, storage::UpdateRequest, Project, ProjectId};
use crate::git::{RepositoryExt};
use crate::projects::AuthKey;
use crate::{error, project_repository};

#[async_trait]
pub trait Watchers {
    /// Watch for filesystem changes on the given project.
    fn watch(&self, project: &Project) -> anyhow::Result<()>;
    /// Stop watching filesystem changes.
    async fn stop(&self, id: ProjectId);
}

#[derive(Clone)]
pub struct Controller {
    local_data_dir: PathBuf,
    projects_storage: storage::Storage,
    watchers: Option<Arc<dyn Watchers + Send + Sync>>,
}

impl Controller {
    pub fn new(
        local_data_dir: PathBuf,
        projects_storage: storage::Storage,
        watchers: Option<impl Watchers + Send + Sync + 'static>,
    ) -> Self {
        Self {
            local_data_dir,
            projects_storage,
            watchers: watchers.map(|w| Arc::new(w) as Arc<_>),
        }
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            projects_storage: storage::Storage::from_path(&path),
            local_data_dir: path,
            watchers: None,
        }
    }

    pub fn add<P: AsRef<Path>>(&self, path: P) -> Result<Project> {
        let path = path.as_ref();
        let all_projects = self
            .projects_storage
            .list()
            .context("failed to list projects from storage")?;
        if all_projects.iter().any(|project| project.path == path) {
            bail!("project already exists");
        }
        if !path.exists() {
            bail!("path not found");
        }
        if !path.is_dir() {
            bail!("not a directory");
        }
        match gix::open_opts(path, gix::open::Options::isolated()) {
            Ok(repo) if repo.is_bare() => {
                bail!("bare repositories are unsupported");
            }
            Ok(repo) if repo.worktree().map_or(false, |wt| !wt.is_main()) => {
                if path.join(".git").is_file() {
                    bail!("can only work in main worktrees");
                };
            }
            Ok(repo) if repo.submodules().map_or(false, |sm| sm.is_some()) => {
                bail!("repositories with git submodules are not supported");
            }
            Ok(_repo) => {}
            Err(err) => {
                return Err(anyhow::Error::from(err))
                    .context(error::Context::new("must be a Git repository"));
            }
        }

        let id = uuid::Uuid::new_v4().to_string();

        // title is the base name of the file
        let title = path
            .iter()
            .last()
            .map_or_else(|| id.clone(), |p| p.to_str().unwrap().to_string());

        let project = Project {
            id: ProjectId::generate(),
            title,
            path: path.to_path_buf(),
            api: None,
            ..Default::default()
        };

        self.projects_storage
            .add(&project)
            .context("failed to add project to storage")?;

        // Create a .git/gitbutler directory for app data
        if let Err(error) = std::fs::create_dir_all(project.gb_dir()) {
            tracing::error!(project_id = %project.id, ?error, "failed to create {:?} on project add", project.gb_dir());
        }

        if let Some(watcher) = &self.watchers {
            watcher.watch(&project)?;
        }

        Ok(project)
    }

    pub async fn update(&self, project: &UpdateRequest) -> Result<Project> {
        #[cfg(not(windows))]
        if let Some(AuthKey::Local {
            private_key_path, ..
        }) = &project.preferred_key
        {
            use resolve_path::PathResolveExt;
            let private_key_path = private_key_path.resolve();

            if !private_key_path.exists() {
                bail!(
                    "private key at \"{}\" not found",
                    private_key_path.display()
                );
            }

            if !private_key_path.is_file() {
                bail!(
                    "private key at \"{}\" is not a file",
                    private_key_path.display()
                );
            }
        }

        // FIXME(qix-): On windows, we have to force to system executable.
        // FIXME(qix-): This is a hack for now, and will be smoothed over in the future.
        #[cfg(windows)]
        let project_owned = {
            let mut project = project.clone();
            project.preferred_key = Some(AuthKey::SystemExecutable);
            project
        };

        #[cfg(windows)]
        let project = &project_owned;

        self.projects_storage.update(project)
    }

    pub fn get(&self, id: ProjectId) -> Result<Project> {
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut project = self.projects_storage.get(id)?;
        if !project.gb_dir().exists() {
            if let Err(error) = std::fs::create_dir_all(project.gb_dir()) {
                tracing::error!(project_id = %project.id, ?error, "failed to create \"{}\" on project get", project.gb_dir().display());
            }
        }
        // Clean up old virtual_branches.toml that was never used
        let old_virtual_branches_path = project.path.join(".git").join("virtual_branches.toml");
        if old_virtual_branches_path.exists() {
            if let Err(error) = std::fs::remove_file(old_virtual_branches_path) {
                tracing::error!(project_id = %project.id, ?error, "failed to remove old virtual_branches.toml");
            }
        }

        // FIXME(qix-): On windows, we have to force to system executable
        #[cfg(windows)]
        {
            project.preferred_key = AuthKey::SystemExecutable;
        }

        Ok(project)
    }

    pub fn list(&self) -> Result<Vec<Project>> {
        self.projects_storage.list().map_err(Into::into)
    }

    pub async fn delete(&self, id: ProjectId) -> Result<()> {
        let Some(project) = self.projects_storage.try_get(id)? else {
            return Ok(());
        };

        if let Some(watchers) = &self.watchers {
            watchers.stop(id).await;
        }

        self.projects_storage
            .purge(project.id)
            .map_err(anyhow::Error::from)?;

        if let Err(error) = std::fs::remove_dir_all(
            self.local_data_dir
                .join("projects")
                .join(project.id.to_string()),
        ) {
            tracing::error!(project_id = %id, ?error, "failed to remove project data",);
        }

        if let Err(error) = std::fs::remove_file(project.path.join(".git/gitbutler.json")) {
            tracing::error!(project_id = %project.id, ?error, "failed to remove .git/gitbutler.json data",);
        }

        if project.gb_dir().exists() {
            if let Err(error) = std::fs::remove_dir_all(project.gb_dir()) {
                tracing::error!(project_id = %project.id, ?error, "failed to remove {:?} on project delete", project.gb_dir());
            }
        }

        Ok(())
    }

    pub fn get_local_config(&self, id: ProjectId, key: &str) -> Result<Option<String>> {
        let project = self.projects_storage.get(id)?;

        let repo = project_repository::Repository::open(&project)?;
        repo.config().get_local(key)
    }

    pub fn set_local_config(&self, id: ProjectId, key: &str, value: &str) -> Result<()> {
        let project = self.projects_storage.get(id)?;

        let repo = project_repository::Repository::open(&project)?;
        repo.config().set_local(key, value)
    }

    pub fn check_signing_settings(&self, id: ProjectId) -> Result<bool> {
        let project = self.projects_storage.get(id)?;

        let repo = project_repository::Repository::open(&project)?;
        let signed = repo.repo().sign_buffer(&"test".to_string().into());
        match signed {
            Ok(_) => Ok(true),
            Err(e) => Err(e),
        }
    }
}

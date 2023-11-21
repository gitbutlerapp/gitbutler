use std::{collections::HashMap, path};

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use crate::{
    gb_repository, git, keys,
    paths::DataDir,
    project_repository::{self, conflicts},
    projects::{self, ProjectId},
    reader,
    sessions::{self, SessionId},
    users,
    virtual_branches::target,
    watcher,
};

pub struct App {
    local_data_dir: DataDir,
    projects: projects::Controller,
    users: users::Controller,
    keys: keys::Controller,
    watchers: watcher::Watchers,
    sessions_database: sessions::Database,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    GetProject(#[from] projects::GetError),
    #[error(transparent)]
    ProjectRemote(#[from] project_repository::RemoteError),
    #[error(transparent)]
    OpenProjectRepository(#[from] project_repository::OpenError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl TryFrom<&AppHandle> for App {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            local_data_dir: DataDir::try_from(value)?,
            keys: keys::Controller::from(value),
            projects: projects::Controller::try_from(value)?,
            users: users::Controller::from(value),
            watchers: value.state::<watcher::Watchers>().inner().clone(),
            sessions_database: sessions::Database::from(value),
        })
    }
}

impl App {
    pub fn init_project(&self, project: &projects::Project) -> Result<()> {
        self.watchers.watch(project).context(format!(
            "failed to start watcher for project {}",
            &project.id
        ))?;

        Ok(())
    }

    pub fn init(&self) -> Result<()> {
        for project in self
            .projects
            .list()
            .with_context(|| "failed to list projects")?
        {
            if let Err(error) = self.init_project(&project) {
                tracing::error!(%project.id, ?error, "failed to init project");
            }
        }
        Ok(())
    }

    pub fn list_session_files(
        &self,
        project_id: &ProjectId,
        session_id: &SessionId,
        paths: Option<&[path::PathBuf]>,
    ) -> Result<HashMap<path::PathBuf, reader::Content>, Error> {
        let session = self
            .sessions_database
            .get_by_project_id_id(project_id, session_id)
            .context("failed to get session")?
            .context("session not found")?;
        let user = self.users.get_user().context("failed to get user")?;
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gb repository")?;
        let session_reader =
            sessions::Reader::open(&gb_repo, &session).context("failed to open session reader")?;
        session_reader
            .files(paths)
            .context("failed to read session files")
            .map_err(Error::Other)
    }

    pub fn mark_resolved(&self, project_id: &ProjectId, path: &str) -> Result<(), Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        // mark file as resolved
        conflicts::resolve(&project_repository, path)?;
        Ok(())
    }

    pub fn fetch_from_target(&self, project_id: &ProjectId) -> Result<(), Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let user = self.users.get_user().context("failed to get user")?;
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gb repository")?;
        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
        let target_reader = target::Reader::new(&current_session_reader);
        let default_target = match target_reader.read_default() {
            Ok(target) => Ok(target),
            Err(reader::Error::NotFound) => Err(anyhow::anyhow!("target not set")),
            Err(e) => Err(e).context("failed to read default target"),
        }?;

        let credentials = git::credentials::Factory::new(
            &project,
            self.keys
                .get_or_create()
                .context("failed to get or create key")?,
            user.as_ref(),
        );

        project_repository.fetch(default_target.branch.remote(), &credentials)?;

        Ok(())
    }

    pub fn git_remote_branches(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<git::RemoteBranchName>, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        project_repository
            .git_remote_branches()
            .map_err(Error::Other)
    }

    pub fn git_head(&self, project_id: &ProjectId) -> Result<String, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let head = project_repository
            .get_head()
            .context("failed to get repository head")?;
        Ok(head.name().unwrap().to_string())
    }

    pub fn git_set_global_config(&self, key: &str, value: &str) -> Result<String> {
        let mut config = git2::Config::open_default()?;
        config.set_str(key, value)?;
        Ok(value.to_string())
    }

    pub fn git_get_global_config(&self, key: &str) -> Result<Option<String>> {
        let config = git2::Config::open_default()?;
        let value = config.get_string(key);
        match value {
            Ok(value) => Ok(Some(value)),
            Err(e) => {
                if e.code() == git2::ErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    pub async fn delete_all_data(&self) -> Result<(), Error> {
        for project in self.projects.list().context("failed to list projects")? {
            self.projects
                .delete(&project.id)
                .await
                .context("failed to delete project")?;
        }
        Ok(())
    }
}

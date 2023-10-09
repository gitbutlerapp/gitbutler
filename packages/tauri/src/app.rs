use std::{collections::HashMap, ops, path};

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use crate::{
    bookmarks, deltas, gb_repository,
    git::{self, diff},
    keys,
    project_repository::{self, conflicts},
    projects, reader, search, sessions, users,
    virtual_branches::{self, target},
    watcher,
};

pub struct App {
    local_data_dir: std::path::PathBuf,
    projects_controller: projects::Controller,
    users_controller: users::Controller,
    keys_controller: keys::Controller,
    searcher: search::Searcher,
    watchers: watcher::Watchers,
    sessions_database: sessions::Database,
    deltas_database: deltas::Database,
    bookmarks_database: bookmarks::Database,
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
            local_data_dir: value
                .path_resolver()
                .app_local_data_dir()
                .context("failed to get local data dir")?,
            keys_controller: keys::Controller::try_from(value)?,
            projects_controller: projects::Controller::try_from(value)?,
            users_controller: users::Controller::try_from(value)?,
            searcher: value.state::<search::Searcher>().inner().clone(),
            watchers: value.state::<watcher::Watchers>().inner().clone(),
            sessions_database: sessions::Database::try_from(value)?,
            deltas_database: deltas::Database::try_from(value)?,
            bookmarks_database: bookmarks::Database::try_from(value)?,
        })
    }
}

impl App {
    pub async fn init_project(&self, project: &projects::Project) -> Result<()> {
        self.watchers.watch(project).await.context(format!(
            "failed to start watcher for project {}",
            &project.id
        ))?;

        Ok(())
    }

    pub async fn init(&self) -> Result<()> {
        for project in self
            .projects_controller
            .list()
            .with_context(|| "failed to list projects")?
        {
            if let Err(error) = self.init_project(&project).await {
                tracing::error!(project.id, ?error, "failed to init project");
            }
        }
        Ok(())
    }

    pub fn get_project(&self, id: &str) -> Result<projects::Project, Error> {
        self.projects_controller.get(id).map_err(Error::GetProject)
    }

    pub fn list_sessions(
        &self,
        project_id: &str,
        earliest_timestamp_ms: Option<u128>,
    ) -> Result<Vec<sessions::Session>> {
        self.sessions_database
            .list_by_project_id(project_id, earliest_timestamp_ms)
    }

    pub fn list_session_files(
        &self,
        project_id: &str,
        session_id: &str,
        paths: Option<Vec<path::PathBuf>>,
    ) -> Result<HashMap<path::PathBuf, reader::Content>, Error> {
        let session = self
            .sessions_database
            .get_by_project_id_id(project_id, session_id)
            .context("failed to get session")?
            .context("session not found")?;
        let user = self
            .users_controller
            .get_user()
            .context("failed to get user")?;
        let project = self.projects_controller.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)?;
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

    pub fn mark_resolved(&self, project_id: &str, path: &str) -> Result<(), Error> {
        let project = self.projects_controller.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)?;
        // mark file as resolved
        conflicts::resolve(&project_repository, path)?;
        Ok(())
    }

    pub fn fetch_from_target(&self, project_id: &str) -> Result<(), Error> {
        let project = self.projects_controller.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)?;
        let user = self
            .users_controller
            .get_user()
            .context("failed to get user")?;
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

        let key = match &project.preferred_key {
            projects::AuthKey::Local {
                private_key_path,
                passphrase,
            } => keys::Key::Local {
                private_key_path: private_key_path.clone(),
                passphrase: passphrase.clone(),
            },
            projects::AuthKey::Generated => {
                let key = self
                    .keys_controller
                    .get_or_create()
                    .map_err(|e| Error::Other(e.into()))?;
                keys::Key::Generated(Box::new(key))
            }
        };

        project_repository.fetch(default_target.branch.remote(), &key)?;

        Ok(())
    }

    pub async fn upsert_bookmark(&self, bookmark: &bookmarks::Bookmark) -> Result<(), Error> {
        {
            let project = self.projects_controller.get(&bookmark.project_id)?;
            let project_repository = project_repository::Repository::try_from(&project)?;
            let user = self
                .users_controller
                .get_user()
                .context("failed to get user")?;
            let gb_repository = gb_repository::Repository::open(
                &self.local_data_dir,
                &project_repository,
                user.as_ref(),
            )
            .context("failed to open gb repository")?;
            let writer = bookmarks::Writer::new(&gb_repository).context("failed to open writer")?;
            writer.write(bookmark).context("failed to write bookmark")?;
        }

        if let Err(error) = self
            .watchers
            .post(watcher::Event::Bookmark(bookmark.clone()))
            .await
        {
            tracing::error!(?error, "failed to send session event");
        }

        Ok(())
    }

    pub fn list_bookmarks(
        &self,
        project_id: &str,
        range: Option<ops::Range<u128>>,
    ) -> Result<Vec<bookmarks::Bookmark>, Error> {
        self.bookmarks_database
            .list_by_project_id(project_id, range)
            .map_err(Error::Other)
    }

    pub fn list_session_deltas(
        &self,
        project_id: &str,
        session_id: &str,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<deltas::Delta>>, Error> {
        self.deltas_database
            .list_by_project_id_session_id(project_id, session_id, paths)
            .map_err(Error::Other)
    }

    pub fn git_wd_diff(
        &self,
        project_id: &str,
        context_lines: u32,
    ) -> Result<HashMap<path::PathBuf, String>, Error> {
        let project = self.projects_controller.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)?;

        let diff = diff::workdir(
            &project_repository.git_repository,
            &project_repository
                .get_head()
                .context("failed to get project head")?
                .peel_to_commit()
                .context("failed to peel head to commit")?
                .id(),
            &diff::Options { context_lines },
        )
        .context("failed to diff")?;

        let diff = diff
            .into_iter()
            .map(|(file_path, hunks)| {
                (
                    file_path,
                    hunks
                        .iter()
                        .map(|hunk| hunk.diff.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            })
            .collect::<HashMap<_, _>>();

        Ok(diff)
    }

    pub fn git_remote_branches(
        &self,
        project_id: &str,
    ) -> Result<Vec<git::RemoteBranchName>, Error> {
        let project = self.projects_controller.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)?;
        project_repository
            .git_remote_branches()
            .map_err(Error::Other)
    }

    pub fn git_remote_branches_data(
        &self,
        project_id: &str,
    ) -> Result<Vec<virtual_branches::RemoteBranch>, Error> {
        let project = self.projects_controller.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)?;
        let user = self
            .users_controller
            .get_user()
            .context("failed to get user")?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gb repo")?;
        virtual_branches::list_remote_branches(&gb_repository, &project_repository)
            .map_err(Error::Other)
    }

    pub fn git_head(&self, project_id: &str) -> Result<String, Error> {
        let project = self.projects_controller.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)?;
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

    pub fn git_gb_push(&self, project_id: &str) -> Result<(), Error> {
        let user = self
            .users_controller
            .get_user()
            .context("failed to get user")?;
        let project = self.projects_controller.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gb repo")?;
        gb_repository.push(user.as_ref()).map_err(Error::Other)
    }

    pub fn search(&self, query: &search::Query) -> Result<search::Results, Error> {
        self.searcher.search(query).map_err(Error::Other)
    }

    pub async fn delete_all_data(&self) -> Result<(), Error> {
        self.searcher
            .delete_all_data()
            .context("failed to delete search data")?;
        for project in self
            .projects_controller
            .list()
            .context("failed to list projects")?
        {
            self.projects_controller
                .delete(&project.id)
                .context("failed to delete project")?;
        }
        Ok(())
    }
}

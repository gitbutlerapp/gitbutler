use std::{collections::HashMap, ops, path};

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

use crate::{
    bookmarks, deltas, gb_repository,
    git::{self, diff},
    keys,
    paths::DataDir,
    project_repository::{self, conflicts},
    projects::{self, ProjectId},
    reader,
    sessions::{self, SessionId},
    users,
    virtual_branches::{self, target},
    watcher,
};

pub struct App {
    local_data_dir: DataDir,
    projects: projects::Controller,
    users: users::Controller,
    keys: keys::Controller,
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
            local_data_dir: DataDir::try_from(value)?,
            keys: keys::Controller::from(value),
            projects: projects::Controller::try_from(value)?,
            users: users::Controller::from(value),
            watchers: value.state::<watcher::Watchers>().inner().clone(),
            sessions_database: sessions::Database::from(value),
            deltas_database: deltas::Database::from(value),
            bookmarks_database: bookmarks::Database::from(value),
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
            .projects
            .list()
            .with_context(|| "failed to list projects")?
        {
            if let Err(error) = self.init_project(&project).await {
                tracing::error!(%project.id, ?error, "failed to init project");
            }
        }
        Ok(())
    }

    pub fn get_project(&self, id: &ProjectId) -> Result<projects::Project, Error> {
        self.projects.get(id).map_err(Error::GetProject)
    }

    pub fn list_sessions(
        &self,
        project_id: &ProjectId,
        earliest_timestamp_ms: Option<u128>,
    ) -> Result<Vec<sessions::Session>> {
        let sessions = self
            .sessions_database
            .list_by_project_id(project_id, earliest_timestamp_ms)?;

        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)?;
        let user = self.users.get_user().context("failed to get user")?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )?;

        // this is a hack to account for a case when we have a session created, but fs was never
        // touched, so the wathcer never picked up the session
        let current_session = gb_repository.get_current_session()?;
        let have_to_index = match (current_session.as_ref(), sessions.first()) {
            (Some(real), Some(from_db)) => !real.eq(from_db),
            (Some(_), None) => true,
            _ => false,
        };
        if !have_to_index {
            return Ok(sessions);
        }

        let sessions_iter = gb_repository.get_sessions_iterator()?;
        let mut sessions = sessions_iter.collect::<Result<Vec<_>, _>>()?;
        self.sessions_database
            .insert(project_id, &sessions.iter().collect::<Vec<_>>())?;
        if let Some(session) = current_session {
            self.sessions_database.insert(project_id, &[&session])?;
            sessions.insert(0, session);
        }
        Ok(sessions)
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

    pub fn mark_resolved(&self, project_id: &ProjectId, path: &str) -> Result<(), Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)?;
        // mark file as resolved
        conflicts::resolve(&project_repository, path)?;
        Ok(())
    }

    pub fn fetch_from_target(&self, project_id: &ProjectId) -> Result<(), Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)?;
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

    pub async fn upsert_bookmark(&self, bookmark: &bookmarks::Bookmark) -> Result<(), Error> {
        {
            let project = self.projects.get(&bookmark.project_id)?;
            let project_repository = project_repository::Repository::try_from(&project)?;
            let user = self.users.get_user().context("failed to get user")?;
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
        project_id: &ProjectId,
        range: Option<ops::Range<u128>>,
    ) -> Result<Vec<bookmarks::Bookmark>, Error> {
        self.bookmarks_database
            .list_by_project_id(project_id, range)
            .map_err(Error::Other)
    }

    pub fn list_session_deltas(
        &self,
        project_id: &ProjectId,
        session_id: &SessionId,
        paths: &Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<deltas::Delta>>, Error> {
        self.deltas_database
            .list_by_project_id_session_id(project_id, session_id, paths)
            .map_err(Error::Other)
    }

    pub fn git_wd_diff(
        &self,
        project_id: &ProjectId,
        context_lines: u32,
    ) -> Result<HashMap<path::PathBuf, String>, Error> {
        let project = self.projects.get(project_id)?;
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
                        .map(|hunk| hunk.diff.clone())
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
            })
            .collect::<HashMap<_, _>>();

        Ok(diff)
    }

    pub fn git_remote_branches(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<git::RemoteBranchName>, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)?;
        project_repository
            .git_remote_branches()
            .map_err(Error::Other)
    }

    pub fn git_remote_branches_data(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<virtual_branches::RemoteBranch>, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)?;
        let user = self.users.get_user().context("failed to get user")?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gb repo")?;
        virtual_branches::list_remote_branches(&gb_repository, &project_repository)
            .map_err(Error::Other)
    }

    pub fn git_head(&self, project_id: &ProjectId) -> Result<String, Error> {
        let project = self.projects.get(project_id)?;
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

    pub fn git_gb_push(&self, project_id: &ProjectId) -> Result<(), Error> {
        let user = self.users.get_user().context("failed to get user")?;
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::try_from(&project)?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gb repo")?;
        gb_repository.push(user.as_ref()).map_err(Error::Other)
    }

    pub fn delete_all_data(&self) -> Result<(), Error> {
        for project in self.projects.list().context("failed to list projects")? {
            self.projects
                .delete(&project.id)
                .context("failed to delete project")?;
        }
        Ok(())
    }
}

use std::{collections::HashMap, ops, path, time};

use anyhow::{Context, Result};
use futures::executor::block_on;
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

#[derive(Clone)]
pub struct App {
    local_data_dir: std::path::PathBuf,
    projects_storage: projects::Storage,
    users_storage: users::Storage,
    keys_controller: keys::Storage,
    searcher: search::Searcher,
    watchers: watcher::Watchers,
    sessions_database: sessions::Database,
    deltas_database: deltas::Database,
    bookmarks_database: bookmarks::Database,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to fetch: {0}")]
    FetchError(#[from] project_repository::Error),
    #[error("failed to create project: {0}")]
    CreateProjectError(String),
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
            keys_controller: keys::Storage::try_from(value)?,
            projects_storage: projects::Storage::try_from(value)?,
            users_storage: users::Storage::try_from(value)?,
            searcher: value.state::<search::Searcher>().inner().clone(),
            watchers: value.state::<watcher::Watchers>().inner().clone(),
            sessions_database: sessions::Database::try_from(value)?,
            deltas_database: deltas::Database::try_from(value)?,
            bookmarks_database: bookmarks::Database::try_from(value)?,
        })
    }
}

impl App {
    pub fn init_project(&self, project: &projects::Project) -> Result<()> {
        block_on(async move {
            self.watchers
                .watch(project)
                .await
                .with_context(|| {
                    format!("failed to start watcher for project {}", project.id.clone())
                })
                .expect("failed to start watcher");
        });

        Ok(())
    }

    pub fn init(&self) -> Result<()> {
        for project in self
            .projects_storage
            .list_projects()
            .with_context(|| "failed to list projects")?
        {
            if let Err(error) = self.init_project(&project) {
                tracing::error!(project.id, ?error, "failed to init project");
            }
        }
        Ok(())
    }

    fn gb_project(&self, project_id: &str) -> Result<projects::Project> {
        self.projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project {} not found", project_id))
    }

    pub fn get_user(&self) -> Result<Option<users::User>> {
        self.users_storage.get()
    }

    pub fn set_user(&self, user: &users::User) -> Result<()> {
        self.users_storage.set(user)
    }

    pub fn delete_user(&self) -> Result<()> {
        self.users_storage.delete()
    }

    pub fn add_project(&self, path: &str) -> Result<projects::Project, Error> {
        let all_projects = self
            .projects_storage
            .list_projects()
            .map_err(Error::Other)?;

        if all_projects.iter().any(|project| project.path == path) {
            return Err(Error::CreateProjectError(format!(
                "project {} already exists",
                path
            )));
        }

        let project = projects::Project::from_path(path.to_string())
            .map_err(|err| Error::CreateProjectError(err.to_string()))?;

        self.projects_storage
            .add_project(&project)
            .context("failed to add project")
            .map_err(Error::Other)?;

        self.init_project(&project)
            .context("failed to init project")
            .map_err(Error::Other)?;

        Ok(project)
    }

    pub fn update_project(&self, project: &projects::UpdateRequest) -> Result<projects::Project> {
        let updated = self.projects_storage.update_project(project)?;

        block_on(async move {
            if let Err(error) = self
                .watchers
                .post(watcher::Event::FetchGitbutlerData(
                    project.id.clone(),
                    time::SystemTime::now(),
                ))
                .await
            {
                tracing::error!(project_id = &project.id, ?error, "failed to fetch project");
            }
        });

        Ok(updated)
    }

    pub fn get_project(&self, id: &str) -> Result<Option<projects::Project>> {
        self.projects_storage.get_project(id)
    }

    pub fn list_projects(&self) -> Result<Vec<projects::Project>> {
        self.projects_storage.list_projects()
    }

    pub fn delete_project(&self, id: &str) -> Result<()> {
        match self.projects_storage.get_project(id)? {
            Some(project) => {
                let gb_repository = match gb_repository::Repository::open(
                    self.local_data_dir.clone(),
                    &project,
                    self.users_storage.get()?.as_ref(),
                ) {
                    Ok(repo) => Ok(Some(repo)),
                    Err(gb_repository::Error::ProjectPathNotFound(_)) => Ok(None),
                    Err(e) => Err(anyhow::anyhow!("failed to open repository: {:#}", e)),
                }?;

                block_on({
                    let project_id = project.id.clone();
                    async move {
                        if let Err(error) = self.watchers.stop(&project_id).await {
                            tracing::error!(
                                project_id,
                                ?error,
                                "failed to stop watcher for project",
                            );
                        }
                    }
                });

                if let Some(gb_repository) = gb_repository {
                    if let Err(error) = gb_repository.purge() {
                        tracing::error!(
                            project_id = project.id,
                            ?error,
                            "failed to remove project dir"
                        );
                    }
                }

                self.projects_storage
                    .purge(&project.id)
                    .context("failed to purge project")?;

                Ok(())
            }
            None => Ok(()),
        }
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
    ) -> Result<HashMap<path::PathBuf, reader::Content>> {
        let session = self
            .sessions_database
            .get_by_project_id_id(project_id, session_id)
            .context("failed to get session")?
            .context("session not found")?;
        let user = self.users_storage.get()?;
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project {} not found", project_id))?;

        let gb_repo =
            gb_repository::Repository::open(&self.local_data_dir, &project, user.as_ref())
                .context("failed to open gb repository")?;
        let session_reader =
            sessions::Reader::open(&gb_repo, &session).context("failed to open session reader")?;
        session_reader
            .files(paths)
            .context("failed to read session files")
    }

    pub fn mark_resolved(&self, project_id: &str, path: &str) -> Result<()> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        // mark file as resolved
        conflicts::resolve(&project_repository, path)?;
        Ok(())
    }

    pub fn fetch_from_target(&self, project_id: &str) -> Result<(), Error> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let user = self.users_storage.get()?;
        let gb_repo =
            gb_repository::Repository::open(&self.local_data_dir, &project, user.as_ref())
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

        project_repository
            .fetch(default_target.branch.remote(), &key)
            .map_err(Error::FetchError)
    }

    pub fn upsert_bookmark(&self, bookmark: &bookmarks::Bookmark) -> Result<()> {
        let user = self.users_storage.get()?;
        let project = self.gb_project(&bookmark.project_id)?;
        let gb_repository =
            gb_repository::Repository::open(&self.local_data_dir, &project, user.as_ref())?;
        let writer = bookmarks::Writer::new(&gb_repository).context("failed to open writer")?;
        writer.write(bookmark).context("failed to write bookmark")?;

        block_on({
            let bookmark = bookmark.clone();
            async move {
                if let Err(error) = self
                    .watchers
                    .post(watcher::Event::Bookmark(bookmark.clone()))
                    .await
                {
                    tracing::error!(?error, "failed to send session event");
                }
            }
        });

        Ok(())
    }

    pub fn list_bookmarks(
        &self,
        project_id: &str,
        range: Option<ops::Range<u128>>,
    ) -> Result<Vec<bookmarks::Bookmark>> {
        self.bookmarks_database
            .list_by_project_id(project_id, range)
    }

    pub fn list_session_deltas(
        &self,
        project_id: &str,
        session_id: &str,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<deltas::Delta>>> {
        self.deltas_database
            .list_by_project_id_session_id(project_id, session_id, paths)
    }

    pub fn git_status(
        &self,
        project_id: &str,
    ) -> Result<HashMap<String, project_repository::FileStatus>> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_status()
    }

    pub fn git_wd_diff(
        &self,
        project_id: &str,
        context_lines: u32,
    ) -> Result<HashMap<path::PathBuf, String>> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;

        let diff = diff::workdir(
            &project_repository.git_repository,
            &project_repository.get_head()?.peel_to_commit()?.id(),
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

    pub fn git_match_paths(&self, project_id: &str, pattern: &str) -> Result<Vec<String>> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_match_paths(pattern)
    }

    pub fn git_remote_branches(&self, project_id: &str) -> Result<Vec<git::RemoteBranchName>> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_remote_branches()
    }

    pub fn git_remote_branches_data(
        &self,
        project_id: &str,
    ) -> Result<Vec<virtual_branches::RemoteBranch>> {
        let user = self.users_storage.get()?;
        let project = self.gb_project(project_id)?;
        let gb_repository =
            gb_repository::Repository::open(&self.local_data_dir, &project, user.as_ref())
                .context("failed to open gb repo")?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        virtual_branches::list_remote_branches(&gb_repository, &project_repository)
    }

    pub fn git_head(&self, project_id: &str) -> Result<String> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        let head = project_repository.get_head()?;
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

    pub fn git_gb_push(&self, project_id: &str) -> Result<()> {
        let user = self.users_storage.get()?;
        let project = self.gb_project(project_id)?;
        let gb_repository =
            gb_repository::Repository::open(&self.local_data_dir, &project, user.as_ref())
                .context("failed to open gb repo")?;
        gb_repository.push(user.as_ref())
    }

    pub fn search(&self, query: &search::Query) -> Result<search::Results> {
        self.searcher.search(query)
    }

    pub fn delete_all_data(&self) -> Result<()> {
        self.searcher
            .delete_all_data()
            .context("failed to delete search data")?;
        for project in self.list_projects()? {
            self.delete_project(&project.id)
                .context("failed to delete project")?;
        }
        Ok(())
    }
}

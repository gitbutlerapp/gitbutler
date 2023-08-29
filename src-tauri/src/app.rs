use std::{collections::HashMap, ops, path, time};

use anyhow::{Context, Result};
use futures::executor::block_on;
use tauri::{AppHandle, Manager};
use tokio::task;

use crate::{
    bookmarks, deltas, files, gb_repository, keys,
    project_repository::{self, activity, branch, conflicts, diff},
    projects, pty, reader, search, sessions, users,
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
    files_database: files::Database,
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
            files_database: files::Database::try_from(value)?,
            bookmarks_database: bookmarks::Database::try_from(value)?,
        })
    }
}

impl App {
    pub fn start_pty_server(&self) -> Result<()> {
        let self_ = self.clone();
        task::Builder::new().name("pty-server").spawn(async move {
            let port = if cfg!(debug_assertions) { 7702 } else { 7703 };
            if let Err(e) = pty::start_server(port, self_).await {
                tracing::error!("failed to start pty server: {:#}", e);
            }
        })?;
        Ok(())
    }

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
            if let Err(e) = self.init_project(&project) {
                tracing::error!("failed to init project {}: {:#}", project.id, e);
            }
        }
        Ok(())
    }

    fn gb_repository(&self, project_id: &str) -> Result<gb_repository::Repository> {
        gb_repository::Repository::open(
            self.local_data_dir.clone(),
            project_id,
            self.projects_storage.clone(),
            self.users_storage.clone(),
        )
        .context("failed to open repository")
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
            if let Err(err) = self
                .watchers
                .post(watcher::Event::FetchGitbutlerData(
                    project.id.clone(),
                    time::SystemTime::now(),
                ))
                .await
            {
                tracing::error!("{}: failed to fetch project: {:#}", &project.id, err);
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
                    id,
                    self.projects_storage.clone(),
                    self.users_storage.clone(),
                ) {
                    Ok(repo) => Ok(Some(repo)),
                    Err(gb_repository::Error::ProjectPathNotFound(_)) => Ok(None),
                    Err(e) => Err(anyhow::anyhow!("failed to open repository: {:#}", e)),
                }?;

                block_on({
                    let project_id = project.id.clone();
                    async move {
                        if let Err(e) = self.watchers.stop(&project_id).await {
                            tracing::error!(
                                "failed to stop watcher for project {}: {}",
                                project_id,
                                e
                            );
                        }
                    }
                });

                if let Some(gb_repository) = gb_repository {
                    if let Err(e) = gb_repository.purge() {
                        tracing::error!("failed to remove project dir {}: {}", project.id, e);
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
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, String>> {
        self.files_database
            .list_by_project_id_session_id(project_id, session_id, paths)
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
        let gb_repo = self.gb_repository(project_id)?;
        let current_session = gb_repo.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repo, &current_session)?;
        let target_reader = target::Reader::new(&current_session_reader);
        let default_target = match target_reader.read_default() {
            Ok(target) => Ok(target),
            Err(reader::Error::NotFound) => Err(anyhow::anyhow!("target not set")),
            Err(e) => Err(e).context("failed to read default target"),
        }?;

        let key = self
            .keys_controller
            .get_or_create()
            .map_err(|e| Error::Other(e.into()))?;

        project_repository
            .fetch(&default_target.remote_name, &key)
            .map_err(Error::FetchError)
    }

    pub fn upsert_bookmark(&self, bookmark: &bookmarks::Bookmark) -> Result<()> {
        let gb_repository = self.gb_repository(&bookmark.project_id)?;
        let writer = bookmarks::Writer::new(&gb_repository).context("failed to open writer")?;
        writer.write(bookmark).context("failed to write bookmark")?;

        block_on({
            let bookmark = bookmark.clone();
            async move {
                if let Err(err) = self
                    .watchers
                    .post(watcher::Event::Bookmark(bookmark.clone()))
                    .await
                {
                    tracing::error!("failed to send session event: {:#}", err);
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

    pub fn git_activity(
        &self,
        project_id: &str,
        start_time_ms: Option<u128>,
    ) -> Result<Vec<activity::Activity>> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_activity(start_time_ms)
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
            &project_repository,
            &project_repository.get_head()?.peel_to_commit()?.id(),
            &diff::Options { context_lines },
        )
        .context("failed to diff")?;
        let diff = Self::diff_hunks_to_string(diff);
        Ok(diff)
    }
    pub fn git_commit_diff(
        &self,
        project_id: &str,
        commit_id: &str,
    ) -> Result<HashMap<path::PathBuf, String>> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;

        let commit = project_repository
            .git_repository
            .find_commit(git2::Oid::from_str(commit_id).unwrap())?;

        let parent = commit.parent(0).context("failed to get parent commit")?;
        let commit_tree = commit.tree().context("failed to get commit tree")?;
        let parent_tree = parent.tree().context("failed to get parent tree")?;
        let diff = diff::trees(&project_repository, &parent_tree, &commit_tree)?;

        let diff = Self::diff_hunks_to_string(diff);
        Ok(diff)
    }

    fn diff_hunks_to_string(
        diff: HashMap<path::PathBuf, Vec<project_repository::diff::Hunk>>,
    ) -> HashMap<path::PathBuf, String> {
        return diff
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
    }

    pub fn git_match_paths(&self, project_id: &str, pattern: &str) -> Result<Vec<String>> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_match_paths(pattern)
    }

    pub fn git_branches(&self, project_id: &str) -> Result<Vec<branch::LocalName>> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_branches()
    }

    pub fn git_remote_branches(&self, project_id: &str) -> Result<Vec<branch::RemoteName>> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_remote_branches()
    }

    pub fn git_remote_branches_data(
        &self,
        project_id: &str,
    ) -> Result<Vec<virtual_branches::RemoteBranch>> {
        let gb_repository = self.gb_repository(project_id)?;
        let project = self.gb_project(project_id)?;
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

    pub fn git_set_config(&self, project_id: &str, key: &str, value: &str) -> Result<String> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        let repo = &project_repository.git_repository;
        let mut config = repo.config()?;
        config.open_level(git2::ConfigLevel::Local)?;
        config.set_str(key, value)?;
        Ok(value.to_string())
    }

    pub fn git_get_config(&self, project_id: &str, key: &str) -> Result<Option<String>> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let repo = &project_repository.git_repository;
        let config = repo.config()?;
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

    pub fn git_switch_branch(&self, project_id: &str, branch: &str) -> Result<()> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        let gb_repository = self.gb_repository(project_id)?;
        gb_repository.flush().context("failed to flush session")?;
        project_repository.git_switch_branch(branch)
    }

    pub fn git_gb_push(&self, project_id: &str) -> Result<()> {
        let gb_repository = self.gb_repository(project_id)?;
        gb_repository.push()
    }

    pub fn git_stage_files<P: AsRef<std::path::Path>>(
        &self,
        project_id: &str,
        paths: Vec<P>,
    ) -> Result<()> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_stage_files(paths)
    }

    pub fn git_unstage_files<P: AsRef<std::path::Path>>(
        &self,
        project_id: &str,
        paths: Vec<P>,
    ) -> Result<()> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_unstage_files(paths)
    }

    pub fn git_commit(&self, project_id: &str, message: &str, push: bool) -> Result<()> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_commit(message, push)
    }

    pub fn search(&self, query: &search::Query) -> Result<search::Results> {
        self.searcher.search(query)
    }

    pub fn record_pty(&self, project_id: &str, typ: pty::Type, bytes: &[u8]) -> Result<()> {
        let gb_repository = self.gb_repository(project_id)?;
        let pty_writer = pty::Writer::new(&gb_repository)?;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let record = pty::Record {
            timestamp,
            typ,
            bytes: bytes.to_vec(),
        };

        pty_writer.write(&record).context("failed to append pty")?;

        Ok(())
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

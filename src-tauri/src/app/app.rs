use std::{collections::HashMap, sync};

use anyhow::{Context, Result};
use crossbeam_channel::{bounded, Sender};

use crate::{deltas, events, git::activity, projects, pty, search, sessions, storage, users};

use super::{gb_repository, project_repository, watcher};

#[derive(Clone)]
pub struct App {
    local_data_dir: std::path::PathBuf,

    projects_storage: projects::Storage,
    users_storage: users::Storage,

    deltas_searcher: search::Deltas,

    repositories: sync::Arc<sync::Mutex<HashMap<String, gb_repository::Repository>>>,
    stop_watchers: sync::Arc<sync::Mutex<HashMap<String, Sender<()>>>>,
}

impl App {
    pub fn new<P: AsRef<std::path::Path>>(local_data_dir: P) -> Result<Self> {
        let local_data_dir = local_data_dir.as_ref();
        let storage = storage::Storage::from_path(local_data_dir.clone());
        let deltas_searcher =
            search::Deltas::at(local_data_dir.clone()).context("failed to open deltas searcher")?;
        Ok(Self {
            local_data_dir: local_data_dir.to_path_buf(),
            projects_storage: projects::Storage::new(storage.clone()),
            users_storage: users::Storage::new(storage.clone()),
            deltas_searcher,
            repositories: sync::Arc::new(sync::Mutex::new(HashMap::new())),
            stop_watchers: sync::Arc::new(sync::Mutex::new(HashMap::new())),
        })
    }

    pub fn start_pty_server(&self) -> Result<()> {
        let self_ = self.clone();
        tauri::async_runtime::spawn(async move {
            let port = if cfg!(debug_assertions) { 7702 } else { 7703 };
            if let Err(e) = pty::start_server(port, self_).await {
                log::error!("failed to start pty server: {:#}", e);
            }
        });
        Ok(())
    }

    pub fn init_project(
        &self,
        project: &projects::Project,
        events: std::sync::mpsc::Sender<events::Event>,
    ) -> Result<()> {
        let gb_repository = gb_repository::Repository::open(
            self.local_data_dir.clone(),
            project.id.clone(),
            self.projects_storage.clone(),
            self.users_storage.clone(),
        )
        .context("failed to open git repository")?;

        self.repositories
            .lock()
            .unwrap()
            .insert(project.id.clone(), gb_repository);

        self.start_watcher(&project, events.clone())
            .with_context(|| {
                format!("failed to start watcher for project {}", project.id.clone())
            })?;

        Ok(())
    }

    pub fn init(&self, events: std::sync::mpsc::Sender<events::Event>) -> Result<()> {
        for project in self
            .projects_storage
            .list_projects()
            .with_context(|| "failed to list projects")?
        {
            if let Err(e) = self.init_project(&project, events.clone()) {
                log::error!("failed to init project {}: {:#}", project.id, e);
            }

            self.reindex_project(&project)
        }
        Ok(())
    }

    fn reindex_project(&self, project: &projects::Project) {
        let project = project.clone();
        let users_storage = self.users_storage.clone();
        let projects_storage = self.projects_storage.clone();
        let local_data_dir = self.local_data_dir.clone();
        let mut deltas_searcher = self.deltas_searcher.clone();

        tauri::async_runtime::spawn_blocking(move || {
            let project = project;

            let gb_repository = gb_repository::Repository::open(
                local_data_dir,
                project.id.clone(),
                projects_storage.clone(),
                users_storage,
            )
            .expect("failed to open git repository");

            if let Err(e) = deltas_searcher.reindex_project(&gb_repository) {
                log::error!("{}: failed to reindex project: {:#}", project.id, e);
            }
        });
    }

    fn start_watcher(
        &self,
        project: &projects::Project,
        events: std::sync::mpsc::Sender<events::Event>,
    ) -> Result<()> {
        let project = project.clone();
        let users_storage = self.users_storage.clone();
        let projects_storage = self.projects_storage.clone();
        let local_data_dir = self.local_data_dir.clone();
        let deltas_searcher = self.deltas_searcher.clone();

        let (stop_tx, stop_rx) = bounded(1);
        self.stop_watchers
            .lock()
            .unwrap()
            .insert(project.id.clone(), stop_tx.clone());

        tauri::async_runtime::spawn_blocking(|| {
            let project = project;

            let gb_repository = gb_repository::Repository::open(
                local_data_dir,
                project.id.clone(),
                projects_storage.clone(),
                users_storage,
            )
            .expect("failed to open git repository");

            let watcher = watcher::Watcher::new(
                &project,
                projects_storage,
                &gb_repository,
                deltas_searcher,
                events,
                stop_rx,
            )
            .expect("failed to create watcher");

            watcher.start().expect("failed to start watcher");
        });

        Ok(())
    }

    fn stop_watcher(&self, project_id: &str) -> Result<()> {
        if let Some((_, stop_tx)) = self.stop_watchers.lock().unwrap().remove_entry(project_id) {
            stop_tx
                .send(())
                .context("failed to send stop signal to watcher")?;
        };
        Ok(())
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

    pub fn add_project(
        &self,
        path: &str,
        events: std::sync::mpsc::Sender<events::Event>,
    ) -> Result<projects::Project> {
        let all_projects = self.projects_storage.list_projects()?;

        if let Some(project) = all_projects.iter().find(|project| project.path == path) {
            return Ok(project.clone());
        }

        let project =
            projects::Project::from_path(path.to_string()).context("failed to create project")?;

        self.projects_storage
            .add_project(&project)
            .context("failed to add project")?;

        self.init_project(&project, events.clone())
            .context("failed to init project")?;

        Ok(project)
    }

    pub fn update_project(&self, project: &projects::UpdateRequest) -> Result<projects::Project> {
        self.projects_storage.update_project(&project)
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
                self.projects_storage
                    .update_project(&projects::UpdateRequest {
                        id: id.to_string(),
                        deleted: Some(true),
                        ..Default::default()
                    })?;

                self.repositories.lock().unwrap().remove(id);

                if let Err(e) = self.stop_watcher(&project.id) {
                    log::error!("failed to stop watcher for project {}: {}", project.id, e);
                }

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
        let repositories = self.repositories.lock().unwrap();
        let gb_repository = repositories
            .get(project_id)
            .ok_or_else(|| anyhow::anyhow!("project {} not found", project_id))?;

        let mut sessions = vec![];
        let mut iter = gb_repository.get_sessions_iterator()?;
        while let Some(session) = iter.next() {
            if let Err(e) = session {
                return Err(e);
            }

            if let Some(earliest_timestamp_ms) = earliest_timestamp_ms {
                if session.as_ref().unwrap().meta.start_timestamp_ms < earliest_timestamp_ms {
                    break;
                }
            }

            sessions.push(session.unwrap());
        }

        if let Some(current_session) = gb_repository.get_current_session()? {
            sessions.insert(0, current_session);
        }

        Ok(sessions)
    }

    pub fn list_session_files(
        &self,
        project_id: &str,
        session_id: &str,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, String>> {
        let repositories = self.repositories.lock().unwrap();
        let gb_repository = repositories
            .get(project_id)
            .ok_or_else(|| anyhow::anyhow!("project {} not found", project_id))?;

        let session = gb_repository
            .get_session(session_id)
            .context("failed to get session")?;

        let reader = gb_repository
            .get_session_reader(&session)
            .context("failed to get session reader")?;

        reader.files(paths)
    }

    pub fn list_session_deltas(
        &self,
        project_id: &str,
        session_id: &str,
        paths: Option<Vec<&str>>,
    ) -> Result<HashMap<String, Vec<deltas::Delta>>> {
        let repositories = self.repositories.lock().unwrap();
        let gb_repository = repositories
            .get(project_id)
            .ok_or_else(|| anyhow::anyhow!("project {} not found", project_id))?;

        let session = gb_repository
            .get_session(session_id)
            .context("failed to get session")?;

        let reader = gb_repository
            .get_session_reader(&session)
            .context("failed to get session reader")?;

        reader.deltas(paths)
    }

    pub fn git_activity(
        &self,
        project_id: &str,
        start_time_ms: Option<u128>,
    ) -> Result<Vec<activity::Activity>> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project {} not found", project_id))?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_activity(start_time_ms)
    }

    pub fn git_status(
        &self,
        project_id: &str,
    ) -> Result<HashMap<String, project_repository::FileStatus>> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project {} not found", project_id))?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_status()
    }

    pub fn git_wd_diff(
        &self,
        project_id: &str,
        max_lines: usize,
    ) -> Result<HashMap<String, String>> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project wd not found"))?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_wd_diff(max_lines)
    }

    pub fn git_match_paths(&self, project_id: &str, pattern: &str) -> Result<Vec<String>> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project wd not found"))?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_match_paths(pattern)
    }

    pub fn git_branches(&self, project_id: &str) -> Result<Vec<String>> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project wd not found"))?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_branches()
    }

    pub fn git_head(&self, project_id: &str) -> Result<String> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project wd not found"))?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        let head = project_repository.get_head()?;
        Ok(head.name().unwrap().to_string())
    }

    pub fn git_switch_branch(&self, project_id: &str, branch: &str) -> Result<()> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project wd not found"))?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        let repositories = self.repositories.lock().unwrap();
        let gb_repository = repositories
            .get(project_id)
            .ok_or_else(|| anyhow::anyhow!("project {} not found", project_id))?;
        gb_repository.flush().context("failed to flush session")?;
        project_repository.git_switch_branch(branch)
    }

    pub fn git_stage_files<P: AsRef<std::path::Path>>(
        &self,
        project_id: &str,
        paths: Vec<P>,
    ) -> Result<()> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_stage_files(paths)
    }

    pub fn git_unstage_files<P: AsRef<std::path::Path>>(
        &self,
        project_id: &str,
        paths: Vec<P>,
    ) -> Result<()> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_unstage_files(paths)
    }

    pub fn git_commit(&self, project_id: &str, message: &str, push: bool) -> Result<()> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project not found"))?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_commit(message, push)
    }

    pub fn search(&self, query: &search::SearchQuery) -> Result<search::SearchResults> {
        self.deltas_searcher.search(query)
    }

    pub fn record_pty(&self, project_id: &str, typ: pty::Type, bytes: &Vec<u8>) -> Result<()> {
        let repositories = self.repositories.lock().unwrap();
        let gb_repository = repositories
            .get(project_id)
            .ok_or_else(|| anyhow::anyhow!("project {} not found", project_id))?;

        let session = gb_repository
            .get_or_create_current_session()
            .context("failed to get session")?;
        let writer = gb_repository.get_session_writer(&session)?;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let record = pty::Record {
            timestamp,
            typ,
            bytes: bytes.to_vec(),
        };

        writer.append_pty(&record).context("failed to append pty")?;

        Ok(())
    }
}

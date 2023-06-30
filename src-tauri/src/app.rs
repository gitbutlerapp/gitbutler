use std::{collections::HashMap, ops, sync};

use anyhow::{Context, Result};
use tokio::sync::{mpsc, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::{
    bookmarks, database, deltas, events, files, gb_repository,
    project_repository::{self, activity},
    projects, pty, reader, search, sessions, storage, users,
    virtual_branches::{
        self,
        branch::{BranchUpdateRequest, Ownership},
    },
    watcher,
};

#[derive(Clone)]
pub struct App {
    local_data_dir: std::path::PathBuf,

    projects_storage: projects::Storage,
    users_storage: users::Storage,

    searcher: search::Searcher,
    events_sender: events::Sender,

    stop_watchers: sync::Arc<sync::Mutex<HashMap<String, CancellationToken>>>,
    proxy_watchers: sync::Arc<sync::Mutex<HashMap<String, mpsc::UnboundedSender<watcher::Event>>>>,

    sessions_database: sessions::Database,
    files_database: files::Database,
    deltas_database: deltas::Database,
    bookmarks_database: bookmarks::Database,

    vbranch_semaphores: sync::Arc<tokio::sync::Mutex<HashMap<String, Semaphore>>>,
}

#[derive(Debug, thiserror::Error)]
pub enum AddProjectError {
    #[error("Project already exists")]
    ProjectAlreadyExists,
    #[error("{0}")]
    OpenError(projects::CreateError),
    #[error("{0}")]
    Other(anyhow::Error),
}

impl App {
    pub fn new<P: AsRef<std::path::Path>>(
        local_data_dir: P,
        event_sender: events::Sender,
    ) -> Result<Self> {
        let local_data_dir = local_data_dir.as_ref();
        let storage = storage::Storage::from_path(local_data_dir);
        let deltas_searcher =
            search::Searcher::at(local_data_dir).context("failed to open deltas searcher")?;
        let database = database::Database::open(local_data_dir.join("database.sqlite3"))?;
        Ok(Self {
            events_sender: event_sender,
            local_data_dir: local_data_dir.to_path_buf(),
            projects_storage: projects::Storage::new(storage.clone()),
            users_storage: users::Storage::new(storage),
            searcher: deltas_searcher,
            stop_watchers: sync::Arc::new(sync::Mutex::new(HashMap::new())),
            proxy_watchers: sync::Arc::new(sync::Mutex::new(HashMap::new())),
            sessions_database: sessions::Database::new(database.clone()),
            deltas_database: deltas::Database::new(database.clone()),
            files_database: files::Database::new(database.clone()),
            bookmarks_database: bookmarks::Database::new(database),
            vbranch_semaphores: sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
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

    pub fn init_project(&self, project: &projects::Project) -> Result<()> {
        self.start_watcher(project).with_context(|| {
            format!("failed to start watcher for project {}", project.id.clone())
        })?;

        Ok(())
    }

    pub fn init(&self) -> Result<()> {
        for project in self
            .projects_storage
            .list_projects()
            .with_context(|| "failed to list projects")?
        {
            if let Err(e) = self.init_project(&project) {
                log::error!("failed to init project {}: {:#}", project.id, e);
            }

            if let Err(e) = self
                .proxy_watchers
                .lock()
                .unwrap()
                .get(&project.id)
                .unwrap()
                .send(watcher::Event::IndexAll)
            {
                log::error!("failed to send session event: {:#}", e);
            }
        }
        Ok(())
    }

    fn start_watcher(&self, project: &projects::Project) -> Result<()> {
        let project = project.clone();
        let users_storage = self.users_storage.clone();
        let projects_storage = self.projects_storage.clone();
        let local_data_dir = self.local_data_dir.clone();
        let deltas_searcher = self.searcher.clone();
        let events_sender = self.events_sender.clone();
        let sessions_database = self.sessions_database.clone();
        let files_database = self.files_database.clone();
        let deltas_database = self.deltas_database.clone();
        let bookmarks_database = self.bookmarks_database.clone();

        let cancellation_token = CancellationToken::new();
        self.stop_watchers
            .lock()
            .unwrap()
            .insert(project.id.clone(), cancellation_token.clone());

        let (proxy_tx, proxy_rx) = mpsc::unbounded_channel();
        self.proxy_watchers
            .lock()
            .unwrap()
            .insert(project.id.clone(), proxy_tx);

        tauri::async_runtime::spawn(async move {
            let project = project;
            let watcher = watcher::Watcher::new(
                local_data_dir,
                &project,
                projects_storage,
                users_storage,
                deltas_searcher,
                cancellation_token,
                events_sender,
                sessions_database,
                deltas_database,
                files_database,
                bookmarks_database,
            )
            .expect("failed to create watcher");

            watcher
                .start(proxy_rx)
                .await
                .expect("failed to init watcher");
        });

        Ok(())
    }

    fn send_event(&self, project_id: &str, event: watcher::Event) -> Result<()> {
        self.proxy_watchers
            .lock()
            .unwrap()
            .get(project_id)
            .unwrap()
            .send(event)
            .context("failed to send event to proxy")
    }

    fn stop_watcher(&self, project_id: &str) -> Result<()> {
        if let Some((_, token)) = self.stop_watchers.lock().unwrap().remove_entry(project_id) {
            token.cancel();
        };
        Ok(())
    }

    fn gb_repository(&self, project_id: &str) -> Result<gb_repository::Repository> {
        gb_repository::Repository::open(
            self.local_data_dir.clone(),
            project_id.to_string(),
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

    pub fn add_project(&self, path: &str) -> Result<projects::Project, AddProjectError> {
        let all_projects = self
            .projects_storage
            .list_projects()
            .map_err(AddProjectError::Other)?;

        if all_projects.iter().any(|project| project.path == path) {
            return Err(AddProjectError::ProjectAlreadyExists);
        }

        let project =
            projects::Project::from_path(path.to_string()).map_err(AddProjectError::OpenError)?;

        self.projects_storage
            .add_project(&project)
            .context("failed to add project")
            .map_err(AddProjectError::Other)?;

        self.init_project(&project)
            .context("failed to init project")
            .map_err(AddProjectError::Other)?;

        Ok(project)
    }

    pub fn update_project(&self, project: &projects::UpdateRequest) -> Result<projects::Project> {
        let updated = self.projects_storage.update_project(project)?;

        if let Err(err) = self.send_event(&project.id, watcher::Event::Fetch) {
            log::error!("{}: failed to fetch project: {:#}", &project.id, err);
        }

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
                let gb_repository = gb_repository::Repository::open(
                    self.local_data_dir.clone(),
                    id.to_string(),
                    self.projects_storage.clone(),
                    self.users_storage.clone(),
                )
                .context("failed to open repository")?;

                if let Err(e) = self.stop_watcher(&project.id) {
                    log::error!("failed to stop watcher for project {}: {}", project.id, e);
                }

                if let Err(e) = gb_repository.purge() {
                    log::error!("failed to remove project dir {}: {}", project.id, e);
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

    pub fn get_target_data(
        &self,
        project_id: &str,
    ) -> Result<Option<virtual_branches::target::Target>> {
        let gb_repository = self.gb_repository(project_id)?;
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        let current_session = gb_repository.get_or_create_current_session()?;
        let current_session_reader = sessions::Reader::open(&gb_repository, &current_session)?;
        let target_reader = virtual_branches::target::Reader::new(&current_session_reader);
        match target_reader.read_default_with_behind(&project_repository) {
            Ok(target) => Ok(Some(target)),
            Err(reader::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_target_branch(
        &self,
        project_id: &str,
        target_branch: &str,
    ) -> Result<Option<virtual_branches::target::Target>> {
        let gb_repository = self.gb_repository(project_id)?;
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        let target =
            gb_repository.set_target_branch(&project_repository, target_branch.to_string())?;
        Ok(Some(target))
    }

    pub fn update_branch_target(&self, project_id: &str) -> Result<()> {
        let gb_repository = self.gb_repository(project_id)?;
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        virtual_branches::update_branch_target(&gb_repository, &project_repository)?;
        Ok(())
    }

    pub async fn list_virtual_branches(
        &self,
        project_id: &str,
    ) -> Result<Vec<virtual_branches::VirtualBranch>> {
        let gb_repository = self.gb_repository(project_id)?;
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;

        let mut semaphores = self.vbranch_semaphores.lock().await;
        let semaphore = semaphores
            .entry(project_id.to_string())
            .or_insert_with(|| Semaphore::new(1));
        let _permit = semaphore.acquire().await?;

        virtual_branches::list_virtual_branches(&gb_repository, &project_repository)
    }

    pub async fn create_virtual_branch(
        &self,
        project_id: &str,
        name: &str,
        ownership: &Ownership,
    ) -> Result<()> {
        let gb_repository = self.gb_repository(project_id)?;

        let mut semaphores = self.vbranch_semaphores.lock().await;
        let semaphore = semaphores
            .entry(project_id.to_string())
            .or_insert_with(|| Semaphore::new(1));
        let _permit = semaphore.acquire().await?;

        let branch_id = virtual_branches::create_virtual_branch(&gb_repository, name)?.id;
        virtual_branches::update_branch(
            &gb_repository,
            BranchUpdateRequest {
                id: branch_id,
                name: None,
                order: None,
                ownership: Some(ownership.clone()),
            },
        )
        .context("failed to update branch")?;
        Ok(())
    }

    pub async fn update_virtual_branch(
        &self,
        project_id: &str,
        branch_update: virtual_branches::branch::BranchUpdateRequest,
    ) -> Result<()> {
        let gb_repository = self.gb_repository(project_id)?;

        let mut semaphores = self.vbranch_semaphores.lock().await;
        let semaphore = semaphores
            .entry(project_id.to_string())
            .or_insert_with(|| Semaphore::new(1));
        let _permit = semaphore.acquire().await?;

        virtual_branches::update_branch(&gb_repository, branch_update)?;
        Ok(())
    }

    pub fn unapply_virtual_branch(&self, project_id: &str, branch_id: &str) -> Result<()> {
        let gb_repository = self.gb_repository(project_id)?;
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        virtual_branches::unapply_branch(&gb_repository, &project_repository, branch_id)?;
        Ok(())
    }

    pub fn apply_virtual_branch(&self, project_id: &str, branch_id: &str) -> Result<()> {
        let gb_repository = self.gb_repository(project_id)?;
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        virtual_branches::apply_branch(&gb_repository, &project_repository, branch_id)?;
        Ok(())
    }

    pub fn commit_virtual_branch(
        &self,
        project_id: &str,
        branch: &str,
        message: &str,
    ) -> Result<()> {
        let gb_repository = self.gb_repository(project_id)?;
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        virtual_branches::commit(&gb_repository, &project_repository, branch, message, None)?;
        Ok(())
    }

    pub fn push_virtual_branch(
        &self,
        project_id: &str,
        commit_id: &str,
        branch_id: &str,
    ) -> Result<()> {
        let gb_repository = self.gb_repository(project_id)?;
        let project = self.gb_project(project_id)?;
        virtual_branches::push(&project.path, &gb_repository, commit_id, branch_id)?;
        Ok(())
    }

    pub fn upsert_bookmark(&self, bookmark: &bookmarks::Bookmark) -> Result<()> {
        let gb_repository = self.gb_repository(&bookmark.project_id)?;
        let writer = bookmarks::Writer::new(&gb_repository).context("failed to open writer")?;
        writer.write(bookmark).context("failed to write bookmark")?;

        if let Err(e) = self
            .proxy_watchers
            .lock()
            .unwrap()
            .get(&bookmark.project_id)
            .unwrap()
            .send(watcher::Event::Bookmark(bookmark.clone()))
        {
            log::error!("failed to send session event: {:#}", e);
        }
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
        context_lines: usize,
    ) -> Result<HashMap<String, String>> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_wd_diff(context_lines)
    }

    pub fn git_match_paths(&self, project_id: &str, pattern: &str) -> Result<Vec<String>> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_match_paths(pattern)
    }

    pub fn git_branches(&self, project_id: &str) -> Result<Vec<String>> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        project_repository.git_branches()
    }

    pub fn git_remote_branches(&self, project_id: &str) -> Result<Vec<String>> {
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
        virtual_branches::remote_branches(&gb_repository, &project_repository)
    }

    pub fn git_head(&self, project_id: &str) -> Result<String> {
        let project = self.gb_project(project_id)?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository")?;
        let head = project_repository.get_head()?;
        Ok(head.name().unwrap().to_string())
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

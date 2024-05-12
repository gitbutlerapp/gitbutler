mod calculate_deltas;
mod index;

use std::path;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use gitbutler_core::ops::entry::{OperationType, SnapshotDetails};
use gitbutler_core::ops::oplog::Oplog;
use gitbutler_core::projects::ProjectId;
use gitbutler_core::sessions::SessionId;
use gitbutler_core::virtual_branches::VirtualBranches;
use gitbutler_core::{
    assets, deltas, gb_repository, git, project_repository, projects, reader, sessions, users,
    virtual_branches,
};
use tracing::instrument;

use super::{events, Change};

/// A type that contains enough state to make decisions based on changes in the filesystem, which themselves
/// may trigger [Changes](Change)
// NOTE: This is `Clone` as each incoming event is spawned onto a thread for processing.
#[derive(Clone)]
pub struct Handler {
    // The following fields our currently required state as we are running in the background
    // and access it as filesystem events are processed. It's still to be decided how granular it
    // should be, and I can imagine having a top-level `app` handle that keeps the application state of
    // the tauri app, assuming that such application would not be `Send + Sync` everywhere and thus would
    // need extra protection.
    users: users::Controller,
    local_data_dir: path::PathBuf,
    projects: projects::Controller,
    vbranch_controller: virtual_branches::Controller,
    assets_proxy: assets::Proxy,
    sessions_db: sessions::Database,
    deltas_db: deltas::Database,

    /// A function to send events - decoupled from app-handle for testing purposes.
    #[allow(clippy::type_complexity)]
    send_event: Arc<dyn Fn(Change) -> Result<()> + Send + Sync + 'static>,
}

impl Handler {
    /// A constructor whose primary use is the test-suite.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        local_data_dir: PathBuf,
        users: users::Controller,
        projects: projects::Controller,
        vbranch_controller: virtual_branches::Controller,
        assets_proxy: assets::Proxy,
        sessions_db: sessions::Database,
        deltas_db: deltas::Database,
        send_event: impl Fn(Change) -> Result<()> + Send + Sync + 'static,
    ) -> Self {
        Handler {
            local_data_dir,
            users,
            projects,
            vbranch_controller,
            assets_proxy,
            sessions_db,
            deltas_db,
            send_event: Arc::new(send_event),
        }
    }

    /// Handle the events that come in from the filesystem, or the public API.
    #[instrument(skip(self), fields(event = %event), err(Debug))]
    pub(super) async fn handle(&self, event: events::InternalEvent) -> Result<()> {
        match event {
            events::InternalEvent::ProjectFilesChange(project_id, path) => {
                self.recalculate_everything(path, project_id).await
            }

            events::InternalEvent::GitFilesChange(project_id, paths) => self
                .git_files_change(paths, project_id)
                .await
                .context("failed to handle git file change event"),

            events::InternalEvent::Flush(project_id, session) => self
                .flush_session(project_id, &session)
                .await
                .context("failed to handle flush session event"),

            events::InternalEvent::CalculateVirtualBranches(project_id) => self
                .calculate_virtual_branches(project_id)
                .await
                .context("failed to handle virtual branch event"),
        }
    }
}

impl Handler {
    fn emit_app_event(&self, event: Change) -> Result<()> {
        (self.send_event)(event).context("failed to send event")
    }

    fn emit_session_file(
        &self,
        project_id: ProjectId,
        session_id: SessionId,
        file_path: &Path,
        contents: Option<reader::Content>,
    ) -> Result<()> {
        self.emit_app_event(Change::File {
            project_id,
            session_id,
            file_path: file_path.to_owned(),
            contents,
        })
    }

    async fn flush_session(
        &self,
        project_id: ProjectId,
        session: &sessions::Session,
    ) -> Result<()> {
        let project = self
            .projects
            .get(&project_id)
            .context("failed to get project")?;
        let user = self.users.get_user()?;
        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        let session = gb_repo
            .flush_session(&project_repository, session, user.as_ref())
            .context(format!("failed to flush session {}", session.id))?;

        self.index_session(project_id, session)?;

        Ok(())
    }

    #[instrument(skip(self, project_id))]
    async fn calculate_virtual_branches(&self, project_id: ProjectId) -> Result<()> {
        match self
            .vbranch_controller
            .list_virtual_branches(&project_id)
            .await
        {
            Ok((branches, skipped_files)) => {
                let branches = self.assets_proxy.proxy_virtual_branches(branches).await;
                self.emit_app_event(Change::VirtualBranches {
                    project_id,
                    virtual_branches: VirtualBranches {
                        branches,
                        skipped_files,
                    },
                })
            }
            Err(err) if err.is::<virtual_branches::errors::VerifyError>() => Ok(()),
            Err(err) => Err(err.context("failed to list virtual branches").into()),
        }
    }

    #[instrument(skip(self, paths, project_id), fields(paths = paths.len()))]
    async fn recalculate_everything(
        &self,
        paths: Vec<PathBuf>,
        project_id: ProjectId,
    ) -> Result<()> {
        let calc_deltas = tokio::task::spawn_blocking({
            let this = self.clone();
            move || this.calculate_deltas(paths, project_id)
        });
        // Create a snapshot every time there are more than a configurable number of new lines of code (default 20)
        let handle_snapshots = tokio::task::spawn_blocking({
            let this = self.clone();
            move || this.maybe_create_snapshot(project_id)
        });
        self.calculate_virtual_branches(project_id).await?;
        let _ = handle_snapshots.await;
        calc_deltas.await??;
        Ok(())
    }

    fn maybe_create_snapshot(&self, project_id: ProjectId) -> anyhow::Result<()> {
        let project = self
            .projects
            .get(&project_id)
            .context("failed to get project")?;
        let changed_lines = project.lines_since_snapshot()?;
        if changed_lines > project.snapshot_lines_threshold() {
            project.create_snapshot(SnapshotDetails::new(OperationType::FileChanges))?;
        }
        Ok(())
    }

    pub async fn git_file_change(
        &self,
        path: impl Into<PathBuf>,
        project_id: ProjectId,
    ) -> Result<()> {
        self.git_files_change(vec![path.into()], project_id).await
    }

    pub async fn git_files_change(&self, paths: Vec<PathBuf>, project_id: ProjectId) -> Result<()> {
        let project = self
            .projects
            .get(&project_id)
            .context("failed to get project")?;
        let open_projects_repository = || {
            project_repository::Repository::open(&project)
                .context("failed to open project repository for project")
        };

        for path in paths {
            let Some(file_name) = path.to_str() else {
                continue;
            };
            match file_name {
                "FETCH_HEAD" => {
                    self.emit_app_event(Change::GitFetch(project_id))?;
                    self.calculate_virtual_branches(project_id).await?;
                }
                "logs/HEAD" => {
                    self.emit_app_event(Change::GitActivity(project.id))?;
                }
                "GB_FLUSH" => {
                    let user = self.users.get_user()?;
                    let project_repository = open_projects_repository()?;
                    let gb_repo = gb_repository::Repository::open(
                        &self.local_data_dir,
                        &project_repository,
                        user.as_ref(),
                    )
                    .context("failed to open repository")?;

                    let gb_flush_path = project.path.join(".git/GB_FLUSH");
                    if gb_flush_path.exists() {
                        if let Err(err) = std::fs::remove_file(&gb_flush_path) {
                            tracing::error!(%project_id, path = %gb_flush_path.display(), "GB_FLUSH file delete error: {err}");
                        }

                        if let Some(current_session) = gb_repo
                            .get_current_session()
                            .context("failed to get current session")?
                        {
                            self.flush_session(project.id, &current_session).await?;
                        }
                    }
                }
                "HEAD" => {
                    let project_repository = open_projects_repository()?;
                    let head_ref = project_repository
                        .get_head()
                        .context("failed to get head")?;
                    let head_ref_name = head_ref.name().context("failed to get head name")?;
                    if head_ref_name.to_string() != "refs/heads/gitbutler/integration" {
                        let mut integration_reference = project_repository
                            .git_repository
                            .find_reference(&git::Refname::from(git::LocalRefname::new(
                                "gitbutler/integration",
                                None,
                            )))?;
                        integration_reference.delete()?;
                    }
                    if let Some(head) = head_ref.name() {
                        self.emit_app_event(Change::GitHead {
                            project_id,
                            head: head.to_string(),
                        })?;
                    }
                }
                "index" => {
                    self.emit_app_event(Change::GitIndex(project.id))?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

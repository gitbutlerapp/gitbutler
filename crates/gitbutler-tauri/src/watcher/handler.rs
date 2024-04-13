mod calculate_deltas;
mod index;
mod push_project_to_gitbutler;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use std::{path, time};

use anyhow::{bail, Context, Result};
use gitbutler_core::projects::ProjectId;
use gitbutler_core::sessions::SessionId;
use gitbutler_core::virtual_branches::VirtualBranches;
use gitbutler_core::{
    assets, deltas, gb_repository, git, project_repository, projects, reader, sessions, users,
    virtual_branches,
};
use governor::clock::QuantaClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use tauri::{AppHandle, Manager};
use tracing::instrument;

use super::events;
use crate::{analytics, events as app_events};

// NOTE: This is `Clone` as each incoming event is spawned onto a thread for processing.
#[derive(Clone)]
pub struct Handler {
    // The following fields our currently required state as we are running in the background
    // and access it as filesystem events are processed. It's still to be decided how granular it
    // should be, and I can imagine having a top-level `app` handle that keeps the application state of
    // the tauri app, assuming that such application would not be `Send + Sync` everywhere and thus would
    // need extra protection.
    users: users::Controller,
    analytics: analytics::Client,
    local_data_dir: path::PathBuf,
    projects: projects::Controller,
    vbranch_controller: virtual_branches::Controller,
    assets_proxy: assets::Proxy,
    /// A rate-limiter for the vbranch calculation.
    calc_vbranch_limit: Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock>>,
    sessions_db: sessions::Database,
    deltas_db: deltas::Database,

    /// A rate-limiter for the `is-ignored` computation
    recalc_all_limit: Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock>>,

    /// A function to send events - decoupled from app-handle for testing purposes.
    #[allow(clippy::type_complexity)]
    send_event: Arc<dyn Fn(&crate::events::Event) -> Result<()> + Send + Sync + 'static>,
}

impl Handler {
    pub fn from_app(app: &AppHandle) -> Result<Self, anyhow::Error> {
        let app_data_dir = app
            .path_resolver()
            .app_data_dir()
            .context("failed to get app data dir")?;
        let analytics = app
            .try_state::<analytics::Client>()
            .map_or(analytics::Client::default(), |client| {
                client.inner().clone()
            });
        let users = app.state::<users::Controller>().inner().clone();
        let projects = app.state::<projects::Controller>().inner().clone();
        let vbranches = app.state::<virtual_branches::Controller>().inner().clone();
        let assets_proxy = app.state::<assets::Proxy>().inner().clone();
        let sessions_db = app.state::<sessions::Database>().inner().clone();
        let deltas_db = app.state::<deltas::Database>().inner().clone();

        Ok(Handler::new(
            app_data_dir.clone(),
            analytics,
            users,
            projects,
            vbranches,
            assets_proxy,
            sessions_db,
            deltas_db,
            {
                let app = app.clone();
                move |event: &crate::events::Event| event.send(&app)
            },
        ))
    }
}

impl Handler {
    /// A constructor whose primary use is the test-suite.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        local_data_dir: PathBuf,
        analytics: analytics::Client,
        users: users::Controller,
        projects: projects::Controller,
        vbranch_controller: virtual_branches::Controller,
        assets_proxy: assets::Proxy,
        sessions_db: sessions::Database,
        deltas_db: deltas::Database,
        send_event: impl Fn(&crate::events::Event) -> Result<()> + Send + Sync + 'static,
    ) -> Self {
        let calc_vbranch_limit = {
            let quota = Quota::with_period(Duration::from_millis(100)).expect("valid quota");
            Arc::new(RateLimiter::direct(quota))
        };
        // There could be an application (e.g an IDE) which is constantly writing, so the threshold cant be too high
        let recalc_all_limit = {
            let quota = Quota::with_period(Duration::from_millis(5)).expect("valid quota");
            Arc::new(RateLimiter::direct(quota))
        };
        Handler {
            local_data_dir,
            analytics,
            users,
            projects,
            vbranch_controller,
            assets_proxy,
            calc_vbranch_limit,
            sessions_db,
            deltas_db,
            recalc_all_limit,
            send_event: Arc::new(send_event),
        }
    }

    /// Handle the events that come in from the filesystem, or the public API.
    #[instrument(skip(self, now), fields(event = %event), err(Debug))]
    pub(super) async fn handle(
        &self,
        event: events::InternalEvent,
        now: time::SystemTime,
    ) -> Result<()> {
        match event {
            events::InternalEvent::ProjectFilesChange(project_id, path) => {
                self.recalculate_everything(path, project_id).await
            }

            events::InternalEvent::GitFilesChange(project_id, paths) => self
                .git_files_change(paths, project_id)
                .await
                .context("failed to handle git file change event"),

            events::InternalEvent::PushGitbutlerData(project_id) => self
                .push_gb_data(project_id)
                .context("failed to push gitbutler data"),

            events::InternalEvent::FetchGitbutlerData(project_id) => self
                .fetch_gb_data(project_id, now)
                .await
                .context("failed to fetch gitbutler data"),

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
    fn emit_app_event(&self, event: &crate::events::Event) -> Result<()> {
        (self.send_event)(event).context("failed to send event")
    }

    fn emit_session_file(
        &self,
        project_id: ProjectId,
        session_id: SessionId,
        file_path: &Path,
        contents: Option<&reader::Content>,
    ) -> Result<()> {
        self.emit_app_event(&app_events::Event::file(
            project_id,
            session_id,
            &file_path.display().to_string(),
            contents,
        ))
    }
    fn send_analytics_event_none_blocking(&self, event: &analytics::Event) -> Result<()> {
        if let Some(user) = self.users.get_user().context("failed to get user")? {
            self.analytics
                .send_non_anonymous_event_nonblocking(&user, event);
        }
        Ok(())
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

        self.index_session(project_id, &session)?;

        let push_gb_data = tokio::task::spawn_blocking({
            let this = self.clone();
            move || this.push_gb_data(project_id)
        });
        self.push_project_to_gitbutler(project_id).await?;
        push_gb_data.await??;
        Ok(())
    }

    #[instrument(skip(self, project_id))]
    async fn calculate_virtual_branches(&self, project_id: ProjectId) -> Result<()> {
        if self.calc_vbranch_limit.check().is_err() {
            tracing::warn!("rate limited");
            return Ok(());
        }
        match self
            .vbranch_controller
            .list_virtual_branches(&project_id)
            .await
        {
            Ok((branches, _, skipped_files)) => {
                let branches = self.assets_proxy.proxy_virtual_branches(branches).await;
                self.emit_app_event(&app_events::Event::virtual_branches(
                    project_id,
                    &VirtualBranches {
                        branches,
                        skipped_files,
                    },
                ))
            }
            Err(err) if err.is::<virtual_branches::errors::VerifyError>() => Ok(()),
            Err(err) => Err(err.context("failed to list virtual branches").into()),
        }
    }

    /// NOTE: this is an honest non-async function, and it should stay that way to avoid
    ///       dealing with git2 repositories across await points, which aren't `Send`.
    fn push_gb_data(&self, project_id: ProjectId) -> Result<()> {
        let user = self.users.get_user()?;
        let project = self.projects.get(&project_id)?;
        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        gb_repo
            .push(user.as_ref())
            .context("failed to push gb repo")
    }

    pub async fn fetch_gb_data(&self, project_id: ProjectId, now: time::SystemTime) -> Result<()> {
        let user = self.users.get_user()?;
        let project = self
            .projects
            .get(&project_id)
            .context("failed to get project")?;

        if !project.api.as_ref().map(|api| api.sync).unwrap_or_default() {
            bail!("sync disabled");
        }

        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;
        let gb_repo = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open repository")?;

        let sessions_before_fetch = gb_repo
            .get_sessions_iterator()?
            .filter_map(Result::ok)
            .collect::<Vec<_>>();

        let policy = backoff::ExponentialBackoffBuilder::new()
            .with_max_elapsed_time(Some(time::Duration::from_secs(10 * 60)))
            .build();

        let fetch_result = backoff::retry(policy, || {
            gb_repo.fetch(user.as_ref()).map_err(|err| {
                match err {
                    gb_repository::RemoteError::Network => backoff::Error::permanent(err),
                    err @ gb_repository::RemoteError::Other(_) => {
                        tracing::warn!(%project_id, ?err, will_retry = true, "failed to fetch project data");
                        backoff::Error::transient(err)
                    }
                }
            })
        });
        let fetch_result = match fetch_result {
            Ok(()) => projects::FetchResult::Fetched { timestamp: now },
            Err(backoff::Error::Permanent(gb_repository::RemoteError::Network)) => {
                projects::FetchResult::Error {
                    timestamp: now,
                    error: "network error".to_string(),
                }
            }
            Err(error) => {
                tracing::error!(%project_id, ?error, will_retry=false, "failed to fetch gitbutler data");
                projects::FetchResult::Error {
                    timestamp: now,
                    error: error.to_string(),
                }
            }
        };

        self.projects
            .update(&projects::UpdateRequest {
                id: project_id,
                gitbutler_data_last_fetched: Some(fetch_result),
                ..Default::default()
            })
            .await
            .context("failed to update fetched result")?;

        let sessions_after_fetch = gb_repo.get_sessions_iterator()?.filter_map(Result::ok);
        let new_sessions = sessions_after_fetch.filter(|s| !sessions_before_fetch.contains(s));
        for session in new_sessions {
            self.index_session(project_id, &session)?;
        }
        Ok(())
    }

    #[instrument(skip(self, paths, project_id), fields(paths = paths.len()))]
    async fn recalculate_everything(
        &self,
        paths: Vec<PathBuf>,
        project_id: ProjectId,
    ) -> Result<()> {
        if self.recalc_all_limit.check().is_err() {
            tracing::warn!("rate limited");
            return Ok(());
        }
        let calc_deltas = tokio::task::spawn_blocking({
            let this = self.clone();
            move || this.calculate_deltas(paths, project_id)
        });
        self.calculate_virtual_branches(project_id).await?;
        calc_deltas.await??;
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
                    self.emit_app_event(&app_events::Event::git_fetch(project_id))?;
                    self.calculate_virtual_branches(project_id).await?;
                }
                "logs/HEAD" => {
                    self.emit_app_event(&app_events::Event::git_activity(project.id))?;
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
                        self.send_analytics_event_none_blocking(&analytics::Event::HeadChange {
                            project_id,
                            reference_name: head_ref_name.to_string(),
                        })?;
                        self.emit_app_event(&app_events::Event::git_head(
                            project_id,
                            &head.to_string(),
                        ))?;
                    }
                }
                "index" => {
                    self.emit_app_event(&app_events::Event::git_index(project.id))?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

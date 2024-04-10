mod calculate_deltas;
mod index;
mod push_project_to_gitbutler;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use std::{path, time};

use anyhow::{bail, Context, Result};
use gitbutler_core::projects::ProjectId;
use gitbutler_core::virtual_branches::VirtualBranches;
use gitbutler_core::{
    assets, deltas, gb_repository, git, project_repository, projects, sessions, users,
    virtual_branches,
};
use governor::clock::QuantaClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};
use tauri::{AppHandle, Manager};
use tracing::instrument;

use super::events;
use crate::{analytics, events as app_events};

// TODO(ST): remove `Clone` once the event-loop is gone.
#[derive(Clone)]
pub struct Handler {
    // TODO(ST): Review this comment as `core` is refactored, the state here should be affected.
    // The following fields our currently required state as we are running in the background
    // and access it as filesystem events are processed. It's still to be decided how granular it
    // should be, and I can imagine having a top-level `app` handle that keeps the application state of
    // the tauri app.
    users: users::Controller,
    client: analytics::Client,
    local_data_dir: path::PathBuf,
    projects: projects::Controller,
    vbranch_controller: virtual_branches::Controller,
    assets_proxy: assets::Proxy,
    /// A rate-limiter for the vbranch calculation.
    calc_vbranch_limit: Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock>>,
    sessions_database: sessions::Database,
    deltas_database: deltas::Database,

    /// A rate-limiter for the `is-ignored` computation
    is_ignored_limit: Arc<RateLimiter<NotKeyed, InMemoryState, QuantaClock>>,

    app_handle: tauri::AppHandle,
}

impl Handler {
    pub fn from_app(app: &AppHandle) -> Result<Self, anyhow::Error> {
        let app_data_dir = app
            .path_resolver()
            .app_data_dir()
            .context("failed to get app data dir")?;
        let client = app
            .try_state::<analytics::Client>()
            .map_or(analytics::Client::default(), |client| {
                client.inner().clone()
            });
        let users = app.state::<users::Controller>().inner().clone();
        let projects = app.state::<projects::Controller>().inner().clone();
        let vbranches = app.state::<virtual_branches::Controller>().inner().clone();
        let proxy = app.state::<assets::Proxy>().inner().clone();
        let calc_vbranch_limit = {
            let quota = Quota::with_period(Duration::from_millis(100)).expect("valid quota");
            Arc::new(RateLimiter::direct(quota))
        };
        // There could be an application (e.g an IDE) which is constantly writing, so the threshold cant be too high
        let is_ignored_limit = {
            let quota = Quota::with_period(Duration::from_millis(5)).expect("valid quota");
            Arc::new(RateLimiter::direct(quota))
        };

        let sessions_database = app.state::<sessions::Database>().inner().clone();
        let deltas_database = app.state::<deltas::Database>().inner().clone();

        Ok(Handler {
            local_data_dir: app_data_dir.clone(),
            client,
            users,
            projects,
            vbranch_controller: vbranches,
            assets_proxy: proxy,
            calc_vbranch_limit,
            sessions_database,
            deltas_database,
            is_ignored_limit,
            app_handle: app.clone(),
        })
    }
}

impl Handler {
    #[instrument(skip(self), fields(event = %event), level = "debug", err)]
    pub async fn handle(
        &self,
        event: &events::PrivateEvent,
        now: time::SystemTime,
    ) -> Result<Vec<events::PrivateEvent>> {
        match event {
            events::PrivateEvent::ProjectFileChange(project_id, path) => {
                Ok(vec![events::PrivateEvent::FilterIgnoredFiles(
                    *project_id,
                    path.clone(),
                )])
            }

            events::PrivateEvent::FilterIgnoredFiles(project_id, path) => self
                .is_ignored(path, project_id)
                .context("failed to handle filter ignored files event"),

            events::PrivateEvent::GitFileChange(project_id, path) => self
                .git_file_change(path, *project_id)
                .context("failed to handle git file change event"),

            events::PrivateEvent::PushGitbutlerData(project_id) => {
                self.push_gb_data(*project_id)
                    .context("failed to push gitbutler data")?;
                Ok(vec![])
            }

            events::PrivateEvent::PushProjectToGitbutler(project_id) => self
                .push_project_to_gitbutler(*project_id)
                .await
                .context("failed to push project to gitbutler"),

            events::PrivateEvent::FetchGitbutlerData(project_id) => self
                .fetch_gb_data(*project_id, now)
                .await
                .context("failed to fetch gitbutler data"),

            events::PrivateEvent::Flush(project_id, session) => self
                .flush_session(project_id, session)
                .context("failed to handle flush session event"),

            events::PrivateEvent::SessionFile((project_id, session_id, file_path, contents)) => {
                Ok(vec![events::PrivateEvent::Emit(app_events::Event::file(
                    *project_id,
                    *session_id,
                    &file_path.display().to_string(),
                    contents.as_ref(),
                ))])
            }

            events::PrivateEvent::SessionDelta((project_id, session_id, path, delta)) => {
                self.index_deltas(*project_id, *session_id, path, std::slice::from_ref(delta))
                    .context("failed to index deltas")?;

                Ok(vec![events::PrivateEvent::Emit(app_events::Event::deltas(
                    *project_id,
                    *session_id,
                    std::slice::from_ref(delta),
                    path,
                ))])
            }

            events::PrivateEvent::CalculateVirtualBranches(project_id) => self
                .calculate_virtual_branches(*project_id)
                .await
                .context("failed to handle virtual branch event"),

            events::PrivateEvent::CalculateDeltas(project_id, path) => {
                self.calculate_deltas(path, *project_id).context(format!(
                    "failed to handle session processing event: {:?}",
                    path.display()
                ))
            }

            events::PrivateEvent::Emit(event) => {
                event
                    .send(&self.app_handle)
                    .context("failed to send event")?;
                Ok(vec![])
            }

            events::PrivateEvent::Analytics(event) => {
                self.send_analytics_event_none_blocking(event)
                    .context("failed to handle analytics event")?;
                Ok(vec![])
            }

            events::PrivateEvent::Session(project_id, session) => self
                .index_session(*project_id, session)
                .context("failed to index session"),

            events::PrivateEvent::IndexAll(project_id) => self.reindex(*project_id),
        }
    }
}

impl Handler {
    fn send_analytics_event_none_blocking(&self, event: &analytics::Event) -> Result<()> {
        if let Some(user) = self.users.get_user().context("failed to get user")? {
            self.client
                .send_non_anonymous_event_nonblocking(&user, event);
        }
        Ok(())
    }

    fn flush_session(
        &self,
        project_id: &ProjectId,
        session: &sessions::Session,
    ) -> Result<Vec<events::PrivateEvent>> {
        let project = self
            .projects
            .get(project_id)
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

        Ok(vec![
            events::PrivateEvent::Session(*project_id, session),
            events::PrivateEvent::PushGitbutlerData(*project_id),
            events::PrivateEvent::PushProjectToGitbutler(*project_id),
        ])
    }

    async fn calculate_virtual_branches(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<events::PrivateEvent>> {
        if self.calc_vbranch_limit.check().is_err() {
            return Ok(vec![]);
        }
        match self
            .vbranch_controller
            .list_virtual_branches(&project_id)
            .await
        {
            Ok((branches, _, skipped_files)) => {
                let branches = self.assets_proxy.proxy_virtual_branches(branches).await;
                Ok(vec![events::PrivateEvent::Emit(
                    app_events::Event::virtual_branches(
                        project_id,
                        &VirtualBranches {
                            branches,
                            skipped_files,
                        },
                    ),
                )])
            }
            Err(err) if err.is::<virtual_branches::errors::VerifyError>() => Ok(vec![]),
            Err(err) => Err(err.context("failed to list virtual branches").into()),
        }
    }

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

    async fn fetch_gb_data(
        &self,
        project_id: ProjectId,
        now: time::SystemTime,
    ) -> Result<Vec<events::PrivateEvent>> {
        Self::fetch_gb_data_pure(
            &self.local_data_dir,
            &self.projects,
            &self.users,
            project_id,
            now,
        )
        .await
    }

    // TODO(ST): figure out if this is needed, it's going to be very slow. The file monitor already filters,
    //           however, it uses a cached project which might not see changes to the .gitignore files.
    //           so opening a fresh repo (or doing the minimal work to get there) seems to be required at first,
    //           but one should handle all paths at once.
    fn is_ignored<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        project_id: &ProjectId,
    ) -> Result<Vec<events::PrivateEvent>> {
        if self.is_ignored_limit.check().is_err() {
            return Ok(vec![]);
        }
        let project = self
            .projects
            .get(project_id)
            .context("failed to get project")?;
        let project_repository = project_repository::Repository::open(&project)
            .with_context(|| "failed to open project repository for project")?;

        if project_repository
            .is_path_ignored(path.as_ref())
            .unwrap_or(false)
        {
            Ok(vec![])
        } else {
            Ok(vec![
                events::PrivateEvent::CalculateDeltas(*project_id, path.as_ref().to_path_buf()),
                events::PrivateEvent::CalculateVirtualBranches(*project_id),
            ])
        }
    }

    fn git_file_change<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        project_id: ProjectId,
    ) -> Result<Vec<events::PrivateEvent>> {
        Self::git_file_change_pure(
            &self.local_data_dir,
            &self.projects,
            &self.users,
            path,
            project_id,
        )
    }
}

/// Functions are used for unbundling to facilitate tests.
impl Handler {
    pub async fn fetch_gb_data_pure(
        local_data_dir: &Path,
        projects: &projects::Controller,
        users: &users::Controller,
        project_id: ProjectId,
        now: time::SystemTime,
    ) -> Result<Vec<events::PrivateEvent>> {
        let user = users.get_user()?;
        let project = projects.get(&project_id).context("failed to get project")?;

        if !project.api.as_ref().map(|api| api.sync).unwrap_or_default() {
            bail!("sync disabled");
        }

        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;
        let gb_repo =
            gb_repository::Repository::open(local_data_dir, &project_repository, user.as_ref())
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

        projects
            .update(&projects::UpdateRequest {
                id: project_id,
                gitbutler_data_last_fetched: Some(fetch_result),
                ..Default::default()
            })
            .await
            .context("failed to update fetched result")?;

        let sessions_after_fetch = gb_repo.get_sessions_iterator()?.filter_map(Result::ok);
        let new_sessions = sessions_after_fetch.filter(|s| !sessions_before_fetch.contains(s));
        let events = new_sessions
            .map(|session| events::PrivateEvent::Session(project_id, session))
            .collect::<Vec<_>>();

        Ok(events)
    }

    pub fn git_file_change_pure<P: AsRef<std::path::Path>>(
        local_data_dir: &Path,
        projects: &projects::Controller,
        users: &users::Controller,
        path: P,
        project_id: ProjectId,
    ) -> Result<Vec<events::PrivateEvent>> {
        let project = projects.get(&project_id).context("failed to get project")?;
        let project_repository = project_repository::Repository::open(&project)
            .context("failed to open project repository for project")?;

        let Some(file_name) = path.as_ref().to_str() else {
            return Ok(vec![]);
        };
        match file_name {
            "FETCH_HEAD" => Ok(vec![
                events::PrivateEvent::Emit(app_events::Event::git_fetch(project_id)),
                events::PrivateEvent::CalculateVirtualBranches(project_id),
            ]),
            "logs/HEAD" => Ok(vec![events::PrivateEvent::Emit(
                app_events::Event::git_activity(project.id),
            )]),
            "GB_FLUSH" => {
                let user = users.get_user()?;
                let gb_repo = gb_repository::Repository::open(
                    local_data_dir,
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
                        return Ok(vec![events::PrivateEvent::Flush(
                            project.id,
                            current_session,
                        )]);
                    }
                }
                Ok(vec![])
            }
            "HEAD" => {
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
                    Ok(vec![
                        events::PrivateEvent::Analytics(analytics::Event::HeadChange {
                            project_id,
                            reference_name: head_ref_name.to_string(),
                        }),
                        events::PrivateEvent::Emit(app_events::Event::git_head(
                            project_id,
                            &head.to_string(),
                        )),
                    ])
                } else {
                    Ok(vec![])
                }
            }
            "index" => Ok(vec![events::PrivateEvent::Emit(
                app_events::Event::git_index(project.id),
            )]),
            _ => Ok(vec![]),
        }
    }
}

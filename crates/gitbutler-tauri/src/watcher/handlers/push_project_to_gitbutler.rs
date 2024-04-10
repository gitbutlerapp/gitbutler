use std::path::PathBuf;
use std::{path, sync::Arc, time};

use anyhow::{Context, Result};
use gitbutler_core::{
    gb_repository,
    git::{self, Oid, Repository},
    project_repository,
    projects::{self, CodePushState, ProjectId},
    users,
};
use itertools::Itertools;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use super::events;

#[derive(Clone)]
pub struct Handler {
    inner: Arc<Mutex<State>>,
}

impl Handler {
    pub fn from_app(app: &AppHandle, app_data_dir: impl Into<PathBuf>) -> Self {
        let projects = app.state::<projects::Controller>().inner().clone();
        let users = app.state::<users::Controller>().inner().clone();
        Handler::new(app_data_dir.into(), projects, users, 1000)
    }
}

impl Handler {
    pub fn new(
        local_data_dir: path::PathBuf,
        project_store: projects::Controller,
        users: users::Controller,
        batch_size: usize,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(State {
                local_data_dir,
                project_store,
                users,
                batch_size,
            })),
        }
    }

    pub async fn handle(&self, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        if let Ok(state) = self.inner.try_lock() {
            Self::handle_inner(&state, project_id).await
        } else {
            Ok(vec![])
        }
    }

    async fn handle_inner(state: &State, project_id: &ProjectId) -> Result<Vec<events::Event>> {
        let project = state
            .project_store
            .get(project_id)
            .context("failed to get project")?;

        if !project.is_sync_enabled() || !project.has_code_url() {
            return Ok(vec![]);
        }

        let user = state.users.get_user()?;
        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;

        let gb_code_last_commit = project
            .gitbutler_code_push_state
            .as_ref()
            .map(|state| &state.id)
            .copied();

        let gb_repository = gb_repository::Repository::open(
            &state.local_data_dir,
            &project_repository,
            user.as_ref(),
        )?;
        let default_target = gb_repository
            .default_target()
            .context("failed to open gb repo")?
            .context("failed to get default target")?;

        let target_changed = !gb_code_last_commit
            .map(|id| id == default_target.sha)
            .unwrap_or_default();

        if target_changed {
            match Self::push_target(
                state,
                &project_repository,
                &default_target,
                gb_code_last_commit,
                project_id,
                &user,
            )
            .await
            {
                Ok(()) => {}
                Err(project_repository::RemoteError::Network) => return Ok(vec![]),
                Err(err) => return Err(err).context("failed to push"),
            };
        }

        match push_all_refs(&project_repository, &user, project_id) {
            Ok(()) => {}
            Err(project_repository::RemoteError::Network) => return Ok(vec![]),
            Err(err) => return Err(err).context("failed to push"),
        };

        // make sure last push time is updated
        Self::update_project(state, project_id, &default_target.sha).await?;

        Ok(vec![])
    }

    async fn push_target(
        state: &State,
        project_repository: &project_repository::Repository,
        default_target: &gitbutler_core::virtual_branches::target::Target,
        gb_code_last_commit: Option<Oid>,
        project_id: &gitbutler_core::id::Id<projects::Project>,
        user: &Option<users::User>,
    ) -> Result<(), project_repository::RemoteError> {
        let ids = batch_rev_walk(
            &project_repository.git_repository,
            state.batch_size,
            default_target.sha,
            gb_code_last_commit,
        )?;

        tracing::info!(
            %project_id,
            batches=%ids.len(),
            "batches left to push",
        );

        let id_count = &ids.len();

        for (idx, id) in ids.iter().enumerate().rev() {
            let refspec = format!("+{}:refs/push-tmp/{}", id, project_id);

            project_repository.push_to_gitbutler_server(user.as_ref(), &[&refspec])?;

            Self::update_project(state, project_id, id).await?;

            tracing::info!(
                %project_id,
                i = id_count.saturating_sub(idx),
                total = id_count,
                "project batch pushed",
            );
        }

        project_repository.push_to_gitbutler_server(
            user.as_ref(),
            &[&format!("+{}:refs/{}", default_target.sha, project_id)],
        )?;

        //TODO: remove push-tmp ref

        tracing::info!(
            %project_id,
            "project target ref fully pushed",
        );

        Ok(())
    }

    async fn update_project(
        state: &State,
        project_id: &gitbutler_core::id::Id<projects::Project>,
        id: &Oid,
    ) -> Result<(), project_repository::RemoteError> {
        state
            .project_store
            .update(&projects::UpdateRequest {
                id: *project_id,
                gitbutler_code_push_state: Some(CodePushState {
                    id: *id,
                    timestamp: time::SystemTime::now(),
                }),
                ..Default::default()
            })
            .await
            .context("failed to update last push")?;

        Ok(())
    }
}

struct State {
    local_data_dir: path::PathBuf,
    project_store: projects::Controller,
    users: users::Controller,
    batch_size: usize,
}

fn push_all_refs(
    project_repository: &project_repository::Repository,
    user: &Option<users::User>,
    project_id: &gitbutler_core::id::Id<projects::Project>,
) -> Result<(), project_repository::RemoteError> {
    let gb_references = collect_refs(project_repository)?;

    let all_refs = gb_references
        .iter()
        .filter(|r| {
            matches!(
                r,
                git::Refname::Remote(_) | git::Refname::Virtual(_) | git::Refname::Local(_)
            )
        })
        .map(|r| format!("+{}:{}", r, r))
        .collect::<Vec<_>>();

    let all_refs = all_refs.iter().map(String::as_str).collect::<Vec<_>>();

    let anything_pushed =
        project_repository.push_to_gitbutler_server(user.as_ref(), all_refs.as_slice())?;

    if anything_pushed {
        tracing::info!(
            %project_id,
            "refs pushed",
        );
    }

    Ok(())
}

fn collect_refs(
    project_repository: &project_repository::Repository,
) -> anyhow::Result<Vec<git::Refname>> {
    Ok(project_repository
        .git_repository
        .references_glob("refs/*")?
        .flatten()
        .filter_map(|r| r.name())
        .collect::<Vec<_>>())
}

fn batch_rev_walk(
    repo: &Repository,
    batch_size: usize,
    from: Oid,
    until: Option<Oid>,
) -> Result<Vec<Oid>> {
    let mut revwalk = repo.revwalk().context("failed to create revwalk")?;
    revwalk
        .push(from.into())
        .context(format!("failed to push {}", from))?;

    if let Some(oid) = until {
        revwalk
            .hide(oid.into())
            .context(format!("failed to hide {}", oid))?;
    }
    let mut oids = Vec::new();
    oids.push(from);
    for batch in &revwalk.chunks(batch_size) {
        if let Some(oid) = batch.last() {
            let oid = oid.context("failed to get oid")?;
            if oid != from.into() {
                oids.push(oid.into());
            }
        }
    }
    Ok(oids)
}

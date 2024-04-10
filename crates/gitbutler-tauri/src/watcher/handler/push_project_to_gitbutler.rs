use std::path::Path;
use std::time;

use anyhow::{Context, Result};
use gitbutler_core::id::Id;
use gitbutler_core::{
    gb_repository,
    git::{self, Oid, Repository},
    project_repository,
    projects::{self, CodePushState, ProjectId},
    users,
};
use itertools::Itertools;

impl super::Handler {
    pub(super) async fn push_project_to_gitbutler(&self, project_id: ProjectId) -> Result<()> {
        Self::push_project_to_gitbutler_pure(
            &self.local_data_dir,
            &self.projects,
            &self.users,
            project_id,
            1000,
        )
        .await
    }
}

/// Currently required to make functionality testable without requiring a `Handler` with all of its state.
impl super::Handler {
    pub async fn push_project_to_gitbutler_pure(
        local_data_dir: &Path,
        projects: &projects::Controller,
        users: &users::Controller,
        project_id: ProjectId,
        batch_size: usize,
    ) -> Result<()> {
        let project = projects.get(&project_id).context("failed to get project")?;

        if !project.is_sync_enabled() || !project.has_code_url() {
            return Ok(());
        }

        let user = users.get_user()?;
        let project_repository =
            project_repository::Repository::open(&project).context("failed to open repository")?;
        let gb_code_last_commit = project
            .gitbutler_code_push_state
            .as_ref()
            .map(|state| &state.id)
            .copied();
        let gb_repository =
            gb_repository::Repository::open(local_data_dir, &project_repository, user.as_ref())?;
        let default_target = gb_repository
            .default_target()
            .context("failed to open gb repo")?
            .context("failed to get default target")?;

        let target_changed = gb_code_last_commit.map_or(true, |id| id != default_target.sha);
        if target_changed {
            match Self::push_target(
                projects,
                &project_repository,
                &default_target,
                gb_code_last_commit,
                project_id,
                user.as_ref(),
                batch_size,
            )
            .await
            {
                Ok(()) => {}
                Err(project_repository::RemoteError::Network) => return Ok(()),
                Err(err) => return Err(err).context("failed to push"),
            };
        }

        tokio::task::spawn_blocking(move || -> Result<()> {
            match push_all_refs(&project_repository, user.as_ref(), project_id) {
                Ok(()) => Ok(()),
                Err(project_repository::RemoteError::Network) => Ok(()),
                Err(err) => Err(err).context("failed to push"),
            }
        })
        .await??;

        // make sure last push time is updated
        Self::update_project(projects, project_id, default_target.sha).await?;
        Ok(())
    }

    async fn update_project(
        projects: &projects::Controller,
        project_id: Id<projects::Project>,
        id: Oid,
    ) -> Result<(), project_repository::RemoteError> {
        projects
            .update(&projects::UpdateRequest {
                id: project_id,
                gitbutler_code_push_state: Some(CodePushState {
                    id,
                    timestamp: time::SystemTime::now(),
                }),
                ..Default::default()
            })
            .await
            .context("failed to update last push")?;
        Ok(())
    }

    async fn push_target(
        projects: &projects::Controller,
        project_repository: &project_repository::Repository,
        default_target: &gitbutler_core::virtual_branches::target::Target,
        gb_code_last_commit: Option<Oid>,
        project_id: Id<projects::Project>,
        user: Option<&users::User>,
        batch_size: usize,
    ) -> Result<(), project_repository::RemoteError> {
        let ids = batch_rev_walk(
            &project_repository.git_repository,
            batch_size,
            default_target.sha,
            gb_code_last_commit,
        )?;

        tracing::info!(
            %project_id,
            batches=%ids.len(),
            "batches left to push",
        );

        let id_count = ids.len();
        for (idx, id) in ids.iter().enumerate().rev() {
            let refspec = format!("+{}:refs/push-tmp/{}", id, project_id);

            project_repository.push_to_gitbutler_server(user, &[&refspec])?;
            Self::update_project(projects, project_id, *id).await?;

            tracing::info!(
                %project_id,
                i = id_count.saturating_sub(idx),
                total = id_count,
                "project batch pushed",
            );
        }

        project_repository.push_to_gitbutler_server(
            user,
            &[&format!("+{}:refs/{}", default_target.sha, project_id)],
        )?;

        //TODO: remove push-tmp ref
        tracing::info!(
            %project_id,
            "project target ref fully pushed",
        );
        Ok(())
    }
}

fn push_all_refs(
    project_repository: &project_repository::Repository,
    user: Option<&users::User>,
    project_id: Id<projects::Project>,
) -> Result<(), project_repository::RemoteError> {
    let gb_references = collect_refs(project_repository)?;
    let all_refs: Vec<_> = gb_references
        .iter()
        .filter(|r| {
            matches!(
                r,
                git::Refname::Remote(_) | git::Refname::Virtual(_) | git::Refname::Local(_)
            )
        })
        .map(|r| format!("+{}:{}", r, r))
        .collect();

    let all_refs: Vec<_> = all_refs.iter().map(String::as_str).collect();
    let anything_pushed = project_repository.push_to_gitbutler_server(user, &all_refs)?;
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

    let from = from.into();
    for batch in &revwalk.chunks(batch_size) {
        let Some(oid) = batch.last() else { continue };
        let oid = oid.context("failed to get oid")?;
        if oid != from {
            oids.push(oid.into());
        }
    }
    Ok(oids)
}

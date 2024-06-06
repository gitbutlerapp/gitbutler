use std::time;

use crate::id::Id;
use crate::{
    git::{self},
    project_repository,
    projects::{self, CodePushState},
    users,
};
use anyhow::{Context, Result};
use itertools::Itertools;

pub async fn sync_with_gitbutler(
    project_repository: &project_repository::Repository,
    user: &users::User,
    projects: &projects::Controller,
) -> Result<()> {
    let project = project_repository.project();
    let vb_state = project.virtual_branches();
    let default_target = vb_state.get_default_target()?;
    let gb_code_last_commit = project
        .gitbutler_code_push_state
        .as_ref()
        .map(|state| &state.id)
        .copied();

    // Push target
    push_target(
        projects,
        project_repository,
        &default_target,
        gb_code_last_commit,
        project.id,
        user,
        12,
    )
    .await?;

    // Push all refs
    push_all_refs(project_repository, user, project.id)?;

    // Push Oplog head
    let oplog_refspec = project_repository
        .project()
        .oplog_head()?
        .map(|sha| format!("+{}:refs/gitbutler/oplog/oplog", sha));

    if let Some(oplog_refspec) = oplog_refspec {
        let x = project_repository.push_to_gitbutler_server(Some(user), &[&oplog_refspec]);
        println!("\n\n\nHERE: {:?}", x?);
    }

    Ok(())
}

async fn push_target(
    projects: &projects::Controller,
    project_repository: &project_repository::Repository,
    default_target: &crate::virtual_branches::target::Target,
    gb_code_last_commit: Option<git2::Oid>,
    project_id: Id<projects::Project>,
    user: &users::User,
    batch_size: usize,
) -> Result<()> {
    let ids = batch_rev_walk(
        project_repository.repo(),
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

        project_repository.push_to_gitbutler_server(Some(user), &[&refspec])?;
        update_project(projects, project_id, *id).await?;

        tracing::info!(
            %project_id,
            i = id_count.saturating_sub(idx),
            total = id_count,
            "project batch pushed",
        );
    }

    project_repository.push_to_gitbutler_server(
        Some(user),
        &[&format!("+{}:refs/{}", default_target.sha, project_id)],
    )?;

    //TODO: remove push-tmp ref
    tracing::info!(
        %project_id,
        "project target ref fully pushed",
    );
    Ok(())
}

fn batch_rev_walk(
    repo: &git2::Repository,
    batch_size: usize,
    from: git2::Oid,
    until: Option<git2::Oid>,
) -> Result<Vec<git2::Oid>> {
    let mut revwalk = repo.revwalk().context("failed to create revwalk")?;
    revwalk
        .push(from)
        .context(format!("failed to push {}", from))?;
    if let Some(oid) = until {
        revwalk
            .hide(oid)
            .context(format!("failed to hide {}", oid))?;
    }
    let mut oids = Vec::new();
    oids.push(from);

    for batch in &revwalk.chunks(batch_size) {
        let Some(oid) = batch.last() else { continue };
        let oid = oid.context("failed to get oid")?;
        if oid != from {
            oids.push(oid);
        }
    }
    Ok(oids)
}

fn collect_refs(
    project_repository: &project_repository::Repository,
) -> anyhow::Result<Vec<git::Refname>> {
    Ok(project_repository
        .repo()
        .references_glob("refs/*")?
        .flatten()
        .filter_map(|r| {
            r.name()
                .map(|name| name.parse().expect("libgit2 provides valid refnames"))
        })
        .collect::<Vec<_>>())
}

fn push_all_refs(
    project_repository: &project_repository::Repository,
    user: &users::User,
    project_id: Id<projects::Project>,
) -> Result<()> {
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

    let anything_pushed = project_repository.push_to_gitbutler_server(Some(user), &all_refs)?;
    if anything_pushed {
        tracing::info!(
            %project_id,
            "refs pushed",
        );
    }
    Ok(())
}
async fn update_project(
    projects: &projects::Controller,
    project_id: Id<projects::Project>,
    id: git2::Oid,
) -> Result<()> {
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

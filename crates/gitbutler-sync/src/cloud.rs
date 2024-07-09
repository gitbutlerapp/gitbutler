use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::time;

use anyhow::{anyhow, Context, Result};
use gitbutler_branch::target::Target;
use gitbutler_branchstate::VirtualBranchesAccess;
use gitbutler_command_context::ProjectRepository;
use gitbutler_core::git::Url;
use gitbutler_error::error::Code;
use gitbutler_id::id::Id;
use gitbutler_oplog::oplog::Oplog;
use gitbutler_project as projects;
use gitbutler_project::{CodePushState, Project};
use gitbutler_reference::Refname;
use gitbutler_user as users;
use itertools::Itertools;

pub async fn sync_with_gitbutler(
    project_repository: &ProjectRepository,
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
        let x = push_to_gitbutler_server(project_repository, Some(user), &[&oplog_refspec]);
        println!("\n\n\nHERE: {:?}", x?);
    }

    Ok(())
}

async fn push_target(
    projects: &projects::Controller,
    project_repository: &ProjectRepository,
    default_target: &Target,
    gb_code_last_commit: Option<git2::Oid>,
    project_id: Id<Project>,
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

        push_to_gitbutler_server(project_repository, Some(user), &[&refspec])?;
        update_project(projects, project_id, *id).await?;

        tracing::info!(
            %project_id,
            i = id_count.saturating_sub(idx),
            total = id_count,
            "project batch pushed",
        );
    }

    push_to_gitbutler_server(
        project_repository,
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

fn collect_refs(project_repository: &ProjectRepository) -> anyhow::Result<Vec<Refname>> {
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
    project_repository: &ProjectRepository,
    user: &users::User,
    project_id: Id<projects::Project>,
) -> Result<()> {
    let gb_references = collect_refs(project_repository)?;
    let all_refs: Vec<_> = gb_references
        .iter()
        .filter(|r| {
            matches!(
                r,
                Refname::Remote(_) | Refname::Virtual(_) | Refname::Local(_)
            )
        })
        .map(|r| format!("+{}:{}", r, r))
        .collect();

    let all_refs: Vec<_> = all_refs.iter().map(String::as_str).collect();

    let anything_pushed = push_to_gitbutler_server(project_repository, Some(user), &all_refs)?;
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

fn push_to_gitbutler_server(
    project_repo: &ProjectRepository,
    user: Option<&users::User>,
    ref_specs: &[&str],
) -> Result<bool> {
    let project = project_repo.project();
    let url = project
        .api
        .as_ref()
        .context("api not set")?
        .code_git_url
        .as_ref()
        .context("code_git_url not set")?
        .as_str()
        .parse::<Url>()?;

    tracing::debug!(
        project_id = %project.id,
        %url,
        "pushing code to gb repo",
    );

    let user = user
        .context("need user to push to gitbutler")
        .context(Code::ProjectGitAuth)?;
    let access_token = user.access_token()?;

    let mut callbacks = git2::RemoteCallbacks::new();
    if project.omit_certificate_check.unwrap_or(false) {
        callbacks.certificate_check(|_, _| Ok(git2::CertificateCheckStatus::CertificateOk));
    }
    let bytes_pushed = Arc::new(AtomicUsize::new(0));
    let total_objects = Arc::new(AtomicUsize::new(0));
    {
        let byte_counter = Arc::<AtomicUsize>::clone(&bytes_pushed);
        let total_counter = Arc::<AtomicUsize>::clone(&total_objects);
        callbacks.push_transfer_progress(move |_current, total, bytes| {
            byte_counter.store(bytes, std::sync::atomic::Ordering::Relaxed);
            total_counter.store(total, std::sync::atomic::Ordering::Relaxed);
        });
    }

    let mut push_options = git2::PushOptions::new();
    push_options.remote_callbacks(callbacks);
    let auth_header = format!("Authorization: {}", access_token.0);
    let headers = &[auth_header.as_str()];
    push_options.custom_headers(headers);

    let mut remote = project_repo.repo().remote_anonymous(&url.to_string())?;

    remote
        .push(ref_specs, Some(&mut push_options))
        .map_err(|err| match err.class() {
            git2::ErrorClass::Net => anyhow!("network failed"),
            _ => match err.code() {
                git2::ErrorCode::Auth => anyhow!("authentication failed")
                    .context(Code::ProjectGitAuth)
                    .context(err),
                _ => anyhow!("push failed"),
            },
        })?;

    let bytes_pushed = bytes_pushed.load(std::sync::atomic::Ordering::Relaxed);
    let total_objects_pushed = total_objects.load(std::sync::atomic::Ordering::Relaxed);

    tracing::debug!(
        project_id = %project.id,
        ref_spec = ref_specs.join(" "),
        bytes = bytes_pushed,
        objects = total_objects_pushed,
        "pushed to gb repo tmp ref",
    );

    Ok(total_objects_pushed > 0)
}

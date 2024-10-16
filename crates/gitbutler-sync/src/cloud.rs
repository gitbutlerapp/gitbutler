use std::{
    sync::{atomic::AtomicUsize, Arc},
    time,
};

use anyhow::{anyhow, Context, Result};
use gitbutler_command_context::CommandContext;
use gitbutler_error::error::Code;
use gitbutler_id::id::Id;
use gitbutler_oplog::{/*entry::SnapshotDetails,*/ OplogExt};
use gitbutler_project as projects;
use gitbutler_project::{CodePushState, Project};
use gitbutler_reference::Refname;
use gitbutler_stack::{Target, VirtualBranchesHandle};
use gitbutler_url::Url;
use gitbutler_user as users;
use itertools::Itertools;

// pub fn take_synced_snapshot(project: &Project, user: &users::User) -> Result<git2::Oid> {
//     let command_context = CommandContext::open(project)?;
//     project.create_snapshot(SnapshotDetails::new(), perm)
// }

/// Pushes the repository to the GitButler remote
pub fn push_repo(
    ctx: &CommandContext,
    user: &users::User,
    projects: &projects::Controller,
) -> Result<()> {
    let project = ctx.project();
    let vb_state = VirtualBranchesHandle::new(project.gb_dir());
    let default_target = vb_state.get_default_target()?;
    let gb_code_last_commit = project
        .gitbutler_code_push_state
        .as_ref()
        .map(|state| &state.id)
        .copied();

    // Push target
    push_target(
        projects,
        ctx,
        &default_target,
        gb_code_last_commit,
        project.id,
        user,
        12,
    )?;

    // Push all refs
    push_all_refs(ctx, user, project.id)?;
    Ok(())
}

/// Pushes the Oplog head to GitButler server
pub fn push_oplog(ctx: &CommandContext, user: &users::User) -> Result<()> {
    // Push Oplog head
    let oplog_refspec = ctx
        .project()
        .oplog_head()?
        .map(|sha| format!("+{}:refs/gitbutler/oplog", sha));

    if let Some(oplog_refspec) = oplog_refspec {
        push_to_gitbutler_server(
            ctx,
            Some(user),
            &[&oplog_refspec],
            remote(ctx, RemoteKind::Oplog)?,
        )?;
    }
    Ok(())
}

fn push_target(
    projects: &projects::Controller,
    ctx: &CommandContext,
    default_target: &Target,
    gb_code_last_commit: Option<git2::Oid>,
    project_id: Id<Project>,
    user: &users::User,
    batch_size: usize,
) -> Result<()> {
    let ids = batch_rev_walk(
        ctx.repository(),
        batch_size,
        default_target.sha,
        gb_code_last_commit,
    )?;

    tracing::info!(
        %project_id,
        batches=%ids.len(),
        "batches left to push",
    );

    let remote = remote(ctx, RemoteKind::Code)?;
    let id_count = ids.len();
    for (idx, id) in ids.iter().enumerate().rev() {
        let refspec = format!("+{}:refs/push-tmp/{}", id, project_id);

        push_to_gitbutler_server(ctx, Some(user), &[&refspec], remote.clone())?;
        update_project(projects, project_id, *id)?;

        tracing::info!(
            %project_id,
            i = id_count.saturating_sub(idx),
            total = id_count,
            "project batch pushed",
        );
    }

    push_to_gitbutler_server(
        ctx,
        Some(user),
        &[&format!("+{}:refs/{}", default_target.sha, project_id)],
        remote.clone(),
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

fn collect_refs(ctx: &CommandContext) -> anyhow::Result<Vec<Refname>> {
    Ok(ctx
        .repository()
        .references_glob("refs/*")?
        .flatten()
        .filter_map(|r| {
            r.name()
                .map(|name| name.parse().expect("libgit2 provides valid refnames"))
        })
        .collect::<Vec<_>>())
}

fn push_all_refs(
    ctx: &CommandContext,
    user: &users::User,
    project_id: Id<projects::Project>,
) -> Result<()> {
    let gb_references = collect_refs(ctx)?;
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

    let anything_pushed =
        push_to_gitbutler_server(ctx, Some(user), &all_refs, remote(ctx, RemoteKind::Code)?)?;
    if anything_pushed {
        tracing::info!(
            %project_id,
            "refs pushed",
        );
    }
    Ok(())
}
fn update_project(
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
        .context("failed to update last push")?;
    Ok(())
}

fn push_to_gitbutler_server(
    ctx: &CommandContext,
    user: Option<&users::User>,
    ref_specs: &[&str],
    mut remote: git2::Remote,
) -> Result<bool> {
    let project = ctx.project();

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

enum RemoteKind {
    Code,
    Oplog,
}
fn remote(ctx: &CommandContext, kind: RemoteKind) -> Result<git2::Remote> {
    let api_project = ctx.project().api.as_ref().context("api not set")?;
    let url = match kind {
        RemoteKind::Code => {
            let url = api_project
                .code_git_url
                .as_ref()
                .context("code_git_url not set")?;
            url.as_str().parse::<Url>()
        }
        RemoteKind::Oplog => api_project.git_url.as_str().parse::<Url>(),
    }?;
    ctx.repository()
        .remote_anonymous(&url.to_string())
        .map_err(Into::into)
}

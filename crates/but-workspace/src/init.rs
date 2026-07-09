//! Project initialization: set the default target without entering the managed workspace.
//!
//! This is the replacement for the legacy `set_base_branch()`, which coupled setting the
//! target with moving the user onto `refs/heads/gitbutler/workspace`. Now that the
//! application can operate on any branch, the target can be set at any time.

use anyhow::{Context as _, Result, bail};
use but_core::{
    RefMetadata,
    git_config::{edit_repo_config, ensure_config_value},
    ref_metadata::ProjectMeta,
};

/// Make `target_ref` the project's default target and initialize project metadata,
/// without changing the currently checked out branch.
///
/// This performs the metadata-only parts of the legacy `set_base_branch()`:
///
/// * repair partially migrated target metadata and persist it if it changed,
/// * store the target as [`ProjectMeta`] via [`ProjectMeta::persist()`], which also
///   back-fills the legacy metadata in `meta`,
/// * set `log.excludeDecoration = refs/gitbutler` in the repository-local Git config.
///
/// The target commit id is only computed - as the merge-base between `HEAD` and
/// `target_ref` - if it isn't already set; an existing value is never overwritten.
/// `push_remote`, if `Some`, is validated and stored; if `None`, an existing push remote
/// is kept as is.
///
/// Unlike `set_base_branch()`, this neither creates stacks, nor updates the workspace
/// commit, nor checks anything out. The caller is expected to hold exclusive worktree
/// access, and to invalidate any cached workspace projection afterwards.
pub fn set_target_ref_and_init_project(
    repo: &gix::Repository,
    meta: &mut impl RefMetadata,
    target_ref: &gix::refs::FullNameRef,
    push_remote: Option<String>,
) -> Result<()> {
    let project_meta = match ProjectMeta::resolve(repo, &*meta) {
        Ok(project_meta) => {
            let repaired =
                but_core::ref_metadata::repair_target_metadata_for_migration(&project_meta, repo);
            if repaired != project_meta {
                repaired.clone().persist(repo, meta)?;
            }
            Some(repaired)
        }
        Err(_) => None,
    };

    if target_ref.category() != Some(gix::refs::Category::RemoteBranch) {
        bail!(
            "target ref '{}' must be a remote tracking branch",
            target_ref.as_bstr()
        );
    }

    let target_head = repo
        .try_find_reference(target_ref)?
        .with_context(|| format!("remote branch '{}' not found", target_ref.as_bstr()))?
        .peel_to_commit()
        .with_context(|| format!("failed to peel branch '{}' to commit", target_ref.as_bstr()))?
        .id;

    // Reject targets whose remote isn't configured - reads like the base-branch data
    // would fail on them later.
    let remote_names = repo.remote_names();
    let (remote_name, _short_name) =
        but_core::extract_remote_name_and_short_name(target_ref, &remote_names).with_context(
            || {
                format!(
                    "failed to determine remote for branch '{}'",
                    target_ref.as_bstr()
                )
            },
        )?;
    repo.find_remote(remote_name.as_str())
        .with_context(|| {
            format!(
                "failed to find remote for branch '{}'",
                target_ref.as_bstr()
            )
        })?
        .url(gix::remote::Direction::Fetch)
        .with_context(|| format!("failed to get remote url for '{remote_name}'"))?;

    let sha = match project_meta.as_ref().and_then(|meta| meta.target_commit_id) {
        Some(existing) => existing,
        None => {
            let head_commit = repo
                .head()
                .context("Failed to get HEAD reference")?
                .peel_to_commit()
                .context("Failed to peel HEAD reference to commit")?
                .id;
            repo.merge_base(head_commit, target_head)
                .with_context(|| {
                    format!(
                        "Failed to calculate merge base between {head_commit} and {target_head}"
                    )
                })?
                .detach()
        }
    };

    let push_remote = match push_remote {
        Some(name) => {
            repo.find_remote(name.as_str())
                .with_context(|| format!("failed to find remote {name}"))?;
            Some(name)
        }
        // Unlike the legacy `set_base_branch()`, keep an existing push remote instead of
        // clearing it - the target may be (re-)set at any time.
        None => project_meta.and_then(|meta| meta.push_remote),
    };

    ProjectMeta {
        target_ref: Some(target_ref.to_owned()),
        target_commit_id: Some(sha),
        push_remote,
    }
    .persist(repo, meta)?;

    edit_repo_config(repo, gix::config::Source::Local, |config| {
        ensure_config_value(config, "log.excludeDecoration", "refs/gitbutler")
            .context("failed to set log.excludeDecoration")?;
        Ok(())
    })?;
    Ok(())
}

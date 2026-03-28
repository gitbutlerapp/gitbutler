use std::{path::Path, time};

use anyhow::{Context as _, Result, anyhow};
use but_core::{
    RepositoryExt as _, git_config::ensure_config_value,
    worktree::checkout::UncommitedWorktreeChanges,
};
use but_ctx::Context;
use but_error::Marker;
use but_oxidize::ObjectIdExt;
use gitbutler_branch::GITBUTLER_WORKSPACE_REFERENCE;
use gitbutler_project::FetchResult;
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::first_parent_commit_ids_until;
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::{Stack, Target, VirtualBranchesHandle, canned_branch_name};
use serde::Serialize;
use tracing::instrument;

use crate::{
    VirtualBranchesExt,
    integration::update_workspace_commit,
    remote::{RemoteCommit, commit_to_remote_commit},
};

#[derive(Debug, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BaseBranch {
    pub branch_name: String,
    pub remote_name: String,
    pub remote_url: String,
    pub push_remote_name: Option<String>,
    pub push_remote_url: String,
    #[serde(with = "but_serde::object_id")]
    pub base_sha: gix::ObjectId,
    #[serde(with = "but_serde::object_id")]
    pub current_sha: gix::ObjectId,
    pub behind: usize,
    pub upstream_commits: Vec<RemoteCommit>,
    pub recent_commits: Vec<RemoteCommit>,
    pub last_fetched_ms: Option<u128>,
    pub conflicted: bool,
    pub diverged: bool,
    #[serde(with = "but_serde::object_id_vec")]
    pub diverged_ahead: Vec<gix::ObjectId>,
    #[serde(with = "but_serde::object_id_vec")]
    pub diverged_behind: Vec<gix::ObjectId>,
}

impl BaseBranch {
    pub fn short_name(&self) -> &str {
        let remote_prefix = format!("{}/", self.remote_name);
        self.branch_name
            .strip_prefix(&remote_prefix)
            .unwrap_or(&self.branch_name)
    }
}

#[instrument(skip(ctx), err(Debug))]
pub fn get_base_branch_data(ctx: &Context) -> Result<BaseBranch> {
    let target = default_target(&ctx.project_data_dir())?;
    let base = target_to_base_branch(ctx, &target)?;
    Ok(base)
}

/// Restore the default target metadata if it is missing in the currently configured storage
/// location while an existing `gitbutler/workspace` ref already proves the repository was
/// initialized before.
///
/// This is intentionally metadata-only recovery for activation flows. Unlike
/// `set_base_branch()`, it must not create stacks, update the workspace commit, or move the
/// `gitbutler/workspace` reference.
///
/// Returns `true` if a target was inferred and written, `false` if no recovery was needed or
/// there wasn't enough repository state to infer a safe target.
#[instrument(skip(ctx), err(Debug))]
pub fn bootstrap_default_target_if_missing(ctx: &Context) -> Result<bool> {
    let mut vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    if vb_state.maybe_get_default_target()?.is_some() {
        return Ok(false);
    }

    let repo = ctx.repo.get()?;
    if repo
        .try_find_reference(GITBUTLER_WORKSPACE_REFERENCE.to_string().as_str())?
        .is_none()
    {
        return Ok(false);
    }

    let Some(remote_name) = repo.remote_default_name(gix::remote::Direction::Push) else {
        return Ok(false);
    };
    let remote_name = remote_name.to_string();

    let target = match inferred_default_target(&repo, &remote_name) {
        Ok(Some(target)) => target,
        Ok(None) => return Ok(false),
        Err(err) => {
            tracing::debug!(
                error = ?err,
                remote_name,
                "failed to infer default target; leaving default target uninitialized"
            );
            return Ok(false);
        }
    };
    vb_state.set_default_target(target)?;
    set_exclude_decoration(ctx)?;
    Ok(true)
}

#[instrument(skip(ctx), err(Debug))]
fn go_back_to_integration(ctx: &Context, default_target: &Target) -> Result<BaseBranch> {
    let repo = ctx.repo.get()?;
    if ctx.settings.feature_flags.cv3 {
        let workspace_commit_to_checkout =
            but_workspace::legacy::remerged_workspace_commit_v2(ctx)?;
        let tree_to_checkout_to_avoid_ref_update =
            repo.find_commit(workspace_commit_to_checkout)?.tree_id()?;
        but_core::worktree::safe_checkout(
            repo.head_id()?.detach(),
            tree_to_checkout_to_avoid_ref_update.detach(),
            &repo,
            but_core::worktree::checkout::Options {
                uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
                skip_head_update: false,
            },
        )?;
    } else {
        let (mut outcome, conflict_kind) =
            but_workspace::legacy::merge_worktree_with_workspace(ctx, &repo)?;

        if outcome.has_unresolved_conflicts(conflict_kind) {
            return Err(anyhow!("Conflicts while going back to gitbutler/workspace"))
                .context(Marker::ProjectConflict);
        }

        let final_tree_id = outcome.tree.write()?.detach();

        #[expect(deprecated, reason = "checkout/materialization boundary")]
        let git2_repo = &*ctx.git2_repo.get()?;
        let final_tree = git2_repo.find_tree(final_tree_id.to_git2())?;
        git2_repo
            .checkout_tree(
                final_tree.as_object(),
                Some(git2::build::CheckoutBuilder::new().force()),
            )
            .context("failed to checkout tree")?;
    }

    let base = target_to_base_branch(ctx, default_target)?;
    update_workspace_commit(ctx, false)?;
    Ok(base)
}

pub(crate) fn set_base_branch(
    ctx: &Context,
    target_branch_ref: &RemoteRefname,
) -> Result<BaseBranch> {
    let repo = ctx.repo.get()?;

    // if target exists, and it is the same as the requested branch, we should go back
    if let Ok(target) = default_target(&ctx.project_data_dir())
        && target.branch.eq(target_branch_ref)
    {
        return go_back_to_integration(ctx, &target);
    }

    // lookup a branch by name
    let mut target_branch = repo
        .try_find_reference(target_branch_ref.to_string().as_str())?
        .ok_or(anyhow!("remote branch '{target_branch_ref}' not found"))?;

    let remote = repo
        .find_remote(target_branch_ref.remote())
        .context(format!(
            "failed to find remote for branch {target_branch_ref}"
        ))?;
    let remote_url = remote
        .url(gix::remote::Direction::Fetch)
        .map(|url| url.to_bstring().to_string())
        .context(format!(
            "failed to get remote url for {}",
            target_branch_ref.remote()
        ))?;

    let target_branch_head = target_branch
        .peel_to_commit()
        .context(format!(
            "failed to peel branch {target_branch_ref} to commit"
        ))?
        .id;

    let mut current_head = repo.head().context("Failed to get HEAD reference")?;
    let current_head_commit = current_head
        .peel_to_commit()
        .context("Failed to peel HEAD reference to commit")?
        .id;

    // calculate the commit as the merge-base between HEAD in ctx and this target commit
    let target_commit_oid = repo
        .merge_base(current_head_commit, target_branch_head)
        .map(|id| id.detach())
        .context(format!(
            "Failed to calculate merge base between {current_head_commit} and {target_branch_head}"
        ))?;

    let target = Target {
        branch: target_branch_ref.clone(),
        remote_url,
        sha: target_commit_oid,
        push_remote_name: None,
    };

    let mut vb_state = ctx.virtual_branches();
    vb_state.set_default_target(target.clone())?;

    // TODO: make sure this is a real branch
    let head_name: Refname = current_head
        .referent_name()
        .map(|name| {
            name.to_string()
                .parse()
                .expect("BUG: we have to avoid using these legacy types")
        })
        .context("Failed to get HEAD reference name")?;
    if !head_name
        .to_string()
        .eq(&GITBUTLER_WORKSPACE_REFERENCE.to_string())
    {
        // if there are any commits on the head branch or uncommitted changes in the working directory, we need to
        // put them into a virtual branch

        let changes = but_core::diff::worktree_changes(&*ctx.repo.get()?)?.changes;
        if !changes.is_empty() || current_head_commit != target.sha {
            let (upstream, branch_matches_target) = if let Refname::Local(head_name) = &head_name {
                let upstream_name = target_branch_ref.with_branch(head_name.branch());
                if upstream_name.eq(target_branch_ref) {
                    (None, true)
                } else {
                    let upstream = repo
                        .try_find_reference(Refname::from(&upstream_name).to_string().as_str())
                        .with_context(|| format!("failed to find upstream for {head_name}"))?;
                    (upstream.map(|_| upstream_name), false)
                }
            } else {
                (None, false)
            };

            let branch_name = if branch_matches_target {
                canned_branch_name(&*ctx.repo.get()?)?
            } else {
                head_name.to_string().replace("refs/heads/", "")
            };

            let branch = if branch_matches_target {
                Stack::new_empty(ctx, branch_name, current_head_commit, 0)
            } else {
                Stack::new_from_existing(
                    ctx,
                    branch_name,
                    Some(head_name),
                    upstream,
                    current_head_commit,
                    0,
                )
            }?;

            vb_state.set_stack(branch)?;
        }
    }

    set_exclude_decoration(ctx)?;

    crate::integration::update_workspace_commit_with_vb_state(&vb_state, ctx, true)?;

    let base = target_to_base_branch(ctx, &target)?;
    Ok(base)
}

pub(crate) fn set_target_push_remote(ctx: &Context, push_remote_name: &str) -> Result<()> {
    ctx.repo
        .get()?
        .find_remote(push_remote_name)
        .context(format!("failed to find remote {push_remote_name}"))?;

    // if target exists, and it is the same as the requested branch, we should go back
    let mut target = default_target(&ctx.project_data_dir())?;
    target.push_remote_name = Some(push_remote_name.to_owned());
    let mut vb_state = ctx.virtual_branches();
    vb_state.set_default_target(target)?;

    Ok(())
}

fn set_exclude_decoration(ctx: &Context) -> Result<()> {
    let repo = ctx.repo.get()?;
    let mut config = repo.local_common_config_for_editing()?;
    let changed = ensure_config_value(&mut config, "log.excludeDecoration", "refs/gitbutler")
        .context("failed to set log.excludeDecoration")?;
    if changed {
        repo.write_local_common_config(&config)?;
    }
    Ok(())
}

pub(crate) fn target_to_base_branch(ctx: &Context, target: &Target) -> Result<BaseBranch> {
    let repo = &*ctx.repo.get()?;
    let target_commit_id = repo
        .find_reference(&target.branch.to_string())?
        .peel_to_commit()?
        .id;
    let merge_base = repo.merge_base(target.sha, target_commit_id)?.detach();

    let diverged_ahead = first_parent_commit_ids_until(repo, target.sha, merge_base)
        .context("failed to get fork point")?;
    let diverged_behind = first_parent_commit_ids_until(repo, target_commit_id, merge_base)
        .context("failed to get fork point")?;

    // if there are commits ahead of the base branch consider it diverged
    let diverged = !diverged_ahead.is_empty();

    // gather a list of commits between oid and target.sha
    let upstream_commits = first_parent_commit_ids_until(repo, target_commit_id, target.sha)
        .context("failed to get upstream commits")?
        .iter()
        .map(|id| {
            let commit = repo.find_commit(*id)?;
            commit_to_remote_commit(&commit)
        })
        .collect::<Result<Vec<_>>>()?;

    // get some recent commits
    let recent_commits = first_parent_commit_ids_with_limit(repo, target.sha, 20)
        .context("failed to get recent commits")?
        .iter()
        .map(|id| {
            let commit = repo.find_commit(*id)?;
            commit_to_remote_commit(&commit)
        })
        .collect::<Result<Vec<_>>>()?;

    // we assume that only local commits can be conflicted
    let conflicted = recent_commits.iter().any(|commit| commit.conflicted);

    // there has got to be a better way to do this.
    let push_remote_url = match target.push_remote_name {
        Some(ref name) => repo
            .find_remote(name.as_str())
            .ok()
            .and_then(|remote| {
                remote
                    .url(gix::remote::Direction::Push)
                    .or_else(|| remote.url(gix::remote::Direction::Fetch))
                    .map(|url| url.to_bstring().to_string())
            })
            .unwrap_or_else(|| target.remote_url.clone()),
        None => target.remote_url.clone(),
    };

    // Fallback to the remote URL of the branch if the target remote URL is empty
    let remote_url = if target.remote_url.is_empty() {
        let remote = repo.find_remote(target.branch.remote()).context(format!(
            "failed to find remote for branch {}",
            target.branch.fullname()
        ))?;
        remote
            .url(gix::remote::Direction::Fetch)
            .map(|url| url.to_bstring().to_string())
            .context(format!(
                "failed to get remote url for {}",
                target.branch.fullname()
            ))?
    } else {
        target.remote_url.clone()
    };

    let base = BaseBranch {
        branch_name: target.branch.fullname(),
        remote_name: target.branch.remote().to_string(),
        remote_url,
        push_remote_name: target.push_remote_name.clone(),
        push_remote_url,
        base_sha: target.sha,
        current_sha: target_commit_id,
        behind: upstream_commits.len(),
        upstream_commits,
        recent_commits,
        last_fetched_ms: ctx
            .legacy_project
            .project_data_last_fetch
            .as_ref()
            .map(FetchResult::timestamp)
            .map(|t| t.duration_since(time::UNIX_EPOCH).unwrap().as_millis()),
        conflicted,
        diverged,
        diverged_ahead,
        diverged_behind,
    };
    Ok(base)
}

fn default_target(base_path: &Path) -> Result<Target> {
    VirtualBranchesHandle::new(base_path).get_default_target()
}

/// Infer the default target from the Git repository without mutating workspace refs.
///
/// Preference order:
/// 1. `refs/remotes/<remote>/HEAD`
/// 2. `refs/remotes/<remote>/main`
/// 3. `refs/remotes/<remote>/master`
fn inferred_default_target(repo: &gix::Repository, remote_name: &str) -> Result<Option<Target>> {
    let remote_url = repo
        .find_remote(remote_name)
        .ok()
        .and_then(|remote| {
            remote
                .url(gix::remote::Direction::Fetch)
                .map(ToOwned::to_owned)
        })
        .map(|url| url.to_bstring().to_string())
        .unwrap_or_default();

    let remote_head_ref = format!("refs/remotes/{remote_name}/HEAD");
    if let Ok(mut head_ref) = repo.find_reference(remote_head_ref.as_str())
        && let Some(branch_name) = head_ref
            .target()
            .try_name()
            .map(|name| name.as_bstr().to_string())
    {
        let branch = branch_name
            .parse()
            .with_context(|| format!("Remote HEAD resolved to invalid ref '{branch_name}'"))?;
        let sha = head_ref
            .peel_to_commit()
            .context("Remote HEAD did not point to a commit")?
            .id;
        return Ok(Some(Target {
            branch,
            remote_url,
            sha,
            push_remote_name: Some(remote_name.to_owned()),
        }));
    }

    for branch_name in ["main", "master"] {
        let full_name = format!("refs/remotes/{remote_name}/{branch_name}");
        if let Ok(mut reference) = repo.find_reference(&full_name) {
            let sha = reference
                .peel_to_commit()
                .with_context(|| {
                    format!("Fallback target '{full_name}' did not point to a commit")
                })?
                .id;
            return Ok(Some(Target {
                branch: full_name.parse()?,
                remote_url,
                sha,
                push_remote_name: Some(remote_name.to_owned()),
            }));
        }
    }

    Ok(None)
}

fn first_parent_commit_ids_with_limit(
    repo: &gix::Repository,
    from: gix::ObjectId,
    limit: usize,
) -> Result<Vec<gix::ObjectId>> {
    use gix::prelude::ObjectIdExt as _;

    from.attach(repo)
        .ancestors()
        .first_parent_only()
        .all()?
        .take(limit)
        .map(|info| Ok(info?.id))
        .collect()
}

pub(crate) fn push(ctx: &Context, with_force: bool) -> Result<()> {
    let target = default_target(&ctx.project_data_dir())?;
    let _ = ctx.push(
        target.sha,
        &target.branch,
        with_force,
        ctx.legacy_project.force_push_protection,
        None,
        None,
        vec![],
    );
    Ok(())
}

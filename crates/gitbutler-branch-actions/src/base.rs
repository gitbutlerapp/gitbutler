use std::time;

use anyhow::{Context as _, Result, anyhow};
use but_core::{
    WORKSPACE_REF_NAME,
    git_config::{edit_repo_config, ensure_config_value},
    sync::RepoShared,
    worktree::checkout::UncommitedWorktreeChanges,
};
use but_ctx::Context;
use but_error::{Code, Marker};
use but_graph::FirstParent;
use but_oxidize::ObjectIdExt;
use gitbutler_git::GitContextExt as _;
use gitbutler_project::{FetchResult, Project};
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::first_parent_commit_ids_until;
use gitbutler_stack::{Stack, Target, canned_branch_name};
use serde::Serialize;
use tracing::instrument;

use crate::{
    VirtualBranchesExt,
    integration::update_workspace_commit,
    remote::{RemoteCommit, commit_to_remote_commit},
};

#[derive(Debug, Serialize, PartialEq, Clone)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct BaseBranch {
    pub branch_name: String,
    pub remote_name: String,
    pub remote_url: String,
    pub push_remote_name: String,
    pub push_remote_url: String,
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::object_id")
    )]
    pub base_sha: gix::ObjectId,
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::object_id")
    )]
    pub current_sha: gix::ObjectId,
    pub behind: usize,
    pub upstream_commits: Vec<RemoteCommit>,
    pub recent_commits: Vec<RemoteCommit>,
    pub last_fetched_ms: Option<u128>,
    pub conflicted: bool,
    pub target_sha_ahead_of_ref: bool,
    pub short_name: String,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(BaseBranch);

impl BaseBranch {
    pub fn compute_short_name(branch_name: &str, remote_name: &str) -> String {
        if !remote_name.is_empty() && branch_name == remote_name {
            return String::new();
        }

        let prefixes: Vec<String> = if !remote_name.is_empty() {
            vec![
                format!("refs/remotes/{remote_name}/"),
                format!("{remote_name}/"),
                "refs/heads/".to_string(),
            ]
        } else {
            vec!["refs/heads/".to_string()]
        };

        for prefix in &prefixes {
            if let Some(stripped) = branch_name.strip_prefix(prefix.as_str()) {
                return stripped.to_string();
            }
        }

        branch_name.to_string()
    }
}

#[instrument(skip(ctx, perm), err(Debug))]
pub fn get_base_branch_data(ctx: &Context, perm: &RepoShared) -> Result<BaseBranch> {
    let (repo, ws, _) = ctx.workspace_and_db_with_perm(perm)?;
    let base = target_to_base_branch(&repo, &ctx.legacy_project, &ws, &ctx.project_meta()?)?;
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
    let repo = ctx.repo.get()?;
    if repo.try_find_reference(WORKSPACE_REF_NAME)?.is_none() {
        return Ok(false);
    }

    if ctx.project_meta()?.target_ref.is_some() {
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
    ctx.set_default_target(target.into())?;
    set_exclude_decoration(ctx)?;
    Ok(true)
}

#[instrument(skip(ctx, perm), err(Debug))]
fn go_back_to_integration(ctx: &Context, perm: &RepoShared) -> Result<BaseBranch> {
    if ctx.settings.feature_flags.cv3 {
        {
            let repo = ctx.repo.get()?;
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
                    ..Default::default()
                },
            )?;
        }
    } else {
        let final_tree_id = {
            let repo = ctx.repo.get()?;
            let (mut outcome, conflict_kind) =
                but_workspace::legacy::merge_worktree_with_workspace(ctx, &repo)?;

            if outcome.has_unresolved_conflicts(conflict_kind) {
                return Err(anyhow!("Conflicts while going back to gitbutler/workspace"))
                    .context(Marker::ProjectConflict);
            }

            outcome.tree.write()?.detach()
        };

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

    update_workspace_commit(ctx, false)?;
    get_base_branch_data(ctx, perm)
}

pub(crate) fn set_base_branch(
    ctx: &Context,
    perm: &RepoShared,
    target_branch_ref: &RemoteRefname,
) -> Result<BaseBranch> {
    let repo = ctx.repo.get()?;

    let existing_target_ref_matches = if let Ok(mut project_meta) = ctx.project_meta() {
        let repaired_project_meta =
            but_core::ref_metadata::repair_target_metadata_for_migration(&project_meta, &repo);
        if repaired_project_meta != project_meta {
            ctx.set_project_meta(repaired_project_meta.clone())?;
            project_meta = repaired_project_meta;
        }
        project_meta.target_commit_id.is_some()
            && project_meta
                .target_ref
                .is_some_and(|target_ref| target_ref.to_string() == target_branch_ref.to_string())
    } else {
        false
    };

    // if target exists, and it is the same as the requested branch, we should go back
    if existing_target_ref_matches {
        return go_back_to_integration(ctx, perm);
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

    ctx.set_default_target(target.clone().into())?;
    let mut vb_state = ctx.virtual_branches();

    // TODO: make sure this is a real branch
    let head_name: Refname = current_head
        .referent_name()
        .map(|name| {
            name.to_string()
                .parse()
                .expect("BUG: we have to avoid using these legacy types")
        })
        .context("Failed to get HEAD reference name")?;
    if !head_name.to_string().eq(WORKSPACE_REF_NAME) {
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

    get_base_branch_data(ctx, perm)
}

pub(crate) fn set_target_push_remote(ctx: &mut Context, push_remote_name: &str) -> Result<()> {
    ctx.repo
        .get()?
        .find_remote(push_remote_name)
        .context(format!("failed to find remote {push_remote_name}"))?;

    let mut project_meta = ctx.project_meta()?;
    project_meta
        .target_ref
        .as_ref()
        .context(Code::DefaultTargetNotFound)
        .context("there is no default target")?;
    project_meta.push_remote = Some(push_remote_name.to_owned());
    ctx.set_project_meta(project_meta)?;

    Ok(())
}

fn set_exclude_decoration(ctx: &Context) -> Result<()> {
    let repo = ctx.repo.get()?;
    edit_repo_config(&repo, gix::config::Source::Local, |config| {
        ensure_config_value(config, "log.excludeDecoration", "refs/gitbutler")
            .context("failed to set log.excludeDecoration")?;
        Ok(())
    })?;
    Ok(())
}

pub(crate) fn target_to_base_branch(
    repo: &gix::Repository,
    project: &Project,
    ws: &but_graph::Workspace,
    project_meta: &but_core::ref_metadata::ProjectMeta,
) -> Result<BaseBranch> {
    let target_ref_name = project_meta.target_ref_or_err()?.clone();
    let target_sha = project_meta.target_commit_id_or_err()?;
    let target_ref = repo
        .find_reference(&target_ref_name)
        .context(Code::DefaultTargetNotFound)?;
    let target_ref_commit_id = target_ref.id().detach();

    // The old integrate_upstream function cares about whether the target sha
    // is ahead of the target ref.
    //
    // The old function provided some options for how to resolve this.
    let target_sha_not_ref = first_parent_commit_ids_until(repo, target_sha, target_ref_commit_id)
        .context("failed to get fork point")?;
    let target_sha_ahead_of_ref = !target_sha_not_ref.is_empty();

    // The longest first-parent list of upstream commit ids.
    let mut upstream_commit_ids = ws
        .upstream_commits(repo, target_ref_name.as_ref(), FirstParent::Yes)?
        .into_iter()
        .map(|h| h.upstream_commits)
        .max_by_key(|us| us.len())
        .unwrap_or_default();
    if upstream_commit_ids.is_empty() && target_ref_commit_id != target_sha {
        upstream_commit_ids = first_parent_commit_ids_until(repo, target_ref_commit_id, target_sha)
            .context("failed to get target commits since stored base")?;
    }

    let upstream_commits = upstream_commit_ids
        .iter()
        .map(|id| {
            let commit = repo.find_commit(*id)?;
            commit_to_remote_commit(&commit)
        })
        .collect::<Result<Vec<_>>>()?;

    let behind = upstream_commits.len();

    // get some recent commits
    let recent_commits = first_parent_commit_ids_with_limit(repo, target_sha, 20)
        .context("failed to get recent commits")?
        .iter()
        .map(|id| {
            let commit = repo.find_commit(*id)?;
            commit_to_remote_commit(&commit)
        })
        .collect::<Result<Vec<_>>>()?;

    // we assume that only local commits can be conflicted
    let conflicted = recent_commits.iter().any(|commit| commit.conflicted);

    let push_remote_url = project_meta.push_remote_url(repo)?;
    let remote_url = project_meta.remote_url_with_fallback(repo)?;

    let branch_name = target_ref_name.shorten().to_string();
    let remote_name = target_ref
        .remote_name(gix::remote::Direction::Push)
        .context("Failed to get current remote name")?
        .to_owned()
        .as_bstr()
        .to_string();
    let push_remote_name = project_meta
        .push_remote
        .clone()
        .unwrap_or_else(|| remote_name.clone());
    let short_name = BaseBranch::compute_short_name(&branch_name, &remote_name);
    let base = BaseBranch {
        branch_name,
        remote_name,
        remote_url,
        push_remote_name,
        push_remote_url,
        base_sha: target_sha,
        current_sha: target_ref_commit_id,
        behind,
        upstream_commits,
        recent_commits,
        last_fetched_ms: project
            .project_data_last_fetch
            .as_ref()
            .map(FetchResult::timestamp)
            .map(|t| t.duration_since(time::UNIX_EPOCH).unwrap().as_millis()),
        conflicted,
        target_sha_ahead_of_ref,
        short_name,
    };
    Ok(base)
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
    let project_meta = ctx.project_meta()?;
    let target_ref: RemoteRefname = project_meta.target_ref_or_err()?.to_string().parse()?;
    let _ = ctx.push(
        project_meta.target_commit_id_or_err()?,
        &target_ref,
        with_force,
        ctx.legacy_project.force_push_protection,
        None,
        None,
        vec![],
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::BaseBranch;

    #[test]
    fn short_name_strips_full_remote_ref() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/remotes/origin/feature/foo", "origin"),
            "feature/foo"
        );
    }

    #[test]
    fn short_name_strips_short_remote_ref() {
        assert_eq!(
            BaseBranch::compute_short_name("origin/feature/foo", "origin"),
            "feature/foo"
        );
    }

    #[test]
    fn short_name_strips_full_remote_ref_simple() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/remotes/origin/main", "origin"),
            "main"
        );
    }

    #[test]
    fn short_name_strips_short_remote_ref_simple() {
        assert_eq!(
            BaseBranch::compute_short_name("origin/main", "origin"),
            "main"
        );
    }

    #[test]
    fn short_name_different_remote() {
        assert_eq!(
            BaseBranch::compute_short_name(
                "refs/remotes/another-remote/feat/complex-branch-name",
                "another-remote"
            ),
            "feat/complex-branch-name"
        );
        assert_eq!(
            BaseBranch::compute_short_name(
                "another-remote/feat/complex-branch-name",
                "another-remote"
            ),
            "feat/complex-branch-name"
        );
    }

    #[test]
    fn short_name_non_matching_remote() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/remotes/origin/feature/foo", "not-origin"),
            "refs/remotes/origin/feature/foo"
        );
        assert_eq!(
            BaseBranch::compute_short_name("origin/feature/foo", "not-origin"),
            "origin/feature/foo"
        );
    }

    #[test]
    fn short_name_heads_ref_with_remote() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/heads/feature/foo", "origin"),
            "feature/foo"
        );
    }

    #[test]
    fn short_name_local_name_with_remote() {
        assert_eq!(
            BaseBranch::compute_short_name("feature/foo", "origin"),
            "feature/foo"
        );
    }

    #[test]
    fn short_name_heads_ref_no_remote() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/heads/feature/foo", ""),
            "feature/foo"
        );
        assert_eq!(
            BaseBranch::compute_short_name("refs/heads/main", ""),
            "main"
        );
    }

    #[test]
    fn short_name_local_name_no_remote() {
        assert_eq!(
            BaseBranch::compute_short_name("feature/foo", ""),
            "feature/foo"
        );
        assert_eq!(BaseBranch::compute_short_name("main", ""), "main");
        assert_eq!(
            BaseBranch::compute_short_name("dev/task/T-123", ""),
            "dev/task/T-123"
        );
    }

    #[test]
    fn short_name_branch_equals_remote() {
        assert_eq!(BaseBranch::compute_short_name("origin", "origin"), "");
    }

    #[test]
    fn short_name_trailing_slash() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/remotes/origin/", "origin"),
            ""
        );
        assert_eq!(BaseBranch::compute_short_name("refs/heads/", ""), "");
    }

    #[test]
    fn short_name_embedded_ref_parts() {
        assert_eq!(
            BaseBranch::compute_short_name(
                "refs/remotes/origin/feature/name-with-refs/heads/in-it",
                "origin"
            ),
            "feature/name-with-refs/heads/in-it"
        );
    }

    #[test]
    fn short_name_empty_branch() {
        assert_eq!(BaseBranch::compute_short_name("", "origin"), "");
        assert_eq!(BaseBranch::compute_short_name("", ""), "");
    }

    #[test]
    fn short_name_remote_with_slashes() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/remotes/dev/feature/branch", "dev/feature"),
            "branch"
        );
        assert_eq!(
            BaseBranch::compute_short_name("dev/feature/branch", "dev/feature"),
            "branch"
        );
    }
}

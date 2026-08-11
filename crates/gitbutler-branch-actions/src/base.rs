use std::time;

use anyhow::{Context as _, Result, anyhow};
use but_core::{
    RefMetadata as _, WORKSPACE_REF_NAME,
    git_config::{edit_repo_config, ensure_config_value},
    ref_metadata::{ProjectMeta, StackId, WorkspaceCommitRelation},
    sync::{RepoExclusive, RepoShared},
};
use but_ctx::Context;
use but_error::{Code, bail_precondition};
use but_graph::FirstParent;
use gitbutler_project::{FetchResult, Project};
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::first_parent_commit_ids_until;
use serde::Serialize;
use tracing::instrument;

use crate::remote::{RemoteCommit, commit_to_remote_commit};

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

    let project_meta = match inferred_default_target(&repo) {
        Ok(Some(project_meta)) => project_meta,
        Ok(None) => return Ok(false),
        Err(err) => {
            tracing::debug!(
                error = ?err,
                "failed to infer default target; leaving default target uninitialized"
            );
            return Ok(false);
        }
    };
    ctx.set_project_meta(project_meta)?;
    set_exclude_decoration(ctx)?;
    Ok(true)
}

#[instrument(skip(ctx, perm), err(Debug))]
fn go_back_to_integration(ctx: &Context, perm: &mut RepoExclusive) -> Result<BaseBranch> {
    let ws =
        ctx.workspace_from_ref_uncached(WORKSPACE_REF_NAME.try_into()?, perm.read_permission())?;
    {
        let repo = ctx.repo.get()?;
        let workspace_commit_to_checkout =
            but_workspace::legacy::remerged_workspace_commit_v2(ctx, &ws)?;
        let tree_to_checkout_to_avoid_ref_update =
            repo.find_commit(workspace_commit_to_checkout)?.tree_id()?;
        but_core::worktree::safe_checkout_from_head(
            tree_to_checkout_to_avoid_ref_update.detach(),
            &repo,
            but_core::worktree::checkout::Options {
                skip_head_update: false,
                ..Default::default()
            },
        )?;
    }

    crate::integration::update_workspace_commit_from_workspace(ctx, false, &ws, perm)?;
    get_base_branch_data(ctx, perm.read_permission())
}

pub(crate) fn set_base_branch(
    ctx: &Context,
    perm: &mut RepoExclusive,
    target_branch_ref: &RemoteRefname,
) -> Result<BaseBranch> {
    let repo = ctx.repo.get()?;
    let workspace_ref_exists = repo.try_find_reference(WORKSPACE_REF_NAME)?.is_some();

    let (existing_target_ref_matches, existing_target_ref) = if let Ok(mut project_meta) =
        ctx.project_meta()
    {
        let repaired_project_meta =
            but_core::ref_metadata::repair_target_metadata_for_migration(&project_meta, &repo);
        if repaired_project_meta != project_meta {
            ctx.set_project_meta(repaired_project_meta.clone())?;
            project_meta = repaired_project_meta;
        }
        let target_ref_matches = project_meta
            .target_ref
            .as_ref()
            .is_some_and(|target_ref| target_ref.to_string() == target_branch_ref.to_string());
        (
            workspace_ref_exists && project_meta.target_commit_id.is_some() && target_ref_matches,
            project_meta.target_ref,
        )
    } else {
        (false, None)
    };

    // if target exists, and it is the same as the requested branch, we should go back
    if existing_target_ref_matches {
        return go_back_to_integration(ctx, perm);
    }

    // lookup a branch by name
    let mut target_branch = repo
        .try_find_reference(target_branch_ref.to_string().as_str())?
        .ok_or(anyhow!("remote branch '{target_branch_ref}' not found"))?;

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

    let project_meta = ProjectMeta {
        target_ref: Some(target_branch_ref.to_string().try_into()?),
        target_commit_id: Some(target_commit_oid),
        push_remote: None,
    };
    project_meta.remote_url_with_fallback(&repo)?;

    // TODO: make sure this is a real branch
    let head_name: Refname = current_head
        .referent_name()
        .map(|name| {
            name.to_string()
                .parse()
                .expect("BUG: we have to avoid using these legacy types")
        })
        .context("Failed to get HEAD reference name")?;
    if workspace_ref_exists
        && !head_name.to_string().eq(WORKSPACE_REF_NAME)
        && existing_target_ref
            .is_none_or(|target_ref| target_ref.to_string() != target_branch_ref.to_string())
    {
        bail_precondition!(
            "cannot change the target while HEAD is outside the GitButler workspace - return to workspace first"
        );
    }
    ctx.set_project_meta(project_meta)?;

    let mut workspace_to_initialize = None;
    if !head_name.to_string().eq(WORKSPACE_REF_NAME) {
        // if there are any commits on the head branch or uncommitted changes in the working directory, we need to
        // put them into a virtual branch

        let changes = but_core::diff::worktree_changes(&*ctx.repo.get()?)?.changes;
        if !changes.is_empty() || current_head_commit != target_commit_oid {
            let branch_matches_target = if let Refname::Local(head_name) = &head_name {
                let upstream_name = target_branch_ref.with_branch(head_name.branch());
                upstream_name.eq(target_branch_ref)
            } else {
                false
            };

            let stack_ref_name = if branch_matches_target {
                let stack_ref_name = but_core::branch::unique_canned_refname(&repo)?;
                repo.reference(
                    stack_ref_name.as_ref(),
                    current_head_commit,
                    gix::refs::transaction::PreviousValue::MustNotExist,
                    "initialize stack",
                )?;
                stack_ref_name
            } else {
                head_name.to_string().try_into()?
            };

            let mut meta = ctx.meta()?;
            let mut workspace = meta.workspace(WORKSPACE_REF_NAME.try_into()?)?;
            workspace.add_or_insert_new_stack_if_not_present(
                stack_ref_name.as_ref(),
                None,
                WorkspaceCommitRelation::Merged,
                |_| StackId::generate(),
            );
            meta.set_workspace(&workspace)?;
            drop((workspace, meta));
            if !branch_matches_target {
                repo.reference(
                    WORKSPACE_REF_NAME,
                    current_head_commit,
                    gix::refs::transaction::PreviousValue::MustNotExist,
                    "initialize workspace",
                )?;
                workspace_to_initialize = Some(ctx.workspace_from_ref_uncached(
                    WORKSPACE_REF_NAME.try_into()?,
                    perm.read_permission(),
                )?);
            }
        }
    }

    set_exclude_decoration(ctx)?;

    if let Some(workspace) = workspace_to_initialize {
        crate::integration::update_workspace_commit_from_workspace(ctx, true, &workspace, perm)?;
    } else {
        crate::integration::update_workspace_commit_with_perm(ctx, true, perm)?;
    }

    get_base_branch_data(ctx, perm.read_permission())
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

    // Upstream integration needs to know whether the stored target is ahead of
    // the target ref so the UI can block integration until divergence is resolved.
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
fn inferred_default_target(repo: &gix::Repository) -> Result<Option<ProjectMeta>> {
    let Some(target_ref) = but_workspace::init::infer_default_target_ref(repo)? else {
        return Ok(None);
    };
    let remote_names = repo.remote_names();
    let (remote_name, _) =
        but_core::extract_remote_name_and_short_name(target_ref.as_ref(), &remote_names)
            .with_context(|| format!("failed to determine remote for branch '{target_ref}'"))?;
    let sha = repo
        .find_reference(target_ref.as_ref())?
        .peel_to_commit()
        .with_context(|| format!("inferred target '{target_ref}' did not point to a commit"))?
        .id;
    Ok(Some(ProjectMeta {
        target_ref: Some(target_ref),
        target_commit_id: Some(sha),
        push_remote: Some(remote_name),
    }))
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

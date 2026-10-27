use std::time;

use anyhow::{Context as _, Result, anyhow};
use but_core::{
    RefMetadata as _, WORKSPACE_REF_NAME,
    git_config::{edit_repo_config, ensure_config_value},
    ref_metadata::{ProjectMeta, StackId, UnresolvedLegacyTargetRefError, WorkspaceCommitRelation},
    sync::{RepoExclusive, RepoShared},
};
use but_ctx::Context;
use but_error::{Code, bail_precondition};
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
    pub remote_name: Option<String>,
    pub remote_url: Option<String>,
    pub push_remote_name: Option<String>,
    pub push_remote_url: Option<String>,
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::object_id")
    )]
    pub base_sha: gix::ObjectId,
    /// Upstream tip, or local target tip when no upstream is configured.
    /// None means the configured upstream ref is unavailable; incoming counts are unknown.
    #[serde(with = "but_serde::object_id_opt")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::object_id_opt")
    )]
    pub current_sha: Option<gix::ObjectId>,
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
    pub fn compute_short_name(branch_name: &str, remote_name: Option<&str>) -> String {
        if let Some(remote_name) = remote_name.filter(|name| !name.is_empty()) {
            if branch_name == remote_name {
                return String::new();
            }

            for prefix in [
                format!("refs/remotes/{remote_name}/"),
                format!("{remote_name}/"),
            ] {
                if let Some(stripped) = branch_name.strip_prefix(&prefix) {
                    return stripped.to_string();
                }
            }
        }

        branch_name
            .strip_prefix("refs/heads/")
            .unwrap_or(branch_name)
            .to_string()
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

    match ctx.project_meta() {
        Ok(project_meta) if project_meta.target_ref.is_some() => return Ok(false),
        Ok(_) => {}
        // A legacy target is set, even if no local branch stands for it, and the user picks the
        // one to replace it with. Failing here would fail activation, leaving the project unwatched.
        Err(err) if err.is::<UnresolvedLegacyTargetRefError>() => return Ok(false),
        Err(err) => return Err(err),
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
    let target_ref_name: gix::refs::FullName = target_branch_ref.to_string().try_into()?;
    let local_target_ref =
        but_core::ref_metadata::resolve_local_target(target_ref_name.as_ref(), &repo)?;

    let (existing_target_ref_matches, existing_target_ref, existing_target_commit_id) =
        if let Ok(mut project_meta) = ctx.project_meta() {
            let repaired_project_meta =
                but_core::ref_metadata::repair_target_metadata_for_migration(&project_meta, &repo);
            if repaired_project_meta != project_meta {
                ctx.set_project_meta(repaired_project_meta.clone())?;
                project_meta = repaired_project_meta;
            }
            let target_ref_matches = project_meta
                .target_ref
                .as_ref()
                .is_some_and(|target_ref| target_ref == &local_target_ref);
            (
                workspace_ref_exists
                    && project_meta.target_commit_id.is_some()
                    && target_ref_matches,
                project_meta.target_ref,
                project_meta.target_commit_id,
            )
        } else {
            (false, None, None)
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

    // Validate shared history, retaining the existing policy for explicit target changes.
    let merge_base = but_workspace::init::resolve_target_commit(
        &repo,
        current_head_commit,
        target_branch_head,
        None,
    )?;

    let changing_target = existing_target_ref
        .as_ref()
        .is_some_and(|existing| existing != &local_target_ref);
    let mut project_meta = ProjectMeta {
        target_ref: Some(local_target_ref),
        target_commit_id: existing_target_commit_id,
        push_remote: None,
    };
    let target_commit_oid = if changing_target {
        merge_base
    } else {
        project_meta.resolve_target_commit(&repo)?
    };
    project_meta.target_commit_id = Some(target_commit_oid);
    if target_branch_ref.remote().is_some() {
        project_meta.target_remote_url(&repo)?;
    }

    // TODO: make sure this is a real branch
    let head_ref_name = current_head
        .referent_name()
        .context("Failed to get HEAD reference name")?;
    let head_is_workspace = head_ref_name == WORKSPACE_REF_NAME;
    let branch_matches_target =
        head_ref_name == project_meta.target_ref_or_err()?.as_local_full_name_ref();
    let head_name: Refname = head_ref_name
        .to_string()
        .parse()
        .expect("BUG: we have to avoid using these legacy types");
    if workspace_ref_exists
        && !head_is_workspace
        && existing_target_ref
            .as_ref()
            .is_none_or(|existing| Some(existing) != project_meta.target_ref.as_ref())
    {
        bail_precondition!(
            "cannot change the target while HEAD is outside the GitButler workspace - return to workspace first"
        );
    }
    let head_has_local_work = !head_is_workspace
        && repo
            .merge_base(current_head_commit, target_commit_oid)
            .context("failed to compare HEAD with the selected cutoff")?
            .is_none_or(|base| base.detach() != current_head_commit);
    ctx.set_project_meta(project_meta)?;

    let mut workspace_to_initialize = None;
    if !head_is_workspace {
        // Preserve non-target branches even when the entire visible stack is empty.
        // The target branch only needs a new stack for commits or uncommitted changes.
        let changes = but_core::diff::worktree_changes(&*ctx.repo.get()?)?.changes;
        if !branch_matches_target || !changes.is_empty() || head_has_local_work {
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
            if !branch_matches_target {
                // Single-branch mode can already have metadata for the visible branches,
                // but mark them as outside the workspace. Preserve the entire visible stack,
                // including empty branches and their ordering, when entering the workspace.
                let current_workspace = ctx.workspace_from_head_uncached(perm.read_permission())?;
                current_workspace.reconcile_metadata(&mut workspace)?;
            }
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
    let target_sha = project_meta.resolve_target_commit(repo)?;
    let mut target_ref = repo
        .find_reference(&target_ref_name)
        .context(Code::DefaultTargetNotFound)?;
    let local_target_tip = target_ref
        .peel_to_commit()
        .with_context(|| format!("target '{target_ref_name}' did not point to a commit"))?
        .id;
    let upstream_name = project_meta.remote_target_ref(repo)?;
    let upstream_tip = upstream_name
        .as_ref()
        .map(|name| repo.try_find_reference(name.as_ref()))
        .transpose()?
        .flatten()
        .map(|mut reference| reference.peel_to_commit().map(|commit| commit.id))
        .transpose()?;
    let target_ref_commit_id = match upstream_name {
        Some(_) => upstream_tip,
        None => Some(local_target_tip),
    };

    // This flag gates upstream integration, not divergence between local main and its upstream.
    let (target_sha_ahead_of_ref, upstream_commit_ids) = match upstream_tip {
        Some(upstream_tip) => {
            let target_sha_not_ref = first_parent_commit_ids_until(repo, target_sha, upstream_tip)
                .context("failed to get fork point")?;
            let commits = incoming_commit_ids(ws, repo, upstream_tip, target_sha)?;
            (!target_sha_not_ref.is_empty(), commits)
        }
        None => (false, Vec::new()),
    };

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

    let push_remote_url = project_meta.push_remote_url(repo).ok();
    let remote_url = project_meta.target_remote_url(repo).ok();

    // Preserve this legacy DTO's upstream-facing branch name when an upstream is configured.
    let branch_name = upstream_name.as_ref().map_or_else(
        || target_ref_name.shorten().to_string(),
        |name| name.shorten().to_string(),
    );
    let remote_name = upstream_name.as_ref().and_then(|name| {
        but_core::extract_remote_name_and_short_name(name.as_ref(), &repo.remote_names())
            .map(|(remote, _)| remote)
    });
    let push_remote_name = project_meta
        .push_remote
        .clone()
        .or_else(|| remote_name.clone());
    let short_name = BaseBranch::compute_short_name(&branch_name, remote_name.as_deref());
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
    let mut project_meta = ProjectMeta {
        target_ref: Some(target_ref),
        ..Default::default()
    };
    project_meta.target_commit_id = Some(project_meta.resolve_target_commit(repo)?);
    Ok(Some(project_meta))
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

/// The first-parent commits on `tip` that the workspace doesn't have yet, newest first: the ones
/// the branch furthest behind lacks, or, when no branch lacks any, the ones after `target_sha`.
///
/// `tip` is the target's upstream tip, or the local target's tip when it has no upstream.
/// Measuring from the branches and not only from `target_sha` matters because updating only some
/// lanes moves `target_sha` past the branches that were left behind.
pub fn incoming_commit_ids(
    ws: &but_graph::Workspace,
    repo: &gix::Repository,
    tip: gix::ObjectId,
    target_sha: gix::ObjectId,
) -> Result<Vec<gix::ObjectId>> {
    let commits = upstream_commits_per_stack_head(ws, repo, tip)?
        .into_iter()
        .max_by_key(|commits| commits.len())
        .unwrap_or_default();
    if commits.is_empty() && tip != target_sha {
        return first_parent_commit_ids_until(repo, tip, target_sha)
            .context("failed to get target commits since stored base");
    }
    Ok(commits)
}

fn upstream_commits_per_stack_head(
    ws: &but_graph::Workspace,
    repo: &gix::Repository,
    target_ref_id: gix::ObjectId,
) -> Result<Vec<Vec<gix::ObjectId>>> {
    let mut heads = ws
        .stacks
        .iter()
        .filter_map(|stack| stack.tip_skip_empty())
        .collect::<Vec<_>>();
    if heads.is_empty()
        && let Some(entrypoint_commit) = ws.graph.entrypoint()?.commit()
    {
        heads.push(entrypoint_commit.id);
    }
    heads
        .into_iter()
        .map(|head| {
            repo.rev_walk([target_ref_id])
                .with_hidden([head])
                .first_parent_only()
                .all()?
                .map(|info| Ok(info?.id))
                .collect::<Result<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::BaseBranch;

    #[test]
    fn initial_local_target_preserves_local_only_work_above_upstream() -> anyhow::Result<()> {
        let (repo, _tmp) = but_testsupport::writable_scenario("legacy-target-local-ahead");
        let local = repo.head_id()?.detach();
        let upstream = repo.rev_parse_single("origin/trunk")?.detach();
        let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?;
        let mut guard = ctx.exclusive_worktree_access();
        super::set_base_branch(&ctx, guard.write_permission(), &"refs/heads/main".parse()?)?;
        assert_eq!(
            ctx.project_meta()?.target_commit_id,
            Some(upstream),
            "local-only work must stay above the cutoff"
        );
        let repo = ctx.repo.get()?;
        assert_eq!(
            repo.rev_parse_single("main")?.detach(),
            local,
            "initialization must not move local main"
        );
        assert_eq!(
            repo.head_commit()?.tree_id()?.detach(),
            repo.find_commit(local)?.tree_id()?.detach(),
            "local work must survive workspace checkout"
        );
        let ws = ctx.workspace_from_head_uncached(guard.read_permission())?;
        assert_eq!(
            ws.stacks.len(),
            1,
            "local-only work must have a preservation stack"
        );
        Ok(())
    }

    #[test]
    fn initial_lagging_target_uses_upstream_without_preservation_stack() -> anyhow::Result<()> {
        use but_core::RefMetadata;

        let (repo, _tmp) = but_testsupport::writable_scenario("legacy-target-local-behind");
        let local = repo.head_id()?.detach();
        let upstream = repo.rev_parse_single("origin/main")?.detach();
        let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?;
        let mut guard = ctx.exclusive_worktree_access();
        super::set_base_branch(&ctx, guard.write_permission(), &"refs/heads/main".parse()?)?;
        assert_eq!(
            ctx.project_meta()?.target_commit_id,
            Some(upstream),
            "initialization must select upstream even when local main lags"
        );
        let meta = ctx.meta()?;
        let workspace = meta.workspace(but_core::WORKSPACE_REF_NAME.try_into()?)?;
        assert_eq!(
            workspace
                .stacks(but_core::ref_metadata::StackKind::Applied)
                .count(),
            0,
            "lagging main has no local-only work to preserve"
        );
        let repo = ctx.repo.get()?;
        assert_eq!(
            repo.rev_parse_single("main")?.detach(),
            local,
            "initialization must not fast-forward main"
        );
        Ok(())
    }

    #[test]
    fn initial_diverged_target_preserves_local_work_with_upstream_cutoff() -> anyhow::Result<()> {
        let (repo, _tmp) = but_testsupport::writable_scenario("legacy-target-diverged");
        let local = repo.head_id()?.detach();
        let upstream = repo.rev_parse_single("origin/main")?.detach();
        let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?;
        let mut guard = ctx.exclusive_worktree_access();
        super::set_base_branch(&ctx, guard.write_permission(), &"refs/heads/main".parse()?)?;
        assert_eq!(
            ctx.project_meta()?.target_commit_id,
            Some(upstream),
            "divergence must not replace upstream with the merge base"
        );
        let repo = ctx.repo.get()?;
        assert_eq!(
            repo.rev_parse_single("main")?.detach(),
            local,
            "diverged local main must stay unchanged"
        );
        assert_eq!(
            repo.head_commit()?.tree_id()?.detach(),
            repo.find_commit(local)?.tree_id()?.detach(),
            "diverged local work must survive workspace checkout"
        );
        let ws = ctx.workspace_from_head_uncached(guard.read_permission())?;
        assert_eq!(
            ws.stacks.len(),
            1,
            "diverged local work must remain visible"
        );
        Ok(())
    }

    #[test]
    fn setting_differently_named_upstream_does_not_apply_local_target() -> anyhow::Result<()> {
        use but_core::RefMetadata;

        let (repo, _tmp) =
            but_testsupport::writable_scenario("local-target-different-upstream-name");
        let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?;
        let mut guard = ctx.exclusive_worktree_access();
        super::set_base_branch(
            &ctx,
            guard.write_permission(),
            &"refs/remotes/origin/trunk".parse()?,
        )?;
        let meta = ctx.meta()?;
        let workspace = meta.workspace(but_core::WORKSPACE_REF_NAME.try_into()?)?;
        assert_eq!(
            workspace
                .stacks(but_core::ref_metadata::StackKind::Applied)
                .count(),
            0,
            "clean local main must not become a feature stack when its upstream is named trunk"
        );
        Ok(())
    }

    #[test]
    fn inferred_target_does_not_replace_missing_upstream_with_local_tip() -> anyhow::Result<()> {
        let (mut repo, _tmp) = but_testsupport::writable_scenario("local-target-upstream-ahead");
        repo.config_snapshot_mut()
            .set_raw_value("branch.main.merge", "refs/heads/missing")?;
        let error = super::inferred_default_target(&repo)
            .expect_err("a missing configured upstream must not become local main");
        assert!(
            error.to_string().contains("refs/remotes/origin/missing"),
            "the error must identify the unavailable selected upstream: {error}"
        );
        Ok(())
    }

    #[test]
    fn bootstrap_leaves_an_unresolved_legacy_target_for_the_user_to_replace() -> anyhow::Result<()>
    {
        use but_testsupport::{CommandExt as _, git_at_dir};

        let (repo, _tmp) = but_testsupport::writable_scenario("local-target-upstream-ahead");
        let workdir = repo
            .workdir()
            .expect("the scenario has a worktree")
            .to_owned();
        // an older GitButler's remote target, which no local branch tracks
        git_at_dir(&workdir)
            .args([
                "config",
                "gitbutler.project.targetRef",
                "refs/remotes/origin/missing",
            ])
            .run();
        git_at_dir(&workdir)
            .args(["config", "gitbutler.project.portedMeta", "true"])
            .run();
        let ctx = but_ctx::Context::from_repo_for_testing(repo)?;
        assert!(
            !super::bootstrap_default_target_if_missing(&ctx)?,
            "a target is set, even if no local branch stands for it, so activation goes on"
        );
        assert!(
            ctx.project_meta().is_err_and(|err| {
                err.is::<but_core::ref_metadata::UnresolvedLegacyTargetRefError>()
            }),
            "the legacy target stays until the user picks a local branch to replace it"
        );
        Ok(())
    }

    #[test]
    fn recovery_with_lagging_local_target_does_not_report_divergence() -> anyhow::Result<()> {
        let (repo, _tmp) = but_testsupport::writable_scenario("local-target-upstream-ahead");
        let upstream = repo.rev_parse_single("origin/main")?.detach();
        let local = repo.rev_parse_single("main")?.detach();
        assert_ne!(upstream, local, "local main must lag its upstream");
        let ctx = but_ctx::Context::from_repo_for_testing(repo)?;
        assert!(
            super::bootstrap_default_target_if_missing(&ctx)?,
            "missing metadata is recovered"
        );
        assert_eq!(
            ctx.project_meta()?.target_commit_id,
            Some(upstream),
            "recovery preserves upstream-first cutoff selection"
        );
        let guard = ctx.shared_worktree_access();
        let base = super::get_base_branch_data(&ctx, guard.read_permission())?;
        assert!(
            !base.target_sha_ahead_of_ref,
            "a lagging local target must not block upstream integration"
        );
        assert_eq!(
            base.current_sha,
            Some(upstream),
            "current SHA describes the upstream"
        );
        assert_eq!(
            base.remote_name.as_deref(),
            Some("origin"),
            "target remote uses fetch, not branch.main.pushRemote"
        );
        Ok(())
    }

    #[test]
    fn upstream_counts_use_remote_tip_with_a_stored_local_base() -> anyhow::Result<()> {
        let (repo, _tmp) = but_testsupport::writable_scenario("local-target-upstream-ahead");
        let local = repo.rev_parse_single("main")?.detach();
        let upstream = repo.rev_parse_single("origin/main")?.detach();
        let mut project_meta =
            super::inferred_default_target(&repo)?.expect("fixture has a target");
        project_meta.target_commit_id = Some(local);
        project_meta.push_remote = Some("fork".into());
        let ctx = but_ctx::Context::from_repo_for_testing(repo)?;
        ctx.set_project_meta(project_meta)?;
        let guard = ctx.shared_worktree_access();
        let base = super::get_base_branch_data(&ctx, guard.read_permission())?;
        assert_eq!(
            base.current_sha,
            Some(upstream),
            "current SHA must not use lagging local main"
        );
        assert_eq!(
            base.behind, 1,
            "one upstream commit is available since the stored base"
        );
        assert_eq!(
            base.upstream_commits.len(),
            1,
            "upstream commits come from origin/main"
        );
        assert_eq!(
            base.remote_name.as_deref(),
            Some("origin"),
            "integration uses the target fetch remote"
        );
        assert_eq!(
            base.push_remote_name.as_deref(),
            Some("fork"),
            "feature pushes retain the explicit override"
        );

        ctx.repo
            .get()?
            .find_reference("refs/remotes/origin/main")?
            .delete()?;
        let missing = super::get_base_branch_data(&ctx, guard.read_permission())?;
        assert_eq!(
            missing.current_sha, None,
            "a missing upstream must not be replaced by local main"
        );
        assert_eq!(
            missing.base_sha, local,
            "the stored cutoff remains available"
        );
        assert_eq!(
            missing.remote_name.as_deref(),
            Some("origin"),
            "the configured remote remains available"
        );
        assert!(
            !missing.recent_commits.is_empty(),
            "cutoff history remains visible"
        );
        Ok(())
    }

    #[test]
    fn upstream_rewind_still_reports_stored_cutoff_divergence() -> anyhow::Result<()> {
        let (repo, _tmp) = but_testsupport::writable_scenario("local-target-upstream-ahead");
        let project_meta = super::inferred_default_target(&repo)?.expect("fixture has a target");
        repo.reference(
            "refs/remotes/origin/main",
            repo.rev_parse_single("main")?.detach(),
            gix::refs::transaction::PreviousValue::Any,
            "rewind upstream",
        )?;
        let ctx = but_ctx::Context::from_repo_for_testing(repo)?;
        ctx.set_project_meta(project_meta)?;
        let guard = ctx.shared_worktree_access();
        let base = super::get_base_branch_data(&ctx, guard.read_permission())?;
        assert!(
            base.target_sha_ahead_of_ref,
            "a stored cutoff removed from upstream must still block integration"
        );
        Ok(())
    }

    #[test]
    fn base_branch_read_without_fetch_url_retains_upstream_history() -> anyhow::Result<()> {
        let (mut repo, _tmp) = but_testsupport::writable_scenario("local-target-upstream-ahead");
        let upstream = repo.rev_parse_single("origin/main")?.detach();
        let local = repo.rev_parse_single("main")?.detach();
        assert_ne!(
            local, upstream,
            "the fixture must distinguish local and upstream tips"
        );
        {
            let mut config = repo.config_snapshot_mut();
            config.raw_values_mut("remote.origin.url")?.delete_all();
            config.set_raw_value("remote.origin.pushUrl", "./remote.git")?;
        }
        assert!(
            repo.find_remote("origin")?
                .url(gix::remote::Direction::Fetch)
                .is_none(),
            "the configured upstream remote has no fetch URL"
        );
        let project_meta = but_core::ref_metadata::ProjectMeta {
            target_ref: Some(but_core::ref_metadata::LocalRefName::new(
                "refs/heads/main".try_into()?,
            )?),
            target_commit_id: Some(local),
            ..Default::default()
        };
        let ctx = but_ctx::Context::from_repo_for_testing(repo)?;
        ctx.set_project_meta(project_meta)?;
        let guard = ctx.shared_worktree_access();
        let base = super::get_base_branch_data(&ctx, guard.read_permission())?;
        assert_eq!(
            base.remote_url, None,
            "a missing fetch URL does not prevent base-branch reads"
        );
        assert_eq!(
            base.remote_name.as_deref(),
            Some("origin"),
            "upstream identity does not depend on a fetch URL"
        );
        assert_eq!(
            base.current_sha,
            Some(upstream),
            "the current tip remains upstream, not local main"
        );
        assert_eq!(
            base.base_sha, local,
            "reading the base preserves the stored cutoff"
        );
        assert!(
            !base.upstream_commits.is_empty(),
            "available upstream history remains visible without a fetch URL"
        );
        Ok(())
    }

    #[test]
    fn local_only_target_has_no_upstream_integration_work() -> anyhow::Result<()> {
        let (mut repo, _tmp) = but_testsupport::writable_scenario("local-target-upstream-ahead");
        let stored = repo.rev_parse_single("origin/main")?.detach();
        let local = repo.rev_parse_single("main")?.detach();
        {
            let mut config = repo.config_snapshot_mut();
            config.set_raw_value("branch.main.remote", "missing")?;
        }
        let project_meta = but_core::ref_metadata::ProjectMeta {
            target_ref: Some(but_core::ref_metadata::LocalRefName::new(
                "refs/heads/main".try_into()?,
            )?),
            target_commit_id: Some(stored),
            ..Default::default()
        };
        assert!(
            project_meta.remote_target_ref(&repo)?.is_none(),
            "local target has no resolvable upstream"
        );
        let ctx = but_ctx::Context::from_repo_for_testing(repo)?;
        ctx.set_project_meta(project_meta)?;
        let guard = ctx.shared_worktree_access();
        let base = super::get_base_branch_data(&ctx, guard.read_permission())?;
        assert!(
            !base.target_sha_ahead_of_ref,
            "without an upstream there is no upstream divergence"
        );
        assert!(
            base.upstream_commits.is_empty(),
            "local-only targets have no upstream commits"
        );
        assert_eq!(
            base.behind, 0,
            "local-only targets are not behind an upstream"
        );
        assert_eq!(
            base.current_sha,
            Some(local),
            "local tip is the fallback without an upstream"
        );
        Ok(())
    }

    #[test]
    fn short_name_strips_full_remote_ref() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/remotes/origin/feature/foo", Some("origin")),
            "feature/foo"
        );
    }

    #[test]
    fn short_name_strips_short_remote_ref() {
        assert_eq!(
            BaseBranch::compute_short_name("origin/feature/foo", Some("origin")),
            "feature/foo"
        );
    }

    #[test]
    fn short_name_strips_full_remote_ref_simple() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/remotes/origin/main", Some("origin")),
            "main"
        );
    }

    #[test]
    fn short_name_strips_short_remote_ref_simple() {
        assert_eq!(
            BaseBranch::compute_short_name("origin/main", Some("origin")),
            "main"
        );
    }

    #[test]
    fn short_name_different_remote() {
        assert_eq!(
            BaseBranch::compute_short_name(
                "refs/remotes/another-remote/feat/complex-branch-name",
                Some("another-remote")
            ),
            "feat/complex-branch-name"
        );
        assert_eq!(
            BaseBranch::compute_short_name(
                "another-remote/feat/complex-branch-name",
                Some("another-remote")
            ),
            "feat/complex-branch-name"
        );
    }

    #[test]
    fn short_name_non_matching_remote() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/remotes/origin/feature/foo", Some("not-origin")),
            "refs/remotes/origin/feature/foo"
        );
        assert_eq!(
            BaseBranch::compute_short_name("origin/feature/foo", Some("not-origin")),
            "origin/feature/foo"
        );
    }

    #[test]
    fn short_name_heads_ref_with_remote() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/heads/feature/foo", Some("origin")),
            "feature/foo"
        );
    }

    #[test]
    fn short_name_local_name_with_remote() {
        assert_eq!(
            BaseBranch::compute_short_name("feature/foo", Some("origin")),
            "feature/foo"
        );
    }

    #[test]
    fn short_name_heads_ref_no_remote() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/heads/feature/foo", None),
            "feature/foo"
        );
        assert_eq!(
            BaseBranch::compute_short_name("refs/heads/main", None),
            "main"
        );
    }

    #[test]
    fn short_name_local_name_no_remote() {
        assert_eq!(
            BaseBranch::compute_short_name("feature/foo", None),
            "feature/foo"
        );
        assert_eq!(BaseBranch::compute_short_name("main", None), "main");
        assert_eq!(
            BaseBranch::compute_short_name("dev/task/T-123", None),
            "dev/task/T-123"
        );
    }

    #[test]
    fn short_name_branch_equals_remote() {
        assert_eq!(BaseBranch::compute_short_name("origin", Some("origin")), "");
    }

    #[test]
    fn short_name_trailing_slash() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/remotes/origin/", Some("origin")),
            ""
        );
        assert_eq!(BaseBranch::compute_short_name("refs/heads/", None), "");
    }

    #[test]
    fn short_name_embedded_ref_parts() {
        assert_eq!(
            BaseBranch::compute_short_name(
                "refs/remotes/origin/feature/name-with-refs/heads/in-it",
                Some("origin")
            ),
            "feature/name-with-refs/heads/in-it"
        );
    }

    #[test]
    fn short_name_empty_branch() {
        assert_eq!(BaseBranch::compute_short_name("", Some("origin")), "");
        assert_eq!(BaseBranch::compute_short_name("", None), "");
    }

    #[test]
    fn short_name_heads_ref_empty_remote() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/heads/main", Some("")),
            "main",
            "an empty remote is treated like no remote"
        );
    }

    #[test]
    fn short_name_remote_with_slashes() {
        assert_eq!(
            BaseBranch::compute_short_name("refs/remotes/dev/feature/branch", Some("dev/feature")),
            "branch"
        );
        assert_eq!(
            BaseBranch::compute_short_name("dev/feature/branch", Some("dev/feature")),
            "branch"
        );
    }
}

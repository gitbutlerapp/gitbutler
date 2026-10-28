//! Project initialization: set the default target without entering the managed workspace.
//!
//! This is the replacement for the legacy `set_base_branch()`, which coupled setting the
//! target with moving the user onto `refs/heads/gitbutler/workspace`. Now that the
//! application can operate on any branch, the target can be set at any time.

use anyhow::{Context as _, Result};
use bstr::ByteSlice;
use but_core::{
    git_config::{
        edit_repo_config, ensure_config_value, open_repo_local_config_for_reading, set_config_value,
    },
    ref_metadata::{LocalRefName, PROJECT_TARGET_REF, ProjectMeta, UnresolvedLegacyTargetRefError},
};
use but_error::bail_precondition;

const FALLBACK_TARGET_BRANCHES: &[&str] = &["main", "master"];

/// Infer a default target branch without changing repository state.
///
/// If the repository has an unambiguous default fetch remote, prefer the unique local branch
/// tracking its symbolic `HEAD`, followed by those tracking its `main` and `master` branches.
/// Skip remote candidates without a unique local counterpart, then try local `main` and `master`.
/// A repository without remotes and with a single branch, whatever its name, targets that branch.
/// Candidates must resolve to a commit. Returns `None` if no suitable local branch is found.
pub fn infer_default_target_ref(repo: &gix::Repository) -> Result<Option<LocalRefName>> {
    if let Some(remote_name) = repo.remote_default_name(gix::remote::Direction::Fetch) {
        let remote_name = remote_name
            .to_str()
            .context("default remote name is not UTF-8")?;

        let remote_head_ref = format!("refs/remotes/{remote_name}/HEAD");
        if let Ok(head_ref) = repo.find_reference(remote_head_ref.as_str())
            && let Some(branch_name) = head_ref.target().try_name()
            && is_existing_branch_on_remote(repo, branch_name, remote_name)?
            && let Some(local_target) = infer_local_tracking_target(repo, branch_name)?
        {
            return Ok(Some(local_target));
        }

        for branch_name in FALLBACK_TARGET_BRANCHES {
            let full_name: gix::refs::FullName =
                format!("refs/remotes/{remote_name}/{branch_name}").try_into()?;
            if is_existing_branch_on_remote(repo, full_name.as_ref(), remote_name)?
                && let Some(local_target) = infer_local_tracking_target(repo, full_name.as_ref())?
            {
                return Ok(Some(local_target));
            }
        }
    }

    for branch_name in FALLBACK_TARGET_BRANCHES {
        let full_name: gix::refs::FullName = format!("refs/heads/{branch_name}").try_into()?;

        if repo
            .try_find_reference(full_name.as_ref())?
            .is_some_and(|mut reference| reference.peel_to_commit().is_ok())
        {
            return Ok(Some(LocalRefName::new(full_name)?));
        }
    }

    if repo.remote_names().is_empty() {
        return only_local_branch(repo);
    }
    Ok(None)
}

/// The repository's only local branch, not counting GitButler's own under `gitbutler/`, if it
/// resolves to a commit.
fn only_local_branch(repo: &gix::Repository) -> Result<Option<LocalRefName>> {
    let references = repo.references()?;
    let mut branches = references
        .local_branches()?
        .filter_map(Result::ok)
        .filter(|branch| !branch.name().shorten().starts_with(b"gitbutler/"));
    let (Some(mut branch), None) = (branches.next(), branches.next()) else {
        return Ok(None);
    };
    if branch.peel_to_commit().is_err() {
        return Ok(None);
    }
    Ok(Some(LocalRefName::new(branch.name().to_owned())?))
}

/// Find the unique local branch whose configured upstream is `remote_ref`.
///
/// Returns `None` if no branch matches, multiple branches match, or the local tip
/// cannot resolve to a commit.
fn infer_local_tracking_target(
    repo: &gix::Repository,
    remote_ref: &gix::refs::FullNameRef,
) -> Result<Option<LocalRefName>> {
    use but_core::ref_metadata::{LocalTrackingBranchMatch, find_local_tracking_branch};

    let candidate = match find_local_tracking_branch(remote_ref, repo)? {
        LocalTrackingBranchMatch::Unique(name) => name,
        LocalTrackingBranchMatch::None | LocalTrackingBranchMatch::Ambiguous { .. } => {
            return Ok(None);
        }
    };
    Ok(repo
        .try_find_reference(candidate.as_local_full_name_ref())?
        .is_some_and(|mut reference| reference.peel_to_commit().is_ok())
        .then_some(candidate))
}

fn is_existing_branch_on_remote(
    repo: &gix::Repository,
    ref_name: &gix::refs::FullNameRef,
    expected_remote: &str,
) -> Result<bool> {
    let belongs_to_expected_remote =
        but_core::extract_remote_name_and_short_name(ref_name, &repo.remote_names())
            .is_some_and(|(remote_name, _)| remote_name == expected_remote);
    if !belongs_to_expected_remote {
        return Ok(false);
    }

    Ok(repo
        .try_find_reference(ref_name)?
        .is_some_and(|mut reference| reference.peel_to_commit().is_ok()))
}

/// Return the merge-base of `head` and `target_head`, or an actionable precondition failure when
/// their histories are unrelated or incomplete.
fn merge_base_with_target(
    repo: &gix::Repository,
    head: gix::ObjectId,
    target_head: gix::ObjectId,
) -> Result<gix::ObjectId> {
    match repo.merge_base(head, target_head) {
        Ok(Some(id)) => Ok(id.detach()),
        Ok(None) => bail_precondition!(
            "The selected target has no common history with HEAD. Fetch more history or choose another branch."
        ),
        Err(err) => Err(err).context(format!(
            "Failed to calculate merge base between {head} and {target_head}"
        )),
    }
}

/// Return whether `commit` exists and is reachable from `target_head`.
fn target_contains_commit(
    repo: &gix::Repository,
    target_head: gix::ObjectId,
    commit: gix::ObjectId,
) -> Result<bool> {
    if repo.try_find_object(commit)?.is_none() {
        return Ok(false);
    }
    match repo.merge_base(commit, target_head) {
        Ok(id) => Ok(id.is_some_and(|id| id.detach() == commit)),
        Err(err) => Err(err).context(format!(
            "Failed to validate existing target commit {commit} against {target_head}"
        )),
    }
}

/// Resolve the cutoff when explicitly changing targets, using stable tip snapshots.
pub fn resolve_target_commit(
    repo: &gix::Repository,
    head: gix::ObjectId,
    target_head: gix::ObjectId,
    existing: Option<gix::ObjectId>,
) -> Result<gix::ObjectId> {
    let merge_base = merge_base_with_target(repo, head, target_head)?;
    match existing {
        Some(existing) if target_contains_commit(repo, target_head, existing)? => Ok(existing),
        _ => Ok(merge_base),
    }
}

/// Make the local branch `target_ref` the project's default target and initialize project metadata,
/// without changing the currently checked out branch.
///
/// This performs the metadata-only parts of the legacy `set_base_branch()`:
///
/// * repair partially migrated target metadata before applying the target change,
/// * store the target as [`ProjectMeta`] via [`ProjectMeta::persist()`],
/// * set `log.excludeDecoration = refs/gitbutler` in the repository-local Git config.
///
/// The workspace branch and a stored cutoff whose object is missing are rejected without
/// changing metadata.
/// Re-setting the same target preserves its stored cutoff. Without a stored cutoff, initialization
/// uses the configured upstream tip, or the local tip if no upstream is configured. An unavailable
/// upstream is an error, not a reason to use local history instead.
/// Changing targets retains the old cutoff if reachable from the new local target, otherwise it
/// uses the merge-base with `HEAD`. The local target is always validated against `HEAD` before writing.
/// `push_remote`, if `Some`, is validated and stored; if `None`, an existing push remote
/// is kept as is.
///
/// A legacy remote target that no local branch or several local branches track is replaced by
/// `target_ref` as if it was the same target, so its stored cutoff and push remote are kept.
///
/// Unlike `set_base_branch()`, this neither creates stacks, nor updates the workspace
/// commit, nor checks anything out. The caller is expected to hold exclusive worktree
/// access, and to invalidate any cached workspace projection afterwards.
pub fn set_target_ref_and_init_project(
    repo: &gix::Repository,
    target_ref: &LocalRefName,
    push_remote: Option<String>,
) -> Result<()> {
    if but_core::is_workspace_ref_name(target_ref.as_local_full_name_ref()) {
        bail_precondition!(
            "GitButler manages {target_ref} itself, so it can't be the target. Choose another branch."
        );
    }
    let project_meta = match ProjectMeta::resolve(repo) {
        Ok(project_meta) => project_meta,
        // Choosing a target is how the user picks the local branch for a legacy remote target
        // that doesn't identify one, so read the stored metadata as if the choice was stored already.
        Err(err) if err.is::<UnresolvedLegacyTargetRefError>() => {
            let mut config = open_repo_local_config_for_reading(repo)?;
            set_config_value(&mut config, PROJECT_TARGET_REF, &target_ref.to_string())?;
            ProjectMeta::try_from_config(&config, repo)?
        }
        Err(err) => return Err(err),
    };
    if let Some(cutoff) = project_meta.target_commit_id
        && repo.try_find_object(cutoff)?.is_none()
    {
        bail_precondition!(
            "The stored workspace cutoff commit {cutoff} is missing. Fetch or restore the missing history before setting the target."
        );
    }
    let mut updated =
        but_core::ref_metadata::repair_target_metadata_for_migration(&project_meta, repo);
    updated.target_ref = Some(target_ref.clone());

    let target_head = repo
        .try_find_reference(target_ref.as_local_full_name_ref())?
        .with_context(|| format!("target branch '{target_ref}' not found"))?
        .peel_to_commit()
        .with_context(|| format!("failed to peel branch '{target_ref}' to commit"))?
        .id;

    let head = repo.head_id().context("Failed to resolve HEAD")?.detach();
    let changing_target = project_meta
        .target_ref
        .as_ref()
        .is_some_and(|existing| existing != target_ref);
    let target_sha = if changing_target {
        resolve_target_commit(repo, head, target_head, updated.target_commit_id)?
    } else {
        merge_base_with_target(repo, head, target_head)?;
        updated.resolve_target_commit(repo)?
    };

    updated.push_remote = match push_remote {
        Some(name) => {
            repo.find_remote(name.as_str())
                .with_context(|| format!("failed to find remote {name}"))?;
            Some(name)
        }
        // Unlike the legacy `set_base_branch()`, keep an existing push remote instead of
        // clearing it - the target may be (re-)set at any time.
        None => updated.push_remote,
    };

    updated.target_commit_id = Some(target_sha);
    updated.persist(repo)?;

    edit_repo_config(repo, gix::config::Source::Local, |config| {
        ensure_config_value(config, "log.excludeDecoration", "refs/gitbutler")
            .context("failed to set log.excludeDecoration")?;
        Ok(())
    })?;
    Ok(())
}

/// Set the remote used to publish branches without changing the default target.
///
/// The remote and existing target are validated before project metadata is persisted. The caller
/// is expected to hold exclusive repository access and invalidate cached workspace projections
/// afterwards.
pub fn set_push_remote(repo: &gix::Repository, push_remote: String) -> Result<()> {
    let mut project_meta = but_core::ref_metadata::repair_target_metadata_for_migration(
        &ProjectMeta::resolve(repo)?,
        repo,
    );
    project_meta
        .target_ref_or_err()
        .context("cannot set push remote without a default target")?;
    repo.find_remote(push_remote.as_str())
        .with_context(|| format!("failed to find remote {push_remote}"))?;

    project_meta.push_remote = Some(push_remote);
    project_meta.persist(repo)?;
    Ok(())
}

/// Make `commit`, created without a parent, the first commit of the unborn branch `branch`, as
/// `git commit` does in a new repository: the index takes the commit's tree, then `branch` is
/// created. A project without a target then takes the one [`infer_default_target_ref()`] finds
/// now that there is history.
///
/// The caller is expected to hold exclusive worktree access, and to invalidate any cached
/// workspace projection afterwards.
pub fn publish_first_commit(
    repo: &gix::Repository,
    branch: &gix::refs::FullNameRef,
    commit: gix::ObjectId,
) -> Result<()> {
    let tree_id = repo.find_commit(commit)?.tree_id()?;
    let tree_index = repo.index_from_tree(&tree_id)?;
    let mut index = repo.index_or_empty()?.into_owned_or_cloned();
    let removed_paths: Vec<_> = index
        .entries()
        .iter()
        .filter(|entry| {
            tree_index
                .entry_by_path_and_stage(entry.path(&index), gix::index::entry::Stage::Unconflicted)
                .is_none()
        })
        .map(|entry| entry.path(&index).to_owned())
        .collect();
    let mut edit = but_core::index::Edit::new(&mut index);
    for path in removed_paths {
        edit.remove(path.as_ref());
    }
    for entry in tree_index.entries() {
        edit.upsert(
            entry.path(&tree_index),
            entry.id,
            entry.mode,
            but_core::index::Stat::Unknown,
        );
    }
    edit.finish();

    // Persist the index before publishing the commit, so an index-write failure leaves HEAD unborn.
    index.write(Default::default())?;

    repo.reference(
        branch,
        commit,
        gix::refs::transaction::PreviousValue::MustNotExist,
        "commit (initial): initial commit",
    )?;

    // Target inference found nothing while HEAD was unborn. Initialize it now so that
    // workspace projections built right after can resolve it. Only a target this root commit
    // starts shares history with it: on an orphan branch beside an existing one, as a new
    // `gh-pages`, the commit stands and the project keeps no target.
    let mut project_meta = ProjectMeta::resolve(repo)?;
    if project_meta.target_ref.is_none()
        && let Some(target) = infer_default_target_ref(repo)?
        && target.as_local_full_name_ref() == branch
    {
        project_meta.target_ref = Some(target.clone());
        if project_meta.target_commit_id.is_none()
            && let Some(upstream) = project_meta.remote_target_ref(repo)?
            && repo.try_find_reference(upstream.as_ref())?.is_none()
        {
            // Empty clones already configure an upstream, but its ref won't exist
            // until the first push. Use this root commit as the explicit cutoff
            // without removing upstream configuration or fabricating a remote ref.
            project_meta.target_commit_id = Some(commit);
            project_meta.persist(repo)?;
        }
        set_target_ref_and_init_project(repo, &target, None)?;
    }
    Ok(())
}

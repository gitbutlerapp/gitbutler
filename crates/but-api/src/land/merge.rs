//! Deciding the merge topology and building the commit that lands on the target.
//!
//! Lifted from the `but land` CLI command. This is pure `gix`/topology logic with no
//! workspace, stack, or `Context` dependency — it takes a repository and two refs.

use anyhow::bail;
use bstr::ByteSlice;
use but_core::{
    RepositoryExt,
    commit::{SignCommit, TreeKind},
};
use gix::prelude::ObjectIdExt;

/// The merge topology decision for a single landing attempt.
pub(super) enum LandOutcome {
    AlreadyIntegrated,
    FastForward {
        feature_oid: gix::ObjectId,
        target_oid: gix::ObjectId,
    },
    Merge {
        oid: gix::ObjectId,
        target_oid: gix::ObjectId,
    },
}

/// Decide how to land `branch_name` onto the target and, for the merge case, build a signed,
/// rename-aware merge commit. Conflicts bail here, before anything is pushed or moved.
pub(super) fn decide_land_outcome(
    repo: &gix::Repository,
    branch_name: &str,
    fetch_remote_name: &str,
    target_branch_name: &str,
    no_ff: bool,
) -> anyhow::Result<LandOutcome> {
    let feature_ref_name = format!("refs/heads/{branch_name}");
    let feature_oid = repo
        .try_find_reference(&feature_ref_name)?
        .ok_or_else(|| anyhow::anyhow!("Branch {branch_name} not found"))?
        .into_fully_peeled_id()?
        .detach();

    let target_ref_name = format!("refs/remotes/{fetch_remote_name}/{target_branch_name}");
    let target_oid = repo
        .try_find_reference(&target_ref_name)?
        .ok_or_else(|| anyhow::anyhow!("Target branch {target_ref_name} not found"))?
        .into_fully_peeled_id()?
        .detach();

    // No common ancestor: refuse rather than merge two unrelated histories onto the target.
    let Some(merge_base) = super::merge_base_opt(repo, feature_oid, target_oid)? else {
        bail!(
            "Cannot land {branch_name}: it shares no history with {fetch_remote_name}/{target_branch_name}"
        );
    };

    if merge_base == feature_oid {
        return Ok(LandOutcome::AlreadyIntegrated);
    }
    if merge_base == target_oid && !no_ff {
        return Ok(LandOutcome::FastForward {
            feature_oid,
            target_oid,
        });
    }

    // Diverged (or `--no-ff`): build a real merge commit. Use GitButler's canonical tree-merge
    // options (rename tracking on, fail-fast) so a rename+edit can't silently mismerge, and sign
    // the commit per config so it survives signed-branch protection on the target.
    // Use each commit's resolved tree as its side of the 3-way merge. For an ordinary
    // (non-conflicted) commit this is just its tree; `AutoResolution` only matters if a side were a
    // conflicted commit, in which case it picks the resolved tree rather than one conflict side.
    let (merge_options, unresolved) = repo.merge_options_fail_fast()?;
    let base_tree = but_core::Commit::from_id(merge_base.attach(repo))?
        .tree_id_or_kind(TreeKind::AutoResolution)?
        .detach();
    let our_tree = but_core::Commit::from_id(target_oid.attach(repo))?
        .tree_id_or_kind(TreeKind::AutoResolution)?
        .detach();
    let their_tree = but_core::Commit::from_id(feature_oid.attach(repo))?
        .tree_id_or_kind(TreeKind::AutoResolution)?
        .detach();

    let mut merge = repo.merge_trees(
        base_tree,
        our_tree,
        their_tree,
        repo.default_merge_labels(),
        merge_options,
    )?;
    if merge.has_unresolved_conflicts(unresolved) {
        let paths: Vec<String> = merge
            .conflicts
            .iter()
            .filter_map(|c| c.ours.location().to_str().ok().map(ToOwned::to_owned))
            .collect();
        let detail = if paths.is_empty() {
            String::new()
        } else {
            format!(" Conflicting paths: {}.", paths.join(", "))
        };
        bail!(
            "Cannot land {branch_name}: merging into {fetch_remote_name}/{target_branch_name} \
             resulted in conflicts.{detail} Rebase {branch_name} onto the target and resolve, then \
             re-run `but land {branch_name}`."
        );
    }
    let merged_tree = merge.tree.write()?.detach();

    // Build the 2-parent merge commit. Parent order is `[target, feature]` so `--first-parent`
    // mainline walks stay correct. `commit_signatures()` honors `gitbutler.gitbutlerCommitter`.
    let (author, committer) = repo.commit_signatures()?;
    let mut commit = gix::objs::Commit {
        tree: merged_tree,
        parents: [target_oid, feature_oid].into_iter().collect(),
        author,
        committer,
        encoding: None,
        message: format!("Merge branch '{branch_name}'").into(),
        extra_headers: Vec::new(),
    };
    // Set the change-id header before creating the commit; `create` only touches the signature.
    but_core::commit::Headers::from_config(&repo.config_snapshot()).set_in_commit(&mut commit);
    let oid = but_core::commit::create(repo, commit, None, SignCommit::IfSignCommitsEnabled)?;

    Ok(LandOutcome::Merge { oid, target_oid })
}

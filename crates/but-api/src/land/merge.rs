//! Deciding the merge topology and building the commit that lands on the target.
//!
//! Lifted from the `but merge` CLI command. This is pure `gix`/topology logic with no
//! workspace, stack, or `Context` dependency — it takes a repository, the branch names, and the
//! target ref.

use anyhow::bail;
use bstr::ByteSlice;
use but_core::{
    RepositoryExt,
    commit::{SignCommit, TreeKind},
};
use gix::prelude::ObjectIdExt;

/// The combined result of landing a sequence of branches onto the target, for a single attempt.
pub(super) struct LandPlan {
    /// The commit the target should point at; equal to `target_oid` when every branch was already
    /// integrated.
    pub new_target_oid: gix::ObjectId,
    /// The target tip the plan was built on.
    pub target_oid: gix::ObjectId,
    /// The branches that were already reachable from the target, or from the branches landed
    /// before them.
    pub already_integrated: Vec<String>,
}

/// Land `branches` onto the target one after another, in order: each branch fast-forwards the
/// running tip when it can, otherwise it gets a signed, rename-aware merge commit on top of it.
/// Nothing is pushed or moved here — the caller delivers `new_target_oid` once — and a conflict in
/// any branch bails before anything is published.
pub(super) fn plan_land(
    repo: &gix::Repository,
    branches: &[String],
    fetch_remote_name: &str,
    target_branch_name: &str,
    no_ff: bool,
) -> anyhow::Result<LandPlan> {
    let target_display = format!("{fetch_remote_name}/{target_branch_name}");
    let target_ref_name = format!("refs/remotes/{target_display}");
    let target_oid = repo
        .try_find_reference(&target_ref_name)?
        .ok_or_else(|| anyhow::anyhow!("Target branch {target_ref_name} not found"))?
        .into_fully_peeled_id()?
        .detach();

    let mut tip = target_oid;
    let mut landed = Vec::new();
    let mut already_integrated = Vec::new();
    for branch in branches {
        match land_onto(repo, branch, tip, &target_display, &landed, no_ff)? {
            Some(new_tip) => {
                tip = new_tip;
                landed.push(branch.clone());
            }
            None => already_integrated.push(branch.clone()),
        }
    }
    Ok(LandPlan {
        new_target_oid: tip,
        target_oid,
        already_integrated,
    })
}

/// Land `branch_name` onto `target_oid`, returning the new tip, or `None` when the branch is
/// already reachable from it. `landed_before` names the branches already landed onto `target_oid`
/// in this plan, for error messages.
fn land_onto(
    repo: &gix::Repository,
    branch_name: &str,
    target_oid: gix::ObjectId,
    target_display: &str,
    landed_before: &[String],
    no_ff: bool,
) -> anyhow::Result<Option<gix::ObjectId>> {
    let feature_ref_name = format!("refs/heads/{branch_name}");
    let feature_oid = repo
        .try_find_reference(&feature_ref_name)?
        .ok_or_else(|| anyhow::anyhow!("Branch {branch_name} not found"))?
        .into_fully_peeled_id()?
        .detach();

    // No common ancestor: refuse rather than merge two unrelated histories onto the target.
    let Some(merge_base) = super::merge_base_opt(repo, feature_oid, target_oid)? else {
        bail!("Cannot merge {branch_name}: it shares no history with {target_display}");
    };

    if merge_base == feature_oid {
        return Ok(None);
    }
    if merge_base == target_oid && !no_ff {
        return Ok(Some(feature_oid));
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
        if landed_before.is_empty() {
            bail!(
                "Cannot merge {branch_name}: merging into {target_display} resulted in \
                 conflicts.{detail} Rebase {branch_name} onto the target and resolve, then re-run \
                 `but merge {branch_name}`."
            );
        }
        bail!(
            "Cannot merge {branch_name}: merging into {target_display} after landing {} resulted \
             in conflicts.{detail} Nothing was landed. Land the others first, then rebase \
             {branch_name} onto the target, resolve, and run `but merge {branch_name}`.",
            landed_before.join(", "),
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

    Ok(Some(oid))
}

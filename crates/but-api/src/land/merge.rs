//! Working out what the target points at after landing a branch, and creating the merge commit
//! when one is needed.
//!
//! Lifted from the `but merge` CLI command. It only needs a repository, the branch names and the
//! target ref, not a workspace, stacks or a `Context`.

use bstr::ByteSlice;
use but_core::{
    RepositoryExt,
    commit::{SignCommit, TreeKind},
};
use but_error::bail_precondition;
use gix::prelude::ObjectIdExt;

/// The combined result of landing a sequence of branches onto the target, for a single attempt.
pub(super) struct LandPlan {
    /// The commit the target should point at; equal to `target_oid` when every branch was already
    /// integrated.
    pub new_target_oid: gix::ObjectId,
    /// The target tip the plan was built on.
    pub target_oid: gix::ObjectId,
    /// The branches that were already reachable from the target before landing. A branch that a
    /// branch landed earlier in the plan brings along isn't one: it lands with this plan.
    pub already_integrated: Vec<String>,
}

/// Land `branches` onto `target_ref` one after another, in order, without moving any refs: each
/// branch fast-forwards the running tip when it can (and `no_ff` isn't set), otherwise it gets a
/// merge commit on top of it, signed if commit signing is configured. Merge commits are only
/// written to the object database; the caller moves the target to `new_target_oid` once.
///
/// `target_ref` is the target's remote-tracking ref, or the local target branch when it has no
/// upstream. `expected_tips` is empty, or holds the tip the caller showed for each branch, in the
/// same order.
///
/// Fails, without moving anything, if any branch shares no history with the target, if merging it
/// conflicts, if `ff_only` is set and it has diverged from the running tip, or if it no longer
/// points at its expected tip.
pub(super) fn plan_land(
    repo: &gix::Repository,
    branches: &[String],
    expected_tips: &[gix::ObjectId],
    target_ref: &gix::refs::FullNameRef,
    no_ff: bool,
    ff_only: bool,
) -> anyhow::Result<LandPlan> {
    if !expected_tips.is_empty() && expected_tips.len() != branches.len() {
        anyhow::bail!(
            "Got {} expected tips for {} branches",
            expected_tips.len(),
            branches.len()
        );
    }
    let target_oid = repo
        .try_find_reference(target_ref)?
        .ok_or_else(|| anyhow::anyhow!("Target branch {target_ref} not found"))?
        .into_fully_peeled_id()?
        .detach();
    let target_display = target_ref.shorten().to_string();

    let mut tip = target_oid;
    let mut landed = Vec::new();
    let mut already_integrated = Vec::new();
    for (idx, branch) in branches.iter().enumerate() {
        let expected_tip = expected_tips.get(idx).copied();
        match land_onto(
            repo,
            branch,
            expected_tip,
            tip,
            &target_display,
            &landed,
            no_ff,
            ff_only,
        )? {
            Some(new_tip) => {
                tip = new_tip;
                landed.push(branch.clone());
            }
            // Reachable from the running tip only because a branch landed earlier in this plan
            // brought it along (as `but merge top bottom` does): it lands now, not before.
            None => {
                let feature_oid = super::peel_ref(repo, &format!("refs/heads/{branch}"))?
                    .ok_or_else(|| anyhow::anyhow!("Branch {branch} not found"))?;
                if super::target_ref_contains(repo, feature_oid, target_oid)? {
                    already_integrated.push(branch.clone());
                } else {
                    landed.push(branch.clone());
                }
            }
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
#[expect(clippy::too_many_arguments)]
fn land_onto(
    repo: &gix::Repository,
    branch_name: &str,
    expected_tip: Option<gix::ObjectId>,
    target_oid: gix::ObjectId,
    target_display: &str,
    landed_before: &[String],
    no_ff: bool,
    ff_only: bool,
) -> anyhow::Result<Option<gix::ObjectId>> {
    let feature_ref_name = format!("refs/heads/{branch_name}");
    let feature_oid = repo
        .try_find_reference(&feature_ref_name)?
        .ok_or_else(|| anyhow::anyhow!("Branch {branch_name} not found"))?
        .into_fully_peeled_id()?
        .detach();
    if expected_tip.is_some_and(|tip| tip != feature_oid) {
        bail_precondition!(
            "Refusing to land `{branch_name}`: it changed after you confirmed landing it. Look at \
             its commits again first."
        );
    }

    // No common ancestor: refuse rather than merge two unrelated histories onto the target.
    let Some(merge_base) = super::merge_base_opt(repo, feature_oid, target_oid)? else {
        bail_precondition!(
            "Cannot merge {branch_name}: it shares no history with {target_display}"
        );
    };

    if merge_base == feature_oid {
        return Ok(None);
    }
    if merge_base == target_oid && !no_ff {
        return Ok(Some(feature_oid));
    }
    if ff_only {
        bail_precondition!(
            "Cannot fast-forward {target_display} to {branch_name}: the \
             branch has diverged from the target. Rebase {branch_name} onto the target (`but pull`), \
             or re-run without `--ff-only` to create a merge commit."
        );
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
            bail_precondition!(
                "Cannot merge {branch_name}: merging into {target_display} resulted in \
                 conflicts.{detail} Rebase {branch_name} onto the target and resolve, then re-run \
                 `but merge {branch_name}`."
            );
        }
        bail_precondition!(
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

#[cfg(test)]
mod tests {
    use but_testsupport::{CommandExt as _, git_at_dir, open_repo};

    use super::plan_land;

    /// A branch that moved after the land was confirmed is refused, as the push can't be undone,
    /// and at the tip the caller showed it lands.
    #[test]
    fn refuses_a_branch_that_moved_after_it_was_confirmed() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        let git = || {
            let mut cmd = git_at_dir(tmp.path());
            cmd.args([
                "-c",
                "user.name=GitButler",
                "-c",
                "user.email=gitbutler@example.com",
            ]);
            cmd
        };
        git().args(["init", "-q"]).run();
        git()
            .args(["commit", "-q", "--allow-empty", "-m", "base"])
            .run();
        git()
            .args(["update-ref", "refs/remotes/origin/main", "HEAD"])
            .run();
        git().args(["checkout", "-q", "-b", "feature"]).run();
        git()
            .args(["commit", "-q", "--allow-empty", "-m", "shown"])
            .run();
        let repo = open_repo(tmp.path())?;
        let shown = repo.rev_parse_single("feature")?.detach();
        git()
            .args(["commit", "-q", "--allow-empty", "-m", "added after"])
            .run();

        let target_ref: gix::refs::FullName = "refs/remotes/origin/main".try_into()?;
        let err = plan_land(
            &repo,
            &["feature".to_owned()],
            &[shown],
            target_ref.as_ref(),
            false,
            false,
        )
        .err()
        .expect("feature moved past the tip that was shown");
        assert_eq!(
            err.to_string(),
            "Refusing to land `feature`: it changed after you confirmed landing it. Look at its \
             commits again first.",
            "the commit added after the confirmation must not be landed"
        );

        let tip = repo.rev_parse_single("feature")?.detach();
        let plan = plan_land(
            &repo,
            &["feature".to_owned()],
            &[tip],
            target_ref.as_ref(),
            false,
            false,
        )?;
        assert_eq!(
            plan.new_target_oid, tip,
            "at the tip that was shown, feature is ahead of the target and fast-forwards it"
        );
        Ok(())
    }
}

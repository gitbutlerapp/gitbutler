//! Working out what the target points at after landing a branch, and creating the merge commit
//! when one is needed.
//!
//! Lifted from the `but merge` CLI command. It only needs a repository and two refs, not a
//! workspace, stacks or a `Context`.

use anyhow::bail;
use bstr::ByteSlice;
use but_core::{
    RepositoryExt,
    commit::{SignCommit, TreeKind},
};
use gix::prelude::ObjectIdExt;

/// Where the target should point after landing a branch, as worked out by
/// [`merge_or_fast_forward`]. For a merge, the merge commit has already been created, but no ref
/// points at it yet: moving the target is up to the caller.
pub(super) enum LandOutcome {
    /// The target already contains the branch, so there is nothing to do.
    AlreadyIntegrated,
    /// The target can move straight to the branch tip, `feature_oid`. It points at `target_oid`
    /// now.
    FastForward {
        feature_oid: gix::ObjectId,
        target_oid: gix::ObjectId,
    },
    /// `oid` is a new merge commit of the target and the branch. The target points at
    /// `target_oid` now.
    Merge {
        oid: gix::ObjectId,
        target_oid: gix::ObjectId,
    },
}

/// Merge `branch_name` into `target_ref`, or fast-forward to it, without moving any refs.
///
/// `target_ref` is the target's remote-tracking ref, or the local target branch when it has no
/// upstream.
///
/// - If the target already contains the branch, this returns [`LandOutcome::AlreadyIntegrated`].
/// - If the target can be fast-forwarded to the branch and `no_ff` isn't set, this returns
///   [`LandOutcome::FastForward`].
/// - Otherwise this creates a merge commit, signed if commit signing is configured, and returns
///   [`LandOutcome::Merge`]. The commit is only written to the object database; it is up to the
///   caller to move the target to it.
///
/// Fails, without moving anything, if the branch shares no history with the target, if merging
/// them conflicts, if `ff_only` is set and the branch has diverged from the target, or if
/// `expected_tip` is given and the branch no longer points at it.
pub(super) fn merge_or_fast_forward(
    repo: &gix::Repository,
    branch_name: &str,
    target_ref: &gix::refs::FullNameRef,
    no_ff: bool,
    ff_only: bool,
    expected_tip: Option<gix::ObjectId>,
) -> anyhow::Result<LandOutcome> {
    let feature_ref_name = format!("refs/heads/{branch_name}");
    let feature_oid = repo
        .try_find_reference(&feature_ref_name)?
        .ok_or_else(|| anyhow::anyhow!("Branch {branch_name} not found"))?
        .into_fully_peeled_id()?
        .detach();
    if expected_tip.is_some_and(|tip| tip != feature_oid) {
        bail!(
            "Refusing to land `{branch_name}`: it changed after you confirmed landing it. Look at \
             its commits again first."
        );
    }

    let target_oid = repo
        .try_find_reference(target_ref)?
        .ok_or_else(|| anyhow::anyhow!("Target branch {target_ref} not found"))?
        .into_fully_peeled_id()?
        .detach();
    let target_display = target_ref.shorten();

    // No common ancestor: refuse rather than merge two unrelated histories onto the target.
    let Some(merge_base) = super::merge_base_opt(repo, feature_oid, target_oid)? else {
        bail!("Cannot merge {branch_name}: it shares no history with {target_display}");
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
    if ff_only {
        bail!(
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
        bail!(
            "Cannot merge {branch_name}: merging into {target_display} \
             resulted in conflicts.{detail} Rebase {branch_name} onto the target and resolve, then \
             re-run `but merge {branch_name}`."
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

#[cfg(test)]
mod tests {
    use but_testsupport::{CommandExt as _, git_at_dir, open_repo};

    use super::{LandOutcome, merge_or_fast_forward};

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
        let err = merge_or_fast_forward(
            &repo,
            "feature",
            target_ref.as_ref(),
            false,
            false,
            Some(shown),
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
        let outcome = merge_or_fast_forward(
            &repo,
            "feature",
            target_ref.as_ref(),
            false,
            false,
            Some(tip),
        )?;
        assert!(
            matches!(outcome, LandOutcome::FastForward { feature_oid, .. } if feature_oid == tip),
            "at the tip that was shown, feature is ahead of the target and fast-forwards it"
        );
        Ok(())
    }
}

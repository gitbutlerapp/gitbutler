//! Workspace-commit replay during graph rebases.

use anyhow::Result;
use but_core::{RepositoryExt, commit::SignCommit};
use gix::prelude::ObjectIdExt;

use super::cherry_pick::{CherryPickOutcome, commit_from_unconflicted_tree};

/// Replay the sole rewritten parent's delta onto a managed workspace commit's
/// recorded tree.
///
/// A managed workspace tree already records how its independent parents were
/// merged. Re-merging those parents from scratch can therefore conflict even
/// when changing one parent is applicable. This fallback preserves the
/// recorded merge and only applies the one parent rewrite we can align
/// unambiguously. If that requires rename detection, only exact renames from
/// the recorded workspace side are considered; a deletion in the rewritten
/// parent makes that distinction ambiguous. More general workspace
/// reconstruction belongs at a higher layer with stack metadata.
pub(crate) fn replay_single_parent_delta(
    repo: &gix::Repository,
    target_id: gix::ObjectId,
    ontos: &[gix::ObjectId],
    sign_commit: SignCommit,
) -> Result<Option<CherryPickOutcome>> {
    let target = but_core::Commit::from_id(target_id.attach(repo))?;
    if !but_graph::workspace::commit::is_managed_workspace_by_message(target.inner.message.as_ref())
        || target.is_conflicted()
        || target.parents.len() != ontos.len()
    {
        return Ok(None);
    }

    let mut changed = target
        .parents
        .iter()
        .zip(ontos)
        .filter(|(old, new)| old != new);
    let Some((old, new)) = changed.next() else {
        return Ok(None);
    };
    if changed.next().is_some() {
        return Ok(None);
    }

    let base = but_core::Commit::from_id(old.attach(repo))?
        .tree_id_or_auto_resolution()?
        .detach();
    let theirs = but_core::Commit::from_id(new.attach(repo))?
        .tree_id_or_auto_resolution()?
        .detach();
    let ours = target.tree_id_or_auto_resolution()?.detach();

    let (options, conflict_kind) = repo.merge_options_no_rewrites_fail_fast()?;
    let mut merge = repo.merge_trees(base, ours, theirs, repo.default_merge_labels(), options)?;
    if merge.has_unresolved_conflicts(conflict_kind) {
        if parent_delta_has_deletions(repo, base, theirs)? {
            return Ok(None);
        }
        let (options, conflict_kind) = repo.merge_options_fail_fast()?;
        let options = options.with_rewrites(Some(gix::diff::Rewrites {
            percentage: None,
            ..Default::default()
        }));
        merge = repo.merge_trees(base, ours, theirs, repo.default_merge_labels(), options)?;
        if merge.has_unresolved_conflicts(conflict_kind) {
            return Ok(None);
        }
    }
    let tree = merge.tree.write()?.detach();
    Ok(Some(CherryPickOutcome::Commit(
        commit_from_unconflicted_tree(ontos, target, tree.attach(repo), sign_commit)?.detach(),
    )))
}

fn parent_delta_has_deletions(
    repo: &gix::Repository,
    base: gix::ObjectId,
    theirs: gix::ObjectId,
) -> Result<bool> {
    let base = repo.find_tree(base)?;
    let theirs = repo.find_tree(theirs)?;
    let changes = repo.diff_tree_to_tree(
        Some(&base),
        Some(&theirs),
        Some(gix::diff::Options::default()),
    )?;
    Ok(changes.into_iter().any(|change| {
        matches!(
            change,
            gix::diff::tree_with_rewrites::Change::Deletion { .. }
        )
    }))
}

#[cfg(test)]
mod tests {
    use but_core::commit::SignCommit;

    use super::replay_single_parent_delta;

    #[test]
    fn rejects_rename_inference() -> anyhow::Result<()> {
        let repo = but_testsupport::read_only_in_memory_scenario("cherry-pick-rename-detection")?;
        let workspace = repo.rev_parse_single("workspace-before")?.detach();
        let stack1 = repo.rev_parse_single("stack-1")?.detach();
        let stack2_after = repo.rev_parse_single("stack-2-after")?.detach();

        let outcome =
            replay_single_parent_delta(&repo, workspace, &[stack1, stack2_after], SignCommit::No)?;

        assert!(
            outcome.is_none(),
            "an exact delete/add must not absorb a sibling modification"
        );
        Ok(())
    }

    #[test]
    fn rejects_more_than_one_rewritten_parent() -> anyhow::Result<()> {
        let repo = but_testsupport::read_only_in_memory_scenario("cherry-pick-rename-detection")?;
        let workspace = repo.rev_parse_single("workspace-before")?.detach();
        let base = repo.rev_parse_single("base")?.detach();
        let stack2_after = repo.rev_parse_single("stack-2-after")?.detach();

        let outcome =
            replay_single_parent_delta(&repo, workspace, &[base, stack2_after], SignCommit::No)?;

        assert!(
            outcome.is_none(),
            "multiple parent rewrites require workspace-level reconstruction"
        );
        Ok(())
    }
}

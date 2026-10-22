use but_error::bail_precondition;
use but_oxidize::{ObjectIdExt, OidExt as _};
use gix::{merge::tree::TreatAsUnresolved, prelude::ObjectIdExt as _, refs::Target};
use tracing::instrument;

use crate::{RepositoryExt, update_head_reference};

use super::{Options, Outcome, write};

/// Perform all file operations necessary to turn the *worktree* of `repo` into
/// `new_head_id^{tree}`.
///
/// If `new_head_id` is a *commit*, we will also set `HEAD` (or the ref it points to if symbolic) to the `new_head_id`.
/// We will also update the `.git/index` to match the `new_head_id^{tree}`.
/// GitButler-conflicted commits are rejected by default before any worktree, index, or ref update.
///
/// We will always handle changes in the worktree safely to avoid loss of uncommitted information. This also means that deletions
/// never cause us to conflict. Conflicted files that would be checked out will cause an error.
///
/// #### Note: No rename tracking
///
/// To keep it simpler, we don't do rename tracking, so deletions and additions are always treated separately.
/// If this changes, then the source sid of a rename could also cause conflicts, maybe? It's a bit unclear what it would mean
/// in practice, but I guess that we bring deleted files back instead of conflicting.
#[instrument(skip(repo), err(Debug))]
pub fn safe_checkout_from_head(
    new_head_id: gix::ObjectId,
    repo: &gix::Repository,
    Options {
        skip_head_update,
        merge_base_override,
        allow_conflicted_commit_checkout,
        allow_uncommitted_changes_to_conflict_with_new_head,
    }: Options,
) -> anyhow::Result<Outcome> {
    let new_object = new_head_id.attach(repo).object()?;
    if !allow_conflicted_commit_checkout
        && new_object.kind.is_commit()
        && crate::Commit::from_id(new_head_id.attach(repo))?.is_conflicted()
    {
        return Err(
            anyhow::anyhow!(but_error::Code::ConflictedCommitCheckout).context(format!(
                "Refusing to check out conflicted commit {new_head_id}"
            )),
        );
    }

    let git2_repo = but_oxidize::open_git2_repo(repo.git_dir())?;
    let head_tree_id = repo.head_tree_id_or_empty()?;
    let head_tree = git2_repo.find_tree(head_tree_id.to_git2())?;
    let old_tree = match merge_base_override {
        Some(id) => git2_repo.find_object(id.to_git2(), None)?.peel_to_tree()?,
        None => git2_repo.find_tree(head_tree_id.to_git2())?,
    };
    let new_tree = git2_repo
        .find_object(new_head_id.to_git2(), None)?
        .peel_to_tree()?;
    let mut conflict_occurred = false;
    if merge_base_override.is_some() || old_tree.id() != new_tree.id() {
        let mut index = write::LockedIndex::acquire(repo)?;
        if merge_base_override.is_some() {
            index.entries = index_with_changes_applied(&git2_repo, repo, &head_tree, &old_tree)?;
        }
        if old_tree.id() == new_tree.id() {
            index.write()?;
        } else {
            conflict_occurred = write_merged_worktree(
                repo,
                [old_tree.id().to_gix(), new_tree.id().to_gix()],
                index,
                allow_uncommitted_changes_to_conflict_with_new_head,
            )?;
        }
    }

    let mut head_update = None;
    if new_object.kind.is_commit() && !skip_head_update {
        let needs_update = repo
            .head()?
            .id()
            .is_none_or(|actual_head_id| actual_head_id != new_head_id);
        if needs_update {
            // We play it loose here, as we assume a repository lock so we won't interfere with ourselves.
            // Git itself enforces no lock either, so we rely on basic locking ref-locking here. Good enough.
            let edits = update_head_reference(
                repo,
                Target::Object(new_head_id),
                true,
                "safe checkout",
                "GitButler".into(),
                new_object.into_commit().parent_ids().count(),
            )?;
            head_update = Some(edits);
        }
    }

    Ok(Outcome {
        head_update,
        conflict_occurred,
    })
}

/// Merge the worktree into the change from `old_tree` to `new_tree`, and write the result to the
/// worktree and `index`. Return `true` if conflicts were written, which `allow_conflicts` permits.
fn write_merged_worktree(
    repo: &gix::Repository,
    [old_tree, new_tree]: [gix::ObjectId; 2],
    index: write::LockedIndex,
    allow_conflicts: bool,
) -> anyhow::Result<bool> {
    // Reopen to ensure that there is no "object memory" (i.e. all object
    // writes actually happen on disk).
    let mut repo = gix::open(repo.git_dir())?;
    // Like Git, don't write over an unresolved conflict, as the write would replace its stages.
    let mut conflicted: Vec<_> = index
        .entries
        .entries()
        .iter()
        .filter(|entry| entry.stage() != gix::index::entry::Stage::Unconflicted)
        .map(|entry| format!("{:?}", entry.path(&index.entries)))
        .collect();
    if !conflicted.is_empty() {
        conflicted.dedup();
        bail_precondition!(
            "Unresolved conflicts in the index must be resolved before checkout: {}",
            conflicted.join(", ")
        );
    }
    #[allow(deprecated)]
    let wd_tree_id = repo.create_wd_tree(u64::MAX)?;
    repo.config_snapshot_mut()
        .set_value(&gix::config::tree::Merge::RENORMALIZE, "true")?;
    let mut outcome = repo.merge_trees(
        old_tree,
        wd_tree_id,
        new_tree,
        repo.default_merge_labels(),
        repo.tree_merge_options()?.with_rewrites(None),
    )?;
    let checkout_target_id = outcome.tree.write()?.detach();

    let unresolved = TreatAsUnresolved::git();
    if outcome.has_unresolved_conflicts(unresolved) {
        if allow_conflicts {
            for conflict in outcome.conflicts.iter() {
                if !conflict.is_unresolved(unresolved) {
                    continue;
                }
                let [base, ours, theirs] = conflict.entries();
                if base.is_some_and(|c| c.mode.is_tree())
                    || ours.is_some_and(|c| c.mode.is_tree())
                    || theirs.is_some_and(|c| c.mode.is_tree())
                {
                    bail_precondition!(
                        "Cannot checkout file-directory conflict: {}",
                        conflict.ours.location()
                    );
                }
            }
        } else {
            let mut paths = outcome
                .conflicts
                .iter()
                .filter(|c| c.is_unresolved(unresolved))
                .map(|c| format!("{:?}", c.ours.location()))
                .collect::<Vec<_>>();
            paths.sort();
            paths.dedup();
            bail_precondition!(
                "Uncommitted files would be overwritten by checkout: {}",
                paths.join(", ")
            );
        }
    }

    let conflict_occurred = outcome.has_unresolved_conflicts(unresolved);
    let trees = write::Trees::new(&repo, old_tree, new_tree, [wd_tree_id, checkout_target_id])?;
    write::write_to_worktree(&repo, trees, conflict_occurred.then_some(&outcome), index)?;
    Ok(conflict_occurred)
}

/// `.git/index` with the changes from `head_tree` to `tree` applied as libgit2 applies them, or
/// `tree` itself if they don't apply.
///
/// libgit2 applies them to a copy, so `.git/index` only changes when the checkout writes it.
fn index_with_changes_applied(
    git2_repo: &git2::Repository,
    repo: &gix::Repository,
    head_tree: &git2::Tree<'_>,
    tree: &git2::Tree<'_>,
) -> anyhow::Result<gix::index::File> {
    let dir = tempfile::tempdir_in(repo.git_dir())?;
    let copy = dir.path().join("index");
    if repo.index_path().exists() {
        std::fs::copy(repo.index_path(), &copy)?;
    }
    let mut index = git2::Index::open_ext(&copy, git2_repo.object_format())?;
    git2_repo.set_index(&mut index)?;
    let mut opts = git2::DiffOptions::new();
    opts.context_lines(1);
    let diff = git2_repo.diff_tree_to_tree(Some(head_tree), Some(tree), Some(&mut opts))?;
    if git2_repo
        .apply(&diff, git2::ApplyLocation::Index, None)
        .is_err()
    {
        index.read_tree(tree)?;
        index.write()?;
    }
    Ok(gix::index::File::at_or_default(
        &copy,
        repo.object_hash(),
        false,
        Default::default(),
    )?)
}

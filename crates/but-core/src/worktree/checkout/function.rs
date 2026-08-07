use anyhow::bail;
use bstr::BStr;
use but_error::bail_precondition;
use but_oxidize::{ObjectIdExt, OidExt as _};
use gix::{
    merge::{plumbing::tree::ConflictIndexEntry, tree::TreatAsUnresolved},
    prelude::ObjectIdExt as _,
    refs::Target,
};
use tracing::instrument;

use crate::{RepositoryExt, update_head_reference};

use super::{Options, Outcome};

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
        bail!("Refusing to check out conflicted commit {new_head_id}");
    }

    let git2_repo = git2::Repository::open(repo.git_dir())?;
    let head_tree_id = repo.head_tree_id_or_empty()?;
    let head_tree = git2_repo.find_tree(head_tree_id.to_git2())?;
    let old_tree = if let Some(id) = merge_base_override {
        let mut opts = git2::DiffOptions::new();
        opts.context_lines(1);
        // Also write the index.
        let tree = git2_repo.find_object(id.to_git2(), None)?.peel_to_tree()?;
        let diff = git2_repo.diff_tree_to_tree(Some(&head_tree), Some(&tree), Some(&mut opts))?;
        if git2_repo
            .apply(&diff, git2::ApplyLocation::Index, None)
            .is_err()
        {
            // Just overwrite the index.
            git2_repo.index()?.read_tree(&tree)?;
        }
        tree
    } else {
        head_tree
    };

    let new_tree = git2_repo
        .find_object(new_head_id.to_git2(), None)?
        .peel_to_tree()?;
    let mut conflict_occurred = false;
    if old_tree.id() != new_tree.id() {
        // Reopen to ensure that there is no "object memory" (i.e. all object
        // writes actually happen on disk).
        let mut repo = gix::open(repo.git_dir())?;
        #[allow(deprecated)]
        let wd_tree_id = repo.create_wd_tree(u64::MAX)?;
        let wd_tree = git2_repo
            .find_object(wd_tree_id.to_git2(), None)?
            .peel_to_tree()?;
        repo.config_snapshot_mut()
            .set_value(&gix::config::tree::Merge::RENORMALIZE, "true")?;
        let mut outcome = repo.merge_trees(
            old_tree.id().to_gix(),
            wd_tree_id,
            new_tree.id().to_gix(),
            Default::default(),
            repo.tree_merge_options()?.with_rewrites(None),
        )?;
        let checkout_target_base_id = outcome.tree.write()?.detach();

        let checkout_target_base = git2_repo.find_tree(checkout_target_base_id.to_git2())?;
        let mut checkout_target = git2::Index::new()?;
        checkout_target.read_tree(&checkout_target_base)?;

        let unresolved = TreatAsUnresolved::git();
        if outcome.has_unresolved_conflicts(unresolved) {
            conflict_occurred = true;
            if allow_uncommitted_changes_to_conflict_with_new_head {
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
                    fn to_git2_index_entry(
                        entry: &ConflictIndexEntry,
                        path: &BStr,
                    ) -> git2::IndexEntry {
                        git2::IndexEntry {
                            ctime: git2::IndexTime::new(0, 0),
                            mtime: git2::IndexTime::new(0, 0),
                            dev: 0,
                            ino: 0,
                            mode: entry.mode.value() as u32,
                            uid: 0,
                            gid: 0,
                            file_size: 0,
                            id: entry.id.to_git2(),
                            flags: 0,
                            flags_extended: 0,
                            path: path.to_vec(),
                        }
                    }
                    let base_index_entry =
                        base.map(|c| to_git2_index_entry(&c, conflict.ours.location()));
                    let ours_index_entry =
                        ours.map(|c| to_git2_index_entry(&c, conflict.ours.location()));
                    let theirs_index_entry =
                        theirs.map(|c| to_git2_index_entry(&c, conflict.ours.location()));
                    checkout_target.conflict_add(
                        base_index_entry.as_ref(),
                        ours_index_entry.as_ref(),
                        theirs_index_entry.as_ref(),
                    )?;
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

        let mut checkout_opts = git2::build::CheckoutBuilder::new();
        checkout_opts.baseline(&wd_tree);
        git2_repo.checkout_index(Some(&mut checkout_target), Some(&mut checkout_opts))?;
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

use anyhow::Context as _;
use but_error::bail_precondition;
use gix::{merge::tree::TreatAsUnresolved, prelude::ObjectIdExt as _, refs::Target};
use tracing::instrument;

use crate::{RepositoryExt, update_head_reference};

use super::{Options, Outcome};

/// Perform all file operations necessary to turn the *worktree* of `repo` into
/// `new_head_id^{tree}`.
///
/// If `new_head_id` is a *commit*, we will also set `HEAD` (or the ref it points to if symbolic) to the `new_head_id`.
/// The index is updated to reflect checkout unless [`Options::skip_index_update`] leaves it to the caller.
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
        skip_index_update,
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

    // Keep generated merge blobs on disk, including those used only by the prepared index.
    let mut checkout_repo = gix::open(repo.git_dir())?;
    let head_tree_id = checkout_repo.head_tree_id_or_empty()?.detach();
    let old_tree_id = match merge_base_override {
        Some(id) => checkout_repo.find_object(id)?.peel_to_tree()?.id,
        None => head_tree_id,
    };
    let new_tree_id = checkout_repo.find_object(new_head_id)?.peel_to_tree()?.id;
    let index_lock = if skip_index_update {
        None
    } else {
        Some(
            gix::lock::File::acquire_to_update_resource(
                checkout_repo.index_path(),
                gix::lock::acquire::Fail::Immediately,
                None,
                0,
            )
            .context("Could not lock the index for checkout")?,
        )
    };
    let mut index = checkout_repo.index_or_empty()?.into_owned_or_cloned();
    let mut index_changed = if skip_index_update {
        index = checkout_repo.index_from_tree_with_index(&old_tree_id, &index)?;
        false
    } else if old_tree_id != head_tree_id {
        prepare_index_for_merge_base(&checkout_repo, &mut index, head_tree_id, old_tree_id)?
    } else {
        false
    };
    let mut conflict_occurred = false;
    if old_tree_id != new_tree_id {
        let repo = &mut checkout_repo;
        #[allow(deprecated)]
        let wd_tree_id = repo.create_wd_tree(u64::MAX)?;
        repo.config_snapshot_mut()
            .set_value(&gix::config::tree::Merge::RENORMALIZE, "true")?;
        let mut outcome = repo.merge_trees(
            old_tree_id,
            wd_tree_id,
            new_tree_id,
            gix::merge::blob::builtin_driver::text::Labels {
                ancestor: Some("ancestor".into()),
                current: Some("ours".into()),
                other: Some("theirs".into()),
            },
            repo.tree_merge_options()?.with_rewrites(None),
        )?;
        let checkout_target_base_id = outcome.tree.write()?.detach();

        let mut checkout_target = repo.index_from_tree(&checkout_target_base_id)?;

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

        let baseline = repo.index_from_tree(&wd_tree_id)?;
        index_changed |=
            remove_index_entries_deleted_on_both_sides(repo, &mut index, old_tree_id, new_tree_id)?;
        index_changed |= repo
            .checkout_index_with_baseline(
                &baseline,
                &mut checkout_target,
                &mut index,
                gix::progress::Discard,
                &Default::default(),
                gix::worktree::checkout::Options {
                    // The merge above already reconciled local changes into the checkout target.
                    overwrite_staged: true,
                    overwrite_ignored: true,
                    ..Default::default()
                },
            )?
            .index_changed;
        if conflict_occurred {
            outcome.index_changed_after_applying_conflicts(
                &mut checkout_target,
                unresolved,
                gix::merge::tree::apply_index_entries::RemovalMode::Prune,
            );
            let mut edit = crate::index::Edit::new(&mut index);
            for entry in checkout_target.entries() {
                let stage = match entry.stage() {
                    gix::index::entry::Stage::Unconflicted => continue,
                    gix::index::entry::Stage::Base => crate::index::ConflictStage::Base,
                    gix::index::entry::Stage::Ours => crate::index::ConflictStage::Ours,
                    gix::index::entry::Stage::Theirs => crate::index::ConflictStage::Theirs,
                };
                edit.set_conflict_stage(entry.path(&checkout_target), stage, entry.mode, entry.id);
            }
            index_changed |= edit.finish();
        }
    }
    if index_changed && let Some(mut index_lock) = index_lock {
        let mut buffered = std::io::BufWriter::new(&mut index_lock);
        index.write_to(&mut buffered, Default::default())?;
        std::io::Write::flush(&mut buffered)?;
        drop(buffered);
        index_lock.commit()?;
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

/// Incorporate consumed changes into staging without discarding independent staged hunks.
/// Only the resulting edits are kept in memory; checkout must succeed before the index is written.
fn prepare_index_for_merge_base(
    repo: &gix::Repository,
    index: &mut gix::index::State,
    head_tree_id: gix::ObjectId,
    merge_base_tree_id: gix::ObjectId,
) -> anyhow::Result<bool> {
    use bstr::ByteSlice;
    use gix::diff::tree_with_rewrites::Change;

    let head_tree = repo.find_tree(head_tree_id)?;
    let merge_base_tree = repo.find_tree(merge_base_tree_id)?;
    let mut diff_options = gix::diff::Options::default();
    diff_options.track_path().track_rewrites(None);
    let conflicts: Vec<_> = index
        .entries()
        .iter()
        .filter(|entry| entry.stage() != gix::index::entry::Stage::Unconflicted)
        .map(|entry| entry.path(index))
        .collect();
    if !conflicts.is_empty() {
        let at_or_below = |path: &bstr::BStr, parent: &bstr::BStr| {
            path.strip_prefix(parent.as_bytes())
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(b"/"))
        };
        for change in repo.diff_tree_to_tree(&head_tree, &merge_base_tree, diff_options)? {
            if !change.entry_mode().is_tree()
                && let Some(path) = conflicts.iter().find(|path| {
                    at_or_below(path, change.location()) || at_or_below(change.location(), path)
                })
            {
                bail_precondition!(
                    "Consumed changes would overwrite an unresolved index conflict: {path:?}"
                );
            }
        }
    }

    // Trees cannot carry conflict stages. Leave them in the original index and merge only stage zero.
    let mut staged = index.clone();
    staged.remove_entries(|_, _, entry| entry.stage() != gix::index::entry::Stage::Unconflicted);
    let staged_tree_id = repo.write_index_tree(&staged)?;
    let mut merged = repo.merge_trees(
        head_tree_id,
        staged_tree_id,
        merge_base_tree_id,
        repo.default_merge_labels(),
        repo.tree_merge_options()?.with_rewrites(None),
    )?;
    if merged.has_unresolved_conflicts(TreatAsUnresolved::git()) {
        bail_precondition!("Consumed changes conflict with staged changes");
    }
    let merged_tree = merged.tree.write()?.object()?.peel_to_tree()?;
    let staged_tree = staged_tree_id.object()?.peel_to_tree()?;
    let mut edit = crate::index::Edit::new(index);
    for change in repo.diff_tree_to_tree(&staged_tree, &merged_tree, diff_options)? {
        if change.entry_mode().is_tree() {
            continue;
        }
        match change {
            Change::Addition {
                location,
                id,
                entry_mode,
                ..
            }
            | Change::Modification {
                location,
                id,
                entry_mode,
                ..
            } => {
                edit.upsert(
                    location.as_bstr(),
                    id,
                    entry_mode.into(),
                    crate::index::Stat::Unknown,
                );
            }
            Change::Deletion { location, .. } => edit.remove(location.as_bstr()),
            Change::Rewrite { .. } => unreachable!("rename tracking is disabled"),
        }
    }
    Ok(edit.finish())
}

/// Remove the index entries of files that both `new_tree` and the user deleted, as Git does,
/// unless the user staged changes to them.
/// The checkout never touches them, as they aren't in the worktree snapshot.
fn remove_index_entries_deleted_on_both_sides(
    repo: &gix::Repository,
    index: &mut gix::index::State,
    old_tree: gix::ObjectId,
    new_tree: gix::ObjectId,
) -> anyhow::Result<bool> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| anyhow::anyhow!("checkout needs a worktree"))?;
    let old_tree = repo.index_from_tree(&old_tree)?;
    let new_tree = repo.index_from_tree(&new_tree)?;
    let mut path_check = gix::status::plumbing::SymlinkCheck::new(workdir.to_owned());
    let mut deleted_on_both_sides = Vec::<bstr::BString>::new();
    for entry in index.entries() {
        if entry.stage() != gix::index::entry::Stage::Unconflicted {
            continue;
        }
        let path = entry.path(index);
        let staged = old_tree
            .entry_by_path(path)
            .is_none_or(|old| old.id != entry.id || old.mode != entry.mode);
        if staged || new_tree.entry_by_path(path).is_some() {
            continue;
        }
        // Like Git, a path behind a symlinked or missing directory, or replaced by a directory,
        // isn't in the worktree.
        let in_worktree = match path_check
            .verified_path(path)
            .and_then(|path| path.symlink_metadata())
        {
            Ok(md) => !md.is_dir(),
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) || is_symlinked_parent_error(&err) =>
            {
                false
            }
            Err(err) => return Err(err.into()),
        };
        if !in_worktree {
            deleted_on_both_sides.push(path.to_owned());
        }
    }
    let mut edit = crate::index::Edit::new(index);
    for path in &deleted_on_both_sides {
        edit.remove(path.as_ref());
    }
    Ok(edit.finish())
}

/// `SymlinkCheck` reports a symlinked parent directory as its only validation error.
fn is_symlinked_parent_error(err: &std::io::Error) -> bool {
    err.get_ref()
        .is_some_and(|source| gix::error::classify(source).is_validation())
}

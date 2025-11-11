use std::collections::BTreeSet;

use anyhow::Context;
use bstr::ByteSlice;
use but_oxidize::ObjectIdExt;
use gix::{
    diff::rewrites::tracker::ChangeKind,
    index::entry::Stage,
    objs::TreeRefIter,
    prelude::ObjectIdExt as _,
    refs::{
        Target,
        transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog},
    },
};
use tracing::instrument;

use super::{Options, Outcome, utils::merge_worktree_changes_into_destination_or_keep_snapshot};

/// Like [`safe_checkout()`], but the current tree will always be fetched from
pub fn safe_checkout_from_head(
    new_head_id: gix::ObjectId,
    repo: &gix::Repository,
    opts: Options,
) -> anyhow::Result<Outcome> {
    safe_checkout(
        repo.head_tree_id_or_empty()?.detach(),
        new_head_id,
        repo,
        opts,
    )
}
/// Given the `current_head_id^{tree}` for the tree that matches what `HEAD` points to, perform all file operations necessary
/// to turn the *worktree* of `repo` into `new_head_id^{tree}`. Note that the current *worktree* is assumed to be at the state of
/// `current_head_id` along with arbitrary uncommitted user changes.
///
/// Note that we don't care if the worktree actually matches the `new_head_id^{tree}`, we only care about the operations from
/// `current_head_id^{tree}` to be performed, and if there are none, we will do nothing.
///
/// If `new_head_id` is a *commit*, we will also set `HEAD` (or the ref it points to if symbolic) to the `new_head_id`.
/// We will also update the `.git/index` to match the `new_head_id^{tree}`.
/// Note that the value for [`UncommitedWorktreeChanges`] is critical to determine what happens if a change would be overwritten.
///
/// We will always handle changes in the worktree safely to avoid loss of uncommited information. This also means that deletions
/// never cause us to conflict. Conflicted files that would be checked out will cause an error.
///
/// #### Note: No rename tracking
///
/// To keep it simpler, we don't do rename tracking, so deletions and additions are always treated separately.
/// If this changes, then the source sid of a rename could also cause conflicts, maybe? It's a bit unclear what it would mean
/// in practice, but I guess that we bring deleted files back instead of conflicting.
#[instrument(skip(repo), err(Debug))]
pub fn safe_checkout(
    current_head_id: gix::ObjectId,
    new_head_id: gix::ObjectId,
    repo: &gix::Repository,
    Options {
        uncommitted_changes: conflicting_worktree_changes_opts,
        skip_head_update,
    }: Options,
) -> anyhow::Result<Outcome> {
    let source_tree = current_head_id.attach(repo).object()?.peel_to_tree()?;
    let new_object = new_head_id.attach(repo).object()?;
    let mut destination_tree = new_object.clone().peel_to_tree()?;

    let mut delegate = super::utils::Delegate::default();
    gix::diff::tree(
        TreeRefIter::from_bytes(&source_tree.data),
        TreeRefIter::from_bytes(&destination_tree.data),
        &mut gix::diff::tree::State::default(),
        repo,
        &mut delegate,
    )?;

    let mut opts = git2::build::CheckoutBuilder::new();
    let changed_files = delegate.changed_files;
    let snapshot_tree = merge_worktree_changes_into_destination_or_keep_snapshot(
        &changed_files,
        repo,
        source_tree.id,
        destination_tree.id,
        &mut opts,
        conflicting_worktree_changes_opts,
    )?
    .map(|(snapshot_id, new_destination_id)| {
        if let Some(id) = new_destination_id {
            destination_tree.id = id;
        }
        snapshot_id
    });

    let num_deleted_files = changed_files
        .iter()
        .filter(|(kind, _)| matches!(kind, ChangeKind::Deletion))
        .count();
    // Finally, perform the actual checkout
    // TODO(gix): use unconditional `gix` checkout implementation as pre-cursor to the real deal (not needed here).
    //            All it has to do is to be able to apply the target changes to any working tree, while using filters,
    //            and while doing it symlink-safe.
    if !changed_files.is_empty() {
        let git2_repo = git2::Repository::open(repo.git_dir())?;
        let destination_tree = git2_repo
            .find_tree(destination_tree.id.to_git2())?
            .into_object();
        let mut dirs_we_tried_to_delete = BTreeSet::new();
        for (kind, path_to_alter) in &changed_files {
            if matches!(kind, ChangeKind::Deletion) {
                // By now we can assume that the destination tree contains all files that should be
                // in the worktree, along with the worktree changes we will touch.
                // Thus, it's safe to delete the files that should be deleted, before possibly recreating them.
                let path_to_delete = repo
                    .workdir_path(path_to_alter)
                    .context("non-bare repository")?;
                if let Err(err) = std::fs::remove_file(&path_to_delete) {
                    if err.kind() == std::io::ErrorKind::NotFound
                        || err.kind() == std::io::ErrorKind::PermissionDenied
                        || err.kind() == std::io::ErrorKind::NotADirectory
                    {
                        continue;
                    };
                    if err.kind() == std::io::ErrorKind::IsADirectory {
                        std::fs::remove_dir_all(path_to_delete)?;
                    } else {
                        return Err(err.into());
                    }
                } else {
                    for dir_to_delete in path_to_delete.ancestors().skip(1) {
                        if !dirs_we_tried_to_delete.insert(dir_to_delete.to_owned()) {
                            break;
                        }
                        if let Err(err) = std::fs::remove_dir(dir_to_delete) {
                            if err.kind() == std::io::ErrorKind::DirectoryNotEmpty {
                                break;
                            } else {
                                return Err(err.into());
                            }
                        }
                    }
                }
            } else {
                opts.path(path_to_alter.as_bytes());
            }
        }

        git2_repo.checkout_tree(
            &destination_tree,
            Some(opts.update_index(true).force().disable_pathspec_match(true)),
        )?;

        if num_deleted_files > 0
            && let Ok(mut index) = repo.open_index()
        {
            for (kind, path_to_alter) in &changed_files {
                if matches!(kind, ChangeKind::Deletion)
                    && let Some(entry) = index
                        .entry_mut_by_path_and_stage(path_to_alter.as_bstr(), Stage::Unconflicted)
                {
                    entry.flags |= gix::index::entry::Flags::REMOVE;
                }
            }
            index.write(Default::default())?;
        }
    }

    let mut head_update = None;
    if new_object.kind.is_commit() && !skip_head_update {
        let needs_update = repo
            .head()?
            .id()
            .is_none_or(|actual_head_id| actual_head_id != new_head_id);
        if needs_update {
            let edits = repo.edit_reference(RefEdit {
                change: Change::Update {
                    log: LogChange {
                        mode: RefLog::AndReference,
                        force_create_reflog: false,
                        message: gix::reference::log::message(
                            "safe checkout",
                            "GitButler".into(),
                            new_object.into_commit().parent_ids().count(),
                        ),
                    },
                    // We play it loose here, as we assume a repository lock so we won't interfere with ourselves.
                    // Git itself enforces no lock either, so we rely on basic locking ref-locking here. Good enough.
                    expected: PreviousValue::Any,
                    new: Target::Object(new_head_id),
                },
                name: "HEAD".try_into().expect("root refs are always valid"),
                deref: true,
            })?;
            head_update = Some(edits);
        }
    }

    Ok(Outcome {
        snapshot_tree,
        head_update,
        num_deleted_files,
        num_added_or_updated_files: changed_files.len() - num_deleted_files,
    })
}

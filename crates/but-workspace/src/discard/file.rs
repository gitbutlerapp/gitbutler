use crate::discard::file::index::mark_entry_for_deletion;
use crate::discard::locked_resource_at;
use anyhow::{Context, bail};
use bstr::{BStr, BString, ByteSlice, ByteVec};
use but_core::ChangeState;
use gix::filter::plumbing::driver::apply::Delay;
use gix::object::tree::EntryKind;
use gix::prelude::ObjectIdExt;
use std::path::Path;

pub enum RestoreMode {
    /// Assume the resource to be restored doesn't exist as it was deleted.
    Deleted,
    /// A similar resource is in its place that needs to be updated.
    Update,
}

/// Restore `state` by writing it into the worktree of `repo`, possibly re-adding or updating the
/// `index` with it so that it matches the worktree.
pub fn restore_state_to_worktree(
    pipeline: &mut gix::filter::Pipeline<'_>,
    index: &mut gix::index::State,
    rela_path: &BStr,
    state: ChangeState,
    mode: RestoreMode,
    path_check: &mut gix::status::plumbing::SymlinkCheck,
    num_sorted_entries: &mut usize,
) -> anyhow::Result<()> {
    if state.id.is_null() {
        bail!(
            "Change to discard at '{rela_path}' didn't have a last-known tracked state - this is a bug"
        );
    }

    let mut update_index = |md| -> anyhow::Result<()> {
        crate::commit_engine::index::upsert_index_entry(
            index,
            rela_path,
            &md,
            state.id,
            state.kind.into(),
            gix::index::entry::Flags::UPDATE,
            num_sorted_entries,
        )?;
        Ok(())
    };

    let repo = pipeline.repo;
    let wt_root = path_check.inner.root().to_owned();
    let file_path = path_check.verified_path_allow_nonexisting(rela_path)?;
    match state.kind {
        EntryKind::Blob | EntryKind::BlobExecutable => {
            let mut dest_lock_file = locked_resource_at(wt_root, &file_path, state.kind)?;
            let obj_in_git = state.id.attach(repo).object()?;
            let mut stream =
                pipeline.convert_to_worktree(&obj_in_git.data, rela_path, Delay::Forbid)?;
            std::io::copy(&mut stream, &mut dest_lock_file)?;

            let (file_path, maybe_file) = match dest_lock_file.commit() {
                Ok(res) => res,
                Err(err) => {
                    if err.error.kind() == std::io::ErrorKind::IsADirectory {
                        // It's OK to remove everything that's in the way.
                        // Alternatives to this is to let it be handled by the stack.
                        std::fs::remove_dir_all(err.instance.resource_path())?;
                        err.instance.commit()?
                    } else {
                        return Err(err.into());
                    }
                }
            };
            update_index(match maybe_file {
                None => gix::index::fs::Metadata::from_path_no_follow(&file_path)?,
                Some(file) => gix::index::fs::Metadata::from_file(&file)?,
            })?;
        }
        EntryKind::Link => {
            let link_path = file_path;
            if let RestoreMode::Update = mode {
                std::fs::remove_file(&link_path)?;
            }
            let link_target = state.id.attach(repo).object()?;
            let link_target = gix::path::from_bstr(link_target.data.as_bstr());
            if let Err(err) = gix::fs::symlink::create(&link_target, &link_path) {
                // When directories are replaced, the user could undo everything. Then
                // it's a matter of order if *we* have already created the directory content.
                if err.kind() != std::io::ErrorKind::AlreadyExists
                    || !link_path.symlink_metadata()?.is_symlink()
                {
                    return Err(err.into());
                }
            }
            update_index(gix::index::fs::Metadata::from_path_no_follow(&link_path)?)?;
        }
        EntryKind::Commit => {
            if let RestoreMode::Update = mode {
                // TODO(gix): actual checkout/reset functionality - it will be fine to support that fully.
                // Since `git2` doesn't support filters, it will save us some trouble to just use Git for that.
                let submodule_repo_dir = &file_path;
                let out = std::process::Command::from(
                    gix::command::prepare(format!(
                        "git reset --hard {id} && git clean -fxd",
                        id = state.id
                    ))
                    .with_shell(),
                )
                .current_dir(submodule_repo_dir)
                .output()?;
                if !out.status.success() {
                    bail!(
                        "Could not reset submodule at '{sm_dir}' to commit {id}: {err}",
                        sm_dir = submodule_repo_dir.display(),
                        id = state.id,
                        err = out.stderr.as_bstr()
                    );
                }
            } else {
                let sm_repo = repo
                    .submodules()?
                    .into_iter()
                    .flatten()
                    .find_map(|sm| {
                        let is_active = sm.is_active().ok()?;
                        is_active.then(|| -> anyhow::Result<_> {
                            Ok(
                                if sm.path().ok().is_some_and(|sm_path| sm_path == rela_path) {
                                    sm.open()?
                                } else {
                                    None
                                },
                            )
                        })
                    })
                    .transpose()?
                    .flatten();
                match sm_repo {
                    None => {
                        // A directory is what git creates with `git restore` even if the thing to restore is a submodule.
                        // We are trying to be better than that if we find a submodule, hoping that this is what users expect.
                        // We do that as baseline as there is no need to fail here.
                    }
                    Some(repo) => {
                        // We will only restore the submodule if there is a local clone already available, to avoid any network
                        // activity that would likely happen during an actual clone.
                        // Thus, all we have to do is to check out the submodule.
                        // TODO(gix): find a way to deal with nested submodules - they should also be checked out which
                        //            isn't done by `gitoxide`, but probably should be an option there.
                        checkout_repo_worktree(&wt_root, repo)?;
                    }
                }
                std::fs::create_dir(&file_path).or_else(|err| {
                    if err.kind() == std::io::ErrorKind::AlreadyExists {
                        Ok(())
                    } else {
                        Err(err)
                    }
                })?;
            }
            update_index(gix::index::fs::Metadata::from_path_no_follow(&file_path)?)?;
        }
        EntryKind::Tree => {
            mark_entry_for_deletion(index, rela_path, *num_sorted_entries);
            let checkout_destination = file_path;
            let mut sub_index = repo.index_from_tree(&state.id)?;
            let mut opts =
                repo.checkout_options(gix::worktree::stack::state::attributes::Source::IdMapping)?;
            // there may be situations where files already exist in that spot, likely because we put them
            // there earlier as part of a sweeping 'discard'. Still, try not to mess with the user.
            opts.overwrite_existing = false;
            if !checkout_destination.exists() {
                std::fs::create_dir(&checkout_destination)?;
                opts.destination_is_initially_empty = true;
            }
            // TODO(gix): make it possible to have this checkout submodules as well.
            let out = gix::worktree::state::checkout(
                &mut sub_index,
                checkout_destination.as_ref(),
                repo.clone().objects.into_arc()?,
                &gix::progress::Discard,
                &gix::progress::Discard,
                &gix::interrupt::IS_INTERRUPTED,
                opts,
            )?;
            tracing::debug!(directory = ?checkout_destination, outcome = ?out, "directory checkout result");

            let (entries, path_storage) = sub_index.into_parts().0.into_entries();
            let mut rela_path = with_trailing_slash(rela_path);
            let prefix_len = rela_path.len();
            for entry in entries {
                let partial_rela_path = entry.path_in(&path_storage);
                rela_path.extend_from_slice(partial_rela_path);

                if index.entry_by_path(rela_path.as_bstr()).is_none() {
                    index.dangerously_push_entry(
                        entry.stat,
                        entry.id,
                        entry.flags | gix::index::entry::Flags::UPDATE,
                        entry.mode,
                        rela_path.as_bstr(),
                    );
                }
                rela_path.truncate(prefix_len);
            }
            // These might be re-visited later if the user was able to add individual deletions in a directory.
            // Sort to make index-lookups work.
            index.sort_entries();
            *num_sorted_entries = index.entries().len();
        }
    };
    Ok(())
}

fn with_trailing_slash(rela_path: &BStr) -> BString {
    if rela_path.ends_with_str(b"/") {
        return rela_path.to_owned();
    }
    let mut buf = rela_path.to_owned();
    buf.push(b'/');
    buf
}

fn checkout_repo_worktree(
    parent_worktree_dir: &Path,
    mut repo: gix::Repository,
) -> anyhow::Result<()> {
    // No need to cache anything, it's just single-use for the most part.
    repo.object_cache_size(0);
    let mut index = repo.index_from_tree(&repo.head_tree_id_or_empty()?)?;
    if index.entries().is_empty() {
        // The worktree directory is created later, so we don't have to deal with it here.
        return Ok(());
    }
    for entry in index.entries_mut().iter_mut().filter(|e| {
        e.mode
            .contains(gix::index::entry::Mode::DIR | gix::index::entry::Mode::COMMIT)
    }) {
        entry.flags.insert(gix::index::entry::Flags::SKIP_WORKTREE);
    }

    let mut opts =
        repo.checkout_options(gix::worktree::stack::state::attributes::Source::IdMapping)?;
    opts.destination_is_initially_empty = true;
    opts.keep_going = true;

    let checkout_destination = repo.workdir().context("non-bare repository")?.to_owned();
    if !checkout_destination.exists() {
        std::fs::create_dir(&checkout_destination)?;
    }
    let sm_repo_dir = gix::path::relativize_with_prefix(
        repo.path().strip_prefix(parent_worktree_dir)?,
        checkout_destination.strip_prefix(parent_worktree_dir)?,
    )
    .into_owned();
    let out = gix::worktree::state::checkout(
        &mut index,
        checkout_destination.clone(),
        repo,
        &gix::progress::Discard,
        &gix::progress::Discard,
        &gix::interrupt::IS_INTERRUPTED,
        opts,
    )?;

    let mut buf = BString::from("gitdir: ");
    buf.extend_from_slice(&gix::path::os_string_into_bstring(sm_repo_dir.into())?);
    buf.push_byte(b'\n');
    std::fs::write(checkout_destination.join(".git"), &buf)?;

    tracing::debug!(directory = ?checkout_destination, outcome = ?out, "submodule checkout result");
    Ok(())
}

/// Remove files present at `rela_path`, restore the index at that place, if possible,
/// and if necessary, checkout everything that this revealed.
/// This is required when handling renames.
pub fn purge_and_restore_from_head_tree(
    index: &mut gix::index::State,
    rela_path: &BStr,
    path_check: &mut gix::status::plumbing::SymlinkCheck,
    num_sorted_entries: usize,
) -> anyhow::Result<()> {
    if let Some(range) = index.entry_range(with_trailing_slash(rela_path).as_bstr()) {
        #[allow(clippy::indexing_slicing)]
        for entry in &mut index.entries_mut()[range] {
            entry.flags.insert(gix::index::entry::Flags::REMOVE);
        }
    } else {
        mark_entry_for_deletion(index, rela_path, num_sorted_entries);
    }

    // TODO(motivational test): restore what was there in the the index, and then on disk by checkout.
    let path = path_check.verified_path(rela_path)?;
    if !path.is_dir() {
        // Should always exist, this is why it's a rename in the first place.
        std::fs::remove_file(path).or_else(|err| {
            if matches!(
                err.kind(),
                std::io::ErrorKind::NotADirectory | std::io::ErrorKind::NotFound
            ) {
                Ok(())
            } else {
                Err(err)
            }
        })?;
    } else {
        bail!("BUG: it's unclear how this case would occur, get a test for it")
    }
    Ok(())
}

pub(super) mod index {
    use bstr::BStr;
    use gix::index::entry::Stage;

    pub fn mark_entry_for_deletion(
        state: &mut gix::index::State,
        rela_path: &BStr,
        num_sorted_entries: usize,
    ) {
        for stage in [Stage::Unconflicted, Stage::Base, Stage::Ours, Stage::Theirs] {
            // TODO(perf): `gix` should offer a way to get the *first* index by path so the
            //             binary search doesn't have to be repeated.
            let Some(entry_idx) =
                state.entry_index_by_path_and_stage_bounded(rela_path, stage, num_sorted_entries)
            else {
                continue;
            };
            #[allow(clippy::indexing_slicing)]
            state.entries_mut()[entry_idx]
                .flags
                .insert(gix::index::entry::Flags::REMOVE);
        }
    }
}

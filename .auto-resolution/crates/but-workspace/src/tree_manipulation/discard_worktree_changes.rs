use anyhow::{Context as _, bail};
use bstr::ByteSlice;
use but_core::{ChangeState, DiffSpec, TreeStatus};

/// Discard the given `changes` in the worktree of `repo`. If a change could not be matched with an actual worktree change, for
/// instance due to a race, that's not an error, instead it will be returned in the result Vec, along with all hunks that couldn't
/// be matched.
/// The returned Vec is typically empty, meaning that all `changes` could be discarded.
///
/// `context_lines` is the amount of context lines we should assume when obtaining hunks of worktree changes to match against
/// the ones we have specified in the hunks contained within `changes`.
///
/// Discarding a change is really more of an 'undo' of a change as it will restore the previous state to the desired extent - Git
/// doesn't have a notion of this on a whole-file basis.
///
/// Each of the `changes` will be matched against actual worktree changes to make this operation as safe as possible, after all, it
/// discards changes without recovery.
///
/// In practice, this is like a selective 'inverse-checkout', as such it must have a lot of the capabilities of checkout, but focussed
/// on just a couple of paths, and with special handling for renamed files, something that `checkout` can't naturally handle
/// as it's only dealing with single file-paths.
///
/// ### Hunk-based discarding
///
/// When an instance in `changes` contains hunks, these are the hunks to be discarded. If they match a whole hunk in the worktree changes,
/// it will be discarded entirely, simply by not applying it.
///
/// ### Sub-Hunk discarding
///
/// It's possible to specify ranges of hunks to discard. To do that, they need an *anchor*. The *anchor* is the pair of
/// `(line_number, line_count)` that should not be changed, paired with the *other* pair with the new `(line_number, line_count)`
/// to discard.
///
/// For instance, when there is a single patch `-1,10 +1,10` and we want to bring back the removed 5th line *and* the added 5th line,
/// we'd specify *just* two selections, one in the old via `-5,1 +1,10` and one in the new via `-1,10 +5,1`.
/// This works because internally, it will always match the hunks (and sub-hunks) with their respective pairs obtained through a
/// worktree status.
pub fn discard_workspace_changes(
    repo: &gix::Repository,
    changes: impl IntoIterator<Item = DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<Vec<DiffSpec>> {
    let wt_changes = but_core::diff::worktree_changes(repo)?;
    let mut dropped = Vec::new();
    let mut index = repo.index_or_empty()?.into_owned_or_cloned();
    let mut initial_entries_len = index.entries().len();
    let (mut pipeline, _) = repo.filter_pipeline(Some(repo.empty_tree().id))?;
    let head_tree = repo.head_tree_id_or_empty()?.object()?.into_tree();

    let mut path_check = gix::status::plumbing::SymlinkCheck::new(
        repo.workdir().context("non-bare repository")?.into(),
    );
    for mut spec in changes {
        let Some(wt_change) = wt_changes.changes.iter().find(|c| {
            c.path == spec.path
                && c.previous_path() == spec.previous_path.as_ref().map(|p| p.as_bstr())
        }) else {
            dropped.push(spec);
            continue;
        };

        if spec.hunk_headers.is_empty() {
            match wt_change.status {
                TreeStatus::Addition { is_untracked, .. } => {
                    std::fs::remove_file(path_check.verified_path(&wt_change.path)?)?;
                    if !is_untracked {
                        file::index::mark_entry_for_deletion(
                            &mut index,
                            wt_change.path.as_bstr(),
                            initial_entries_len,
                        );
                    }
                    if let Some(entry) =
                        head_tree.lookup_entry(wt_change.path.split(|b| *b == b'/'))?
                    {
                        file::restore_state_to_worktree(
                            &mut pipeline,
                            &mut index,
                            wt_change.path.as_bstr(),
                            ChangeState {
                                id: entry.object_id(),
                                kind: entry.mode().into(),
                            },
                            file::RestoreMode::Deleted,
                            &mut path_check,
                            &mut initial_entries_len,
                        )?
                    }
                }
                TreeStatus::Deletion { previous_state } => {
                    file::restore_state_to_worktree(
                        &mut pipeline,
                        &mut index,
                        wt_change.path.as_bstr(),
                        previous_state,
                        file::RestoreMode::Deleted,
                        &mut path_check,
                        &mut initial_entries_len,
                    )?;
                }
                TreeStatus::Modification { previous_state, .. } => {
                    file::restore_state_to_worktree(
                        &mut pipeline,
                        &mut index,
                        wt_change.path.as_bstr(),
                        previous_state,
                        file::RestoreMode::Update,
                        &mut path_check,
                        &mut initial_entries_len,
                    )?;
                }
                TreeStatus::Rename {
                    ref previous_path,
                    previous_state,
                    ..
                } => {
                    file::restore_state_to_worktree(
                        &mut pipeline,
                        &mut index,
                        previous_path.as_bstr(),
                        previous_state,
                        file::RestoreMode::Deleted,
                        &mut path_check,
                        &mut initial_entries_len,
                    )?;
                    file::purge_and_restore_from_head_tree(
                        &mut index,
                        wt_change.path.as_bstr(),
                        &mut path_check,
                        initial_entries_len,
                    )?;
                }
            }
        } else {
            match wt_change.status {
                TreeStatus::Addition { .. } | TreeStatus::Deletion { .. } => {
                    bail!(
                        "Deletions or additions aren't well-defined for hunk-based operations - use the whole-file mode instead: '{}'",
                        wt_change.path
                    )
                }
                TreeStatus::Modification {
                    previous_state,
                    flags,
                    ..
                }
                | TreeStatus::Rename {
                    previous_state,
                    flags,
                    ..
                } => {
                    if flags.is_some_and(|f| f.is_typechange()) {
                        bail!(
                            "Type-changed items can't be discard by hunks - use the whole-file mode instead"
                        )
                    }
                    hunk::restore_state_to_worktree(
                        wt_change,
                        previous_state,
                        &mut spec.hunk_headers,
                        &mut path_check,
                        &mut pipeline,
                        &index,
                        context_lines,
                    )?;
                    if !spec.hunk_headers.is_empty() {
                        dropped.push(spec);
                        continue;
                    }
                }
            }
        }
    }

    let has_removals_or_updates = index.entries().iter().any(|e| {
        e.flags
            .intersects(gix::index::entry::Flags::REMOVE | gix::index::entry::Flags::UPDATE)
    });
    if has_removals_or_updates {
        index.remove_tree();
        index.remove_resolve_undo();
        // Always sort, we currently don't keep track of whether this is truly required
        // and checking the amount of entries isn't safe in light of conflicts (that may get removed).
        index.sort_entries();
        index.write(Default::default())?;
    }
    Ok(dropped)
}

mod file {
    use std::path::{Path, PathBuf};

    use anyhow::{Context as _, bail};
    use bstr::{BStr, BString, ByteSlice, ByteVec};
    use but_core::ChangeState;
    use gix::{
        filter::plumbing::driver::apply::Delay, object::tree::EntryKind, prelude::ObjectIdExt,
        tempfile::create_dir::Retries,
    };

    use crate::tree_manipulation::discard_worktree_changes::file::index::mark_entry_for_deletion;

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
                Some(&md),
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
                let mut tempfile = tempfile_in_root_with_permissions_at(wt_root, state.kind)?;
                let obj_in_git = state.id.attach(repo).object()?;
                let mut stream =
                    pipeline.convert_to_worktree(&obj_in_git.data, rela_path, Delay::Forbid)?;
                std::io::copy(&mut stream, &mut tempfile)?;
                gix::tempfile::create_dir::all(
                    file_path.parent().context("encountered strange filepath")?,
                    Retries::default(),
                )?;
                let file = match tempfile.persist(&file_path) {
                    Ok(res) => res,
                    Err(err) => {
                        if err.error.kind() == std::io::ErrorKind::IsADirectory {
                            // It's OK to remove everything that's in the way.
                            // Alternatives to this is to let it be handled by the stack.
                            std::fs::remove_dir_all(&file_path)?;
                            err.file.persist(file_path)?
                        } else {
                            return Err(err.into());
                        }
                    }
                };
                update_index(gix::index::fs::Metadata::from_file(&file)?)?;
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
                let mut opts = repo
                    .checkout_options(gix::worktree::stack::state::attributes::Source::IdMapping)?;
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
            #[expect(clippy::indexing_slicing)]
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
                let Some(entry_idx) = state.entry_index_by_path_and_stage_bounded(
                    rela_path,
                    stage,
                    num_sorted_entries,
                ) else {
                    continue;
                };
                #[expect(clippy::indexing_slicing)]
                state.entries_mut()[entry_idx]
                    .flags
                    .insert(gix::index::entry::Flags::REMOVE);
            }
        }
    }

    fn tempfile_in_root_with_permissions_at(
        root: PathBuf,
        kind: EntryKind,
    ) -> anyhow::Result<tempfile::NamedTempFile> {
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut builder = tempfile::Builder::new();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(std::fs::Permissions::from_mode(kind as u32));
        }
        Ok(builder.tempfile_in(root)?)
    }
}

mod hunk {
    use anyhow::bail;
    use bstr::ByteSlice;
    use but_core::{
        ChangeState, HunkHeader, TreeChange, UnifiedPatch, apply_hunks,
        worktree::worktree_file_to_git_in_buf,
    };
    use gix::{
        filter::plumbing::{
            driver::apply::{Delay, MaybeDelayed},
            pipeline::convert::ToWorktreeOutcome,
        },
        prelude::ObjectIdExt,
    };

    use crate::tree_manipulation::hunk::{HunkSubstraction, subtract_hunks};

    /// Discard `hunks_to_discard` in the resource at `wt_change`, whose previous version is `previous_state` and is expected to
    /// be tracked and readable from the object database. We will always read what's currently on disk as the current version
    /// and overwrite it.
    ///
    /// The general idea is to rebuild the file, but without the `hunks_to_discard`, write it into the worktree replacing
    /// the previous file. `hunks_to_discard` are hunks based on a diff of what's in Git.
    ///
    /// Note that an index update isn't necessary, as for the purpose of the application it might as well not exist due to the
    /// way our worktree-changes function ignores the index entirely.
    ///
    /// Note that `hunks_to_discard` that weren't present in the actual set of worktree changes will remain, so when everything
    /// worked `hunks_to_discard` will be empty.
    pub fn restore_state_to_worktree(
        wt_change: &TreeChange,
        previous_state: ChangeState,
        hunks_to_discard: &mut Vec<HunkHeader>,
        path_check: &mut gix::status::plumbing::SymlinkCheck,
        pipeline: &mut gix::filter::Pipeline<'_>,
        index: &gix::index::State,
        context_lines: u32,
    ) -> anyhow::Result<()> {
        let repo = pipeline.repo;
        let state_in_worktree = ChangeState {
            id: repo.object_hash().null(),
            kind: previous_state.kind,
        };
        let mut diff_filter = but_core::unified_diff::filter_from_state(
            repo,
            Some(state_in_worktree),
            UnifiedPatch::CONVERSION_MODE,
        )?;
        let Some(UnifiedPatch::Patch {
            hunks: hunks_in_worktree,
            ..
        }) = wt_change.unified_patch_with_filter(repo, context_lines, &mut diff_filter)?
        else {
            bail!("Couldn't obtain diff for worktree changes.")
        };

        let mut hunks_to_keep: Vec<HunkHeader> = (hunks_in_worktree)
            .into_iter()
            .map(Into::into)
            .filter(|hunk| {
                match hunks_to_discard
                    .iter()
                    .enumerate()
                    .find_map(|(idx, hunk_to_discard)| (hunk_to_discard == hunk).then_some(idx))
                {
                    None => true,
                    Some(idx_to_remove) => {
                        hunks_to_discard.remove(idx_to_remove);
                        false
                    }
                }
            })
            .collect();

        // Find associations with sub-hunks
        if !hunks_to_discard.is_empty() {
            // TODO(perf): instead of brute-force searching, assure hunks_to_discard are sorted and speed up the search that way.
            let mut hunks_to_keep_with_splits = Vec::new();
            for hunk_to_split in hunks_to_keep {
                let mut subtractions = Vec::new();
                hunks_to_discard.retain(|sub_hunk_to_discard| {
                    if sub_hunk_to_discard.old_range() == hunk_to_split.old_range() {
                        subtractions.push(HunkSubstraction::New(sub_hunk_to_discard.new_range()));
                        false
                    } else if sub_hunk_to_discard.new_range() == hunk_to_split.new_range() {
                        subtractions.push(HunkSubstraction::Old(sub_hunk_to_discard.old_range()));
                        false
                    } else {
                        true
                    }
                });
                if subtractions.is_empty() {
                    hunks_to_keep_with_splits.push(hunk_to_split);
                } else {
                    hunks_to_keep_with_splits.extend(subtract_hunks(hunk_to_split, subtractions)?);
                }
            }
            hunks_to_keep = hunks_to_keep_with_splits;
        }

        let old = previous_state.id.attach(repo).object()?.detach().data;
        let mut new = repo.empty_reusable_buffer();
        let rela_path = wt_change.path.as_bstr();
        let worktree_path = path_check.verified_path_allow_nonexisting(rela_path)?;
        let md = gix::index::fs::Metadata::from_path_no_follow(&worktree_path)?;
        if !md.is_file() {
            bail!(
                "Cannot discard lines in '{}' - invalid type",
                worktree_path.display()
            );
        }
        worktree_file_to_git_in_buf(&mut new, &md, rela_path, &worktree_path, pipeline, index)?;

        let base_with_patches = apply_hunks(old.as_bstr(), new.as_bstr(), &hunks_to_keep)?;

        let to_worktree =
            pipeline.convert_to_worktree(&base_with_patches, rela_path, Delay::Forbid)?;
        match to_worktree {
            ToWorktreeOutcome::Unchanged(buf) => {
                std::fs::write(&worktree_path, buf)?;
            }
            ToWorktreeOutcome::Buffer(buf) => {
                std::fs::write(&worktree_path, buf)?;
            }
            ToWorktreeOutcome::Process(MaybeDelayed::Immediate(mut stream)) => {
                let mut file = std::fs::File::create(&worktree_path)?;
                std::io::copy(&mut stream, &mut file)?;
            }
            ToWorktreeOutcome::Process(MaybeDelayed::Delayed(_)) => unreachable!("disabled"),
        }
        Ok(())
    }
}

//! Utility types related to discarding changes in the worktree.

use std::collections::HashMap;

use anyhow::Context;
use bstr::ByteSlice as _;
use but_core::ChangeState;
use but_rebase::{RebaseOutput, RebaseStep};

use super::hunk::{HunkSubstraction, subtract_hunks};
use crate::{DiffSpec, HunkHeader, commit_engine::apply_hunks};

/// Takes a rebase output and returns the commit mapping with any extra
/// mapping overrides provided.
///
/// This will only include commits that have actually changed. If a commit was
/// mapped to itself it will not be included in the resulting HashMap.
///
/// Overrides are used to handle the case where the caller of the rebase engine
/// has manually replaced a particular commit with a rewritten one. This is
/// needed because a manually re-written commit that ends up matching the
/// base when the rebase occurs will end up showing up as a no-op in the
/// resulting commit_mapping.
///
/// Overrides should be provided as a vector that contains tuples of object
/// ids, where the first item is the before object_id, and the second item is
/// the after object_id.
pub(crate) fn rebase_mapping_with_overrides(
    rebase_output: &RebaseOutput,
    overrides: impl IntoIterator<Item = (gix::ObjectId, gix::ObjectId)>,
) -> HashMap<gix::ObjectId, gix::ObjectId> {
    let mut mapping = rebase_output
        .commit_mapping
        .iter()
        .filter(|(_, old, new)| old != new)
        .map(|(_, old, new)| (*old, *new))
        .collect::<HashMap<_, _>>();

    for (old, new) in overrides {
        if old != new {
            mapping.insert(old, new);
        }
    }

    mapping
}

pub enum ChangesSource {
    Commit {
        id: gix::ObjectId,
    },
    #[expect(dead_code)]
    Tree {
        after_id: gix::ObjectId,
        before_id: gix::ObjectId,
    },
}

impl ChangesSource {
    fn before<'a>(&self, repository: &'a gix::Repository) -> anyhow::Result<gix::Tree<'a>> {
        match self {
            ChangesSource::Commit { id } => {
                let commit = repository.find_commit(*id)?;
                let parent_id = commit.parent_ids().next().context("no parent")?;
                let parent = repository.find_commit(parent_id)?;
                Ok(parent.tree()?)
            }
            ChangesSource::Tree { before_id, .. } => Ok(repository.find_tree(*before_id)?),
        }
    }

    fn after<'a>(&self, repository: &'a gix::Repository) -> anyhow::Result<gix::Tree<'a>> {
        match self {
            ChangesSource::Commit { id } => Ok(repository.find_commit(*id)?.tree()?),
            ChangesSource::Tree { after_id, .. } => Ok(repository.find_tree(*after_id)?),
        }
    }
}

/// Discard the given `changes` in either the work tree or an arbitrary commit or tree. If a change could not be matched with an
/// actual worktree change, for instance due to a race, that's not an error, instead it will be returned in the result Vec, along
/// with all hunks that couldn't be matched.
///
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
pub fn create_tree_without_diff(
    repository: &gix::Repository,
    changes_source: ChangesSource,
    changes_to_discard: impl IntoIterator<Item = DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<(gix::ObjectId, Vec<DiffSpec>)> {
    let mut dropped = Vec::new();

    let before = changes_source.before(repository)?;
    let after = changes_source.after(repository)?;

    let mut builder = repository.edit_tree(after.id())?;

    for change in changes_to_discard {
        let before_path = change
            .previous_path
            .clone()
            .unwrap_or_else(|| change.path.clone());
        let before_entry = before.lookup_entry(before_path.clone().split_str("/"))?;

        let Some(after_entry) = after.lookup_entry(change.path.clone().split_str("/"))? else {
            let Some(before_entry) = before_entry else {
                // If there is no before entry and no after entry, then
                // something has gone wrong.
                dropped.push(change);
                continue;
            };

            if change.hunk_headers.is_empty() {
                // If there is no after_change, then it must have been deleted.
                // Therefore, we can just add it again.
                builder.upsert(
                    change.path.as_bstr(),
                    before_entry.mode().kind(),
                    before_entry.object_id(),
                )?;
                continue;
            } else {
                anyhow::bail!(
                    "Deletions or additions aren't well-defined for hunk-based operations - use the whole-file mode instead"
                );
            }
        };

        match after_entry.mode().kind() {
            gix::objs::tree::EntryKind::Blob | gix::objs::tree::EntryKind::BlobExecutable => {
                let after_blob = after_entry.object()?.into_blob();
                if change.hunk_headers.is_empty() {
                    revert_file_to_before_state(&before_entry, &mut builder, &change)?;
                } else {
                    let Some(before_entry) = before_entry else {
                        anyhow::bail!(
                            "Deletions or additions aren't well-defined for hunk-based operations - use the whole-file mode instead"
                        );
                    };

                    let diff = but_core::UnifiedPatch::compute(
                        repository,
                        change.path.as_bstr(),
                        Some(before_path.as_bstr()),
                        ChangeState {
                            id: after_entry.id().detach(),
                            kind: after_entry.mode().kind(),
                        },
                        ChangeState {
                            id: before_entry.id().detach(),
                            kind: before_entry.mode().kind(),
                        },
                        context_lines,
                    )?
                    .context(
                        "Cannot diff submodules - if this is encountered we should look into it",
                    )?;

                    let but_core::UnifiedPatch::Patch {
                        hunks: diff_hunks, ..
                    } = diff
                    else {
                        anyhow::bail!("expected a patch");
                    };

                    let mut good_hunk_headers = vec![];
                    let mut bad_hunk_headers = vec![];

                    for hunk in &change.hunk_headers {
                        if diff_hunks
                            .iter()
                            .any(|diff_hunk| HunkHeader::from(diff_hunk.clone()).contains(*hunk))
                        {
                            good_hunk_headers.push(*hunk);
                        } else {
                            bad_hunk_headers.push(*hunk);
                        }
                    }

                    if !bad_hunk_headers.is_empty() {
                        dropped.push(DiffSpec {
                            previous_path: change.previous_path.clone(),
                            path: change.path.clone(),
                            hunk_headers: bad_hunk_headers,
                        });
                    }

                    // TODO: Validate that the hunks coorespond with actual changes?
                    let before_blob = before_entry.object()?.into_blob();

                    let new_hunks = new_hunks_after_removals(
                        diff_hunks.into_iter().map(Into::into).collect(),
                        good_hunk_headers,
                    )?;
                    let new_after_contents = apply_hunks(
                        before_blob.data.as_bstr(),
                        after_blob.data.as_bstr(),
                        &new_hunks,
                    )?;
                    let mode = if new_after_contents == before_blob.data {
                        before_entry.mode().kind()
                    } else {
                        after_entry.mode().kind()
                    };
                    let new_after_contents = repository.write_blob(&new_after_contents)?;

                    // Keep the mode of the after state. We _should_ at some
                    // point introduce the mode specifically as part of the
                    // DiscardSpec, but for now, we can just use the after state.
                    builder.upsert(change.path.as_bstr(), mode, new_after_contents)?;
                }
            }
            _ => {
                revert_file_to_before_state(&before_entry, &mut builder, &change)?;
            }
        }
    }

    let final_tree = builder.write()?;
    Ok((final_tree.detach(), dropped))
}

fn new_hunks_after_removals(
    change_hunks: Vec<HunkHeader>,
    mut removal_hunks: Vec<HunkHeader>,
) -> anyhow::Result<Vec<HunkHeader>> {
    // If a removal hunk matches completly then we can drop it entirely.
    let hunks_to_keep: Vec<HunkHeader> = change_hunks
        .into_iter()
        .filter(|hunk| {
            match removal_hunks
                .iter()
                .enumerate()
                .find_map(|(idx, hunk_to_discard)| (hunk_to_discard == hunk).then_some(idx))
            {
                None => true,
                Some(idx_to_remove) => {
                    removal_hunks.remove(idx_to_remove);
                    false
                }
            }
        })
        .collect();

    // TODO(perf): instead of brute-force searching, assure hunks_to_discard are sorted and speed up the search that way.
    let mut hunks_to_keep_with_splits = Vec::new();
    for hunk_to_split in hunks_to_keep {
        let mut subtractions = Vec::new();
        removal_hunks.retain(|sub_hunk_to_discard| {
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
            let hunk_with_subtractions = subtract_hunks(hunk_to_split, subtractions)?;
            hunks_to_keep_with_splits.extend(hunk_with_subtractions);
        }
    }
    Ok(hunks_to_keep_with_splits)
}

fn revert_file_to_before_state(
    before_entry: &Option<gix::object::tree::Entry<'_>>,
    builder: &mut gix::object::tree::Editor<'_>,
    change: &DiffSpec,
) -> Result<(), anyhow::Error> {
    // If there are no hunk headers, then we want to revert the
    // whole file to the state it was in before tree.
    if let Some(before_entry) = before_entry {
        builder.remove(change.path.as_bstr())?;
        builder.upsert(
            change
                .previous_path
                .clone()
                .unwrap_or(change.path.clone())
                .as_bstr(),
            before_entry.mode().kind(),
            before_entry.object_id(),
        )?;
    } else {
        builder.remove(change.path.as_bstr())?;
    }
    Ok(())
}

pub fn replace_pick_with_commit(
    steps: &mut Vec<RebaseStep>,
    target_commit_id: gix::ObjectId,
    replacement_commit_id: gix::ObjectId,
) -> anyhow::Result<()> {
    let mut found = false;
    for step in steps {
        if step.commit_id() != Some(&target_commit_id) {
            continue;
        }
        let RebaseStep::Pick { commit_id, .. } = step else {
            continue;
        };
        found = true;
        *commit_id = replacement_commit_id;
    }

    if found {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Failed to replace pick step {} with {}",
            target_commit_id,
            replacement_commit_id
        ))
    }
}

pub fn replace_pick_with_multiple_commits(
    steps: &mut Vec<RebaseStep>,
    target_commit_id: gix::ObjectId,
    replacement_commit_ids: &[(gix::ObjectId, Option<String>)],
) -> anyhow::Result<()> {
    let mut found = false;
    let mut new_steps =
        Vec::with_capacity(steps.len() + replacement_commit_ids.len().saturating_sub(1));
    for step in steps.drain(..) {
        if step.commit_id() == Some(&target_commit_id) {
            let RebaseStep::Pick { .. } = step else {
                new_steps.push(step);
                continue;
            };
            found = true;
            for (replacement_commit_id, new_message) in replacement_commit_ids {
                new_steps.push(RebaseStep::Pick {
                    commit_id: *replacement_commit_id,
                    new_message: new_message.clone().map(|msg| msg.into()),
                });
            }
        } else {
            new_steps.push(step);
        }
    }
    *steps = new_steps;

    if found {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Failed to replace pick step {} with multiple commits",
            target_commit_id
        ))
    }
}

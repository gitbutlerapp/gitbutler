use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::{Context, Result, bail};
use bstr::{BString, ByteSlice as _};
use but_core::RepositoryExt as _;
use but_core::commit::TreeKind;
use but_core::{HunkHeader, TreeChange, TreeStatus, UnifiedPatch};
use gix::prelude::ObjectIdExt as _;

use crate::graph_rebase::{
    PickDivergent, Step, StepGraph, StepGraphIndex,
    cherry_pick::{
        CherryPickOutcome, MergeOutcome, auto_resolution_tree_from_merging_commits, cherry_pick,
        merge_trees_into_target_commit,
    },
};

use super::resolved_cherry_pick_id;

pub(super) struct State {
    families: HashMap<[u8; 20], DivergentFamilyState>,
    family_sizes: HashMap<[u8; 20], usize>,
}

impl State {
    pub(super) fn new(graph: &StepGraph, steps_to_pick: &VecDeque<StepGraphIndex>) -> Self {
        let mut family_sizes = HashMap::new();
        for step_idx in steps_to_pick {
            if let Step::PickDivergent(pick_divergent) = &graph[*step_idx] {
                *family_sizes.entry(pick_divergent.family_id).or_insert(0) += 1;
            }
        }

        Self {
            families: HashMap::new(),
            family_sizes,
        }
    }

    pub(super) fn rebase_step(
        &mut self,
        repo: &gix::Repository,
        ontos: &[gix::ObjectId],
        pick_divergent: &PickDivergent,
    ) -> Result<gix::ObjectId> {
        let family_state = self
            .families
            .entry(pick_divergent.family_id)
            .or_insert_with(DivergentFamilyState::new);
        family_state.advance_member();

        // Attempt the remote materialization before enforcing the current
        // single-output-parent restriction so merge-parent failures still
        // surface through the existing graph cherry-pick error path.
        let remote_id = resolved_cherry_pick_id(
            pick_divergent.remote_commit,
            // TODO(CTO): This should tie into signing settings better. This may
            // not be used in the final tree either which would be an extra
            // signing that is not required.
            cherry_pick(repo, pick_divergent.remote_commit, ontos, true)?,
        )?;

        let onto_tree = rebase_base_tree(repo, ontos)?;
        let local_tree =
            family_state.current_local_tree(repo, onto_tree, &pick_divergent.local_commits)?;

        let merge_base_tree = match pick_divergent.ancestor {
            Some(ancestor) => rebase_divergence_ancestor_tree(repo, onto_tree, ancestor)?,
            None => onto_tree,
        };
        let remote_tree = commit_tree(repo, remote_id)?;
        let is_final_family_member = family_state.is_final_member(
            *self
                .family_sizes
                .get(&pick_divergent.family_id)
                .expect("family sizes are precomputed for all PickDivergent steps"),
        );

        let partitioned_pool = if is_final_family_member {
            PartitionedHunkPool {
                matching_tree: local_tree,
                remainder_tree: onto_tree,
                overlaps_remote: local_tree != onto_tree,
            }
        } else {
            partition_local_hunk_pool(repo, onto_tree, remote_tree, local_tree)?
        };

        let resolved_id = if !partitioned_pool.overlaps_remote
            || remote_tree == partitioned_pool.matching_tree
        {
            remote_id
        } else {
            match merge_trees_into_target_commit(
                repo,
                ontos,
                pick_divergent.remote_commit,
                merge_base_tree,
                remote_tree,
                partitioned_pool.matching_tree,
                true,
            )? {
                CherryPickOutcome::Commit(new_id)
                | CherryPickOutcome::ConflictedCommit(new_id)
                | CherryPickOutcome::Identity(new_id) => new_id,
                CherryPickOutcome::FailedToMergeBases { .. } => {
                    bail!(
                        "BUG: merge_trees_into_target_commit unexpectedly failed for PickDivergent"
                    );
                }
            }
        };

        if !is_final_family_member {
            family_state.store_remainder(onto_tree, partitioned_pool.remainder_tree);
        }

        Ok(resolved_id)
    }
}

/// Mutable state shared across all [`PickDivergent`] steps that belong to the
/// same family (identified by `family_id`) during a single rebase execution.
///
/// The struct is created when the first family member is encountered and
/// carried forward as subsequent members are processed in graph traversal
/// order. Future passes will populate the optional fields to drive the
/// divergent-change combining algorithm.
#[derive(Debug)]
enum DivergentHunkPool {
    Uninitialized,
    Tree {
        base_tree: gix::ObjectId,
        tree: gix::ObjectId,
    },
    Empty,
}

#[derive(Debug)]
struct DivergentFamilyState {
    /// How many family members have been processed so far.
    members_processed: usize,
    /// The carry-forward hunk-pool tree together with the tree it
    /// is currently based on.
    hunk_pool: DivergentHunkPool,
}

impl DivergentFamilyState {
    fn new() -> Self {
        Self {
            members_processed: 0,
            hunk_pool: DivergentHunkPool::Uninitialized,
        }
    }

    fn advance_member(&mut self) {
        self.members_processed += 1;
    }

    fn is_final_member(&self, family_size: usize) -> bool {
        self.members_processed == family_size
    }

    fn current_local_tree(
        &mut self,
        repo: &gix::Repository,
        onto_tree: gix::ObjectId,
        local_commits: &[gix::ObjectId],
    ) -> Result<gix::ObjectId> {
        match self.hunk_pool {
            DivergentHunkPool::Tree { base_tree, tree } => {
                let tree = rebase_hunk_pool_tree(repo, onto_tree, base_tree, tree)?;
                self.hunk_pool = DivergentHunkPool::Tree {
                    base_tree: onto_tree,
                    tree,
                };
                Ok(tree)
            }
            DivergentHunkPool::Empty => Ok(onto_tree),
            DivergentHunkPool::Uninitialized => {
                let local_tree = flatten_local_sequence_once(repo, onto_tree, local_commits)?;
                self.hunk_pool = DivergentHunkPool::Tree {
                    base_tree: onto_tree,
                    tree: local_tree,
                };
                Ok(local_tree)
            }
        }
    }

    fn store_remainder(&mut self, onto_tree: gix::ObjectId, remainder_tree: gix::ObjectId) {
        self.hunk_pool = if remainder_tree == onto_tree {
            DivergentHunkPool::Empty
        } else {
            DivergentHunkPool::Tree {
                base_tree: onto_tree,
                tree: remainder_tree,
            }
        };
    }
}

fn flatten_local_sequence_once(
    repo: &gix::Repository,
    onto_tree: gix::ObjectId,
    local_commits: &[gix::ObjectId],
) -> Result<gix::ObjectId> {
    let mut local_cursor_tree = onto_tree;
    for local_commit in local_commits {
        local_cursor_tree =
            replay_local_commit_preferring_local_result(repo, local_cursor_tree, *local_commit)?;
    }

    Ok(local_cursor_tree)
}

fn replay_local_commit_preferring_local_result(
    repo: &gix::Repository,
    onto_tree: gix::ObjectId,
    local_commit: gix::ObjectId,
) -> Result<gix::ObjectId> {
    let local_commit = but_core::Commit::from_id(local_commit.attach(repo))?;

    if local_commit.parents.len() > 1 {
        bail!("Cannot yet flatten merge commits into the divergent hunk pool");
    }

    let (base_tree, local_tree) = find_local_replay_trees(&local_commit)?;
    let mut replay = repo.merge_trees(
        base_tree,
        onto_tree.attach(repo),
        local_tree,
        repo.default_merge_labels(),
        repo.merge_options_force_theirs()?,
    )?;
    Ok(replay.tree.write()?.detach())
}

fn find_local_replay_trees<'repo>(
    local_commit: &but_core::Commit<'repo>,
) -> Result<(gix::Id<'repo>, gix::Id<'repo>)> {
    let repo = local_commit.id.repo;
    let base_tree = if local_commit.is_conflicted() {
        find_commit_tree(local_commit, TreeKind::Base)?
    } else {
        let base_commit_id = local_commit.parents.first().context("no parent")?;
        let base_commit = but_core::Commit::from_id(base_commit_id.attach(repo))?;
        find_commit_tree(&base_commit, TreeKind::AutoResolution)?
    };

    Ok((base_tree, find_commit_tree(local_commit, TreeKind::Theirs)?))
}

fn find_commit_tree<'repo>(
    commit: &but_core::Commit<'repo>,
    side: TreeKind,
) -> Result<gix::Id<'repo>> {
    Ok(if commit.is_conflicted() {
        let tree = commit.id.repo.find_tree(commit.tree)?;
        tree.find_entry(side.as_tree_entry_name())
            .context("Failed to get conflicted side of commit")?
            .id()
    } else {
        commit.tree_id_or_auto_resolution()?
    })
}

fn commit_tree(repo: &gix::Repository, commit: gix::ObjectId) -> Result<gix::ObjectId> {
    Ok(but_core::Commit::from_id(commit.attach(repo))?
        .tree_id_or_auto_resolution()?
        .detach())
}

#[derive(Debug)]
struct PartitionedHunkPool {
    matching_tree: gix::ObjectId,
    remainder_tree: gix::ObjectId,
    overlaps_remote: bool,
}

fn partition_local_hunk_pool(
    repo: &gix::Repository,
    base: gix::ObjectId,
    remote: gix::ObjectId,
    local: gix::ObjectId,
) -> Result<PartitionedHunkPool> {
    let remote_changes = tree_changes_with_rewrites(repo, base, remote)?;
    let local_changes = tree_changes_with_rewrites(repo, base, local)?;
    let forced_full_overlap_additions =
        split_rename_addition_overlap_paths(&remote_changes, &local_changes);
    let mut matching_tree = repo.edit_tree(base)?;
    let mut remainder_tree = repo.edit_tree(base)?;
    let mut overlaps_remote = false;

    for local_change in local_changes {
        let overlap = if forced_full_overlap_additions.contains(&local_change.path) {
            LocalChangeOverlap::Full
        } else {
            classify_local_change_overlap(repo, &remote_changes, &local_change)?
        };
        match overlap {
            LocalChangeOverlap::None => {
                apply_whole_tree_change(&mut remainder_tree, &local_change)?;
            }
            LocalChangeOverlap::Full => {
                overlaps_remote = true;
                apply_whole_tree_change(&mut matching_tree, &local_change)?;
            }
            LocalChangeOverlap::Partial {
                matching_hunks,
                remainder_hunks,
            } => {
                overlaps_remote = true;
                apply_selected_hunks(repo, &mut matching_tree, &local_change, &matching_hunks)?;
                apply_selected_hunks(repo, &mut remainder_tree, &local_change, &remainder_hunks)?;
            }
        }
    }

    Ok(PartitionedHunkPool {
        matching_tree: matching_tree.write()?.detach(),
        remainder_tree: remainder_tree.write()?.detach(),
        overlaps_remote,
    })
}

fn tree_changes_with_rewrites(
    repo: &gix::Repository,
    base: gix::ObjectId,
    tree: gix::ObjectId,
) -> Result<Vec<TreeChange>> {
    let base_tree = repo.find_tree(base)?;
    let tree = repo.find_tree(tree)?;
    let options = gix::diff::Options::default().with_rewrites(Some(gix::diff::Rewrites::default()));
    let mut changes: Vec<_> = repo
        .diff_tree_to_tree(Some(&base_tree), Some(&tree), Some(options))?
        .into_iter()
        .filter(|change| !change.entry_mode().is_tree())
        .map(Into::into)
        .collect();
    changes.sort_by(|a: &TreeChange, b: &TreeChange| a.path.cmp(&b.path));
    Ok(changes)
}

fn split_rename_addition_overlap_paths(
    remote_changes: &[TreeChange],
    local_changes: &[TreeChange],
) -> HashSet<BString> {
    let Some(remote_deleted_path) = only_matching_path(remote_changes, deletion_path) else {
        return HashSet::new();
    };
    let Some(remote_added_path) = only_matching_path(remote_changes, addition_path) else {
        return HashSet::new();
    };
    let Some(local_deleted_path) = only_matching_path(local_changes, deletion_path) else {
        return HashSet::new();
    };
    let Some(local_added_path) = only_matching_path(local_changes, addition_path) else {
        return HashSet::new();
    };

    if remote_deleted_path == local_deleted_path && remote_added_path != local_added_path {
        HashSet::from([local_added_path.to_owned()])
    } else {
        HashSet::new()
    }
}

fn only_matching_path<'a, F>(changes: &'a [TreeChange], matcher: F) -> Option<&'a bstr::BStr>
where
    F: Fn(&'a TreeChange) -> Option<&'a bstr::BStr>,
{
    let mut matches = changes.iter().filter_map(matcher);
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn deletion_path(change: &TreeChange) -> Option<&bstr::BStr> {
    matches!(change.status, TreeStatus::Deletion { .. }).then(|| change.path.as_bstr())
}

fn addition_path(change: &TreeChange) -> Option<&bstr::BStr> {
    matches!(change.status, TreeStatus::Addition { .. }).then(|| change.path.as_bstr())
}

enum LocalChangeOverlap {
    None,
    Full,
    Partial {
        matching_hunks: Vec<HunkHeader>,
        remainder_hunks: Vec<HunkHeader>,
    },
}

fn classify_local_change_overlap(
    repo: &gix::Repository,
    remote_changes: &[TreeChange],
    local_change: &TreeChange,
) -> Result<LocalChangeOverlap> {
    let overlapping_remote_changes: Vec<_> = remote_changes
        .iter()
        .filter(|remote_change| changes_touch_same_path(remote_change, local_change))
        .collect();

    if overlapping_remote_changes.is_empty() {
        return Ok(LocalChangeOverlap::None);
    }

    if change_requires_full_file_overlap(local_change)
        || overlapping_remote_changes
            .iter()
            .any(|remote_change| change_requires_full_file_overlap(remote_change))
    {
        return Ok(LocalChangeOverlap::Full);
    }

    let Some(UnifiedPatch::Patch {
        hunks: local_hunks, ..
    }) = local_change.unified_patch(repo, 0)?
    else {
        return Ok(LocalChangeOverlap::Full);
    };

    let mut remote_hunks = Vec::<HunkHeader>::new();
    for remote_change in overlapping_remote_changes {
        let Some(UnifiedPatch::Patch { hunks, .. }) = remote_change.unified_patch(repo, 0)? else {
            return Ok(LocalChangeOverlap::Full);
        };
        remote_hunks.extend(hunks.into_iter().map(HunkHeader::from));
    }

    let (matching_hunks, remainder_hunks): (Vec<_>, Vec<_>) = local_hunks
        .into_iter()
        .map(HunkHeader::from)
        .partition(|local_hunk| {
            remote_hunks.iter().any(|remote_hunk| {
                remote_hunk.old_range().intersects(local_hunk.old_range())
                    || remote_hunk.new_range().intersects(local_hunk.new_range())
            })
        });

    if matching_hunks.is_empty() {
        Ok(LocalChangeOverlap::None)
    } else {
        Ok(LocalChangeOverlap::Partial {
            matching_hunks,
            remainder_hunks,
        })
    }
}

fn change_requires_full_file_overlap(change: &TreeChange) -> bool {
    change.previous_path().is_some()
        || match &change.status {
            TreeStatus::Addition { .. }
            | TreeStatus::Deletion { .. }
            | TreeStatus::Rename { .. } => true,
            TreeStatus::Modification { flags, .. } => {
                flags.is_some_and(|flag| flag.is_typechange())
            }
        }
}

fn changes_touch_same_path(a: &TreeChange, b: &TreeChange) -> bool {
    let a_path = a.path.as_bstr();
    let b_path = b.path.as_bstr();
    let a_previous = a.previous_path();
    let b_previous = b.previous_path();

    a_path == b_path
        || Some(a_path) == b_previous
        || a_previous == Some(b_path)
        || a_previous
            .zip(b_previous)
            .is_some_and(|(a_prev, b_prev)| a_prev == b_prev)
}

fn apply_whole_tree_change(
    builder: &mut gix::object::tree::Editor<'_>,
    change: &TreeChange,
) -> Result<()> {
    if let Some(previous_path) = change.previous_path() {
        builder.remove(previous_path)?;
    }

    match change.status.state() {
        Some(state) => {
            builder.upsert(change.path.as_bstr(), state.kind, state.id)?;
        }
        None => {
            builder.remove(change.path.as_bstr())?;
        }
    }

    Ok(())
}

fn apply_selected_hunks(
    repo: &gix::Repository,
    builder: &mut gix::object::tree::Editor<'_>,
    change: &TreeChange,
    selected_hunks: &[HunkHeader],
) -> Result<()> {
    if selected_hunks.is_empty() {
        return Ok(());
    }

    let Some(current_state) = change.status.state() else {
        bail!(
            "BUG: hunk-based application requires a current state for path {}",
            change.path
        );
    };
    let Some((previous_state, _previous_path)) = change.status.previous_state_and_path() else {
        bail!(
            "BUG: hunk-based application requires a previous state for path {}",
            change.path
        );
    };

    let previous_blob = repo.find_blob(previous_state.id)?;
    let current_blob = repo.find_blob(current_state.id)?;
    let blob = but_core::apply_hunks(
        previous_blob.data.as_bstr(),
        current_blob.data.as_bstr(),
        selected_hunks,
    )?;
    let blob_id = repo.write_blob(blob.as_slice())?;
    builder.upsert(change.path.as_bstr(), current_state.kind, blob_id)?;
    Ok(())
}

fn rebase_base_tree(repo: &gix::Repository, ontos: &[gix::ObjectId]) -> Result<gix::ObjectId> {
    match ontos {
        [onto] => commit_tree(repo, *onto),
        [] => bail!("PickDivergent does not yet support root commits without output parents"),
        _ => match auto_resolution_tree_from_merging_commits(repo, ontos)? {
            MergeOutcome::Success(tree) => Ok(tree),
            MergeOutcome::NoCommit => Ok(gix::ObjectId::empty_tree(repo.object_hash())),
            MergeOutcome::Conflict { .. } => bail!(
                "BUG: PickDivergent output parents should already have been merged successfully"
            ),
        },
    }
}

fn rebase_hunk_pool_tree(
    repo: &gix::Repository,
    onto_tree: gix::ObjectId,
    base_tree: gix::ObjectId,
    tree: gix::ObjectId,
) -> Result<gix::ObjectId> {
    if onto_tree == base_tree {
        return Ok(tree);
    }

    let mut rebased = repo.merge_trees(
        base_tree.attach(repo),
        onto_tree.attach(repo),
        tree.attach(repo),
        repo.default_merge_labels(),
        repo.merge_options_force_ours()?,
    )?;
    Ok(rebased.tree.write()?.detach())
}

fn rebase_divergence_ancestor_tree(
    repo: &gix::Repository,
    onto_tree: gix::ObjectId,
    ancestor: gix::ObjectId,
) -> Result<gix::ObjectId> {
    let ancestor_commit = but_core::Commit::from_id(ancestor.attach(repo))?;
    if ancestor_commit.parents.len() > 1 {
        bail!("Cannot yet cherry-pick merge-commits - use rebasing for that");
    }

    let base_tree = if ancestor_commit.is_conflicted() {
        find_commit_tree(&ancestor_commit, TreeKind::Base)?
    } else if let Some(parent) = ancestor_commit.parents.first() {
        let parent_commit = but_core::Commit::from_id(parent.attach(repo))?;
        find_commit_tree(&parent_commit, TreeKind::AutoResolution)?
    } else {
        gix::ObjectId::empty_tree(repo.object_hash()).attach(repo)
    };
    let theirs_tree = find_commit_tree(&ancestor_commit, TreeKind::Theirs)?;
    let mut rebased = repo.merge_trees(
        base_tree,
        onto_tree.attach(repo),
        theirs_tree,
        repo.default_merge_labels(),
        repo.merge_options_force_ours()?,
    )?;
    Ok(rebased.tree.write()?.detach())
}

#[cfg(test)]
mod test {
    mod split_rename_addition_overlap_paths {
        use bstr::BString;
        use but_core::{ChangeState, TreeChange, TreeStatus};
        use gix::{hash::Kind, object::tree::EntryKind};

        use crate::graph_rebase::rebase::pick_divergent::split_rename_addition_overlap_paths;

        fn state() -> ChangeState {
            ChangeState {
                id: gix::ObjectId::null(Kind::Sha1),
                kind: EntryKind::Blob,
            }
        }

        fn addition(path: &str) -> TreeChange {
            TreeChange {
                path: path.into(),
                status: TreeStatus::Addition {
                    state: state(),
                    is_untracked: false,
                },
            }
        }

        fn deletion(path: &str) -> TreeChange {
            TreeChange {
                path: path.into(),
                status: TreeStatus::Deletion {
                    previous_state: state(),
                },
            }
        }

        fn modification(path: &str) -> TreeChange {
            TreeChange {
                path: path.into(),
                status: TreeStatus::Modification {
                    previous_state: state(),
                    state: state(),
                    flags: None,
                },
            }
        }

        #[test]
        fn ignores_unrelated_changes_while_detecting_split_rename() {
            let remote_changes = vec![
                deletion("old.txt"),
                addition("remote.txt"),
                modification("remote-only.txt"),
            ];
            let local_changes = vec![
                deletion("old.txt"),
                addition("local.txt"),
                modification("local-only.txt"),
            ];

            assert_eq!(
                split_rename_addition_overlap_paths(&remote_changes, &local_changes),
                [BString::from("local.txt")].into_iter().collect()
            );
        }

        #[test]
        fn bails_out_when_split_rename_shape_is_ambiguous() {
            let remote_changes = vec![deletion("old.txt"), addition("remote.txt")];
            let local_changes = vec![
                deletion("old.txt"),
                addition("local.txt"),
                addition("another-local.txt"),
            ];

            assert!(
                split_rename_addition_overlap_paths(&remote_changes, &local_changes).is_empty()
            );
        }
    }

    mod divergent_hunk_pool_partitioning {
        use anyhow::Result;
        use gix::prelude::ObjectIdExt as _;

        use crate::graph_rebase::rebase::pick_divergent::partition_local_hunk_pool;

        fn tree_id(repo: &gix::Repository, commit_id: gix::ObjectId) -> Result<gix::ObjectId> {
            Ok(but_core::Commit::from_id(commit_id.attach(repo))?
                .tree_id_or_auto_resolution()?
                .detach())
        }

        #[test]
        fn earlier_member_claims_ambiguous_overlap_before_later_member() -> Result<()> {
            let (repo, _tmpdir) =
                but_testsupport::writable_scenario("pick-divergent-hunk-pool-partitioning");

            let base_id = repo.rev_parse_single("base")?.detach();
            let local_id = repo.rev_parse_single("local")?.detach();
            let remote_one_id = repo.rev_parse_single("remote-one")?.detach();
            let carried_local_id = repo.rev_parse_single("carried-local")?.detach();
            let remote_two_id = repo.rev_parse_single("remote-two")?.detach();

            let base_tree = tree_id(&repo, base_id)?;
            let local_tree = tree_id(&repo, local_id)?;
            let remote_one_tree = tree_id(&repo, remote_one_id)?;
            let carried_local_tree = tree_id(&repo, carried_local_id)?;
            let remote_two_tree = tree_id(&repo, remote_two_id)?;

            let first_member =
                partition_local_hunk_pool(&repo, base_tree, remote_one_tree, local_tree)?;
            let second_member_if_unclaimed = partition_local_hunk_pool(
                &repo,
                remote_one_tree,
                remote_two_tree,
                carried_local_tree,
            )?;

            assert!(
                first_member.overlaps_remote,
                "expected the first remote member to overlap the local insertion"
            );
            assert!(
                second_member_if_unclaimed.overlaps_remote,
                "expected the same local insertion to also overlap the later remote member if it remained unclaimed"
            );
            assert_eq!(
                first_member.matching_tree, local_tree,
                "expected the earlier remote member to claim the entire ambiguous local hunk"
            );
            assert_eq!(
                first_member.remainder_tree, base_tree,
                "expected no remainder after the earlier member claims the ambiguous local hunk"
            );

            Ok(())
        }
    }

    mod divergent_family_state {
        use crate::graph_rebase::rebase::pick_divergent::{
            DivergentFamilyState, DivergentHunkPool,
        };

        #[test]
        fn new_state_starts_at_zero_members() {
            let state = DivergentFamilyState::new();
            assert_eq!(state.members_processed, 0);
            assert!(matches!(state.hunk_pool, DivergentHunkPool::Uninitialized));
        }

        #[test]
        fn members_processed_increments() {
            let mut state = DivergentFamilyState::new();
            state.advance_member();
            assert_eq!(state.members_processed, 1);
            state.advance_member();
            assert_eq!(state.members_processed, 2);
        }

        #[test]
        fn separate_families_get_separate_state() {
            use std::collections::HashMap;

            let family_a = [0xAAu8; 20];
            let family_b = [0xBBu8; 20];

            let mut families: HashMap<[u8; 20], DivergentFamilyState> = HashMap::new();

            families
                .entry(family_a)
                .or_insert_with(DivergentFamilyState::new)
                .advance_member();
            families
                .entry(family_b)
                .or_insert_with(DivergentFamilyState::new)
                .advance_member();
            families
                .entry(family_a)
                .or_insert_with(DivergentFamilyState::new)
                .advance_member();

            assert_eq!(families[&family_a].members_processed, 2);
            assert_eq!(families[&family_b].members_processed, 1);
        }

        #[test]
        fn final_member_detection_tracks_processed_members() {
            let mut state = DivergentFamilyState::new();

            state.advance_member();
            assert!(!state.is_final_member(2));

            state.advance_member();
            assert!(state.is_final_member(2));
        }
    }
}

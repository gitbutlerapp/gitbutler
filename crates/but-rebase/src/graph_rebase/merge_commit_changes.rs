#![doc = include_str!("../../docs/merge_commit_changes.md")]

use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::{Context, Result, bail};
use but_core::{RefMetadata, RepositoryExt};
use gix::prelude::ObjectIdExt;
use petgraph::Direction;

use crate::graph_rebase::{Editor, Pick, Step, StepGraphIndex, util::collect_ordered_parents};

/// A selected commit change range that should be merged into an accumulated
/// tree.
///
/// `base_tree_id` is the tree from which `commit_id`'s contribution should be
/// measured. Merging `base_tree_id..commit_id^{tree}` applies only that
/// commit's selected change range, not all tree state reachable through its
/// parents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannedCommitChange {
    /// The selected commit whose change range should be merged.
    pub commit_id: gix::ObjectId,
    /// The tree that acts as the base for `commit_id`'s contribution.
    pub base_tree_id: gix::ObjectId,
}

/// The result of merging selected commit changes into one tree.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeCommitChangesOutcome {
    /// The resulting tree.
    ///
    /// If [`Self::conflict`] is `Some`, this is the auto-resolved tree that
    /// should be presented as the conflicted commit's visible tree.
    pub tree_id: gix::ObjectId,
    /// Details about the last unresolved merge encountered while producing
    /// [`Self::tree_id`].
    pub conflict: Option<MergeCommitChangesConflict>,
}

/// Conflict metadata needed to persist a GitButler conflicted commit from a
/// merged change-range result.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeCommitChangesConflict {
    /// The merge base trees for the conflicted merge step.
    pub base_tree_ids: Vec<gix::ObjectId>,
    /// The merge side trees for the conflicted merge step.
    pub side_tree_ids: Vec<gix::ObjectId>,
    /// The paths that remained conflicted after auto-resolution.
    pub conflict_entries: but_core::commit::ConflictEntries,
}

impl<M: RefMetadata> Editor<'_, '_, M> {
    /// Return the tree produced by preserving the target commit's full tree
    /// and then merging only the surviving selected commits' own change
    /// ranges into it.
    ///
    /// See the module docs for the planner and merge semantics.
    pub fn merge_commit_changes_to_tree(
        &self,
        target_commit_id: gix::ObjectId,
        subject_commit_ids: Vec<gix::ObjectId>,
        merge_options: gix::merge::tree::Options,
    ) -> Result<MergeCommitChangesOutcome> {
        let target_commit = but_core::Commit::from_id(target_commit_id.attach(&self.repo))?;
        let merge_plan =
            self.plan_commit_changes_for_merge(target_commit_id, subject_commit_ids)?;

        let mut bases = VecDeque::from(target_commit.base_tree_ids()?);
        let mut sides = VecDeque::from(target_commit.side_tree_ids()?);
        for change in merge_plan {
            bases.push_back(change.base_tree_id);
            let commit = but_core::Commit::from_id(change.commit_id.attach(&self.repo))?;
            bases.extend(commit.base_tree_ids()?);
            sides.extend(commit.side_tree_ids()?);
        }

        // Simplify: a base and a side that are identical cancel each other out
        for i in (0..bases.len()).rev() {
            if let Some(position) = sides.iter().position(|side| *side == bases[i]) {
                sides.remove(position);
                bases.remove(i);
            }
        }

        let conflict_kind = gix::merge::tree::TreatAsUnresolved::forced_resolution();
        let mut ours = sides
            .pop_front()
            .context("BUG: target_commit should have at least one side")?;
        while let Some(base) = bases.pop_front() {
            let theirs = sides
                .pop_front()
                .context("attempting to merge commit with unbalanced bases and sides")?;
            let mut merge = self.repo.merge_trees(
                base,
                ours,
                theirs,
                self.repo.default_merge_labels(),
                merge_options.clone(),
            )?;
            let merged_tree_id = merge.tree.write()?.detach();

            if merge.has_unresolved_conflicts(conflict_kind) {
                // Push back the things that conflict when merged. Note that
                // "ours" needs to be the frontmost, so we push "theirs" then
                // "ours".
                sides.push_front(theirs);
                sides.push_front(ours);
                bases.push_front(base);
                let conflict = Some(MergeCommitChangesConflict {
                    base_tree_ids: bases.into(),
                    side_tree_ids: sides.into(),
                    conflict_entries: but_core::commit::conflict_entries_from_merge_outcome(
                        &self.repo,
                        merged_tree_id,
                        &merge,
                        conflict_kind,
                    )?,
                });
                return Ok(MergeCommitChangesOutcome {
                    // As described in the module-level documentation, we stop
                    // at the first unresolved merge conflict.
                    tree_id: merged_tree_id,
                    conflict,
                });
            }

            ours = merged_tree_id;
        }

        Ok(MergeCommitChangesOutcome {
            tree_id: ours,
            conflict: None,
        })
    }

    /// Turn the selected commits into mergeable `base..commit` change ranges
    /// relative to a target commit.
    ///
    /// This planner assumes the editor step graph and the editor's in-memory
    /// repository describe the same commit topology. The step graph is used to
    /// traverse selected commits deterministically and to find the target's
    /// ancestry cone quickly, while first-parent/base semantics still come
    /// from the commit objects stored in the in-memory repository.
    ///
    /// See the module docs for the planner semantics and invariants.
    ///
    ///
    /// Returns a vector of [PlannedCommitChange].
    pub fn plan_commit_changes_for_merge(
        &self,
        target_commit_id: gix::ObjectId,
        subject_commit_ids: Vec<gix::ObjectId>,
    ) -> Result<Vec<PlannedCommitChange>> {
        self.try_select_commit(target_commit_id).with_context(|| {
            format!("Failed to find commit {target_commit_id} in rebase editor")
        })?;
        let selected_commit_ids = deduplicate_commit_ids(subject_commit_ids);
        if selected_commit_ids.is_empty() {
            return Ok(Vec::new());
        }

        let selected_commit_set = selected_commit_ids.iter().copied().collect::<HashSet<_>>();
        let traversal = traverse_graph_for_planning(self, target_commit_id, &selected_commit_set)?;

        for commit_id in &selected_commit_ids {
            if !traversal
                .first_parent_metadata_by_selected_commit_id
                .contains_key(commit_id)
            {
                bail!("Failed to find commit {commit_id} in rebase editor");
            }
        }

        // A map from selected commit ID to how many child commits it has that
        // are also selected.
        let mut selected_child_count = HashMap::<gix::ObjectId, usize>::with_capacity(
            traversal.ordered_selected_commit_ids.len(),
        );
        for commit_id in traversal.ordered_selected_commit_ids.iter().copied() {
            selected_child_count.insert(commit_id, 0);
        }
        for selected_parent_id in traversal
            .first_parent_metadata_by_selected_commit_id
            .values()
            .filter_map(|metadata| metadata.selected_first_parent_id)
        {
            if let Some(child_count) = selected_child_count.get_mut(&selected_parent_id) {
                *child_count += 1;
            }
        }

        let mut planned_commit_changes = Vec::new();
        for commit_id in traversal.ordered_selected_commit_ids {
            let is_selected_chain_tip = selected_child_count
                .get(&commit_id)
                .copied()
                .unwrap_or_default()
                == 0;
            // Skip over non-tips (i.e. selected commits that have at least one
            // child that is itself selected) and commits that are ancestors of
            // the target. The changes in non-tips are subsumed into the tip of
            // the chain it's part of.
            //
            // For example, consider the chain M<-A<-B<-C where A,B,C are
            // selected. A and B will be skipped; we will only emit one change C
            // with base M, which thus includes all changes in A and B too.
            if !is_selected_chain_tip || traversal.target_ancestor_commit_ids.contains(&commit_id) {
                continue;
            }

            let base_tree_id = base_tree_id_for_emitted_tip(
                self,
                commit_id,
                &traversal.first_parent_metadata_by_selected_commit_id,
                &traversal.target_ancestor_commit_ids,
            )?;
            planned_commit_changes.push(PlannedCommitChange {
                commit_id,
                base_tree_id,
            });
        }

        Ok(planned_commit_changes)
    }
}

/// First-parent metadata for a selected commit. This is subsequently used to
/// detect chains of selected commits, where each link of the chain is between a
/// selected commit and its also-selected first parent.
#[derive(Debug, Clone, Copy)]
struct FirstParentMetadata {
    /// The first parent ID of the commit, if one exists.
    first_parent_id: Option<gix::ObjectId>,
    /// Identical to `first_parent_id` if it is also selected; otherwise,
    /// `None`.
    selected_first_parent_id: Option<gix::ObjectId>,
}

#[derive(Debug, Default)]
struct SelectedCommitPlanningTraversal {
    /// Selected commit IDs in reverse topological order.
    ordered_selected_commit_ids: Vec<gix::ObjectId>,
    /// First parent metadata for each element of [Self::ordered_selected_commit_ids].
    first_parent_metadata_by_selected_commit_id: HashMap<gix::ObjectId, FirstParentMetadata>,
    /// The target commit ID and all its ancestor commits in the graph.
    target_ancestor_commit_ids: HashSet<gix::ObjectId>,
}

#[derive(Debug, Clone, Copy, Default)]
enum TraversalMode {
    #[default]
    Normal,
    MarkTargetAncestors,
}

/// Traverse the editor graph and return information relevant for the planning.
///
/// The order of elements in the returned
/// [SelectedCommitPlanningTraversal::ordered_selected_commit_ids] is
/// deterministic and based solely on the graph.
///
/// This walk intentionally uses the editor step graph only as a traversal
/// structure. Parent/tree semantics are still read from the in-memory
/// repository commits, so callers must only use this planner when the editor
/// graph and the in-memory repository represent the same commit topology.
fn traverse_graph_for_planning<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    target_commit_id: gix::ObjectId,
    selected_commit_ids: &HashSet<gix::ObjectId>,
) -> Result<SelectedCommitPlanningTraversal> {
    let mut traversal = SelectedCommitPlanningTraversal::default();
    let mut seen_selected_commit_ids = HashSet::new();
    let mut seen_normal = HashSet::<StepGraphIndex>::new();
    let mut seen_target_ancestor_walk = HashSet::<StepGraphIndex>::new();

    let mut roots = editor
        .graph
        .externals(Direction::Incoming)
        .collect::<Vec<StepGraphIndex>>();
    roots.sort_unstable_by_key(|idx| idx.index());

    for root in roots {
        let mut stack = vec![(root, false, TraversalMode::Normal)];
        while let Some((node, expanded, mode)) = stack.pop() {
            match mode {
                TraversalMode::Normal => {
                    if expanded {
                        if let Step::Pick(Pick { id, .. }) = editor.graph[node] {
                            if let Some(first_parent_metadata) =
                                get_first_parent_metadata(editor, id, selected_commit_ids)?
                            {
                                traversal
                                    .first_parent_metadata_by_selected_commit_id
                                    .insert(id, first_parent_metadata);
                                if seen_selected_commit_ids.insert(id) {
                                    traversal.ordered_selected_commit_ids.push(id);
                                }
                            }

                            if id == target_commit_id {
                                traversal.target_ancestor_commit_ids.insert(id);
                                for parent_idx in collect_ordered_parents(&editor.graph, node) {
                                    stack.push((
                                        parent_idx,
                                        false,
                                        TraversalMode::MarkTargetAncestors,
                                    ));
                                }
                            }
                        }
                        continue;
                    }

                    if !seen_normal.insert(node) {
                        continue;
                    }

                    let parents = collect_ordered_parents(&editor.graph, node);
                    stack.push((node, true, TraversalMode::Normal));
                    for parent_idx in parents.into_iter() {
                        stack.push((parent_idx, false, TraversalMode::Normal));
                    }
                }
                TraversalMode::MarkTargetAncestors => {
                    if !seen_target_ancestor_walk.insert(node) {
                        continue;
                    }

                    if let Step::Pick(Pick { id, .. }) = editor.graph[node] {
                        traversal.target_ancestor_commit_ids.insert(id);
                    }

                    for parent_idx in collect_ordered_parents(&editor.graph, node) {
                        stack.push((parent_idx, false, TraversalMode::MarkTargetAncestors));
                    }
                }
            }
        }
    }

    Ok(traversal)
}

/// Returns first-parent metadata if `commit_id` is selected.
///
/// This intentionally reads the commit object from the editor's in-memory
/// repository instead of inferring first-parent semantics from the step graph.
/// The step graph only provides deterministic traversal order and fast target
/// ancestry discovery. The actual `base..commit` merge ranges must match the
/// commit DAG that owns the trees and SHAs being merged, so callers are
/// expected to keep the editor step graph aligned with that in-memory
/// repository topology.
fn get_first_parent_metadata<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    commit_id: gix::ObjectId,
    selected_commit_ids: &HashSet<gix::ObjectId>,
) -> Result<Option<FirstParentMetadata>> {
    if !selected_commit_ids.contains(&commit_id) {
        return Ok(None);
    }

    let commit = but_core::Commit::from_id(commit_id.attach(&editor.repo))?;
    let first_parent_id = commit.inner.parents.first().copied();
    let selected_first_parent_id =
        first_parent_id.filter(|parent_id| selected_commit_ids.contains(parent_id));

    Ok(Some(FirstParentMetadata {
        first_parent_id,
        selected_first_parent_id,
    }))
}

/// Get the base tree ID to use for the merger of the given commit.
///
/// The base tree ID is obtained by following the first parent link until a
/// non-selected commit or a target ancestor is encountered; the base tree ID is
/// thus the tree of that commit.
///
/// If the first parent link cannot be followed due to a commit not having any
/// parents, the empty tree is returned.
fn base_tree_id_for_emitted_tip<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    tip_commit_id: gix::ObjectId,
    selected_metadata_by_commit: &HashMap<gix::ObjectId, FirstParentMetadata>,
    target_ancestor_commit_ids: &HashSet<gix::ObjectId>,
) -> Result<gix::ObjectId> {
    let mut current_commit_id = tip_commit_id;

    loop {
        let metadata = selected_metadata_by_commit
            .get(&current_commit_id)
            .with_context(|| {
                format!("BUG: missing planning metadata for selected commit {current_commit_id}")
            })?;

        match metadata.selected_first_parent_id {
            Some(selected_parent_id)
                if target_ancestor_commit_ids.contains(&selected_parent_id) =>
            {
                return tree_id_for_commit(editor, selected_parent_id);
            }
            Some(selected_parent_id) => {
                current_commit_id = selected_parent_id;
            }
            None => {
                return match metadata.first_parent_id {
                    Some(parent_id) => tree_id_for_commit(editor, parent_id),
                    None => Ok(gix::ObjectId::empty_tree(editor.repo.object_hash())),
                };
            }
        }
    }
}

fn tree_id_for_commit<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    commit_id: gix::ObjectId,
) -> Result<gix::ObjectId> {
    Ok(but_core::Commit::from_id(commit_id.attach(&editor.repo))?
        .tree_id_or_auto_resolution()?
        .detach())
}

/// Remove exact duplicate commit IDs.
fn deduplicate_commit_ids(commit_ids: Vec<gix::ObjectId>) -> Vec<gix::ObjectId> {
    let mut seen = HashSet::with_capacity(commit_ids.len());
    let mut deduplicated = Vec::with_capacity(commit_ids.len());
    for commit_id in commit_ids {
        if seen.insert(commit_id) {
            deduplicated.push(commit_id);
        }
    }
    deduplicated
}

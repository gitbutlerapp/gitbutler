//! Provides some slightly higher level tools to help with manipulating commits, in preparation for use in the editor.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};
use but_core::commit::SignCommit;
use but_core::{RefMetadata, RepositoryExt, commit::Headers};
use but_graph::init::Overlay;
use gix::prelude::ObjectIdExt;

use crate::{
    commit::{DateMode, create},
    graph_rebase::{
        Editor, Pick, Selector, Step, ToCommitSelector, ToReferenceSelector,
        util::collect_ordered_parents,
    },
};

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
    /// The merge base tree for the conflicted merge step.
    pub base_tree_id: gix::ObjectId,
    /// The accumulated tree on the "ours" side of the conflicted merge step.
    pub ours_tree_id: gix::ObjectId,
    /// The selected commit tree on the "theirs" side of the conflicted merge step.
    pub theirs_tree_id: gix::ObjectId,
    /// The paths that remained conflicted after auto-resolution.
    pub conflict_entries: but_core::commit::ConflictEntries,
}

impl<M: RefMetadata> Editor<'_, '_, M> {
    /// Returns a reference to the in-memory repository.
    pub fn repo(&self) -> &gix::Repository {
        &self.repo
    }

    /// Return the tree produced by merging only the changes attributable to
    /// `commit_ids`.
    ///
    /// This helper is intentionally narrower than "merge the trees of these
    /// commits". Each selected commit contributes only its own selected change
    /// range, and not the full tree state inherited through unselected
    /// parents.
    ///
    /// The merge proceeds as follows:
    /// - Exact duplicate commit IDs are ignored.
    /// - The selected commits are turned into a merge plan by following each
    ///   selected commit's first-parent chain until it leaves the selected set.
    /// - Selected commits that are the direct first parent of another selected
    ///   commit are omitted from the plan, because their contribution is
    ///   already part of the descendant's selected first-parent range.
    /// - If that leaves a single planned commit, its tree is returned as-is.
    /// - Otherwise, the merge starts from the shared merge-base of the planned
    ///   commits.
    /// - The helper then merges each planned commit's `base..commit` change
    ///   range into the accumulated tree.
    /// - If one of those merge steps has unresolved conflicts, folding stops
    ///   immediately and later planned commits are not merged into the result.
    ///
    /// In practice, this means contiguous selected first-parent chains are
    /// collapsed into one planned change range, while non-selected commits in
    /// the middle of a selected history remain excluded from the result.
    ///
    /// Example:
    /// - selected commits: `A`, `C`
    /// - history: `M <- B <- C` and `M <- A`
    ///
    /// Then the result contains the changes from `A` and `C`, but not the
    /// changes introduced by `B`. `C` is measured relative to `B`, not
    /// relative to `M`.
    ///
    /// If an unresolved merge is encountered, the returned outcome keeps the
    /// auto-resolved tree from that first conflicted step in
    /// [`MergeCommitChangesOutcome::tree_id`] together with enough metadata in
    /// [`MergeCommitChangesOutcome::conflict`] to materialize a GitButler
    /// conflicted commit.
    pub fn merge_commit_changes_to_tree(
        &self,
        commit_ids: Vec<gix::ObjectId>,
        merge_options: gix::merge::tree::Options,
    ) -> Result<MergeCommitChangesOutcome> {
        let (graph, merge_plan) = self.plan_commit_changes_for_merge(commit_ids)?;
        let Some(first_commit_change) = merge_plan.first().copied() else {
            bail!("Cannot merge an empty set of commits");
        };
        if merge_plan.len() == 1 {
            let commit =
                but_core::Commit::from_id(first_commit_change.commit_id.attach(&self.repo))?;
            let tree_id = commit.tree_id_or_auto_resolution()?.detach();
            let conflict = if commit.is_conflicted() {
                let (base_tree_id, ours_tree_id, theirs_tree_id) = commit
                    .conflicted_tree_ids()?
                    .context("conflicted commit is missing conflicted tree sides")?;
                Some(MergeCommitChangesConflict {
                    base_tree_id: base_tree_id.detach(),
                    ours_tree_id: ours_tree_id.detach(),
                    theirs_tree_id: theirs_tree_id.detach(),
                    conflict_entries: commit
                        .conflict_entries()?
                        .context("conflicted commit is missing conflict entries")?,
                })
            } else {
                None
            };
            return Ok(MergeCommitChangesOutcome { tree_id, conflict });
        }

        let merge_base = graph
            .find_merge_base_octopus_by_commit_id(merge_plan.iter().map(|change| change.commit_id))?
            .context("failed to compute merge-base for planned commit changes")?;
        let mut ours = but_core::Commit::from_id(merge_base.attach(&self.repo))?
            .tree_id_or_auto_resolution()?
            .detach();
        let mut conflict = None;
        let conflict_kind = gix::merge::tree::TreatAsUnresolved::forced_resolution();

        for change in merge_plan {
            let theirs = but_core::Commit::from_id(change.commit_id.attach(&self.repo))?
                .tree_id_or_auto_resolution()?
                .detach();
            let mut merge = self
                .repo
                .merge_trees(
                    change.base_tree_id,
                    ours,
                    theirs,
                    self.repo.default_merge_labels(),
                    merge_options.clone(),
                )
                .context("failed to merge commit trees")?;
            let merged_tree_id = merge.tree.write()?.detach();
            if merge.has_unresolved_conflicts(conflict_kind) {
                conflict = Some(MergeCommitChangesConflict {
                    base_tree_id: change.base_tree_id,
                    ours_tree_id: ours,
                    theirs_tree_id: theirs,
                    conflict_entries: but_core::commit::conflict_entries_from_merge_outcome(
                        &self.repo,
                        merged_tree_id,
                        &merge,
                        conflict_kind,
                    )?,
                });
                ours = merged_tree_id;
                break;
            }
            ours = merged_tree_id;
        }

        Ok(MergeCommitChangesOutcome {
            tree_id: ours,
            conflict,
        })
    }

    /// Exposed only when the crate is built with the `testing` feature.
    ///
    /// This keeps merge-plan assertions available to the integration tests
    /// without making the helper part of the normal public API surface.
    #[cfg(feature = "testing")]
    #[doc(hidden)]
    pub fn plan_commit_changes_for_merge_for_tests(
        &self,
        commit_ids: Vec<gix::ObjectId>,
    ) -> Result<Vec<PlannedCommitChange>> {
        let (_, plan) = self.plan_commit_changes_for_merge(commit_ids)?;
        Ok(plan)
    }

    /// Set a merge-base override for checkout so that consumed worktree
    /// changes don't reappear as uncommitted after materialization.
    pub fn set_merge_base_override(&mut self, tree_id: gix::ObjectId) {
        for checkout in &mut self.checkouts {
            match checkout {
                super::Checkout::Head {
                    merge_base_override,
                    ..
                } => {
                    *merge_base_override = Some(tree_id);
                }
            }
        }
    }

    /// Finds a commit from inside the editor's in memory repository.
    pub fn find_commit(&self, id: gix::ObjectId) -> Result<but_core::CommitOwned> {
        but_core::Commit::from_id(id.attach(&self.repo)).map(|c| c.detach())
    }

    /// Finds a commit that is selectable in the editor graph and is
    /// found in the editor's repo.
    ///
    /// Returns the normalized selector and the found commit.
    pub fn find_selectable_commit(
        &self,
        selector: impl ToCommitSelector,
    ) -> Result<(Selector, but_core::CommitOwned)> {
        let selector = self
            .history
            .normalize_selector(selector.to_commit_selector(self)?)?;
        let Step::Pick(Pick { id, .. }) = &self.graph[selector.id] else {
            bail!("BUG: Expected pick step from commit selector. This should never happen");
        };
        Ok((selector, self.find_commit(*id)?))
    }

    /// Finds the first pick parent of a reference
    pub fn find_reference_target(
        &self,
        selector: impl ToReferenceSelector,
    ) -> Result<(Selector, but_core::CommitOwned)> {
        let selector = self
            .history
            .normalize_selector(selector.to_reference_selector(self)?)?;

        let parents = collect_ordered_parents(&self.graph, selector.id);
        let first_parent = parents
            .first()
            .context("Failed to find a parent for selected reference in the step graph.")?;

        let Step::Pick(pick) = &self.graph[*first_parent] else {
            bail!("BUG: collect_ordered_parents provided a non-pick return value");
        };

        Ok((self.new_selector(*first_parent), self.find_commit(pick.id)?))
    }

    /// Writes a commit with correct signing to the in memory repository,
    /// without updating the history log.
    pub fn new_commit_untracked(
        &self,
        commit: but_core::CommitOwned,
        date_mode: DateMode,
    ) -> Result<gix::ObjectId> {
        create(
            &self.repo,
            commit.inner,
            date_mode,
            SignCommit::IfSignCommitsEnabled,
        )
    }

    /// Writes a commit with correct signing to the in memory repository.
    pub fn new_commit(
        &mut self,
        commit: but_core::CommitOwned,
        date_mode: DateMode,
    ) -> Result<gix::ObjectId> {
        let commit_id = commit.id;
        let new_id = self.new_commit_untracked(commit, date_mode)?;
        self.history.update_mapping(commit_id, new_id);
        Ok(new_id)
    }

    /// Creates a commit with only the signature, author, and headers set correctly.
    ///
    /// The ID of the commit is all zeros & the commit hasn't been written into any ODB
    pub fn empty_commit(&self) -> Result<but_core::CommitOwned> {
        let kind = gix::hash::Kind::Sha1;
        let committer = self
            .repo
            .committer()
            .transpose()?
            .context("Need committer to be configured when creating a new commit")?
            .into();
        let author = self
            .repo
            .committer()
            .transpose()?
            .context("Need author to be configured when creating a new commit")?
            .into();
        let obj = gix::objs::Commit {
            tree: gix::ObjectId::empty_tree(kind),
            parents: vec![].into(),
            committer,
            author,
            encoding: None,
            message: b"".into(),
            extra_headers: (&Headers::from_config(&self.repo.config_snapshot())).into(),
        };

        Ok(but_core::CommitOwned {
            id: gix::ObjectId::null(kind),
            inner: obj,
        })
    }

    /// Rebuild the workspace graph as it would look after applying the
    /// editor's current ref and checkout edits.
    ///
    /// This helper projects the editor's in-memory state back into a
    /// traversable graph by:
    /// - collecting the references currently represented in the editor,
    /// - marking mutable references that were removed in the editor as dropped,
    /// - preserving the current checkout entrypoint, and
    /// - re-running graph traversal against the editor's in-memory repository.
    ///
    /// Any commits reachable only through in-editor rewrites are therefore
    /// visible in the returned graph and must be accessed through `self.repo`.
    fn overlayed_graph(&self) -> Result<but_graph::Graph> {
        let mut current_references = Vec::new();
        let mut current_ref_names = HashSet::new();
        for node_idx in self.graph.node_indices() {
            let Step::Reference { refname } = &self.graph[node_idx] else {
                continue;
            };
            let first_parent_idx = collect_ordered_parents(&self.graph, node_idx)
                .first()
                .copied()
                .context("References should have at least one parent")?;
            let Step::Pick(Pick { id, .. }) = self.graph[first_parent_idx] else {
                bail!("BUG: collect_ordered_parents provided a non-pick return value");
            };
            current_ref_names.insert(refname.clone());
            current_references.push(gix::refs::Reference {
                name: refname.clone(),
                target: gix::refs::Target::Object(id),
                peeled: None,
            });
        }

        let dropped_refs = self
            .initial_references
            .iter()
            .filter(|reference| {
                !self.immutable_references.contains(*reference)
                    && !current_ref_names.contains(*reference)
            })
            .cloned()
            .collect::<Vec<_>>();

        let Some((entrypoint_id, entrypoint_refname)) = self
            .checkouts
            .iter()
            .filter_map(|checkout| match checkout {
                super::Checkout::Head { selector, .. } => {
                    let selector = self.history.normalize_selector(*selector).ok()?;
                    let step = &self.graph[selector.id];

                    match step {
                        Step::None => None,
                        Step::Pick(Pick { id, .. }) => Some((*id, None)),
                        Step::Reference { refname } => {
                            let parents = collect_ordered_parents(&self.graph, selector.id);

                            if let Some(to_reference) = parents.first()
                                && let Step::Pick(Pick { id, .. }) = self.graph[*to_reference]
                            {
                                Some((id, Some(refname.clone())))
                            } else {
                                None
                            }
                        }
                    }
                }
            })
            .next()
        else {
            bail!("BUG: Tried to construct rebase editor graph overlay with no entrypoints");
        };

        let overlay = Overlay::default()
            .with_references(current_references)
            .with_dropped_references(dropped_refs)
            .with_entrypoint(entrypoint_id, entrypoint_refname);
        self.workspace
            .graph
            .redo_traversal_with_overlay(&self.repo, &*self.meta, overlay)
    }

    /// Turn the selected commits into mergeable `base..commit` change ranges.
    ///
    /// The returned plan preserves the semantics of
    /// [`Self::merge_commit_changes_to_tree()`]:
    /// - exact duplicate selections are ignored,
    /// - contiguous selected first-parent chains collapse into one range, and
    /// - non-contiguous selections remain separate so unselected commits in
    ///   between do not leak into the merge result.
    ///
    /// The helper also returns the overlayed graph used during planning so the
    /// caller can compute merge-bases against the same editor-visible history.
    fn plan_commit_changes_for_merge(
        &self,
        commit_ids: Vec<gix::ObjectId>,
    ) -> Result<(but_graph::Graph, Vec<PlannedCommitChange>)> {
        let selected_commit_ids = deduplicate_commit_ids(commit_ids);
        let selected_commit_set = selected_commit_ids.iter().copied().collect::<HashSet<_>>();
        let overlayed_graph = self.overlayed_graph()?;
        let mut first_parent_cache = HashMap::<gix::ObjectId, Option<gix::ObjectId>>::new();
        let direct_selected_parent_by_commit = selected_commit_ids
            .iter()
            .copied()
            .map(|commit_id| {
                let selected_parent_id =
                    first_parent_of(self, &overlayed_graph, commit_id, &mut first_parent_cache)?
                        .filter(|parent_id| selected_commit_set.contains(parent_id));
                Ok((commit_id, selected_parent_id))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        let mut selected_child_count =
            HashMap::<gix::ObjectId, usize>::with_capacity(selected_commit_ids.len());
        for commit_id in selected_commit_ids.iter().copied() {
            selected_child_count.insert(commit_id, 0);
        }
        for selected_parent_id in direct_selected_parent_by_commit.values().flatten() {
            if let Some(child_count) = selected_child_count.get_mut(selected_parent_id) {
                *child_count += 1;
            }
        }

        let mut consumed_commit_ids = HashSet::with_capacity(selected_commit_ids.len());
        let mut planned_commit_changes = Vec::new();

        for commit_id in selected_commit_ids {
            let is_direct_parent_of_selected_commit = selected_child_count
                .get(&commit_id)
                .copied()
                .unwrap_or_default()
                > 0;
            if is_direct_parent_of_selected_commit || !consumed_commit_ids.insert(commit_id) {
                continue;
            }

            let mut current_commit_id = commit_id;
            while let Some(selected_parent_id) = direct_selected_parent_by_commit
                .get(&current_commit_id)
                .copied()
                .flatten()
            {
                consumed_commit_ids.insert(selected_parent_id);
                current_commit_id = selected_parent_id;
            }

            let base_tree_id = match first_parent_of(
                self,
                &overlayed_graph,
                current_commit_id,
                &mut first_parent_cache,
            )? {
                Some(parent_id) => but_core::Commit::from_id(parent_id.attach(&self.repo))?
                    .tree_id_or_auto_resolution()?
                    .detach(),
                None => gix::ObjectId::empty_tree(self.repo.object_hash()),
            };

            planned_commit_changes.push(PlannedCommitChange {
                commit_id,
                base_tree_id,
            });
        }

        Ok((overlayed_graph, planned_commit_changes))
    }
}

/// Resolve a commit's first parent from the editor-visible history.
///
/// For selected commits we prefer the editor's step graph, because the commit
/// may already have been rewritten there. We still verify that the resulting
/// parent relationship exists in the overlayed `but_graph`, which keeps
/// planning aligned with the graph-based merge-base queries used later.
///
/// The fallback to the commit object itself only fills in parent information
/// when the editor step graph does not expose one directly, such as for
/// in-memory rewritten commits. Selected commits must still be present in the
/// editor graph; missing commits are treated as an error.
fn first_parent_of<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    graph: &but_graph::Graph,
    commit_id: gix::ObjectId,
    first_parent_cache: &mut HashMap<gix::ObjectId, Option<gix::ObjectId>>,
) -> Result<Option<gix::ObjectId>> {
    if let Some(parent_id) = first_parent_cache.get(&commit_id).copied() {
        return Ok(parent_id);
    }

    let selector = editor
        .try_select_commit(commit_id)
        .with_context(|| format!("Failed to find commit {commit_id} in rebase editor"))?;
    let selector = editor.history.normalize_selector(selector)?;
    let parent_id = collect_ordered_parents(&editor.graph, selector.id)
        .first()
        .copied()
        .map(|parent_idx| match editor.graph[parent_idx] {
            Step::Pick(Pick { id, .. }) => Ok(id),
            _ => bail!("BUG: collect_ordered_parents provided a non-pick return value"),
        })
        .transpose()?
        .or_else(|| {
            but_core::Commit::from_id(commit_id.attach(&editor.repo))
                .ok()?
                .inner
                .parents
                .first()
                .copied()
        });

    if let Some(parent_id) = parent_id {
        graph
            .find_merge_base_by_commit_id(commit_id, parent_id)?
            .with_context(|| {
                format!(
                    "BUG: commit {commit_id} is missing first-parent {parent_id} in the overlayed graph"
                )
            })?;
    }

    first_parent_cache.insert(commit_id, parent_id);
    Ok(parent_id)
}

/// Remove exact duplicate commit IDs while preserving the first-seen order.
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

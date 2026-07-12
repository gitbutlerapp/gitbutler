//! Feature-local planning helpers for editable upstream integration.

use std::collections::{HashMap, HashSet};

use anyhow::{Context as _, Result, bail};
use but_core::{ChangeId, RefMetadata, RepositoryExt};
use but_error::bail_precondition;
use but_rebase::graph_rebase::{
    CommitSpec, Editor, EditorIndex,
    anchor::{Cut, Range},
    mutate::Reconnect,
};

use crate::divergence::TargetCommitRelation;

use super::{InteractiveIntegrationStep, display::relation_for};

/// Preset used to generate the initial editable branch integration steps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BranchIntegrationStrategy {
    /// Rebase local commits on top of the upstream commits.
    #[default]
    PullRebase,
    /// Keep local commits first, then merge the upstream tip.
    Merge,
    /// Rebuild the branch by picking upstream commits only.
    PickRemote,
    /// Fold upstream commits with matching explicit Change-Ids into local commits.
    SmartSquash,
}

/// Build the initial editable integration script for the selected strategy.
///
/// `local_commits` and `upstream_commits` are ordered child-to-parent from the
/// graph traversal. The returned steps are ordered parent-to-child for
/// execution by the integration editor.
pub(super) fn initial_integration_steps(
    strategy: BranchIntegrationStrategy,
    local_commits: &[gix::ObjectId],
    upstream_commits: &[gix::ObjectId],
    target_relations: &HashMap<gix::ObjectId, TargetCommitRelation>,
    change_ids: &HashMap<gix::ObjectId, ChangeId>,
) -> Vec<InteractiveIntegrationStep> {
    match strategy {
        BranchIntegrationStrategy::PullRebase => {
            pull_rebase_steps(local_commits, upstream_commits, target_relations)
        }
        BranchIntegrationStrategy::Merge => {
            let mut steps = editable_local_commits(local_commits, target_relations)
                .map(|commit_id| InteractiveIntegrationStep::Pick { commit_id })
                .collect::<Vec<_>>();
            if let Some(upstream_tip) = upstream_commits.first().copied() {
                steps.push(InteractiveIntegrationStep::Merge {
                    commit_id: upstream_tip,
                });
            }
            steps
        }
        BranchIntegrationStrategy::PickRemote => upstream_commits
            .iter()
            .rev()
            .copied()
            .map(|commit_id| InteractiveIntegrationStep::Pick { commit_id })
            .collect::<Vec<_>>(),
        BranchIntegrationStrategy::SmartSquash => smart_squash_steps(
            local_commits,
            upstream_commits,
            target_relations,
            change_ids,
        ),
    }
}

fn pull_rebase_steps(
    local_commits: &[gix::ObjectId],
    upstream_commits: &[gix::ObjectId],
    target_relations: &HashMap<gix::ObjectId, TargetCommitRelation>,
) -> Vec<InteractiveIntegrationStep> {
    upstream_commits
        .iter()
        .rev()
        .copied()
        .map(|commit_id| InteractiveIntegrationStep::Pick { commit_id })
        .chain(
            editable_local_commits(local_commits, target_relations)
                .map(|commit_id| InteractiveIntegrationStep::Pick { commit_id }),
        )
        .collect()
}

pub(super) fn editable_local_commits<'a>(
    local_commits: &'a [gix::ObjectId],
    target_relations: &'a HashMap<gix::ObjectId, TargetCommitRelation>,
) -> impl Iterator<Item = gix::ObjectId> + 'a {
    local_commits
        .iter()
        .rev()
        .copied()
        .filter(|commit_id| !relation_for(target_relations, *commit_id).is_integrated())
}

fn smart_squash_steps(
    local_commits: &[gix::ObjectId],
    upstream_commits: &[gix::ObjectId],
    target_relations: &HashMap<gix::ObjectId, TargetCommitRelation>,
    change_ids: &HashMap<gix::ObjectId, ChangeId>,
) -> Vec<InteractiveIntegrationStep> {
    // Step 1: Find all the local commits associated with a change ID.
    // If multiple local commits are associated with the same change ID,
    // pick the child-most.
    let mut local_targets_by_change_id = HashMap::<ChangeId, gix::ObjectId>::new();
    // Iterate over all the non-integrated commits.
    for commit_id in local_commits
        .iter()
        .copied()
        .filter(|commit_id| !relation_for(target_relations, *commit_id).is_integrated())
    {
        if let Some(change_id) = change_ids.get(&commit_id) {
            local_targets_by_change_id
                .entry(change_id.clone())
                .or_insert(commit_id);
        }
    }

    // If there are no local commits that have change IDs, fallback to returning pull-rebase steps.
    if local_targets_by_change_id.is_empty() {
        return pull_rebase_steps(local_commits, upstream_commits, target_relations);
    }

    // Step 2: Figure out which upstream-commits to squash into which local commits.
    // We already know which local commits are associated to which change IDs.
    // Based on that, we find all upstream commits that are associated with the same
    // change ID and track them.
    let mut upstream_commits_by_target = HashMap::<gix::ObjectId, Vec<gix::ObjectId>>::new();
    let mut matched_upstream_commits = HashSet::<gix::ObjectId>::new();
    // Iterate over the upstream-only commits.
    for upstream_commit_id in upstream_commits.iter().rev().copied() {
        let Some(change_id) = change_ids.get(&upstream_commit_id) else {
            continue;
        };
        let Some(local_target) = local_targets_by_change_id.get(change_id) else {
            continue;
        };
        upstream_commits_by_target
            .entry(*local_target)
            .or_default()
            .push(upstream_commit_id);
        matched_upstream_commits.insert(upstream_commit_id);
    }

    // If there are no upstream commits tha have matched change IDs, fallback to returning the pull-rebase steps.
    if matched_upstream_commits.is_empty() {
        return pull_rebase_steps(local_commits, upstream_commits, target_relations);
    }

    // Step 3: Return the steps in the right order.
    // We pick the unmatched upstream commits first, and then the local non-integrated
    // commits. If they have matching upstream commits, we return squash steps of all the
    // matching upstream commits into the local commit.
    upstream_commits
        .iter()
        .rev()
        .copied()
        .filter(|commit_id| !matched_upstream_commits.contains(commit_id))
        .map(|commit_id| InteractiveIntegrationStep::Pick { commit_id })
        .chain(
            editable_local_commits(local_commits, target_relations).map(|commit_id| {
                if let Some(upstream_commits) = upstream_commits_by_target.get(&commit_id) {
                    let mut commits = Vec::with_capacity(upstream_commits.len() + 1);
                    commits.push(commit_id);
                    commits.extend(upstream_commits.iter().copied());
                    InteractiveIntegrationStep::Squash {
                        commits,
                        message: None,
                    }
                } else {
                    InteractiveIntegrationStep::Pick { commit_id }
                }
            }),
        )
        .collect()
}

#[derive(Debug, Clone)]
pub(super) enum PreparedIntegrationStep {
    Pick { commit_id: gix::ObjectId },
    Merge { commit_id: gix::ObjectId },
}

/// Prepare user-facing integration steps for execution in the graph editor.
///
/// The main role of this function is to pre-compute the squash commits, before
/// we start altering the editor graph. Turning them into ordinary specs for the
/// chain build.
///
/// `editor` provides the current repository and graph state needed to
/// materialize derived steps such as scripted squashes.
///
/// `steps` is the editable integration script in parent-to-child execution
/// order.
///
/// Returns the normalized execution plan used by later graph-building helpers.
pub(super) fn prepare_integration_steps_for_editor<M: RefMetadata>(
    editor: &Editor<'_, M>,
    steps: &[InteractiveIntegrationStep],
) -> Result<Vec<PreparedIntegrationStep>> {
    let mut prepared = Vec::with_capacity(steps.len());
    let mut commit_ids = HashSet::new();

    for step in steps {
        let step = match step {
            InteractiveIntegrationStep::Pick { commit_id } => PreparedIntegrationStep::Pick {
                commit_id: *commit_id,
            },
            InteractiveIntegrationStep::Merge { commit_id } => PreparedIntegrationStep::Merge {
                commit_id: *commit_id,
            },
            InteractiveIntegrationStep::Squash { commits, message } => {
                PreparedIntegrationStep::Pick {
                    commit_id: prepare_squash_step_for_editor(editor, commits, message.as_deref())?,
                }
            }
        };
        let commit_id = match &step {
            PreparedIntegrationStep::Pick { commit_id }
            | PreparedIntegrationStep::Merge { commit_id } => *commit_id,
        };
        if !commit_ids.insert(commit_id) {
            bail_precondition!(
                "Integration plan is invalid: prepared commit {commit_id} appears more than once"
            );
        }
        prepared.push(step);
    }

    Ok(prepared)
}

/// Precompute the squash payload from the current editor/repository state,
/// before later integration graph mutations can rewire commit-graph ancestry.
fn prepare_squash_step_for_editor<M: RefMetadata>(
    editor: &Editor<'_, M>,
    commit_ids: &[gix::ObjectId],
    message: Option<&str>,
) -> Result<gix::ObjectId> {
    if commit_ids.len() < 2 {
        bail!("Squash step must have at least two commits");
    }

    let maybe_entries = commit_ids
        .iter()
        .map(|commit_id| editor.try_select_commit(*commit_id))
        .collect::<Vec<_>>();
    let ordered_commit_ids = if maybe_entries.iter().all(Option::is_some) {
        let ordered_entries = editor.order_by_parentage(
            maybe_entries
                .into_iter()
                .map(|entry| entry.expect("checked all entries are present"))
                .collect::<Vec<_>>(),
        )?;
        ordered_entries
            .iter()
            .map(|entry| editor.commit_of(*entry).map(|commit| commit.id))
            .collect::<Result<Vec<_>>>()?
    } else {
        commit_ids.to_vec()
    };

    let target_commit_id = *ordered_commit_ids
        .first()
        .expect("validated non-empty squash commit list");
    let merge_subject_ids = commit_ids
        .iter()
        .copied()
        .filter(|commit_id| *commit_id != target_commit_id)
        .collect::<Vec<_>>();
    let merge_outcome = editor.merge_commit_changes_to_tree(
        target_commit_id,
        merge_subject_ids,
        editor.repo().merge_options_force_ours()?,
    )?;
    let squashed_parent = editor
        .repo()
        .merge_base_octopus(ordered_commit_ids.iter().copied())
        .context("failed to compute squash merge-base")?
        .detach();

    let tip_commit_id = *ordered_commit_ids
        .last()
        .expect("validated non-empty squash commit list");
    let squashed_commit = editor.find_commit(tip_commit_id)?;
    let commit_message = message
        .map(|message| message.as_bytes().to_vec())
        .unwrap_or_else(|| Vec::from(squashed_commit.message.clone()));
    editor.new_squashed_commit(
        squashed_commit,
        vec![squashed_parent],
        merge_outcome,
        commit_message,
    )
}

/// Builds and inserts the integrated commit chain under `ref_name` down to the last step.
///
/// `editor` is the mutable graph editor that will receive the rebuilt chain.
///
/// `ref_name` is the branch reference whose parent chain should be rebuilt.
///
/// `steps` is the prepared execution plan to insert under `ref_name`, ending
/// at the deepest rebuilt parent step.
///
/// Returns the range spanning from the reference node to the deepest
/// inserted parent.
pub(crate) fn integration_steps_into_segment_nodes<M: RefMetadata>(
    editor: &mut Editor<'_, M>,
    ref_name: &gix::refs::FullNameRef,
    steps: &[PreparedIntegrationStep],
) -> Result<Range> {
    // Step 1: We interpret the integration steps and transform them into graph steps disconnected from their parents.
    // We disconnect them in order to be able to allow for reordering.
    let segment_steps = integration_steps_to_segment_steps_for_editor(editor, ref_name, steps)?;

    // Step 2. We build the new local branch out of the steps.
    // We start by disconnecting all the parents of the local branch reference step, as we will connect it to the new
    // set of commits.
    let child_most: EditorIndex = editor.select_reference(ref_name)?.into();
    unparent_entry(editor, child_most)?;
    let mut parent_most = child_most;

    for step in segment_steps.into_iter().skip(1) {
        if let Some(existing_parent) =
            already_connected_parent_for_step(editor, parent_most, &step)?
        {
            parent_most = existing_parent;
            continue;
        }

        parent_most = connect_parent_step(editor, parent_most, step)?;
    }

    Ok(Range {
        child: child_most,
        parent: parent_most,
    })
}

/// Convert user-provided integration steps into graph steps in insertion order.
///
/// `editor` is the mutable graph editor used to reuse existing commits, create
/// synthetic merge steps, and detach reusable commits from their current parent
/// edges.
///
/// `ref_name` is the branch reference that anchors the rebuilt segment.
///
/// `steps` is the prepared integration plan in execution order.
///
/// Returns the graph steps to insert, starting with a reference step and then
/// the parent chain steps in insertion order.
fn integration_steps_to_segment_steps_for_editor<M: RefMetadata>(
    editor: &mut Editor<'_, M>,
    ref_name: &gix::refs::FullNameRef,
    steps: &[PreparedIntegrationStep],
) -> Result<Vec<ParentNode>> {
    let mut out = vec![ParentNode::Reference(ref_name.to_owned())];

    for step in steps.iter().rev() {
        match step {
            PreparedIntegrationStep::Pick { commit_id, .. } => {
                out.push(ParentNode::CommitSpec(existing_or_new_spec(
                    editor, *commit_id,
                )?));
            }
            PreparedIntegrationStep::Merge { commit_id } => {
                let merge_commit =
                    editor.new_merge_commit(format!("Merge {commit_id} into previous commit"))?;
                let preserved_parents = editor
                    .find_commit(*commit_id)?
                    .inner
                    .parents
                    .iter()
                    .copied()
                    .collect::<Vec<_>>();
                let mut commit_to_merge = CommitSpec::untracked(*commit_id);
                commit_to_merge.preserved_parents = Some(preserved_parents);
                let commit_to_merge = editor.add_commit(commit_to_merge)?;
                let merge_commit = editor.add_commit(CommitSpec::untracked(merge_commit))?;
                // The merged side is the merge's only parent for now; the rebuilt chain's
                // parent prepends at parent number 0 later (`connect_parent_step`), shifting it to
                // the merge-side lane.
                editor.insert_parent(merge_commit, commit_to_merge, 0)?;
                out.push(ParentNode::CommitSpec(editor.spec_of(merge_commit)?));
            }
        }
    }

    Ok(out)
}

/// Produce a pick step for `commit_id`, detaching selected parent edges when needed.
///
/// `editor` is the mutable graph editor used to inspect or detach an existing
/// selectable commit.
///
/// `commit_id` is the commit that should be represented as a pick step in the
/// rebuilt integration segment.
///
/// Returns either the existing pick step for `commit_id` after detaching the
/// selected parent edges, or a brand-new pick step when the commit is not yet
/// selectable in the editor.
fn existing_or_new_spec<M: RefMetadata>(
    editor: &mut Editor<'_, M>,
    commit_id: gix::ObjectId,
) -> Result<CommitSpec> {
    if let Some(existing) = editor.try_select_commit(commit_id) {
        let parents = editor.base_of(existing)?;
        editor.disconnect(
            Range::single(existing),
            Cut::Nothing,
            parents,
            Reconnect::Skip,
        )?;

        // The integration rebuilds this commit onto new parents, so it must be
        // cherry-picked. Reused upstream commits live in immutable segments
        // (they aren't reachable from HEAD), so force them mutable here.
        let mut spec = editor.spec_of(existing)?;
        if !spec.mutable {
            spec.mutable = true;
            editor.replace_commit(existing, spec.clone())?;
        }
        return Ok(spec);
    }

    Ok(CommitSpec::new(commit_id))
}

/// Disconnect all parent edges from a single entry without reconnecting them.
///
/// `editor` is the mutable graph editor whose connectivity will be updated.
///
/// `entry` is the node whose parent edges should be removed.
///
/// Returns `Ok(())` after all direct parent edges of `entry` have been
/// removed from the editor graph.
fn unparent_entry<M: RefMetadata>(editor: &mut Editor<'_, M>, entry: EditorIndex) -> Result<()> {
    editor.disconnect(
        Range::single(entry),
        Cut::Nothing,
        Cut::All,
        Reconnect::Skip,
    )?;

    Ok(())
}

/// Return a direct parent of `child` when `step` refers to a commit that is already connected.
///
/// This is useful when rebuilding an editor segment and we want to reuse an existing
/// commit without adding a duplicate parent entry to the same commit.
///
/// `editor` provides access to the current parent edges and commit entries.
///
/// `child` is the node whose direct parents should be inspected.
///
/// `step` is the candidate step whose commit should be matched against the
/// already-connected parents of `child`.
///
/// Returns the matching direct parent entry when `step` already corresponds
/// to an attached commit parent, or `None` otherwise.
fn already_connected_parent_for_step<M: RefMetadata>(
    editor: &Editor<'_, M>,
    child: EditorIndex,
    node: &ParentNode,
) -> Result<Option<EditorIndex>> {
    let ParentNode::CommitSpec(spec) = node else {
        return Ok(None);
    };

    let Some(existing) = editor.try_select_commit(spec.id) else {
        return Ok(None);
    };

    let direct_parents = editor.direct_parents(child)?;
    Ok(direct_parents
        .into_iter()
        .find_map(|(parent, _)| (parent == existing.into()).then_some(parent)))
}

/// Connect `child` to `parent_step`, reusing an existing commit when possible.
///
/// The new edge is inserted at parent number 0: the rebuilt chain defines `child`'s
/// first-parent lane, and any parents `child` kept (a merge's side parent, parents that
/// survived a partial disconnect) shift after it.
///
/// `editor` is the mutable graph editor that may reuse an existing commit or add a
/// new step before creating the edge.
///
/// `child` is the handle that should gain a new direct parent.
///
/// `parent_step` describes the parent node to connect, either by reusing an
/// existing commit/reference entry or by adding a new commit first.
///
/// Returns the entry of the connected parent node.
fn connect_parent_step<M: RefMetadata>(
    editor: &mut Editor<'_, M>,
    child: EditorIndex,
    parent_node: ParentNode,
) -> Result<EditorIndex> {
    let parent: EditorIndex = match parent_node {
        ParentNode::CommitSpec(spec) => {
            if let Some(existing) = editor.try_select_commit(spec.id) {
                existing.into()
            } else {
                editor.add_commit(spec)?.into()
            }
        }
        ParentNode::Reference(refname) => editor.select_reference(refname.as_ref())?.into(),
    };

    editor.insert_parent(child, parent, 0)?;
    Ok(parent)
}

/// A parent to lay down while rebuilding a chain: a commit spec — reused when the editor
/// already holds it — or a reference already registered in the editor.
enum ParentNode {
    /// A commit spec, added to the graph when not already present.
    CommitSpec(CommitSpec),
    /// An existing reference, found by name.
    Reference(gix::refs::FullName),
}

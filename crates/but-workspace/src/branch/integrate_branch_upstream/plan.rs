//! Feature-local planning helpers for editable upstream integration.

use std::collections::{HashMap, HashSet};

use anyhow::{Context as _, Result, bail};
use bstr::BStr;
use but_core::{
    ChangeId, RefMetadata, RepositoryExt,
    commit::{add_conflict_markers, write_conflicted_tree},
};
use but_rebase::{
    commit::DateMode,
    graph_rebase::{
        Editor, LookupStep, Selector, Step,
        merge_commit_changes::MergeCommitChangesOutcome,
        mutate::{SegmentDelimiter, SelectorSet},
    },
};

use crate::graph_manipulation::{
    already_connected_parent_for_step, connect_parent_step, disconnect_selector_from_all_parents,
};
use crate::{divergence::TargetCommitRelation, graph_manipulation::determine_parent_selector};

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
/// we start altering the editor graph. Turning them into normal pick steps for the
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
    editor: &Editor<'_, '_, M>,
    steps: &[InteractiveIntegrationStep],
) -> Result<Vec<PreparedIntegrationStep>> {
    steps
        .iter()
        .map(|step| match step {
            InteractiveIntegrationStep::Pick { commit_id } => Ok(PreparedIntegrationStep::Pick {
                commit_id: *commit_id,
            }),
            InteractiveIntegrationStep::Merge { commit_id } => Ok(PreparedIntegrationStep::Merge {
                commit_id: *commit_id,
            }),
            InteractiveIntegrationStep::Squash { commits, message } => {
                Ok(PreparedIntegrationStep::Pick {
                    commit_id: prepare_squash_step_for_editor(editor, commits, message.as_deref())?,
                })
            }
        })
        .collect()
}

/// Precompute the squash payload from the current editor/repository state,
/// before later integration graph mutations can rewire step-graph ancestry.
fn prepare_squash_step_for_editor<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    commit_ids: &[gix::ObjectId],
    message: Option<&str>,
) -> Result<gix::ObjectId> {
    if commit_ids.len() < 2 {
        bail!("Squash step must have at least two commits");
    }

    let maybe_selectors = commit_ids
        .iter()
        .map(|commit_id| editor.try_select_commit(*commit_id))
        .collect::<Vec<_>>();
    let ordered_commit_ids = if maybe_selectors.iter().all(Option::is_some) {
        let ordered_selectors = editor.order_commit_selectors_by_parentage(
            maybe_selectors
                .into_iter()
                .map(|selector| selector.expect("checked all selectors are present"))
                .collect::<Vec<_>>(),
        )?;
        ordered_selectors
            .iter()
            .map(|selector| {
                editor
                    .find_selectable_commit(*selector)
                    .map(|(_, commit)| commit.id)
            })
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
    let mut squashed_commit = editor.find_commit(tip_commit_id)?;
    squashed_commit.inner.parents = vec![squashed_parent].into();
    let commit_message = message
        .map(|message| message.as_bytes().to_vec())
        .unwrap_or_else(|| Vec::from(squashed_commit.message.clone()));
    apply_merge_commit_changes_outcome(
        editor.repo(),
        &mut squashed_commit,
        merge_outcome,
        commit_message,
    )?;

    editor.new_commit_untracked(squashed_commit, DateMode::CommitterUpdateAuthorKeep)
}

fn apply_merge_commit_changes_outcome(
    repo: &gix::Repository,
    commit: &mut but_core::CommitOwned,
    outcome: MergeCommitChangesOutcome,
    message: Vec<u8>,
) -> Result<()> {
    if let Some(conflict) = outcome.conflict {
        commit.tree = write_conflicted_tree(
            repo,
            outcome.tree_id,
            conflict.base_tree_ids,
            conflict.side_tree_ids,
            &conflict.conflict_entries,
        )?;
        commit.message = add_conflict_markers(BStr::new(&message));
    } else {
        commit.tree = outcome.tree_id;
        commit.message = message.into();
    }

    Ok(())
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
/// Returns the delimiter spanning from the reference node to the deepest
/// inserted parent.
pub(crate) fn integration_steps_into_segment_nodes<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    ref_name: &gix::refs::FullNameRef,
    steps: &[PreparedIntegrationStep],
) -> Result<SegmentDelimiter<Selector, Selector>> {
    // Step 1: We interpret the integration steps and transform them into graph steps disconnected from their parents.
    // We disconnect them in order to be able to allow for reordering.
    let segment_steps = integration_steps_to_segment_steps_for_editor(editor, ref_name, steps)?;

    // Step 2. We build the new local branch out of the steps.
    // We start by disconnecting all the parents of the local branch reference step, as we will connect it to the new
    // set of commits.
    let child_most = editor.select_reference(ref_name)?;
    disconnect_selector_from_all_parents(editor, child_most)?;
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

    Ok(SegmentDelimiter {
        child: child_most,
        parent: parent_most,
    })
}

/// Convert user-provided integration steps into graph steps in insertion order.
///
/// `editor` is the mutable graph editor used to reuse existing picks, create
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
    editor: &mut Editor<'_, '_, M>,
    ref_name: &gix::refs::FullNameRef,
    steps: &[PreparedIntegrationStep],
) -> Result<Vec<Step>> {
    let mut out = vec![Step::Reference {
        refname: ref_name.to_owned(),
    }];

    for step in steps.iter().rev() {
        match step {
            PreparedIntegrationStep::Pick { commit_id, .. } => {
                out.push(existing_or_new_pick_step(editor, *commit_id)?);
            }
            PreparedIntegrationStep::Merge { commit_id } => {
                let mut merge_commit = editor.empty_commit()?;
                merge_commit.message = format!("Merge {commit_id} into previous commit").into();
                let merge_commit = editor.new_commit_untracked(
                    merge_commit,
                    but_rebase::commit::DateMode::CommitterKeepAuthorKeep,
                )?;
                let preserved_parents = editor
                    .find_commit(*commit_id)?
                    .inner
                    .parents
                    .iter()
                    .copied()
                    .collect::<Vec<_>>();
                let mut commit_to_merge = Step::new_untracked_pick(*commit_id);
                let Step::Pick(pick) = &mut commit_to_merge else {
                    bail!("BUG: expected merge side parent to be a pick step");
                };
                pick.preserved_parents = Some(preserved_parents);
                let commit_to_merge = editor.add_step(commit_to_merge)?;
                let merge_commit = editor.add_step(Step::new_untracked_pick(merge_commit))?;
                editor.add_edge(merge_commit, commit_to_merge, 1)?;
                out.push(editor.lookup_step(merge_commit)?);
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
fn existing_or_new_pick_step<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    commit_id: gix::ObjectId,
) -> Result<Step> {
    if let Some(existing) = editor.try_select_commit(commit_id) {
        let parents_to_disconnect = determine_parent_selector(editor, existing)?;
        editor.disconnect_segment_from(
            SegmentDelimiter {
                child: existing,
                parent: existing,
            },
            SelectorSet::None,
            parents_to_disconnect,
            true,
        )?;

        return editor.lookup_step(existing);
    }

    Ok(Step::new_pick(commit_id))
}

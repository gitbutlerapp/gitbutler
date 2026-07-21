//! Shared helpers for branch/upstream divergence discovery.

use anyhow::{Context as _, Result, bail};
use but_graph::{
    MutableNodeGraph, NodeIndex,
    edit::{Pick, ToSelector},
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

/// Commit ancestry information for a branch and its configured upstream.
#[derive(Debug)]
pub(crate) struct BranchMergeBaseCommits {
    /// Local branch first-parent commits from tip down to, but excluding, the merge base.
    pub(crate) local_commits: Vec<NodeIndex>,
    /// Upstream branch first-parent commits from tip down to, but excluding, the merge base.
    pub(crate) upstream_commits: Vec<NodeIndex>,
    /// Shared merge base between the local branch and its upstream.
    pub(crate) merge_base: NodeIndex,
}

/// How a candidate commit relates to a comparison target branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetCommitRelation {
    /// The commit is not reachable from the target branch.
    NotIntegrated,
    /// The exact commit is reachable from target branch history.
    HistoricallyIntegrated {
        /// The target branch commit that establishes the relation.
        target_commit_id: gix::ObjectId,
    },
}

impl TargetCommitRelation {
    /// Return true when this relation means the commit is already integrated.
    pub(crate) fn is_integrated(self) -> bool {
        matches!(self, Self::HistoricallyIntegrated { .. })
    }
}

/// Return the commit id of the pick at `index`, or an error when the node is
/// not a pick.
pub(crate) fn pick_id(graph: &MutableNodeGraph, index: NodeIndex) -> Result<gix::ObjectId> {
    match graph.pick_at(index) {
        Some(Pick { id, .. }) => Ok(id),
        None => bail!(
            "Expected selector to point to a pick, got {:?}",
            graph.nodes()[index].kind()
        ),
    }
}

/// Compute local and upstream commit lists together with their merge base.
///
/// `ref_name` is the local branch whose first-parent-only divergence should be
/// described.
///
/// `upstream_ref_name` is the effective tracking ref paired with `ref_name`.
///
/// `graph` provides the in-memory graph view used to walk refs, picks, and
/// preserved parentage consistently within the current operation.
///
/// Returns the local-only node indexes, upstream-only node indexes, and the
/// node index for their shared merge base.
pub(crate) fn get_commits_until_merge_base<'a>(
    ref_name: &'a gix::refs::FullNameRef,
    upstream_ref_name: Cow<'a, gix::refs::FullNameRef>,
    graph: &MutableNodeGraph,
) -> Result<BranchMergeBaseCommits> {
    let local_tip = tip_for_ref(graph, ref_name, graph.repo())
        .with_context(|| format!("Could not determine tip commit for '{ref_name}'"))?;
    let upstream_tip =
        tip_for_ref(graph, upstream_ref_name.as_ref(), graph.repo()).with_context(|| {
            format!("Could not determine tip commit for upstream '{upstream_ref_name}'")
        })?;
    let upstream_ancestor_ids = traverse_pick_ancestor_ids(graph, upstream_tip)?;
    let merge_base = find_first_parent_merge_base(graph, local_tip, &upstream_ancestor_ids)?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No merge-base found between '{ref_name}' and its tracking branch '{upstream_ref_name}'"
            )
        })?;
    let merge_base_selector = graph.select_commit(merge_base)?;
    let local_commits = first_parent_path_until(graph, local_tip, |selector| {
        pick_id(graph, *selector).ok() == Some(merge_base)
    })?
    .into_iter()
    .take_while(|selector| *selector != merge_base_selector)
    .collect::<Vec<_>>();
    let upstream_commits = first_parent_path_until(graph, upstream_tip, |selector| {
        pick_id(graph, *selector).ok() == Some(merge_base)
    })?
    .into_iter()
    .take_while(|selector| *selector != merge_base_selector)
    .collect::<Vec<_>>();
    Ok(BranchMergeBaseCommits {
        local_commits,
        upstream_commits,
        merge_base: merge_base_selector,
    })
}

/// Convert node indexes into their current picked commit ids.
///
/// `graph` provides the graph lookup used to resolve each index to its
/// current picked commit id.
///
/// `selectors` is the sequence of graph node indexes to convert.
///
/// Returns the commit ids for all provided indexes in iteration order.
pub(crate) fn commit_ids_from_selectors(
    graph: &MutableNodeGraph,
    selectors: impl IntoIterator<Item = NodeIndex>,
) -> Result<Vec<gix::ObjectId>> {
    selectors
        .into_iter()
        .map(|selector| pick_id(graph, selector))
        .collect()
}

/// Classify candidate node indexes by whether the target branch reaches their commits.
///
/// `graph` provides the graph traversal and pick lookup operations used during
/// classification.
///
/// `target_reachable_commits` contains the commit ids reachable from the target
/// branch.
///
/// `candidate_selectors` are the node indexes to classify against the target
/// branch reachability set.
///
/// Returns a map keyed by candidate commit id describing whether each candidate
/// is historically integrated into the target branch.
pub(crate) fn classify_selectors_against_target_commits(
    graph: &MutableNodeGraph,
    target_reachable_commits: &HashSet<gix::ObjectId>,
    candidate_selectors: &[NodeIndex],
) -> Result<HashMap<gix::ObjectId, TargetCommitRelation>> {
    candidate_selectors
        .iter()
        .copied()
        .map(|candidate_selector| {
            let candidate_commit_id = pick_id(graph, candidate_selector)?;
            let relation = if target_reachable_commits.contains(&candidate_commit_id) {
                TargetCommitRelation::HistoricallyIntegrated {
                    target_commit_id: candidate_commit_id,
                }
            } else {
                TargetCommitRelation::NotIntegrated
            };
            Ok((candidate_commit_id, relation))
        })
        .collect()
}

fn first_pick_parent(graph: &MutableNodeGraph, selector: NodeIndex) -> Result<NodeIndex> {
    let mut adjacent = graph.direct_parents(selector)?;
    adjacent.extend(graph.direct_children(selector)?);
    adjacent.sort_by_key(|(_, order)| *order);
    adjacent
        .into_iter()
        .find_map(|(candidate, _)| graph.pick_at(candidate).is_some().then_some(candidate))
        .ok_or_else(|| anyhow::anyhow!("Expected reference selector to point to a commit"))
}

fn tip_for_ref(
    graph: &MutableNodeGraph,
    ref_name: &gix::refs::FullNameRef,
    repo: &gix::Repository,
) -> Result<NodeIndex> {
    let reference_selector = ref_name.to_selector(graph)?;
    let head_id = repo.head_id()?.detach();
    if let Some(child_on_head_path) =
        child_on_head_first_parent_path(graph, reference_selector, head_id)?
    {
        return Ok(child_on_head_path);
    }
    first_pick_parent(graph, reference_selector).or_else(|_| {
        let tip = repo.find_reference(ref_name)?.id().detach();
        graph.select_commit(tip)
    })
}

fn child_on_head_first_parent_path(
    graph: &MutableNodeGraph,
    reference_selector: NodeIndex,
    head_id: gix::ObjectId,
) -> Result<Option<NodeIndex>> {
    let head_selector = graph.select_commit(head_id)?;
    let mut current = Some(head_selector);
    while let Some(selector) = current {
        let mut parents = graph.direct_parents(selector)?;
        parents.sort_by_key(|(_, order)| *order);
        if parents
            .iter()
            .any(|(parent, _)| *parent == reference_selector)
        {
            return Ok((selector != head_selector).then_some(selector));
        }
        current = first_parent(graph, selector)?;
    }
    Ok(None)
}

fn find_first_parent_merge_base(
    graph: &MutableNodeGraph,
    local_tip: NodeIndex,
    upstream_ancestors: &HashMap<gix::ObjectId, NodeIndex>,
) -> Result<Option<gix::ObjectId>> {
    let mut current = Some(local_tip);
    while let Some(selector) = current {
        let Some(Pick {
            id,
            preserved_parents,
            ..
        }) = graph.pick_at(selector)
        else {
            return Ok(None);
        };
        if upstream_ancestors.contains_key(&id) {
            return Ok(Some(id));
        }
        if let Some(preserved_parents) = preserved_parents {
            for parent_id in preserved_parents {
                if upstream_ancestors.contains_key(&parent_id) {
                    return Ok(Some(parent_id));
                }
            }
        }
        if let Some(parent) = first_parent(graph, selector)? {
            current = Some(parent);
        } else {
            return Ok(None);
        }
    }
    Ok(None)
}

fn traverse_pick_ancestor_ids(
    graph: &MutableNodeGraph,
    tip: NodeIndex,
) -> Result<HashMap<gix::ObjectId, NodeIndex>> {
    let mut out = HashMap::new();
    let mut seen = std::collections::HashSet::from([tip]);
    let mut tips = vec![tip];

    while let Some(tip) = tips.pop() {
        let preserved_parents = match graph.pick_at(tip) {
            Some(Pick {
                id,
                preserved_parents,
                ..
            }) => {
                out.entry(id).or_insert(tip);
                preserved_parents
            }
            None => None,
        };

        for (parent, _) in graph.direct_parents(tip)? {
            if seen.insert(parent) {
                tips.push(parent);
            }
        }

        if let Some(preserved_parents) = preserved_parents {
            for parent_id in preserved_parents {
                out.entry(parent_id).or_insert(tip);
                if let Some(parent) = graph.try_select_commit(parent_id)
                    && seen.insert(parent)
                {
                    tips.push(parent);
                }
            }
        }
    }

    Ok(out)
}

fn first_parent(graph: &MutableNodeGraph, selector: NodeIndex) -> Result<Option<NodeIndex>> {
    let mut parents = graph.direct_parents(selector)?;
    parents.sort_by_key(|(_, order)| *order);
    for (parent, _) in parents {
        if graph.pick_at(parent).is_some() {
            return Ok(Some(parent));
        }
        if let Some(parent) = first_parent(graph, parent)? {
            return Ok(Some(parent));
        }
    }

    let Some(Pick {
        preserved_parents: Some(parents),
        ..
    }) = graph.pick_at(selector)
    else {
        return Ok(None);
    };

    Ok(parents
        .first()
        .copied()
        .and_then(|parent| graph.try_select_commit(parent)))
}

fn first_parent_path_until(
    graph: &MutableNodeGraph,
    tip: NodeIndex,
    mut stop: impl FnMut(&NodeIndex) -> bool,
) -> Result<Vec<NodeIndex>> {
    let mut path = Vec::new();
    let mut current = Some(tip);
    while let Some(selector) = current {
        path.push(selector);
        if stop(&selector) {
            return Ok(path);
        }
        current = first_parent(graph, selector)?;
    }
    Ok(path)
}

//! Shared helpers for branch/upstream divergence discovery.

use anyhow::{Context as _, Result};
use but_core::RefMetadata;
use but_rebase::graph_rebase::{Editor, LookupStep, Pick, Selector, Step, ToSelector};
use std::{borrow::Cow, collections::HashMap};

use crate::graph_manipulation::traverse_nodes;

/// Commit ancestry information for a branch and its configured upstream.
#[derive(Debug)]
pub(crate) struct BranchMergeBaseCommits {
    /// Local branch first-parent commits from tip down to, but excluding, the merge base.
    pub(crate) local_commits: Vec<Selector>,
    /// Upstream branch first-parent commits from tip down to, but excluding, the merge base.
    pub(crate) upstream_commits: Vec<Selector>,
    /// Shared merge base between the local branch and its upstream.
    pub(crate) merge_base: Selector,
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

/// Compute local and upstream commit lists together with their merge base.
///
/// `ref_name` is the local branch whose first-parent-only divergence should be
/// described.
///
/// `upstream_ref_name` is the effective tracking ref paired with `ref_name`.
///
/// `editor` provides the in-memory graph view used to walk refs, picks, and
/// preserved parentage consistently within the current operation.
///
/// Returns the local-only selectors, upstream-only selectors, and the selector
/// for their shared merge base.
pub(crate) fn get_commits_until_merge_base<'a, M: RefMetadata>(
    ref_name: &'a gix::refs::FullNameRef,
    upstream_ref_name: Cow<'a, gix::refs::FullNameRef>,
    editor: &Editor<'_, '_, M>,
) -> Result<BranchMergeBaseCommits> {
    let local_tip = tip_for_ref(editor, ref_name, editor.repo())
        .with_context(|| format!("Could not determine tip commit for '{ref_name}'"))?;
    let upstream_tip = tip_for_ref(editor, upstream_ref_name.as_ref(), editor.repo())
        .with_context(|| {
            format!("Could not determine tip commit for upstream '{upstream_ref_name}'")
        })?;
    let upstream_ancestor_ids = traverse_pick_ancestor_ids(editor, upstream_tip)?;
    let merge_base = find_first_parent_merge_base(editor, local_tip, &upstream_ancestor_ids)?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No merge-base found between '{ref_name}' and its tracking branch '{upstream_ref_name}'"
            )
        })?;
    let merge_base_selector = editor.select_commit(merge_base)?;
    let local_commits = first_parent_path_until(editor, local_tip, |selector| {
        editor.lookup_pick(*selector).ok() == Some(merge_base)
    })?
    .into_iter()
    .take_while(|selector| *selector != merge_base_selector)
    .collect::<Vec<_>>();
    let upstream_commits = first_parent_path_until(editor, upstream_tip, |selector| {
        editor.lookup_pick(*selector).ok() == Some(merge_base)
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

/// Convert selectors into their current picked commit ids.
///
/// `editor` provides the graph lookup used to resolve each selector to its
/// current picked commit id.
///
/// `selectors` is the sequence of graph selectors to convert.
///
/// Returns the commit ids for all provided selectors in iteration order.
pub(crate) fn commit_ids_from_selectors<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    selectors: impl IntoIterator<Item = Selector>,
) -> Result<Vec<gix::ObjectId>> {
    selectors
        .into_iter()
        .map(|selector| editor.lookup_pick(selector))
        .collect()
}

/// Classify candidate selectors by whether the target branch reaches them.
///
/// `editor` provides the graph traversal and pick lookup operations used during
/// classification.
///
/// `target_ref_selector` is the selector whose reachable history defines what
/// counts as already integrated.
///
/// `candidate_selectors` are the selectors to classify against the target
/// branch reachability set.
///
/// Returns a map keyed by candidate commit id describing whether each candidate
/// is historically integrated into the target branch.
pub(crate) fn classify_selectors_against_target_ref<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    target_ref_selector: Selector,
    candidate_selectors: &[Selector],
) -> Result<HashMap<gix::ObjectId, TargetCommitRelation>> {
    let target_reachable_selectors = traverse_nodes(editor, target_ref_selector)?;
    candidate_selectors
        .iter()
        .copied()
        .map(|candidate_selector| {
            let candidate_commit_id = editor.lookup_pick(candidate_selector)?;
            let relation = if target_reachable_selectors.contains(&candidate_selector) {
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

fn first_pick_parent<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    selector: Selector,
) -> Result<Selector> {
    let mut adjacent = editor.direct_parents(selector)?;
    adjacent.extend(editor.direct_children(selector)?);
    adjacent.sort_by_key(|(_, order)| *order);
    adjacent
        .into_iter()
        .find_map(|(candidate, _)| {
            matches!(editor.lookup_step(candidate).ok()?, Step::Pick(_)).then_some(candidate)
        })
        .ok_or_else(|| anyhow::anyhow!("Expected reference selector to point to a commit"))
}

fn tip_for_ref<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    ref_name: &gix::refs::FullNameRef,
    repo: &gix::Repository,
) -> Result<Selector> {
    let reference_selector = ref_name.to_selector(editor)?;
    let head_id = repo.head_id()?.detach();
    if let Some(child_on_head_path) =
        child_on_head_first_parent_path(editor, reference_selector, head_id)?
    {
        return Ok(child_on_head_path);
    }
    first_pick_parent(editor, reference_selector).or_else(|_| {
        let tip = repo.find_reference(ref_name)?.id().detach();
        editor.select_commit(tip)
    })
}

fn child_on_head_first_parent_path<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    reference_selector: Selector,
    head_id: gix::ObjectId,
) -> Result<Option<Selector>> {
    let head_selector = editor.select_commit(head_id)?;
    let mut current = Some(head_selector);
    while let Some(selector) = current {
        let mut parents = editor.direct_parents(selector)?;
        parents.sort_by_key(|(_, order)| *order);
        if parents
            .iter()
            .any(|(parent, _)| *parent == reference_selector)
        {
            return Ok((selector != head_selector).then_some(selector));
        }
        current = first_parent(editor, selector)?;
    }
    Ok(None)
}

fn find_first_parent_merge_base<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    local_tip: Selector,
    upstream_ancestors: &HashMap<gix::ObjectId, Selector>,
) -> Result<Option<gix::ObjectId>> {
    let mut current = Some(local_tip);
    while let Some(selector) = current {
        let Step::Pick(Pick {
            id,
            preserved_parents,
            ..
        }) = editor.lookup_step(selector)?
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
        if let Some(parent) = first_parent(editor, selector)? {
            current = Some(parent);
        } else {
            return Ok(None);
        }
    }
    Ok(None)
}

fn traverse_pick_ancestor_ids<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    tip: Selector,
) -> Result<HashMap<gix::ObjectId, Selector>> {
    let mut out = HashMap::new();
    let mut seen = std::collections::HashSet::from([tip]);
    let mut tips = vec![tip];

    while let Some(tip) = tips.pop() {
        let preserved_parents = match editor.lookup_step(tip)? {
            Step::Pick(Pick {
                id,
                preserved_parents,
                ..
            }) => {
                out.entry(id).or_insert(tip);
                preserved_parents
            }
            Step::Reference { .. } | Step::None => None,
        };

        for (parent, _) in editor.direct_parents(tip)? {
            if seen.insert(parent) {
                tips.push(parent);
            }
        }

        if let Some(preserved_parents) = preserved_parents {
            for parent_id in preserved_parents {
                out.entry(parent_id).or_insert(tip);
                if let Some(parent) = editor.try_select_commit(parent_id)
                    && seen.insert(parent)
                {
                    tips.push(parent);
                }
            }
        }
    }

    Ok(out)
}

fn first_parent<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    selector: Selector,
) -> Result<Option<Selector>> {
    let mut parents = editor.direct_parents(selector)?;
    parents.sort_by_key(|(_, order)| *order);
    for (parent, _) in parents {
        match editor.lookup_step(parent)? {
            Step::Pick(_) => return Ok(Some(parent)),
            Step::Reference { .. } | Step::None => {
                if let Some(parent) = first_parent(editor, parent)? {
                    return Ok(Some(parent));
                }
            }
        }
    }

    let Step::Pick(Pick {
        preserved_parents: Some(parents),
        ..
    }) = editor.lookup_step(selector)?
    else {
        return Ok(None);
    };

    Ok(parents
        .first()
        .copied()
        .and_then(|parent| editor.try_select_commit(parent)))
}

fn first_parent_path_until<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    tip: Selector,
    mut stop: impl FnMut(&Selector) -> bool,
) -> Result<Vec<Selector>> {
    let mut path = Vec::new();
    let mut current = Some(tip);
    while let Some(selector) = current {
        path.push(selector);
        if stop(&selector) {
            return Ok(path);
        }
        current = first_parent(editor, selector)?;
    }
    Ok(path)
}

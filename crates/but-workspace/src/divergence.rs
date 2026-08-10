//! Shared helpers for branch/upstream divergence discovery.

use anyhow::{Context as _, Result};
use but_core::RefMetadata;
use but_rebase::graph_rebase::{CommitIndex, Editor, EditorIndex};
use std::{borrow::Cow, collections::HashMap};

/// Commit ancestry information for a branch and its configured upstream.
#[derive(Debug)]
pub(crate) struct BranchMergeBaseCommits {
    /// Local branch first-parent commits from tip down to, but excluding, the merge base.
    pub(crate) local_commits: Vec<CommitIndex>,
    /// Upstream branch first-parent commits from tip down to, but excluding, the merge base.
    pub(crate) upstream_commits: Vec<CommitIndex>,
    /// Shared merge base between the local branch and its upstream.
    pub(crate) merge_base: CommitIndex,
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
/// `editor` provides the in-memory graph view used to walk refs, commits, and
/// preserved parentage consistently within the current operation.
///
/// Returns the local-only entries, upstream-only entries, and the entry
/// for their shared merge base.
pub(crate) fn get_commits_until_merge_base<'a, M: RefMetadata>(
    ref_name: &'a gix::refs::FullNameRef,
    upstream_ref_name: Cow<'a, gix::refs::FullNameRef>,
    editor: &Editor<'_, M>,
) -> Result<BranchMergeBaseCommits> {
    let local_tip = tip_for_ref(editor, ref_name, editor.repo())
        .with_context(|| format!("Could not determine tip commit for '{ref_name}'"))?;
    let upstream_tip = tip_for_ref(editor, upstream_ref_name.as_ref(), editor.repo())
        .with_context(|| {
            format!("Could not determine tip commit for upstream '{upstream_ref_name}'")
        })?;
    let upstream_ancestor_ids = traverse_ancestor_ids(editor, upstream_tip)?;
    let merge_base = find_first_parent_merge_base(editor, local_tip, &upstream_ancestor_ids)?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No merge-base found between '{ref_name}' and its tracking branch '{upstream_ref_name}'"
            )
        })?;
    let merge_base_entry = editor.select_commit(merge_base)?;
    let local_commits = first_parent_path_until(editor, local_tip, |entry| {
        editor.id_of(*entry).ok() == Some(merge_base)
    })?
    .into_iter()
    .take_while(|entry| *entry != merge_base_entry)
    .collect::<Vec<_>>();
    let upstream_commits = first_parent_path_until(editor, upstream_tip, |entry| {
        editor.id_of(*entry).ok() == Some(merge_base)
    })?
    .into_iter()
    .take_while(|entry| *entry != merge_base_entry)
    .collect::<Vec<_>>();
    Ok(BranchMergeBaseCommits {
        local_commits,
        upstream_commits,
        merge_base: merge_base_entry,
    })
}

/// Convert indices into their current commit ids.
///
/// `editor` provides the graph lookup used to resolve each entry to its
/// current commit id.
///
/// `entries` is the sequence of graph entries to convert.
///
/// Returns the commit ids for all provided entries in iteration order.
pub(crate) fn commit_ids_from_indices<M: RefMetadata>(
    editor: &Editor<'_, M>,
    entries: impl IntoIterator<Item = CommitIndex>,
) -> Result<Vec<gix::ObjectId>> {
    entries
        .into_iter()
        .map(|entry| editor.id_of(entry))
        .collect()
}

/// Find `commit_id` on the effective local first-parent path above `merge_base`.
///
/// The effective tip includes workspace commits above the local branch ref, while
/// bounding the walk at the plan's merge base keeps commits from other stacks out.
pub(crate) fn find_local_commit_until_merge_base<M: RefMetadata>(
    ref_name: &gix::refs::FullNameRef,
    commit_id: gix::ObjectId,
    merge_base: gix::ObjectId,
    editor: &Editor<'_, M>,
) -> Result<Option<CommitIndex>> {
    let local_tip = tip_for_ref(editor, ref_name, editor.repo())?;
    let mut path = first_parent_path_until(editor, local_tip, |entry| {
        editor.id_of(*entry).ok() == Some(merge_base)
    })?;
    let Some(path_end) = path.pop() else {
        return Ok(None);
    };
    if editor.id_of(path_end).ok() != Some(merge_base) {
        return Ok(None);
    }
    Ok(path
        .into_iter()
        .find(|entry| editor.id_of(*entry).ok() == Some(commit_id)))
}

/// Classify candidate entries by whether the target branch reaches them.
///
/// `editor` provides the graph traversal and spec lookup operations used during
/// classification.
///
/// `target_ref_entry` is the entry whose reachable history defines what
/// counts as already integrated.
///
/// `candidate_entries` are the entries to classify against the target
/// branch reachability set.
///
/// Returns a map keyed by candidate commit id describing whether each candidate
/// is historically integrated into the target branch.
pub(crate) fn classify_against_target_ref<M: RefMetadata>(
    editor: &Editor<'_, M>,
    target_ref_entry: EditorIndex,
    candidate_entries: &[CommitIndex],
) -> Result<HashMap<gix::ObjectId, TargetCommitRelation>> {
    let target_reachable_entries = editor.position_reachable(target_ref_entry)?;
    candidate_entries
        .iter()
        .copied()
        .map(|candidate_entry| {
            let candidate_commit_id = editor.id_of(candidate_entry)?;
            let relation = if target_reachable_entries.contains(&candidate_entry.into()) {
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

fn first_adjacent_commit<M: RefMetadata>(
    editor: &Editor<'_, M>,
    entry: EditorIndex,
) -> Result<CommitIndex> {
    let mut adjacent = editor.direct_parents(entry)?;
    adjacent.extend(editor.direct_children(entry)?);
    adjacent.sort_by_key(|(_, order)| *order);
    adjacent
        .into_iter()
        .find_map(|(candidate, _)| match candidate {
            EditorIndex::Commit(commit) if !editor.is_removed(commit) => Some(commit),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("Expected reference entry to point to a commit"))
}

fn tip_for_ref<M: RefMetadata>(
    editor: &Editor<'_, M>,
    ref_name: &gix::refs::FullNameRef,
    repo: &gix::Repository,
) -> Result<CommitIndex> {
    let reference_entry = editor.resolve_anchor(ref_name)?;
    let head_id = repo.head_id()?.detach();
    if let Some(child_on_head_path) =
        child_on_head_first_parent_path(editor, reference_entry, head_id)?
    {
        return Ok(child_on_head_path);
    }
    first_adjacent_commit(editor, reference_entry).or_else(|_| {
        let tip = repo.find_reference(ref_name)?.id().detach();
        editor.select_commit(tip)
    })
}

fn child_on_head_first_parent_path<M: RefMetadata>(
    editor: &Editor<'_, M>,
    reference_entry: EditorIndex,
    head_id: gix::ObjectId,
) -> Result<Option<CommitIndex>> {
    let head_entry = editor.select_commit(head_id)?;
    let mut current = Some(head_entry);
    while let Some(entry) = current {
        if editor.position_parents(entry)?.contains(&reference_entry) {
            return Ok((entry != head_entry).then_some(entry));
        }
        current = first_parent(editor, entry)?;
    }
    Ok(None)
}

fn find_first_parent_merge_base<M: RefMetadata>(
    editor: &Editor<'_, M>,
    local_tip: CommitIndex,
    upstream_ancestors: &HashMap<gix::ObjectId, EditorIndex>,
) -> Result<Option<gix::ObjectId>> {
    let mut current = Some(local_tip);
    while let Some(entry) = current {
        let Ok(spec) = editor.spec_of(entry) else {
            return Ok(None);
        };
        if upstream_ancestors.contains_key(&spec.id) {
            return Ok(Some(spec.id));
        }
        if let Some(preserved_parents) = spec.preserved_parents {
            for parent_id in preserved_parents {
                if upstream_ancestors.contains_key(&parent_id) {
                    return Ok(Some(parent_id));
                }
            }
        }
        if let Some(parent) = first_parent(editor, entry)? {
            current = Some(parent);
        } else {
            return Ok(None);
        }
    }
    Ok(None)
}

fn traverse_ancestor_ids<M: RefMetadata>(
    editor: &Editor<'_, M>,
    tip: CommitIndex,
) -> Result<HashMap<gix::ObjectId, EditorIndex>> {
    let mut out = HashMap::new();
    let mut seen = std::collections::HashSet::from([tip.into()]);
    let mut tips: Vec<EditorIndex> = vec![tip.into()];

    while let Some(tip) = tips.pop() {
        let preserved_parents = match tip {
            EditorIndex::Commit(commit) if !editor.is_removed(commit) => {
                let spec = editor.spec_of(commit)?;
                out.entry(spec.id).or_insert(tip);
                spec.preserved_parents
            }
            _ => None,
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
                    && seen.insert(parent.into())
                {
                    tips.push(parent.into());
                }
            }
        }
    }

    Ok(out)
}

fn first_parent<M: RefMetadata>(
    editor: &Editor<'_, M>,
    entry: CommitIndex,
) -> Result<Option<CommitIndex>> {
    let mut parents = editor.direct_parents(entry)?;
    parents.sort_by_key(|(_, order)| *order);
    for (parent, _) in parents {
        match parent {
            EditorIndex::Commit(commit) if !editor.is_removed(commit) => {
                return Ok(Some(commit));
            }
            _ => {
                if let Some(parent) = first_parent_through(editor, parent)? {
                    return Ok(Some(parent));
                }
            }
        }
    }

    let Ok(pick) = editor.spec_of(entry) else {
        return Ok(None);
    };
    let Some(parents) = pick.preserved_parents else {
        return Ok(None);
    };

    Ok(parents
        .first()
        .copied()
        .and_then(|parent| editor.try_select_commit(parent)))
}

/// Continue the first-parent descent through a non-commit entry (a reference or a removed
/// entry sitting on the path) down to the next commit.
fn first_parent_through<M: RefMetadata>(
    editor: &Editor<'_, M>,
    waypoint: EditorIndex,
) -> Result<Option<CommitIndex>> {
    let mut parents = editor.direct_parents(waypoint)?;
    parents.sort_by_key(|(_, order)| *order);
    for (parent, _) in parents {
        match parent {
            EditorIndex::Commit(commit) if !editor.is_removed(commit) => {
                return Ok(Some(commit));
            }
            _ => {
                if let Some(parent) = first_parent_through(editor, parent)? {
                    return Ok(Some(parent));
                }
            }
        }
    }
    Ok(None)
}

fn first_parent_path_until<M: RefMetadata>(
    editor: &Editor<'_, M>,
    tip: CommitIndex,
    mut stop: impl FnMut(&CommitIndex) -> bool,
) -> Result<Vec<CommitIndex>> {
    let mut path = Vec::new();
    let mut current = Some(tip);
    while let Some(entry) = current {
        path.push(entry);
        if stop(&entry) {
            return Ok(path);
        }
        current = first_parent(editor, entry)?;
    }
    Ok(path)
}

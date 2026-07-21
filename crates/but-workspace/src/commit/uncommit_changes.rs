//! Actions to remove changes from commits.

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use anyhow::{Result, bail};
use bstr::BString;
use but_core::{DiffSpec, commit::write::DateMode};
use but_graph::{
    MutableNodeGraph, NodeIndex, Rebased,
    edit::{Pick, ToCommitSelector},
};

use crate::tree_manipulation::{ChangesSource, create_tree_without_diff};

/// The result of an uncommit_changes operation.
#[derive(Debug)]
pub struct UncommitChangesOutcome {
    /// The successful rebase result
    pub rebase: Rebased,
    /// Node index pointing to the modified commit (with changes removed)
    pub commit_selector: NodeIndex,
}

/// A source entry for uncommitting changes from a commit.
///
/// Multiple entries may target the same commit; they are grouped by commit id
/// before the changes are removed.
#[derive(Debug, Clone)]
pub struct UncommitChangesSource {
    /// The commit to remove `changes` from.
    pub commit_id: gix::ObjectId,
    /// The changes to remove from the commit.
    pub changes: Vec<DiffSpec>,
}

/// A grouped source that could not be uncommitted.
#[derive(Debug, Clone)]
pub struct UncommitChangesFailure {
    /// The commit whose changes failed to uncommit.
    pub commit_id: gix::ObjectId,
    /// All changes requested for this commit.
    pub changes: Vec<DiffSpec>,
    /// Human-readable failure reason.
    pub error: String,
}

/// The result of uncommitting changes from multiple commits.
#[derive(Debug)]
pub struct UncommitChangesFromCommitsOutcome {
    /// The successful rebase result, present when at least one source was uncommitted.
    pub rebase: Option<Rebased>,
    /// Sources that could not be uncommitted.
    pub failures: Vec<UncommitChangesFailure>,
}

#[derive(Debug)]
struct GroupedUncommitChanges {
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
}

/// Removes the specified changes from a commit.
///
/// The changes are removed from the commit's tree, effectively "uncommitting"
/// them so they appear in the working directory as uncommitted changes.
pub fn uncommit_changes(
    graph: MutableNodeGraph,
    commit: impl ToCommitSelector,
    changes: impl IntoIterator<Item = DiffSpec>,
    context_lines: u32,
) -> Result<UncommitChangesOutcome> {
    let (graph, commit_selector) =
        uncommit_changes_no_rebase(graph, commit, changes, context_lines)
            .map_err(|err| err.error)?;

    let rebase = graph.rebase()?;

    Ok(UncommitChangesOutcome {
        rebase,
        commit_selector,
    })
}

/// Removes changes from multiple commits, grouped by commit id and applied in
/// child-to-parent order.
///
/// Invalid or inapplicable grouped sources are collected in `failures`. When at
/// least one source succeeds, all successful replacements are rebased once at
/// the end. When no source succeeds, `rebase` is `None`.
pub fn uncommit_changes_from_commits(
    mut graph: MutableNodeGraph,
    sources: impl IntoIterator<Item = UncommitChangesSource>,
    context_lines: u32,
) -> Result<UncommitChangesFromCommitsOutcome> {
    let groups = group_sources_by_commit(sources);
    if groups.is_empty() {
        bail!("No changes were provided to uncommit")
    }

    // Index groups by commit id so the apply loop below is O(n) rather than
    // re-scanning `groups` for every ordered commit. Each commit id is unique
    // after grouping, so this map is 1:1.
    let group_by_commit: HashMap<gix::ObjectId, usize> = groups
        .iter()
        .enumerate()
        .map(|(index, group)| (group.commit_id, index))
        .collect();

    let mut failures = Vec::new();
    let mut valid_commit_ids = Vec::new();
    for group in &groups {
        match graph.find_selectable_commit(group.commit_id) {
            Ok((_selector, commit)) => {
                if commit.clone().attach(graph.repo()).is_conflicted() {
                    failures.push(failure(
                        group,
                        "Cannot uncommit changes from a conflicted commit",
                    ));
                } else {
                    valid_commit_ids.push(group.commit_id);
                }
            }
            Err(err) => failures.push(failure(group, err.to_string())),
        }
    }

    let ordered_selectors = graph.order_commit_selectors_by_parentage(valid_commit_ids)?;
    let mut ordered_ids = ordered_selectors
        .iter()
        .map(|selector| {
            // `pick_at` also covers convergence boundaries, which are
            // selectable for selectors resolved from commit ids.
            graph.pick_at(*selector).map(|pick| pick.id).ok_or_else(|| {
                anyhow::anyhow!(
                    "Expected selector to point to a pick, got {:?}",
                    graph.nodes()[*selector].kind()
                )
            })
        })
        .collect::<Result<Vec<_>>>()?;
    ordered_ids.reverse();

    let mut success_count = 0usize;
    for commit_id in ordered_ids {
        let Some(group) = group_by_commit
            .get(&commit_id)
            .and_then(|&index| groups.get(index))
        else {
            continue;
        };

        match uncommit_changes_no_rebase(graph, commit_id, group.changes.clone(), context_lines) {
            Ok((updated_graph, _selector)) => {
                graph = updated_graph;
                success_count += 1;
            }
            Err(err) => {
                failures.push(failure(group, err.to_string()));
                graph = err.into_graph;
            }
        }
    }

    let rebase = if success_count == 0 {
        None
    } else {
        Some(graph.rebase()?)
    };

    Ok(UncommitChangesFromCommitsOutcome { rebase, failures })
}

struct UncommitChangesNoRebaseError {
    into_graph: MutableNodeGraph,
    error: anyhow::Error,
}

impl std::fmt::Display for UncommitChangesNoRebaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

impl std::fmt::Debug for UncommitChangesNoRebaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

fn uncommit_changes_no_rebase(
    mut graph: MutableNodeGraph,
    commit: impl ToCommitSelector,
    changes: impl IntoIterator<Item = DiffSpec>,
    context_lines: u32,
) -> std::result::Result<(MutableNodeGraph, NodeIndex), UncommitChangesNoRebaseError> {
    match uncommit_changes_no_rebase_inner(&mut graph, commit, changes, context_lines) {
        Ok(selector) => Ok((graph, selector)),
        Err(error) => Err(UncommitChangesNoRebaseError {
            into_graph: graph,
            error,
        }),
    }
}

fn uncommit_changes_no_rebase_inner(
    graph: &mut MutableNodeGraph,
    commit: impl ToCommitSelector,
    changes: impl IntoIterator<Item = DiffSpec>,
    context_lines: u32,
) -> Result<NodeIndex> {
    let (commit_selector, commit) = graph.find_selectable_commit(commit)?;

    if commit.clone().attach(graph.repo()).is_conflicted() {
        bail!("Cannot uncommit changes from a conflicted commit")
    }

    let (tree_without_changes, dropped_diffs) = create_tree_without_diff(
        graph.repo(),
        ChangesSource::Commit { id: commit.id },
        changes,
        context_lines,
    )?;

    if !dropped_diffs.is_empty() {
        bail!("Failed to remove specified changes from commit");
    }

    let new_commit_id = {
        let mut new_commit = commit.clone();
        new_commit.tree = tree_without_changes;
        graph.new_commit(new_commit, DateMode::CommitterUpdateAuthorKeep)?
    };

    graph.replace_commit(commit_selector, Pick::new_pick(new_commit_id))?;
    Ok(commit_selector)
}

fn group_sources_by_commit(
    sources: impl IntoIterator<Item = UncommitChangesSource>,
) -> Vec<GroupedUncommitChanges> {
    let mut groups = Vec::<GroupedUncommitChanges>::new();
    for source in sources {
        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.commit_id == source.commit_id)
        {
            group.changes.extend(source.changes);
        } else {
            groups.push(GroupedUncommitChanges {
                commit_id: source.commit_id,
                changes: source.changes,
            });
        }
    }
    // Several sources can target the same file within a commit (for example, one
    // `DiffSpec` per selected hunk). `create_tree_without_diff` rebuilds each
    // path from the original commit tree and overwrites it, so two specs for the
    // same path would make the last one win and silently drop the others. Merge
    // them into a single spec per path up front.
    for group in &mut groups {
        group.changes = merge_specs_by_path(std::mem::take(&mut group.changes));
    }
    groups
}

/// Collapse `DiffSpec`s that touch the same file into one, preserving first-seen
/// order.
///
/// Specs are identified by their `(previous_path, path)` pair, matching how the
/// tree manipulation looks them up. Their hunk headers are unioned (deduped).
/// An empty `hunk_headers` list means "the whole file"; when it appears for a
/// path it supersedes any hunk subset, since removing the whole file already
/// covers every hunk.
fn merge_specs_by_path(specs: Vec<DiffSpec>) -> Vec<DiffSpec> {
    let mut order = Vec::<(Option<BString>, BString)>::new();
    let mut by_path = HashMap::<(Option<BString>, BString), DiffSpec>::new();
    for spec in specs {
        let key = (spec.previous_path.clone(), spec.path.clone());
        match by_path.entry(key.clone()) {
            Entry::Vacant(entry) => {
                order.push(key);
                entry.insert(spec);
            }
            Entry::Occupied(mut entry) => {
                let existing = entry.get_mut();
                if existing.hunk_headers.is_empty() || spec.hunk_headers.is_empty() {
                    // Whole-file removal supersedes any hunk selection.
                    existing.hunk_headers.clear();
                } else {
                    for header in spec.hunk_headers {
                        if !existing.hunk_headers.contains(&header) {
                            existing.hunk_headers.push(header);
                        }
                    }
                }
            }
        }
    }
    order
        .into_iter()
        .map(|key| by_path.remove(&key).expect("key was just inserted"))
        .collect()
}

fn failure(group: &GroupedUncommitChanges, error: impl Into<String>) -> UncommitChangesFailure {
    UncommitChangesFailure {
        commit_id: group.commit_id,
        changes: group.changes.clone(),
        error: error.into(),
    }
}

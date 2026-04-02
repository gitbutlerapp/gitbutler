use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap, HashSet},
};

use anyhow::{Context as _, Result, bail};
use but_core::RefMetadata;
use but_graph::{SegmentIndex, SegmentRelation};
use but_rebase::graph_rebase::{Editor, Selector, ToCommitSelector};

use crate::workspace_graph::find_commit_segment_index;

#[derive(Debug, Clone, Copy)]
struct SelectedCommit {
    selector: Selector,
    id: gix::ObjectId,
    segment_id: SegmentIndex,
    input_order: usize,
}

/// Order commit selectors by parentage, with parents first and children last.
///
/// If two commits are unrelated by ancestry, their relative order is determined by
/// workspace traversal order. Duplicate selectors are deduplicated by commit-id
/// with first occurrence winning.
///
/// Returns an error if any selected commit isn't present in the editor workspace
/// traversal.
pub fn order_commit_selectors_by_parentage<'ws, 'meta, M: RefMetadata, I, S>(
    editor: &Editor<'ws, 'meta, M>,
    selectors: I,
) -> Result<Vec<Selector>>
where
    I: IntoIterator<Item = S>,
    S: ToCommitSelector,
{
    // Normalize user input to unique commits while retaining first-seen order for tie-breaking.
    let mut selected = Vec::<SelectedCommit>::new();
    let mut seen_ids = HashSet::<gix::ObjectId>::new();
    for (input_order, selector_like) in selectors.into_iter().enumerate() {
        let (selector, commit) = editor.find_selectable_commit(selector_like)?;
        if seen_ids.insert(commit.id) {
            let segment_id =
                find_commit_segment_index(editor.workspace, commit.id).with_context(|| {
                    format!(
                        "Selected commit {id} is not part of the workspace traversal",
                        id = commit.id
                    )
                })?;
            selected.push(SelectedCommit {
                selector,
                id: commit.id,
                segment_id,
                input_order,
            });
        }
    }

    if selected.len() <= 1 {
        return Ok(selected.into_iter().map(|s| s.selector).collect());
    }

    // Build a deterministic fallback rank from workspace traversal order for unrelated commits.
    let workspace_rank = workspace_parent_to_child_rank(editor, &selected)?;

    // Build a DAG over selected commits where edges always point ancestor -> descendant.
    let mut adjacency = vec![Vec::<usize>::new(); selected.len()];
    let mut indegree = vec![0usize; selected.len()];

    for (i, left_commit) in selected.iter().enumerate() {
        for (offset, right_commit) in selected.iter().skip(i + 1).enumerate() {
            let j = i + 1 + offset;
            match ancestry_relation(editor, left_commit, right_commit)? {
                Relation::LeftIsAncestorOfRight => {
                    adjacency
                        .get_mut(i)
                        .context("BUG: adjacency index should always be valid")?
                        .push(j);
                    *indegree
                        .get_mut(j)
                        .context("BUG: indegree index should always be valid")? += 1;
                }
                Relation::RightIsAncestorOfLeft => {
                    adjacency
                        .get_mut(j)
                        .context("BUG: adjacency index should always be valid")?
                        .push(i);
                    *indegree
                        .get_mut(i)
                        .context("BUG: indegree index should always be valid")? += 1;
                }
                Relation::Unrelated => {}
            }
        }
    }

    // Kahn topological sort with a min-priority queue so output order is stable across unrelated nodes.
    let mut output = Vec::with_capacity(selected.len());
    let mut ready: BinaryHeap<Reverse<(usize, usize, usize)>> = indegree
        .iter()
        .enumerate()
        .filter_map(|(idx, degree)| {
            if *degree != 0 {
                return None;
            }
            let commit = selected.get(idx)?;
            let rank = *workspace_rank.get(&commit.id)?;
            Some(Reverse((rank, commit.input_order, idx)))
        })
        .collect();

    // Repeatedly emit the best available node and unlock its descendants.
    while let Some(Reverse((_, _, next))) = ready.pop() {
        output.push(
            selected
                .get(next)
                .context("BUG: ready index should be in-bounds")?
                .selector,
        );
        for &child in adjacency
            .get(next)
            .context("BUG: adjacency index should be in-bounds")?
        {
            let degree = indegree
                .get_mut(child)
                .context("BUG: child index should be in-bounds")?;
            *degree -= 1;
            if *degree == 0 {
                let commit = selected
                    .get(child)
                    .context("BUG: child index should point to selected commits")?;
                let rank = *workspace_rank
                    .get(&commit.id)
                    .context("BUG: selected child commit should be ranked")?;
                ready.push(Reverse((rank, commit.input_order, child)));
            }
        }
    }

    // Any leftovers indicate impossible/cyclic constraints in what should be a DAG.
    if output.len() != selected.len() {
        bail!("Cannot order selected commits by parentage due to cyclic ancestry constraints")
    }

    Ok(output)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Relation {
    LeftIsAncestorOfRight,
    RightIsAncestorOfLeft,
    Unrelated,
}

fn ancestry_relation(
    editor: &Editor<'_, '_, impl RefMetadata>,
    left: &SelectedCommit,
    right: &SelectedCommit,
) -> Result<Relation> {
    match editor
        .workspace
        .graph
        .relation_between(left.segment_id, right.segment_id)
    {
        SegmentRelation::Ancestor => return Ok(Relation::LeftIsAncestorOfRight),
        SegmentRelation::Descendant => return Ok(Relation::RightIsAncestorOfLeft),
        SegmentRelation::Disjoint | SegmentRelation::Diverged => return Ok(Relation::Unrelated),
        SegmentRelation::Identity => {
            // Commits can still be in parent/child relation inside one segment.
        }
    }

    let merge_base = match editor.repo().merge_base(left.id, right.id) {
        Ok(base) => base.detach(),
        Err(error) => match error {
            gix::repository::merge_base::Error::FindMergeBase(_)
            | gix::repository::merge_base::Error::NotFound { .. } => {
                return Ok(Relation::Unrelated);
            }
            _ => return Err(error.into()),
        },
    };

    if merge_base == left.id {
        return Ok(Relation::LeftIsAncestorOfRight);
    }
    if merge_base == right.id {
        return Ok(Relation::RightIsAncestorOfLeft);
    }
    Ok(Relation::Unrelated)
}

fn workspace_parent_to_child_rank<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    selected: &[SelectedCommit],
) -> Result<HashMap<gix::ObjectId, usize>> {
    let mut rank_by_id = HashMap::<gix::ObjectId, usize>::new();
    let mut rank = 0usize;
    for stack in &editor.workspace.stacks {
        for segment in &stack.segments {
            for commit in segment.commits.iter().rev() {
                rank_by_id.entry(commit.id).or_insert_with(|| {
                    let current = rank;
                    rank += 1;
                    current
                });
            }
        }
    }

    for selected_commit in selected {
        rank_by_id.get(&selected_commit.id).with_context(|| {
            format!(
                "Selected commit {id} is not part of the workspace traversal",
                id = selected_commit.id
            )
        })?;
    }

    Ok(rank_by_id)
}

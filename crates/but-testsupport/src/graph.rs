use std::collections::{BTreeMap, BTreeSet};

use but_graph::{
    EntryPoint, Graph, SegmentIndex, SegmentMetadata, projection::StackCommitDebugFlags,
};
use termtree::Tree;

type StringTree = Tree<String>;

/// Visualize `graph` as a tree.
pub fn graph_workspace(workspace: &but_graph::projection::Workspace<'_>) -> StringTree {
    let commit_flags = if workspace.graph.hard_limit_hit() {
        StackCommitDebugFlags::HardLimitReached
    } else {
        Default::default()
    };
    let mut root = Tree::new(workspace.debug_string());
    for stack in &workspace.stacks {
        root.push(tree_for_stack(stack, commit_flags));
    }
    root
}

fn tree_for_stack(
    stack: &but_graph::projection::Stack,
    commit_flags: StackCommitDebugFlags,
) -> StringTree {
    let mut root = Tree::new(stack.debug_string());
    for segment in &stack.segments {
        root.push(tree_for_stack_segment(segment, commit_flags));
    }
    root
}

fn tree_for_stack_segment(
    segment: &but_graph::projection::StackSegment,
    commit_flags: StackCommitDebugFlags,
) -> StringTree {
    let mut root = Tree::new(segment.debug_string());
    if let Some(outside) = &segment.commits_outside {
        for commit in outside {
            root.push(format!("{}*", commit.debug_string(commit_flags)));
        }
    }
    for commit in &segment.commits_on_remote {
        root.push(commit.debug_string(commit_flags | StackCommitDebugFlags::RemoteOnly));
    }
    for commit in &segment.commits {
        root.push(commit.debug_string(commit_flags));
    }
    root
}

/// Visualize `graph` as a tree.
pub fn graph_tree(graph: &Graph) -> StringTree {
    let mut root = Tree::new("".to_string());
    let mut seen = Default::default();
    for sidx in graph.tip_segments() {
        root.push(recurse_segment(graph, sidx, &mut seen));
    }
    let missing = graph.num_segments() - seen.len();
    if missing > 0 {
        let mut missing = Tree::new(format!(
            "ERROR: disconnected {missing} nodes unreachable through base"
        ));
        let mut newly_seen = Default::default();
        for sidx in graph.segments().filter(|sidx| !seen.contains(sidx)) {
            missing.push(recurse_segment(graph, sidx, &mut newly_seen));
        }
        root.push(missing);
        seen.extend(newly_seen);
    }

    if seen.is_empty() {
        "<UNBORN>".to_string().into()
    } else {
        root
    }
}

fn no_first_commit_on_named_segments(mut ep: EntryPoint<'_>) -> EntryPoint<'_> {
    if ep.segment.ref_name.is_some() && ep.commit_index == Some(0) {
        ep.commit_index = None;
    }
    ep
}

fn tree_for_commit(
    commit: &but_graph::Commit,
    is_entrypoint: bool,
    is_early_end: bool,
    hard_limit_hit: bool,
) -> StringTree {
    Graph::commit_debug_string(commit, is_entrypoint, is_early_end, hard_limit_hit).into()
}
fn recurse_segment(
    graph: &but_graph::Graph,
    sidx: SegmentIndex,
    seen: &mut BTreeSet<SegmentIndex>,
) -> StringTree {
    let segment = &graph[sidx];
    if seen.contains(&sidx) {
        return format!(
            "→:{sidx}:{name}",
            sidx = sidx.index(),
            name = graph[sidx]
                .ref_name
                .as_ref()
                .map(|n| format!(
                    " ({}{maybe_sibling})",
                    Graph::ref_debug_string(n),
                    maybe_sibling = segment
                        .sibling_segment_id
                        .map_or_else(String::new, |sid| format!(" →:{}:", sid.index()))
                ))
                .unwrap_or_default()
        )
        .into();
    }
    seen.insert(sidx);
    let ep = no_first_commit_on_named_segments(graph.lookup_entrypoint().unwrap());
    let segment_is_entrypoint = ep.segment_index == sidx;
    let mut show_segment_entrypoint = segment_is_entrypoint;
    if segment_is_entrypoint {
        // Reduce noise by preferring ref-based entry-points.
        if segment.ref_name.is_none() && ep.commit_index.is_some() {
            show_segment_entrypoint = false;
        }
    }
    let connected_segments = {
        let mut m = BTreeMap::<_, Vec<_>>::new();
        let below = graph.segments_below_in_order(sidx).collect::<Vec<_>>();
        for (source_cidx, sidx) in below {
            m.entry(source_cidx).or_default().push(sidx);
        }
        m
    };

    let mut root = Tree::new(format!(
        "{entrypoint}{meta}{arrow}:{id}[{generation}]:{ref_name_and_remote}",
        meta = match segment.metadata {
            None => {
                ""
            }
            Some(SegmentMetadata::Workspace(_)) => {
                "📕"
            }
            Some(SegmentMetadata::Branch(_)) => {
                "📙"
            }
        },
        id = segment.id.index(),
        generation = segment.generation,
        arrow = if segment.workspace_metadata().is_some() {
            "►►►"
        } else {
            "►"
        },
        entrypoint = if show_segment_entrypoint {
            if ep.commit.is_none() && ep.commit_index.is_some() {
                "🫱"
            } else {
                "👉"
            }
        } else {
            ""
        },
        ref_name_and_remote = Graph::ref_and_remote_debug_string(
            segment.ref_name.as_ref(),
            segment.remote_tracking_ref_name.as_ref(),
            segment.sibling_segment_id
        ),
    ));
    for (cidx, commit) in segment.commits.iter().enumerate() {
        let mut commit_tree = tree_for_commit(
            commit,
            segment_is_entrypoint && Some(cidx) == ep.commit_index,
            if cidx + 1 != segment.commits.len() {
                false
            } else {
                graph.is_early_end_of_traversal(sidx)
            },
            graph.hard_limit_hit(),
        );
        if let Some(segment_indices) = connected_segments.get(&Some(cidx)) {
            for sidx in segment_indices {
                commit_tree.push(recurse_segment(graph, *sidx, seen));
            }
        }
        root.push(commit_tree);
    }
    // Get the segments that are directly connected.
    if let Some(segment_indices) = connected_segments.get(&None) {
        for sidx in segment_indices {
            root.push(recurse_segment(graph, *sidx, seen));
        }
    }

    root
}

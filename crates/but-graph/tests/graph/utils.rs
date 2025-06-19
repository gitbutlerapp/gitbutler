use but_graph::{EntryPoint, Graph, SegmentIndex};
use std::collections::{BTreeMap, BTreeSet};
use termtree::Tree;

type SegmentTree = Tree<String>;

/// Visualize `graph` as a tree.
pub fn graph_tree(graph: &but_graph::Graph) -> SegmentTree {
    fn tree_for_commit(
        commit: &but_graph::Commit,
        has_conflicts: bool,
        is_entrypoint: bool,
        is_early_end: bool,
        hard_limit_hit: bool,
    ) -> SegmentTree {
        Graph::commit_debug_string(
            commit,
            has_conflicts,
            is_entrypoint,
            true, /* show message */
            is_early_end,
            hard_limit_hit,
        )
        .into()
    }
    fn recurse_segment(
        graph: &but_graph::Graph,
        sidx: SegmentIndex,
        seen: &mut BTreeSet<SegmentIndex>,
    ) -> SegmentTree {
        if seen.contains(&sidx) {
            return format!(
                "→:{sidx}:{name}",
                sidx = sidx.index(),
                name = graph[sidx]
                    .ref_name
                    .as_ref()
                    .map(|n| format!(" ({})", Graph::ref_debug_string(n)))
                    .unwrap_or_default()
            )
            .into();
        }
        seen.insert(sidx);
        let segment = &graph[sidx];
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
            for (cidx, sidx) in graph.segments_on_top(sidx) {
                m.entry(cidx).or_default().push(sidx);
            }
            m
        };

        let mut root = Tree::new(format!(
            "{entrypoint}{kind}:{id}:{ref_name}{remote}",
            id = segment.id.index(),
            kind = if segment.workspace_metadata().is_some() {
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
            ref_name = segment
                .ref_name
                .as_ref()
                .map(Graph::ref_debug_string)
                .unwrap_or("anon:".into()),
            remote = if let Some(remote_ref_name) = segment.remote_tracking_ref_name.as_ref() {
                format!(
                    " <> {remote_name}",
                    remote_name = Graph::ref_debug_string(remote_ref_name)
                )
            } else {
                "".into()
            },
        ));
        for (cidx, commit) in segment.commits.iter().enumerate() {
            let mut commit_tree = tree_for_commit(
                commit,
                commit.has_conflicts,
                segment_is_entrypoint && Some(cidx) == ep.commit_index,
                graph.is_early_end_of_traversal(sidx, cidx),
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

    fn no_first_commit_on_named_segments(mut ep: EntryPoint) -> EntryPoint {
        if ep.segment.ref_name.is_some() && ep.commit_index == Some(0) {
            ep.commit_index = None;
        }
        ep
    }

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

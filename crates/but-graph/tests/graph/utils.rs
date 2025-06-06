use bstr::ByteSlice;
use but_graph::{LocalCommitRelation, RefLocation, SegmentIndex};
use std::collections::{BTreeMap, BTreeSet};
use termtree::Tree;

type SegmentTree = Tree<String>;

/// Visualize `graph` as a tree.
pub fn graph_tree(graph: &but_graph::Graph) -> SegmentTree {
    enum CommitKind {
        Local,
        Remote,
    }
    fn tree_for_commit<'a>(
        commit: &but_graph::Commit,
        extra: impl Into<Option<&'a str>>,
        has_conflicts: bool,
        kind: CommitKind,
    ) -> SegmentTree {
        let extra = extra.into();
        format!(
            "{kind}{conflict}{hex}{extra}❱{msg:?}{refs}",
            kind = match kind {
                CommitKind::Local => {
                    "🔵"
                }
                CommitKind::Remote => {
                    "🟣"
                }
            },
            conflict = if has_conflicts { "💥" } else { "" },
            extra = if let Some(extra) = extra {
                format!(" [{extra}]")
            } else {
                "".into()
            },
            hex = commit.id.to_hex_with_len(7),
            msg = commit.message.trim().as_bstr(),
            refs = if commit.refs.is_empty() {
                "".to_string()
            } else {
                format!(
                    " {}",
                    commit
                        .refs
                        .iter()
                        .map(|rn| format!("►{}", rn.shorten()))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
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
                "ERROR: Reached segment {sidx} for a second time: {name:?}",
                sidx = sidx.index(),
                name = graph[sidx].ref_name.as_ref().map(|n| n.as_bstr())
            )
            .into();
        }
        seen.insert(sidx);
        let segment = &graph[sidx];
        let on_top = {
            let mut m = BTreeMap::<_, Vec<_>>::new();
            for (cidx, sidx) in graph.segments_on_top(sidx) {
                m.entry(cidx).or_default().push(sidx);
            }
            m
        };

        let mut root = Tree::new(format!(
            "{ref_name}{location}{remote}",
            ref_name = segment
                .ref_name
                .as_ref()
                .map(|n| format!("►{}", n))
                .unwrap_or("<anon>".into()),
            location = if let Some(RefLocation::OutsideOfWorkspace) = segment.ref_location {
                "(OUTSIDE)"
            } else {
                ""
            },
            remote = if let Some(remote_ref_name) = segment.remote_tracking_ref_name.as_ref() {
                format!(" <> {remote_name}", remote_name = remote_ref_name.as_bstr())
            } else {
                "".into()
            }
        ));
        for (cidx, commit) in segment.commits_unique_from_tip.iter().enumerate().rev() {
            let mut commit_tree = tree_for_commit(
                commit,
                if commit.relation == LocalCommitRelation::LocalOnly {
                    None
                } else {
                    Some(commit.relation.display(commit.id))
                },
                commit.has_conflicts,
                CommitKind::Local,
            );
            if let Some(segment_indices) = on_top.get(&Some(cidx)) {
                for sidx in segment_indices {
                    commit_tree.push(recurse_segment(graph, *sidx, seen));
                }
            }
            root.push(commit_tree);
        }
        // Get the segments that are directly connected.
        if let Some(segment_indices) = on_top.get(&None) {
            for sidx in segment_indices {
                root.push(recurse_segment(graph, *sidx, seen));
            }
        }

        for commit in segment
            .commits_unique_in_remote_tracking_branch
            .iter()
            .rev()
        {
            root.push(tree_for_commit(
                &commit.inner,
                None,
                commit.has_conflicts,
                CommitKind::Remote,
            ));
        }

        root
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

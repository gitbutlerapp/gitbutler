use bstr::ByteSlice;
use but_graph::{EntryPoint, LocalCommitRelation, SegmentIndex};
use gix::refs::Category;
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
        is_entrypoint: bool,
        kind: CommitKind,
    ) -> SegmentTree {
        let extra = extra.into();
        format!(
            "{ep}{kind}{conflict}{hex}{extra}{flags}❱{msg:?}{refs}",
            ep = if is_entrypoint { "👉" } else { "" },
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
            flags = if !commit.flags.is_empty() {
                format!(" ({})", commit.flags.debug_string())
            } else {
                "".to_string()
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
                        .map(|rn| format!("►{}", {
                            let (cat, sn) = rn.category_and_short_name().expect("valid refs");
                            // Only shorten those that look good and are unambiguous enough.
                            if matches!(cat, Category::LocalBranch | Category::RemoteBranch) {
                                sn
                            } else {
                                rn.as_bstr()
                                    .strip_prefix(b"refs/")
                                    .map(|n| n.as_bstr())
                                    .unwrap_or(rn.as_bstr())
                            }
                        }))
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
            id = segment.id,
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
                .map(|n| n.to_string())
                .unwrap_or("anon:".into()),
            remote = if let Some(remote_ref_name) = segment.remote_tracking_ref_name.as_ref() {
                format!(" <> {remote_name}", remote_name = remote_ref_name.as_bstr())
            } else {
                "".into()
            },
        ));
        for (cidx, commit) in segment.commits.iter().enumerate() {
            let mut commit_tree = tree_for_commit(
                commit,
                if commit.relation == LocalCommitRelation::LocalOnly {
                    None
                } else {
                    Some(commit.relation.display(commit.id))
                },
                commit.has_conflicts,
                segment_is_entrypoint && Some(cidx) == ep.commit_index,
                CommitKind::Local,
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

        for commit in segment.commits_unique_in_remote_tracking_branch.iter() {
            root.push(tree_for_commit(
                &commit.inner,
                None,
                commit.has_conflicts,
                false, /* is_entrypoint */
                CommitKind::Remote,
            ));
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

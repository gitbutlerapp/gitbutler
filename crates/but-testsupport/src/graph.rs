use std::collections::{BTreeMap, BTreeSet};

use but_core::ref_metadata::StackId;
use but_graph::{Graph, SegmentIndex, SegmentMetadata, workspace::StackCommitDebugFlags};
use termtree::Tree;

type StringTree = Tree<String>;

/// Like [`Graph::from_head()`], but from the [nosegment graph](but_graph::nosegment::workspace()):
/// seen from `HEAD`, with the linked worktrees if `options.worktrees` is set.
pub fn graph_from_head(
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
    project_meta: but_core::ref_metadata::ProjectMeta,
    db: &mut but_db::DbHandle,
    options: but_graph::init::Options,
) -> anyhow::Result<Graph> {
    let head = repo.head()?;
    let mut graph = but_graph::nosegment::workspace(
        repo,
        meta,
        but_graph::init::Overlay::default(),
        project_meta,
        head.referent_name(),
        head.id().map(|id| id.detach()),
        [],
        but_graph::init::discover_worktree_tips(repo, db, options.worktrees)?,
        options.collect_tags,
        options.commits_limit_hint,
    )?
    .graph;
    graph.options = options;
    Ok(graph)
}

/// Like [`Graph::from_commit_traversal()`], but from the
/// [nosegment graph](but_graph::nosegment::workspace()): seen from `ref_name` at `tip`, with the
/// linked worktrees if `options.worktrees` is set.
pub fn graph_from_commit_traversal(
    tip: gix::Id<'_>,
    ref_name: impl Into<Option<gix::refs::FullName>>,
    meta: &impl but_core::RefMetadata,
    project_meta: but_core::ref_metadata::ProjectMeta,
    db: &mut but_db::DbHandle,
    options: but_graph::init::Options,
) -> anyhow::Result<Graph> {
    let ref_name = ref_name.into();
    let mut graph = but_graph::nosegment::workspace(
        tip.repo(),
        meta,
        but_graph::init::Overlay::default(),
        project_meta,
        ref_name.as_ref().map(|name| name.as_ref()),
        Some(tip.detach()),
        [],
        but_graph::init::discover_worktree_tips(tip.repo(), db, options.worktrees)?,
        options.collect_tags,
        options.commits_limit_hint,
    )?
    .graph;
    graph.options = options;
    Ok(graph)
}

pub fn graph_workspace(workspace: &but_graph::Workspace) -> StringTree {
    graph_workspace_inner(workspace, None)
}

/// Remaps random stack ids to something deterministic.
pub fn graph_workspace_determinisitcally(workspace: &but_graph::Workspace) -> StringTree {
    graph_workspace_inner(workspace, Some(Default::default()))
}

fn graph_workspace_inner(
    workspace: &but_graph::Workspace,
    mut stack_id_map: Option<BTreeMap<StackId, StackId>>,
) -> StringTree {
    let commit_flags = if workspace.graph.hard_limit_hit() {
        StackCommitDebugFlags::HardLimitReached
    } else {
        Default::default()
    };
    let mut root = Tree::new(workspace.debug_string());
    for stack in &workspace.stacks {
        root.push(tree_for_stack(
            &workspace.graph,
            stack,
            commit_flags,
            stack_id_map.as_mut(),
        ));
    }
    for worktree in &workspace.worktrees {
        let mut tree = Tree::new(worktree.debug_string());
        for segment in &worktree.segments {
            tree.push(tree_for_stack_segment(
                &workspace.graph,
                segment,
                commit_flags,
            ));
        }
        root.push(tree);
    }
    root
}

fn tree_for_stack(
    graph: &Graph,
    stack: &but_graph::workspace::Stack,
    commit_flags: StackCommitDebugFlags,
    stack_id_map: Option<&mut BTreeMap<StackId, StackId>>,
) -> StringTree {
    let mut root = Tree::new(stack.debug_string_with_graph_context(
        graph,
        stack.id.zip(stack_id_map).map(|(id, map)| {
            let next_id = StackId::from_number_for_testing((map.len() + 1) as u128);
            *map.entry(id).or_insert(next_id)
        }),
    ));
    for segment in &stack.segments {
        root.push(tree_for_stack_segment(graph, segment, commit_flags));
    }
    root
}

fn tree_for_stack_segment(
    graph: &Graph,
    segment: &but_graph::workspace::StackSegment,
    commit_flags: StackCommitDebugFlags,
) -> StringTree {
    let mut root = Tree::new(segment.debug_string_with_graph_context(graph));
    for commit in &segment.commits_on_remote {
        root.push(commit.debug_string(commit_flags | StackCommitDebugFlags::RemoteOnly));
    }
    for commit in &segment.commits {
        root.push(commit.debug_string(commit_flags));
    }
    root
}

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

fn tree_for_commit(
    graph: &Graph,
    commit: &but_graph::Commit,
    is_entrypoint: bool,
    stop_condition: Option<but_graph::StopCondition>,
    hard_limit_hit: bool,
) -> StringTree {
    graph
        .commit_debug_string_with_graph_context(
            commit,
            is_entrypoint,
            stop_condition,
            hard_limit_hit,
        )
        .into()
}

/// Name the segment at `sidx` for a reference to it: by its reference, or by its first commit
/// if it has none, so references don't depend on how the graph numbered its segments.
/// Only an empty segment without a reference falls back to its index.
fn segment_label(graph: &Graph, sidx: SegmentIndex) -> String {
    let segment = &graph[sidx];
    match (segment.ref_info.as_ref(), segment.commits.first()) {
        (Some(ri), _) => format!(
            "segment {}",
            graph.ref_debug_string_with_graph_context(ri.ref_name.as_ref(), ri.worktree.as_ref())
        ),
        (None, Some(commit)) => format!("segment ·{}", commit.id.to_hex_with_len(7)),
        (None, None) => format!("segment :{}", sidx.index()),
    }
}

fn recurse_segment(
    graph: &but_graph::Graph,
    sidx: SegmentIndex,
    seen: &mut BTreeSet<SegmentIndex>,
) -> StringTree {
    let segment = &graph[sidx];
    if seen.contains(&sidx) {
        return format!(
            "→ {label}{maybe_sibling}",
            label = segment_label(graph, sidx),
            maybe_sibling = segment
                .ref_info
                .as_ref()
                .and(
                    segment
                        .remote_tracking_branch_segment_id
                        .or(segment.sibling_segment_id)
                )
                .map_or_else(String::new, |sid| format!(
                    " (→ {})",
                    segment_label(graph, sid)
                ))
        )
        .into();
    }
    seen.insert(sidx);
    let ep = graph.entrypoint().unwrap();
    let segment_is_entrypoint = ep.segment.id == sidx;
    let entrypoint_commit = ep.commit();
    let mut entrypoint_commit_index =
        entrypoint_commit.and_then(|commit| ep.segment.commit_index_of(commit.id));
    if ep.segment.ref_info.is_some() && entrypoint_commit_index == Some(0) {
        entrypoint_commit_index = None;
    }
    let mut show_segment_entrypoint = segment_is_entrypoint;
    if segment_is_entrypoint {
        // Reduce noise by preferring ref-based entry-points.
        if segment.ref_info.is_none() && entrypoint_commit_index.is_some() {
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
        "{entrypoint}{meta}{arrow}{id}[{generation}]:{ref_name_and_remote}",
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
        // Only a segment that `segment_label()` can't name otherwise shows its index.
        id = if segment.ref_info.is_none() && segment.commits.is_empty() {
            format!(":{}", sidx.index())
        } else {
            String::new()
        },
        generation = segment.generation,
        arrow = if segment.workspace_metadata().is_some() {
            "►►►"
        } else {
            "►"
        },
        entrypoint = if show_segment_entrypoint {
            if entrypoint_commit.is_none() && entrypoint_commit_index.is_some() {
                "🫱"
            } else {
                "👉"
            }
        } else {
            ""
        },
        ref_name_and_remote = graph.ref_and_remote_debug_string_with_graph_context(
            segment.ref_info.as_ref(),
            segment.remote_tracking_ref_name.as_ref(),
        ),
    ));
    for (cidx, commit) in segment.commits.iter().enumerate() {
        let mut commit_tree = tree_for_commit(
            graph,
            commit,
            segment_is_entrypoint && Some(cidx) == entrypoint_commit_index,
            if cidx + 1 != segment.commits.len() {
                None
            } else {
                graph.stop_condition(sidx)
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

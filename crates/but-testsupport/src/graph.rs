use std::collections::BTreeMap;

use but_core::ref_metadata::StackId;
use but_graph::workspace::StackCommitDebugFlags;
use termtree::Tree;

type StringTree = Tree<String>;

/// Visualize `graph` as a tree.
pub fn graph_workspace(workspace: &but_graph::Workspace) -> StringTree {
    graph_workspace_inner(workspace, None)
}

/// Visualize `graph` as a tree, and remap random stack ids to something deterministic.
pub fn graph_workspace_determinisitcally(workspace: &but_graph::Workspace) -> StringTree {
    graph_workspace_inner(workspace, Some(Default::default()))
}

fn graph_workspace_inner(
    workspace: &but_graph::Workspace,
    mut stack_id_map: Option<BTreeMap<StackId, StackId>>,
) -> StringTree {
    let commit_flags = if workspace.hard_limit_hit() {
        StackCommitDebugFlags::HardLimitReached
    } else {
        Default::default()
    };
    let mut root = Tree::new(workspace.debug_string());
    for stack in &workspace.stacks {
        root.push(tree_for_stack(
            workspace,
            stack,
            commit_flags,
            stack_id_map.as_mut(),
        ));
    }
    root
}

fn tree_for_stack(
    ws: &but_graph::Workspace,
    stack: &but_graph::workspace::Stack,
    commit_flags: StackCommitDebugFlags,
    stack_id_map: Option<&mut BTreeMap<StackId, StackId>>,
) -> StringTree {
    let mut root = Tree::new(stack.debug_string_with_graph_context(
        ws,
        stack.id.zip(stack_id_map).map(|(id, map)| {
            let next_id = StackId::from_number_for_testing((map.len() + 1) as u128);
            *map.entry(id).or_insert(next_id)
        }),
    ));
    for segment in &stack.segments {
        root.push(tree_for_stack_segment(ws, segment, commit_flags));
    }
    root
}

fn tree_for_stack_segment(
    ws: &but_graph::Workspace,
    segment: &but_graph::workspace::StackSegment,
    commit_flags: StackCommitDebugFlags,
) -> StringTree {
    let mut root = Tree::new(segment.debug_string_with_graph_context(ws));
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

/// Visualize `graph` as a commit DAG over its carried commit graph, with a `layout:`
/// footer for the metadata-driven ref placements. Local refs with a derived
/// remote-tracking branch show it as ` <> remote`.
pub fn graph_dag(ws: &but_graph::Workspace) -> String {
    use renderdag::{Ancestor, GraphRowRenderer, Renderer as _};

    let cg = ws.commit_graph();
    let ids: Vec<gix::ObjectId> = cg.commit_ids().collect();
    if ids.is_empty() {
        let mut out = "<UNBORN>".to_string();
        if let Some(ref_name) = cg.entrypoint_ref() {
            out.push_str(&format!(" 👉►{}", ref_name.as_ref().shorten()));
        }
        return out;
    }

    // git-log style topo order: every commit after all its children; tips seed the
    // stack in the walk's seeding order.
    let mut indegree: BTreeMap<gix::ObjectId, usize> = ids.iter().map(|&id| (id, 0)).collect();
    for &id in &ids {
        for parent in cg.connected_parents(id) {
            *indegree.get_mut(&parent).expect("parent iterated above") += 1;
        }
    }
    let mut stack: Vec<gix::ObjectId> = ids
        .iter()
        .rev()
        .filter(|id| indegree[*id] == 0)
        .copied()
        .collect();
    let mut order = Vec::with_capacity(ids.len());
    while let Some(id) = stack.pop() {
        order.push(id);
        let parents: Vec<_> = cg.connected_parents(id).collect();
        for parent in parents.iter().rev() {
            let d = indegree.get_mut(parent).expect("known id");
            *d -= 1;
            if *d == 0 {
                stack.push(*parent);
            }
        }
    }

    // The workspace-level entrypoint exists on every build path, the arena's only on fresh walks.
    let entrypoint = ws
        .entrypoint_commit_id()
        .ok()
        .flatten()
        .or_else(|| cg.entrypoint());
    let mut renderer = GraphRowRenderer::<gix::ObjectId>::new()
        .output()
        .with_min_row_height(1)
        .build_box_drawing();
    let mut out = String::new();
    for id in order {
        let commit = cg.node(id).expect("iterating existing ids");
        // Workspace refs are machinery, not commit decoration — filter so walk-built and
        // seam-built substrates render identically.
        let commit = {
            let mut commit = commit.clone();
            commit
                .refs
                .retain(|ri| !but_core::is_workspace_ref_name(ri.ref_name.as_ref()));
            commit
        };
        let commit = &commit;
        let followed: Vec<_> = cg.connected_parents(id).collect();
        let stop_condition = if followed.is_empty() {
            let mut condition = but_graph::StopCondition::empty();
            if commit.parent_ids.is_empty() {
                condition |= but_graph::StopCondition::FirstCommit;
            }
            if commit
                .flags
                .contains(but_graph::CommitFlags::ShallowBoundary)
            {
                condition |= but_graph::StopCondition::ShallowBoundary;
            }
            if !commit.parent_ids.is_empty()
                && !condition.contains(but_graph::StopCondition::ShallowBoundary)
            {
                condition |= but_graph::StopCondition::Limit;
            }
            (!condition.is_empty()).then_some(condition)
        } else {
            None
        };
        let mut line = ws.commit_debug_string(commit, entrypoint == Some(id), stop_condition);
        let remotes: Vec<String> = commit
            .refs
            .iter()
            .filter_map(|ri| ws.remote_tracking_branch(ri.ref_name.as_ref()))
            .map(|remote| remote.as_ref().shorten().to_string())
            .collect();
        if !remotes.is_empty() {
            line.push_str(&format!(" <> {}", remotes.join(", ")));
        }
        out.push_str(&renderer.next_row(
            id,
            followed.into_iter().map(Ancestor::Parent).collect(),
            "*".to_string(),
            line,
        ));
    }
    let out = out.trim_end().to_string();
    match layout_footer(ws.commit_graph()) {
        Some(footer) => format!("{out}\n{footer}"),
        None => out,
    }
}

/// The `layout:` footer of [`graph_dag`]: the stored layout's empty-chain anchors
/// (`^` marks an anchor that joins an owning chain) and the workspace commit's
/// materialized parents.
fn layout_footer(cg: &but_graph::CommitGraph) -> Option<String> {
    let layout = cg.layout()?;
    let mut lines = Vec::new();
    if let Some(m) = &layout.materialized_ws_parents {
        let (ws, parents) = (&m.commit, &m.parents);
        lines.push(format!(
            "  materialized parents: {}: {}",
            ws.to_hex_with_len(7),
            parents
                .iter()
                .map(|id| id.to_hex_with_len(7).to_string())
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    if !layout.empty_chain_anchors.is_empty() {
        lines.push(format!(
            "  empty chain anchors: {}",
            layout
                .empty_chain_anchors
                .iter()
                .map(|anchor| format!(
                    "{}{}",
                    anchor.commit.to_hex_with_len(7),
                    if anchor.joins_owning_chain { "^" } else { "" }
                ))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    (!lines.is_empty()).then(|| format!("layout:\n{}", lines.join("\n")))
}

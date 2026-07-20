use std::{cmp::Ordering, collections::BTreeMap};

use but_core::ref_metadata::StackId;
use but_graph::{
    CommitFlags, Graph, NodeGraphEntrypoint, NodeIndex, NodeKind, RefInfo, Reference,
    ReferenceMetadata, WorktreeKind,
    workspace::{Stack, StackCommitDebugFlags, StackCommitFlags, StackSegment, WorkspaceKind},
};
use gix::{bstr::ByteSlice as _, refs::Category};
use renderdag::{Ancestor, GraphRowRenderer, Renderer as _};
use termtree::Tree;

type StringTree = Tree<String>;

/// Visualize a workspace projection as a tree.
pub fn graph_workspace(workspace: &but_graph::Workspace) -> StringTree {
    graph_workspace_inner(workspace, None)
}

/// Visualize a workspace projection, remapping random stack IDs deterministically.
pub fn graph_workspace_determinisitcally(workspace: &but_graph::Workspace) -> StringTree {
    graph_workspace_inner(workspace, Some(Default::default()))
}

fn graph_workspace_inner(
    workspace: &but_graph::Workspace,
    mut stack_id_map: Option<BTreeMap<StackId, StackId>>,
) -> StringTree {
    let commit_flags = StackCommitDebugFlags::default();
    let mut root = Tree::new(workspace_label(workspace));
    for stack in &workspace.stacks {
        let id = stack.id.zip(stack_id_map.as_mut()).map(|(id, map)| {
            let next_id = StackId::from_number_for_testing((map.len() + 1) as u128);
            *map.entry(id).or_insert(next_id)
        });
        let mut stack_tree = Tree::new(stack_label(&workspace.graph, stack, id));
        for segment in &stack.segments {
            let mut segment_tree = Tree::new(stack_segment_label(&workspace.graph, segment));
            if let Some(outside) = &segment.commits_outside {
                for commit in outside {
                    segment_tree.push(format!("{}*", commit.debug_string(commit_flags)));
                }
            }
            for commit in &segment.commits_on_remote {
                segment_tree
                    .push(commit.debug_string(commit_flags | StackCommitDebugFlags::RemoteOnly));
            }
            for commit in &segment.commits {
                segment_tree.push(commit.debug_string(commit_flags));
            }
            stack_tree.push(segment_tree);
        }
        root.push(stack_tree);
    }
    root
}

fn workspace_label(workspace: &but_graph::Workspace) -> String {
    let (name, sign) = match &workspace.kind {
        WorkspaceKind::Managed { ref_info } => (ref_info_label(&workspace.graph, ref_info), "🏘️"),
        WorkspaceKind::ManagedMissingWorkspaceCommit { ref_info } => {
            (ref_info_label(&workspace.graph, ref_info), "🏘️⚠️")
        }
        WorkspaceKind::AdHoc => (
            workspace
                .ref_name()
                .map(|name| ref_name_label(&workspace.graph, name, None))
                .unwrap_or_else(|| "DETACHED".into()),
            "⌂",
        ),
    };
    let target = workspace.target_ref.as_ref().map_or_else(
        || "!".into(),
        |target| {
            format!(
                "{}{ahead}",
                target.ref_name,
                ahead = if target.commits_ahead == 0 {
                    String::new()
                } else {
                    format!("⇣{}", target.commits_ahead)
                }
            )
        },
    );
    format!(
        "{meta}{sign}:{id}:{name} <> ✓{target}{bound}",
        id = workspace.id.map_or_else(|| "-".into(), |id| id.to_string()),
        meta = if workspace.metadata.is_some() {
            "📕"
        } else {
            ""
        },
        bound = workspace
            .lower_bound
            .map(|base| format!(" on {}", base.to_hex_with_len(7)))
            .unwrap_or_default(),
    )
}

fn stack_label(graph: &Graph, stack: &Stack, id_override: Option<StackId>) -> String {
    let mut label = stack.segments.first().map_or_else(
        || "<anon>".into(),
        |segment| stack_segment_label(graph, segment),
    );
    if let Some(base) = stack.base() {
        label.push_str(&format!(" on {}", base.to_hex_with_len(7)));
    }
    label.insert(0, '≡');
    if let Some(id) = id_override.or(stack.id) {
        let id = id.to_string().replace(['0', '-'], "");
        label.push_str(&format!(" {{{}}}", if id.is_empty() { "0" } else { &id }));
    }
    label
}

fn stack_segment_label(graph: &Graph, segment: &StackSegment) -> String {
    let local_commits = segment
        .remote_tracking_ref_name
        .as_ref()
        .map(|_| {
            segment
                .commits
                .iter()
                .filter(|commit| {
                    !commit.flags.intersects(
                        StackCommitFlags::ReachableByRemote | StackCommitFlags::Integrated,
                    )
                })
                .count()
        })
        .unwrap_or_default();
    format!(
        "{entrypoint}{metadata}:{}:{}{local}{remote}",
        segment.id,
        ref_and_remote_label(
            graph,
            segment.ref_info.as_ref(),
            segment.remote_tracking_ref_name.as_ref(),
            segment.sibling_node_id,
            segment.remote_tracking_branch_node_id,
        ),
        entrypoint = if segment.is_entrypoint { "👉" } else { "" },
        metadata = if segment.metadata.is_some() {
            "📙"
        } else {
            ""
        },
        local = if local_commits == 0 {
            String::new()
        } else {
            format!("⇡{local_commits}")
        },
        remote = if segment.commits_on_remote.is_empty() {
            String::new()
        } else {
            format!("⇣{}", segment.commits_on_remote.len())
        },
    )
}

fn ref_and_remote_label(
    graph: &Graph,
    ref_info: Option<&RefInfo>,
    remote_ref_name: Option<&gix::refs::FullName>,
    sibling_id: Option<NodeIndex>,
    remote_tracking_branch_id: Option<NodeIndex>,
) -> String {
    let local = ref_info.map_or_else(
        || format!("anon{}", node_arrow(sibling_id)),
        |ref_info| {
            format!(
                "{}{}",
                ref_info_label(graph, ref_info),
                if remote_ref_name.is_none() {
                    node_arrow(sibling_id)
                } else {
                    String::new()
                }
            )
        },
    );
    remote_ref_name.map_or(local.clone(), |remote| {
        format!(
            "{local} <> {}{}",
            ref_name_label(graph, remote.as_ref(), None),
            node_arrow(remote_tracking_branch_id.or(sibling_id))
        )
    })
}

fn node_arrow(index: Option<NodeIndex>) -> String {
    index.map_or_else(String::new, |index| format!(" →:{index}:"))
}

/// Visualize the canonical commit/reference DAG.
pub fn graph_tree(graph: &Graph) -> StringTree {
    graph_dag(graph).into()
}

fn graph_dag(graph: &Graph) -> String {
    if graph.nodes().is_empty() {
        return match graph.entrypoint() {
            NodeGraphEntrypoint::Unborn(reference) => {
                render_unborn_reference(graph, reference.as_ref())
            }
            NodeGraphEntrypoint::Node(_) => "<UNBORN>".into(),
        };
    }

    let mut renderer = GraphRowRenderer::<NodeIndex>::new()
        .output()
        .with_min_row_height(1)
        .build_box_drawing();
    let mut out = String::new();
    for index in topological_order(graph) {
        let (glyph, label) = node_label(graph, index);
        let parents = graph.nodes()[index]
            .parents()
            .iter()
            .copied()
            .filter(|parent| !matches!(graph.nodes()[*parent].kind(), NodeKind::Boundary { .. }))
            .map(Ancestor::Parent)
            .collect();
        out.push_str(&renderer.next_row(index, parents, glyph.into(), label));
    }
    out.trim_end().into()
}

fn render_unborn_reference(graph: &Graph, reference: &Reference) -> String {
    let mut renderer = GraphRowRenderer::<NodeIndex>::new()
        .output()
        .with_min_row_height(1)
        .build_box_drawing();
    renderer
        .next_row(
            0,
            Vec::new(),
            "◎".into(),
            reference_label(graph, reference, true),
        )
        .trim_end()
        .into()
}

fn node_label(graph: &Graph, index: NodeIndex) -> (&'static str, String) {
    let is_entrypoint =
        matches!(graph.entrypoint(), NodeGraphEntrypoint::Node(entrypoint) if *entrypoint == index);
    match graph.nodes()[index].kind() {
        NodeKind::Commit { id } => {
            let flags = graph.annotations()[index] & CommitFlags::all();
            let stop = commit_stop_marker(graph, index);
            let flags_label = if flags.is_empty() {
                String::new()
            } else {
                format!(" ({})", flags.debug_string())
            };
            (
                "●",
                format!(
                    "{}{stop}{}{}{flags_label}",
                    if is_entrypoint { "👉" } else { "" },
                    if flags.contains(CommitFlags::EntrypointSide) {
                        "·"
                    } else {
                        "🟣"
                    },
                    id.to_hex_with_len(7),
                ),
            )
        }
        NodeKind::Reference(reference) => ("◎", reference_label(graph, reference, is_entrypoint)),
        NodeKind::Boundary { .. } => unreachable!("boundaries are not rendered"),
        NodeKind::None => ("◌", "no-op".into()),
    }
}

fn commit_stop_marker(graph: &Graph, index: NodeIndex) -> String {
    let node = &graph.nodes()[index];
    let mut stop = if node.parents().is_empty() {
        "🏁".to_owned()
    } else {
        String::new()
    };
    for parent in node.parents() {
        if let NodeKind::Boundary { reason, .. } = graph.nodes()[*parent].kind() {
            stop.push_str(reason.debug_string());
        }
    }
    stop
}

fn reference_label(graph: &Graph, reference: &Reference, is_entrypoint: bool) -> String {
    let metadata = match reference.metadata {
        Some(ReferenceMetadata::Workspace(_)) => "📕",
        Some(ReferenceMetadata::Branch(_)) => "📙",
        None => "",
    };
    let remote = reference
        .remote_tracking_ref_name
        .as_ref()
        .map(|name| format!(" <> {}", ref_name_label(graph, name.as_ref(), None)))
        .unwrap_or_default();
    format!(
        "{}{metadata}{}{remote}",
        if is_entrypoint { "👉" } else { "" },
        ref_info_label(graph, &reference.ref_info),
    )
}

fn ref_info_label(graph: &Graph, ref_info: &RefInfo) -> String {
    ref_name_label(
        graph,
        ref_info.ref_name.as_ref(),
        ref_info.worktree.as_ref(),
    )
}

fn ref_name_label(
    graph: &Graph,
    name: &gix::refs::FullNameRef,
    worktree: Option<&but_graph::Worktree>,
) -> String {
    let (category, short_name) = name.category_and_short_name().expect("valid reference");
    let rendered_name = if matches!(category, Category::LocalBranch | Category::RemoteBranch) {
        short_name.to_string()
    } else {
        name.as_bstr()
            .strip_prefix(b"refs/")
            .unwrap_or(name.as_bstr())
            .as_bstr()
            .to_string()
    };
    let worktree = worktree
        .map(|worktree| {
            worktree.debug_string_with_graph_context(name, has_multiple_worktrees(graph))
        })
        .unwrap_or_default();
    format!("{rendered_name}{worktree}")
}

fn has_multiple_worktrees(graph: &Graph) -> bool {
    let mut first: Option<&WorktreeKind> = None;
    graph
        .nodes()
        .iter()
        .filter_map(|node| match node.kind() {
            NodeKind::Reference(reference) => reference.ref_info.worktree.as_ref(),
            NodeKind::Commit { .. } | NodeKind::Boundary { .. } | NodeKind::None => None,
        })
        .any(|worktree| {
            if let Some(first) = first {
                first != &worktree.kind
            } else {
                first = Some(&worktree.kind);
                false
            }
        })
}

fn topological_order(graph: &Graph) -> Vec<NodeIndex> {
    let mut children = graph
        .nodes()
        .iter()
        .enumerate()
        .filter(|(_, node)| !matches!(node.kind(), NodeKind::Boundary { .. }))
        .map(|(index, _)| (index, 0usize))
        .collect::<BTreeMap<_, _>>();
    for node in graph.nodes() {
        if matches!(node.kind(), NodeKind::Boundary { .. }) {
            continue;
        }
        for parent in node.parents() {
            if let Some(count) = children.get_mut(parent) {
                *count += 1;
            }
        }
    }

    let mut tips = children
        .iter()
        .filter_map(|(index, count)| (*count == 0).then_some(*index))
        .collect::<Vec<_>>();
    tips.sort_by(|left, right| compare_nodes(graph, *left, *right));

    fn visit(
        graph: &Graph,
        index: NodeIndex,
        children: &mut BTreeMap<NodeIndex, usize>,
        out: &mut Vec<NodeIndex>,
    ) {
        if children[&index] != 0 || out.contains(&index) {
            return;
        }
        out.push(index);
        for parent in graph.nodes()[index].parents() {
            if let Some(count) = children.get_mut(parent) {
                *count -= 1;
            }
        }
        for parent in graph.nodes()[index].parents() {
            if children.contains_key(parent) {
                visit(graph, *parent, children, out);
            }
        }
    }

    let mut out = Vec::with_capacity(children.len());
    for tip in tips {
        visit(graph, tip, &mut children, &mut out);
    }
    out
}

fn compare_nodes(graph: &Graph, left: NodeIndex, right: NodeIndex) -> Ordering {
    match (graph.nodes()[left].kind(), graph.nodes()[right].kind()) {
        (NodeKind::Commit { id: left }, NodeKind::Commit { id: right }) => left.cmp(right),
        (NodeKind::Reference(left), NodeKind::Reference(right)) => {
            left.ref_info.ref_name.cmp(&right.ref_info.ref_name)
        }
        (NodeKind::Commit { .. }, NodeKind::Reference(_)) => Ordering::Less,
        (NodeKind::Reference(_), NodeKind::Commit { .. }) => Ordering::Greater,
        (NodeKind::Boundary { .. }, _) | (_, NodeKind::Boundary { .. }) => {
            unreachable!("shallow points are not ordered")
        }
        (NodeKind::None, _) | (_, NodeKind::None) => {
            unreachable!("placeholders are not ordered")
        }
    }
}

use std::collections::{BTreeMap, BTreeSet};

use but_graph::{Commit, Graph, Segment, SegmentIndex, SegmentMetadata};
use renderdag::{Ancestor, GraphRowRenderer, Renderer as _};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum Node {
    Commit(gix::ObjectId),
    Reference(gix::refs::FullName),
}

struct RenderNode {
    glyph: &'static str,
    label: String,
    parents: Vec<Node>,
}

/// Render the commit and reference DAG without exposing graph segments.
pub fn graph_dag(graph: &Graph) -> String {
    let commit_ids: BTreeSet<_> = graph
        .segments()
        .flat_map(|sidx| graph[sidx].commits.iter().map(|commit| commit.id))
        .collect();
    let entrypoint = graph.entrypoint().ok();
    let entrypoint_segment_id = entrypoint.as_ref().map(|entrypoint| entrypoint.segment.id);
    let detached_entrypoint_id = entrypoint
        .as_ref()
        .filter(|entrypoint| entrypoint.segment.ref_info.is_none())
        .and_then(|entrypoint| entrypoint.commit())
        .map(|commit| commit.id);

    let mut nodes = BTreeMap::new();
    for sidx in graph.segments() {
        let segment = &graph[sidx];
        for commit in &segment.commits {
            let parents = commit
                .parent_ids
                .iter()
                .filter(|id| commit_ids.contains(*id))
                .copied()
                .map(Node::Commit)
                .collect();
            nodes.insert(
                Node::Commit(commit.id),
                RenderNode {
                    glyph: "●",
                    label: commit_label(
                        graph,
                        commit,
                        detached_entrypoint_id == Some(commit.id),
                        if segment.commits.last() == Some(commit) {
                            graph.stop_condition(sidx)
                        } else {
                            None
                        },
                    ),
                    parents,
                },
            );
        }

        let Some(ref_info) = segment.ref_info.as_ref() else {
            continue;
        };
        let mut parents = segment
            .commits
            .first()
            .map(|commit| vec![Node::Commit(commit.id)])
            .unwrap_or_else(|| {
                graph
                    .segments_below_in_order(sidx)
                    .flat_map(|(_, child)| segment_entry_nodes(graph, child))
                    .collect()
            });
        if parents.is_empty()
            && let Some(target) = ref_info.commit_id.filter(|id| commit_ids.contains(id))
        {
            parents.push(Node::Commit(target));
        }
        deduplicate(&mut parents);
        nodes.insert(
            Node::Reference(ref_info.ref_name.clone()),
            RenderNode {
                glyph: "◎",
                label: reference_label(graph, segment, entrypoint_segment_id == Some(segment.id)),
                parents,
            },
        );
    }

    for sidx in graph.segments() {
        for commit in &graph[sidx].commits {
            for ref_info in &commit.refs {
                let reference = Node::Reference(ref_info.ref_name.clone());
                nodes
                    .entry(reference)
                    .and_modify(|node| {
                        if node.parents.is_empty() {
                            node.parents.push(Node::Commit(commit.id));
                        }
                    })
                    .or_insert_with(|| RenderNode {
                        glyph: "◎",
                        label: graph.ref_debug_string_with_graph_context(
                            ref_info.ref_name.as_ref(),
                            ref_info.worktree.as_ref(),
                        ),
                        parents: vec![Node::Commit(commit.id)],
                    });
            }
        }
    }

    for sidx in graph.segments() {
        for (source_commit, child) in graph.segments_below_in_order(sidx) {
            let Some(source_commit) = source_commit else {
                continue;
            };
            let child_nodes = segment_entry_nodes(graph, child);
            let child_commit = graph.tip_skip_empty(child).map(|commit| commit.id);
            replace_commit_parent(
                &mut nodes
                    .get_mut(&Node::Commit(graph[sidx].commits[source_commit].id))
                    .expect("source commit node")
                    .parents,
                child_commit,
                child_nodes,
            );
        }
    }

    if nodes.is_empty() {
        return "<UNBORN>".into();
    }

    let mut renderer = GraphRowRenderer::<Node>::new()
        .output()
        .with_min_row_height(1)
        .build_box_drawing();
    let mut out = String::new();
    for node in topological_order(&nodes) {
        let rendered = &nodes[&node];
        out.push_str(
            &renderer.next_row(
                node,
                rendered
                    .parents
                    .iter()
                    .cloned()
                    .map(Ancestor::Parent)
                    .collect(),
                rendered.glyph.into(),
                rendered.label.clone(),
            ),
        );
    }
    out.trim_end().into()
}

fn commit_label(
    graph: &Graph,
    commit: &Commit,
    is_entrypoint: bool,
    stop_condition: Option<but_graph::StopCondition>,
) -> String {
    let mut commit = commit.clone();
    commit.refs.clear();
    graph.commit_debug_string_with_graph_context(
        &commit,
        is_entrypoint,
        stop_condition,
        graph.hard_limit_hit(),
        graph.max_goals(),
    )
}

fn reference_label(graph: &Graph, segment: &Segment, is_entrypoint: bool) -> String {
    let ref_info = segment.ref_info.as_ref().expect("reference segment");
    let metadata = match &segment.metadata {
        Some(SegmentMetadata::Workspace(_)) => "📕",
        Some(SegmentMetadata::Branch(_)) => "📙",
        None => "",
    };
    let remote = segment
        .remote_tracking_ref_name
        .as_ref()
        .map(|name| format!(" <> {}", Graph::ref_debug_string(name.as_ref(), None)))
        .unwrap_or_default();
    format!(
        "{}{metadata}{}{remote}",
        if is_entrypoint { "👉" } else { "" },
        graph.ref_debug_string_with_graph_context(
            ref_info.ref_name.as_ref(),
            ref_info.worktree.as_ref(),
        )
    )
}

fn segment_entry_nodes(graph: &Graph, sidx: SegmentIndex) -> Vec<Node> {
    fn recurse(graph: &Graph, sidx: SegmentIndex, seen: &mut BTreeSet<SegmentIndex>) -> Vec<Node> {
        if !seen.insert(sidx) {
            return Vec::new();
        }
        let segment = &graph[sidx];
        let nodes = if let Some(ref_info) = segment.ref_info.as_ref() {
            vec![Node::Reference(ref_info.ref_name.clone())]
        } else if let Some(commit) = segment.commits.first() {
            vec![Node::Commit(commit.id)]
        } else {
            graph
                .segments_below_in_order(sidx)
                .flat_map(|(_, child)| recurse(graph, child, seen))
                .collect()
        };
        seen.remove(&sidx);
        nodes
    }

    let mut nodes = recurse(graph, sidx, &mut BTreeSet::new());
    deduplicate(&mut nodes);
    nodes
}

fn replace_commit_parent(
    parents: &mut Vec<Node>,
    child_commit: Option<gix::ObjectId>,
    child_nodes: Vec<Node>,
) {
    if child_nodes.is_empty() {
        return;
    }
    if let Some(position) = child_commit.and_then(|child_commit| {
        parents
            .iter()
            .position(|node| *node == Node::Commit(child_commit))
    }) {
        parents.splice(position..=position, child_nodes);
    } else if child_commit.is_none() {
        parents.extend(child_nodes);
    }
    deduplicate(parents);
}

fn deduplicate(nodes: &mut Vec<Node>) {
    let mut seen = BTreeSet::new();
    nodes.retain(|node| seen.insert(node.clone()));
}

fn topological_order(nodes: &BTreeMap<Node, RenderNode>) -> Vec<Node> {
    let mut children = nodes
        .keys()
        .cloned()
        .map(|node| (node, 0usize))
        .collect::<BTreeMap<_, _>>();
    for rendered in nodes.values() {
        for parent in &rendered.parents {
            if let Some(count) = children.get_mut(parent) {
                *count += 1;
            }
        }
    }

    fn visit(
        node: Node,
        nodes: &BTreeMap<Node, RenderNode>,
        children: &mut BTreeMap<Node, usize>,
        seen: &mut BTreeSet<Node>,
        out: &mut Vec<Node>,
    ) {
        if children[&node] != 0 || !seen.insert(node.clone()) {
            return;
        }
        out.push(node.clone());
        for parent in &nodes[&node].parents {
            children
                .entry(parent.clone())
                .and_modify(|count| *count -= 1);
        }
        for parent in &nodes[&node].parents {
            visit(parent.clone(), nodes, children, seen, out);
        }
    }

    let tips = children
        .iter()
        .filter_map(|(node, children)| (*children == 0).then_some(node.clone()))
        .collect::<Vec<_>>();
    let mut out = Vec::with_capacity(nodes.len());
    let mut seen = BTreeSet::new();
    for tip in tips {
        visit(tip, nodes, &mut children, &mut seen, &mut out);
    }
    out
}

use std::collections::{BTreeSet, HashMap};

use anyhow::Result;
use bstr::BString;

type NodeIndex = usize;

#[derive(Debug)]
enum NodeWeight {
    Pick { oid: gix::ObjectId },
    Ref { full_name: BString },
    ShallowPoint { oid: gix::ObjectId },
}

#[derive(Debug)]
struct Node {
    weight: NodeWeight,
    parents: Vec<NodeIndex>,
}

#[derive(Debug)]
pub struct WorkspaceGraph {
    nodes: Vec<Node>,
}

pub fn graph(
    repo: &gix::Repository,
    head: gix::ObjectId,
    target: Option<gix::ObjectId>,
) -> Result<WorkspaceGraph> {
    let mut oid_to_ref_full_names = HashMap::<gix::ObjectId, BTreeSet<BString>>::new();
    for reference in repo
        .references()?
        .local_branches()?
        .chain(repo.references()?.remote_branches()?)
    {
        let reference = reference.map_err(|err| anyhow::anyhow!(err))?;
        if let Some(id) = reference.try_id() {
            oid_to_ref_full_names
                .entry(id.detach())
                .or_default()
                .insert(reference.name().as_bstr().to_owned());
        }
    }

    let mut heads = vec![head];
    for reference in repo.references()?.remote_branches()? {
        let mut reference = reference.map_err(|err| anyhow::anyhow!(err))?;
        heads.push(reference.peel_to_commit()?.id);
    }
    let mut platform = repo.rev_walk(heads);
    if target.is_some() {
        platform = platform.with_hidden(target);
    }

    let mut nodes = Vec::<Node>::new();
    let mut oid_to_node_index = HashMap::<gix::ObjectId, NodeIndex>::new();
    for commit in platform.all()? {
        let commit = commit?;
        let mut node_index_option = oid_to_node_index.get(&commit.id);
        if let Some(ref_full_names) = oid_to_ref_full_names.remove(&commit.id) {
            for full_name in ref_full_names.into_iter() {
                if let Some(node_index) = node_index_option {
                    let node = Node {
                        weight: NodeWeight::Ref { full_name },
                        // nodes[nodes.len()] will exist either in a subsequent
                        // iteration of this `for` loop or when a node is created
                        // for the commit itself later.
                        parents: vec![nodes.len()],
                    };
                    nodes[*node_index] = node;
                } else {
                    let node = Node {
                        weight: NodeWeight::Ref { full_name },
                        // As above, but leave one more space for this node
                        // itself.
                        parents: vec![nodes.len() + 1],
                    };
                    nodes.push(node);
                }
                node_index_option = None;
            }
        }
        let node = Node {
            weight: NodeWeight::Pick { oid: commit.id },
            parents: Vec::with_capacity(commit.parent_ids.len()),
        };
        let node_index = if let Some(node_index) = node_index_option {
            nodes[*node_index] = node;
            *node_index
        } else {
            nodes.push(node);
            oid_to_node_index.insert(commit.id, nodes.len() - 1);
            nodes.len() - 1
        };
        for parent_id in commit.parent_ids {
            if let Some(parent_node_index) = oid_to_node_index.get(&parent_id) {
                nodes[node_index].parents.push(*parent_node_index);
            } else {
                nodes.push(Node {
                    weight: NodeWeight::ShallowPoint { oid: parent_id },
                    parents: Vec::new(),
                });
                let new_node_index = nodes.len() - 1;
                oid_to_node_index.insert(parent_id, new_node_index);
                nodes[node_index].parents.push(new_node_index);
            }
        }
    }

    Ok(WorkspaceGraph { nodes })
}

struct PredecessorSegmentInfo {
    segment_indexes: Vec<crate::SegmentIndex>,
    must_start_new_segment: bool,
    not_remote: bool,
}

fn new_predecessor_segment_info(not_remote: bool) -> PredecessorSegmentInfo {
    PredecessorSegmentInfo {
        segment_indexes: Vec::new(),
        must_start_new_segment: false,
        not_remote,
    }
}

impl WorkspaceGraph {
    pub fn to_segment_graph(
        &self,
        _repo: &gix::Repository,
        overlay_repo: &crate::init::OverlayRepo,
        overlay_meta: &crate::init::OverlayMetadata<'_, impl but_core::RefMetadata>,
    ) -> Result<crate::Graph> {
        eprintln!("graph {:?}", self);
        let mut inner_graph = crate::init::PetGraph::default();
        let mut node_index_to_predecessor_segment_info =
            HashMap::<NodeIndex, PredecessorSegmentInfo>::new();
        let mut entrypoint = None;
        let worktree_by_branch = overlay_repo.worktree_branches(None)?;
        for (i, node) in self.nodes.iter().enumerate() {
            let predecessor_segment_info = node_index_to_predecessor_segment_info
                .remove(&i)
                .unwrap_or_else(|| {
                    new_predecessor_segment_info(match &node.weight {
                        NodeWeight::Ref { full_name }
                            if full_name.starts_with(b"refs/remotes/") =>
                        {
                            false
                        }
                        _ => true,
                    })
                });
            let segment_index = if !predecessor_segment_info.must_start_new_segment
                && let [segment_index] = predecessor_segment_info.segment_indexes[..]
                && (!matches!(node.weight, NodeWeight::Ref { .. })
                    || !inner_graph[segment_index].ref_info.is_some())
            {
                segment_index
            } else {
                let segment_index = inner_graph.add_node(crate::Segment::default());
                for (i, predecessor_segment_index) in
                    predecessor_segment_info.segment_indexes.iter().enumerate()
                {
                    let predecessor_segment = &inner_graph[*predecessor_segment_index];
                    let src = predecessor_segment.last_commit_index();
                    inner_graph.add_edge(
                        *predecessor_segment_index,
                        segment_index,
                        crate::Edge {
                            src,
                            src_id: predecessor_segment.commit_id_by_index(src),
                            dst: None,
                            dst_id: None,
                            parent_order: i as u32,
                        },
                    );
                }
                segment_index
            };
            match &node.weight {
                NodeWeight::Pick { oid } => {
                    inner_graph[segment_index].commits.push(crate::Commit {
                        id: *oid,
                        parent_ids: Vec::new(),
                        flags: if predecessor_segment_info.not_remote {
                            crate::CommitFlags::NotInRemote
                        } else {
                            Default::default()
                        },
                        refs: Vec::new(),
                    });
                    if entrypoint.is_none() {
                        entrypoint = Some((segment_index, crate::EntryPointCommit::AtCommit(*oid)));
                    }
                    if inner_graph[segment_index].commits.len() == 1 {
                        let mut walker = inner_graph
                            .neighbors_directed(segment_index, petgraph::Direction::Incoming)
                            .detach();
                        while let Some((edge_index, node_index)) = walker.next(&inner_graph) {
                            let edge = &mut inner_graph[edge_index];
                            edge.dst = Some(0);
                            edge.dst_id = Some(*oid);
                        }
                    }
                }
                NodeWeight::Ref { full_name } => {
                    let ref_name = gix::refs::FullName::try_from(full_name.clone())?;
                    inner_graph[segment_index].metadata =
                        crate::init::walk::extract_local_branch_metadata(
                            ref_name.as_ref(),
                            overlay_meta,
                        )?;
                    inner_graph[segment_index].ref_info = Some(crate::RefInfo::from_ref(
                        ref_name,
                        None,
                        &worktree_by_branch,
                    ));
                }
                NodeWeight::ShallowPoint { .. } => {
                    // do nothing
                }
            }
            for parent in &node.parents {
                let parent_predecessor_segment_info = node_index_to_predecessor_segment_info
                    .entry(*parent)
                    .and_modify(|e| {
                        e.not_remote |= predecessor_segment_info.not_remote;
                    })
                    .or_insert_with(|| {
                        new_predecessor_segment_info(predecessor_segment_info.not_remote)
                    });
                parent_predecessor_segment_info
                    .segment_indexes
                    .push(segment_index);
                if node.parents.len() > 1 {
                    parent_predecessor_segment_info.must_start_new_segment = true;
                }
            }
        }
        Ok(crate::Graph {
            inner: inner_graph,
            entrypoint,
            entrypoint_ref: None,
            traversal_tips: Vec::new(),
            ad_hoc_branch_stack_orders: Vec::new(),
            hard_limit_hit: false,
            options: Default::default(),
            project_meta: Default::default(),
            symbolic_remote_names: Vec::new(),
        })
    }
}

// graph_workspace(head, target) -> (GraphWorkspace, Vec<UiHint>)

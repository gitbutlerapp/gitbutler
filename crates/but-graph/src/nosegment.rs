use std::collections::{BTreeSet, HashMap};

use anyhow::Result;
use gix::refs::FullName;

use crate::{CommitFlags, SegmentMetadata};

type NodeIndex = usize;

#[derive(Debug)]
enum NodeWeight {
    Pick { oid: gix::ObjectId },
    Ref { full_name: FullName },
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
    let mut oid_to_ref_full_names = HashMap::<gix::ObjectId, BTreeSet<FullName>>::new();
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
                .insert(reference.name().to_owned());
        }
    }

    let mut heads = vec![head];
    for reference in repo.references()?.remote_branches()? {
        let mut reference = reference.map_err(|err| anyhow::anyhow!(err))?;
        heads.push(reference.peel_to_commit()?.id);
    }
    let commit_ids = crate::topowalk::walk(repo, heads, target)?;

    let mut nodes = Vec::<Node>::new();
    struct UnassignedOidInfo {
        node_index: NodeIndex,
        parent_index: usize,
    }
    let mut unassigned_oid_to_infos = HashMap::<gix::ObjectId, Vec<UnassignedOidInfo>>::new();
    for commit_id in commit_ids {
        let commit = repo.find_commit(commit_id)?;
        let first_node_index = nodes.len();
        if let Some(ref_full_names) = oid_to_ref_full_names.remove(&commit.id) {
            for full_name in ref_full_names.into_iter() {
                let node = Node {
                    weight: NodeWeight::Ref { full_name },
                    // The node's parent (either another ref, to be inserted
                    // by this `for` loop, or the commit referred to, to be
                    // inserted below) goes into nodes.len()+1.
                    parents: vec![nodes.len() + 1],
                };
                // The node itself goes into nodes.len().
                nodes.push(node);
            }
        }
        let node = Node {
            weight: NodeWeight::Pick { oid: commit.id },
            parents: vec![0; commit.parent_ids().count()],
        };
        nodes.push(node);
        for (parent_index, parent_id) in commit.parent_ids().enumerate() {
            unassigned_oid_to_infos
                .entry(parent_id.detach())
                .or_default()
                .push(UnassignedOidInfo {
                    node_index: nodes.len() - 1,
                    parent_index,
                });
        }

        if let Some(infos) = unassigned_oid_to_infos.remove(&commit.id) {
            for info in infos {
                nodes[info.node_index].parents[info.parent_index] = first_node_index;
            }
        }
    }
    for (oid, infos) in unassigned_oid_to_infos {
        nodes.push(Node {
            weight: NodeWeight::ShallowPoint { oid },
            parents: Vec::new(),
        });
        for info in infos {
            nodes[info.node_index].parents[info.parent_index] = nodes.len() - 1;
        }
    }

    Ok(WorkspaceGraph { nodes })
}

#[derive(Default)]
struct PredecessorSegmentInfo {
    segment_indexes: Vec<crate::SegmentIndex>,
    must_start_new_segment: bool,
    commit_flags: CommitFlags,
}

impl WorkspaceGraph {
    pub fn to_segment_graph(
        &self,
        repo: &gix::Repository,
        overlay_repo: &crate::init::OverlayRepo,
        overlay_meta: &crate::init::OverlayMetadata<'_, impl but_core::RefMetadata>,
        project_meta: but_core::ref_metadata::ProjectMeta,
    ) -> Result<crate::Graph> {
        let mut inner_graph = crate::init::PetGraph::default();
        let mut node_index_to_predecessor_segment_info =
            HashMap::<NodeIndex, PredecessorSegmentInfo>::new();
        let mut entrypoint = None;
        let worktree_by_branch = overlay_repo.worktree_branches(None)?;

        struct LocalBranchInfo {
            segment_index: crate::SegmentIndex,
            full_name: gix::refs::FullName,
        }
        let mut local_branch_infos = Vec::new();
        let mut remote_tracking_branch_name_to_segment_index = HashMap::new();

        for (i, node) in self.nodes.iter().enumerate() {
            let mut predecessor_segment_info = node_index_to_predecessor_segment_info
                .remove(&i)
                .unwrap_or_default();
            if let NodeWeight::Ref { full_name } = &node.weight {
                if !full_name.as_bstr().starts_with(b"refs/remotes/") {
                    predecessor_segment_info
                        .commit_flags
                        .insert(CommitFlags::NotInRemote);
                }
                if full_name.as_bstr() == b"refs/heads/gitbutler/workspace" {
                    predecessor_segment_info
                        .commit_flags
                        .insert(CommitFlags::InWorkspace);
                }
                if let Some(ref target_ref) = project_meta.target_ref
                    && full_name == target_ref
                {
                    predecessor_segment_info
                        .commit_flags
                        .insert(CommitFlags::Integrated);
                }
            }
            let segment_index = if !predecessor_segment_info.must_start_new_segment
                && let [segment_index] = predecessor_segment_info.segment_indexes[..]
                && (!matches!(node.weight, NodeWeight::Ref { .. })
                    || !inner_graph[segment_index].ref_info.is_some())
            {
                segment_index
            } else {
                // crates/but-graph/tests/graph/init/with_workspace.rs
                let metadata: Option<crate::SegmentMetadata> =
                    if let NodeWeight::Ref { full_name } = &node.weight {
                        if full_name.as_bstr() == b"refs/heads/gitbutler/workspace" {
                            overlay_meta
                                .workspace_opt(full_name.as_ref())?
                                .map(SegmentMetadata::Workspace)
                        } else {
                            overlay_meta
                                .branch_opt(full_name.as_ref())?
                                .map(SegmentMetadata::Branch)
                        }
                    } else {
                        None
                    };
                let segment_index = inner_graph.add_node(crate::Segment {
                    metadata,
                    ..Default::default()
                });
                inner_graph[segment_index].id = segment_index;
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
                NodeWeight::Pick { oid } | NodeWeight::ShallowPoint { oid } => {
                    let parent_ids: Vec<_> = if matches!(&node.weight, NodeWeight::Pick { .. }) {
                        repo.find_commit(*oid)?
                            .parent_ids()
                            .map(|id| id.detach())
                            .collect()
                    } else {
                        Vec::new()
                    };
                    inner_graph[segment_index].commits.push(crate::Commit {
                        id: *oid,
                        parent_ids,
                        flags: predecessor_segment_info.commit_flags,
                        refs: Vec::new(),
                    });
                    if entrypoint.is_none() {
                        entrypoint = Some((segment_index, crate::EntryPointCommit::AtCommit(*oid)));
                    }
                    if inner_graph[segment_index].commits.len() == 1 {
                        let mut walker = inner_graph
                            .neighbors_directed(segment_index, petgraph::Direction::Incoming)
                            .detach();
                        while let Some(edge_index) = walker.next_edge(&inner_graph) {
                            let edge = &mut inner_graph[edge_index];
                            edge.dst = Some(0);
                            edge.dst_id = Some(*oid);
                        }
                    }
                }
                NodeWeight::Ref { full_name } => {
                    let ref_name = gix::refs::FullName::try_from(full_name.clone())?;
                    if full_name.as_bstr().starts_with(b"refs/heads/") {
                        local_branch_infos.push(LocalBranchInfo {
                            segment_index,
                            full_name: ref_name.clone(),
                        });
                    } else if full_name.as_bstr().starts_with(b"refs/remotes/") {
                        remote_tracking_branch_name_to_segment_index
                            .insert(ref_name.clone(), segment_index);
                    }
                    inner_graph[segment_index].metadata =
                        crate::init::walk::extract_local_branch_metadata(
                            ref_name.as_ref(),
                            overlay_meta,
                        )?;
                    let commit_id = repo.find_reference(&ref_name)?.peel_to_commit()?.id;
                    inner_graph[segment_index].ref_info = Some(crate::RefInfo::from_ref(
                        ref_name,
                        Some(commit_id),
                        &worktree_by_branch,
                    ));
                }
            }
            for parent in &node.parents {
                let parent_predecessor_segment_info = node_index_to_predecessor_segment_info
                    .entry(*parent)
                    .or_default();
                parent_predecessor_segment_info
                    .segment_indexes
                    .push(segment_index);
                if node.parents.len() > 1 {
                    parent_predecessor_segment_info.must_start_new_segment = true;
                }
                parent_predecessor_segment_info
                    .commit_flags
                    .insert(predecessor_segment_info.commit_flags);
            }
        }

        for local_branch_info in local_branch_infos {
            if let Some(remote_tracking_branch_name) =
                crate::init::remotes::lookup_remote_tracking_branch(
                    overlay_repo,
                    local_branch_info.full_name.as_ref(),
                )?
            {
                if let Some(remote_tracking_segment_index) =
                    remote_tracking_branch_name_to_segment_index
                        .remove(remote_tracking_branch_name.as_ref())
                {
                    let segment = &mut inner_graph[local_branch_info.segment_index];
                    segment.remote_tracking_branch_segment_id = Some(remote_tracking_segment_index);
                    segment.remote_tracking_ref_name = Some(remote_tracking_branch_name);
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
            project_meta,
            symbolic_remote_names: Vec::new(),
        })
    }
}

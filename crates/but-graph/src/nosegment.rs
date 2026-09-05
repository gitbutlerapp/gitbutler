use std::collections::{BTreeSet, HashMap};

use anyhow::Result;
use bstr::BString;
use but_core::RefMetadata;
use gix::refs::{FullName, FullNameRef};

use crate::{CommitFlags, SegmentMetadata, init::Overlay};

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

const PLACEHOLDER: usize = usize::MAX;

fn last_segment(full_name: &FullNameRef) -> BString {
    let Some(last_slash_position) = full_name.as_bstr().iter().rposition(|c| *c == b'/') else {
        return full_name.as_bstr().to_owned();
    };
    full_name.as_bstr()[(last_slash_position + 1)..].to_owned()
}

pub fn graph(
    repo: &gix::Repository,
    meta: &impl RefMetadata,
    mut heads: Vec<gix::ObjectId>,
    target: Option<gix::ObjectId>,
) -> Result<WorkspaceGraph> {
    let (_overlay_repo, overlay_meta, _entrypoint) = Overlay::default().into_parts(repo, meta);
    let segment_metadata = overlay_meta
        .workspace_opt(FullName::try_from("refs/heads/gitbutler/workspace")?.as_ref())?;
    let mut stacks = Vec::<Vec<FullName>>::new();
    if let Some(segment_metadata) = segment_metadata {
        for stack in segment_metadata.stacks {
            stacks.push(
                stack
                    .branches
                    .into_iter()
                    .map(|branch| branch.ref_name)
                    .collect(),
            );
        }
    }
    let stack_of_branch = |branch: &FullNameRef| -> Vec<FullName> {
        let branch = branch.to_owned();
        for stack in &stacks {
            if stack.contains(&branch) {
                return stack.clone();
            }
        }
        Vec::new()
    };

    let mut oid_to_refs = HashMap::<gix::ObjectId, Vec<FullName>>::new();
    for reference in repo
        .references()?
        .local_branches()?
        .chain(repo.references()?.remote_branches()?)
    {
        let reference = reference.map_err(|err| anyhow::anyhow!(err))?;
        if let Some(id) = reference.try_id() {
            oid_to_refs
                .entry(id.detach())
                .or_default()
                .push(reference.name().to_owned());
        }
    }
    for refs in oid_to_refs.values_mut() {
        refs.sort_by_cached_key(|reference| {
            if reference.as_bstr() == b"refs/heads/main" {
                return usize::MAX;
            } else if reference.as_bstr() == b"refs/heads/gitbutler/workspace" {
                return usize::MAX - 1;
            }
            stack_of_branch(reference.as_ref())
                .iter()
                .position(|r| r == reference)
                .unwrap_or(0usize)
        });
    }

    for reference in repo.references()?.remote_branches()? {
        let mut reference = reference.map_err(|err| anyhow::anyhow!(err))?;
        heads.push(reference.peel_to_commit()?.id);
    }
    for stack in &stacks {
        for full_name in stack {
            if let Some(mut reference) = repo.try_find_reference(full_name)? {
                heads.push(reference.peel_to_commit()?.id);
            }
        }
    }
    let commit_ids = dbg!(crate::topowalk::walk(repo, heads, target)?);

    /// The children of each commit can be partitioned by stack lineages. The
    /// stack lineage determines whether a child points to a branch or directly
    /// to the commit itself. For example, suppose a commit C has a child A of
    /// stack lineage P,Q and a child B of stack lineage R,S. If branch P points
    /// to C, the resulting graph will be as follows:
    ///
    /// A
    /// |
    /// P   B
    ///  \ /
    ///   C
    ///
    /// Notice that A points to the branch P, but B points directly to C.
    ///
    /// (The stacks are not stored directly in this data structure; only their
    /// branches are.)
    #[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
    struct StackLineage {
        branches_in_lineage: BTreeSet<gix::refs::FullName>,
        /// This commit is the parent of the workspace commit through this stack
        /// lineage. For each commit, at most one [StackLineage] can have this
        /// field set.
        is_parent_of_workspace_commit: bool,
    }
    /// An element of [Node::parents] that must be updated when an unassigned
    /// OID becomes assigned to a node.
    #[derive(Debug)]
    struct ToRepoint {
        node_index: NodeIndex,
        parent_index: usize,
    }
    // Data regarding known OIDs that have not yet been assigned a `NodeIndex`.
    // Each OID is known because we have parsed at least one of its children.
    let mut unassigned_oid_map =
        HashMap::<gix::ObjectId, HashMap<StackLineage, Vec<ToRepoint>>>::new();

    let mut nodes = Vec::<Node>::new();

    for commit_id in commit_ids {
        let commit = repo.find_commit(commit_id)?;
        let mut is_workspace_commit = false;

        for full_name in oid_to_refs.remove(&commit_id).unwrap_or_default() {
            dbg!(&full_name);
            dbg!(&unassigned_oid_map);
            if full_name.as_bstr() == b"refs/heads/gitbutler/workspace" {
                is_workspace_commit = true;
            }

            let mut new_stack_lineage = StackLineage::default();
            let mut new_map = HashMap::<StackLineage, Vec<ToRepoint>>::new();
            let node_index = nodes.len();
            for (stack_lineage, to_repoints) in
                unassigned_oid_map.remove(&commit_id).unwrap_or_default()
            {
                let in_lineage = full_name.as_bstr() == b"refs/heads/main"
                    || full_name.as_bstr() == b"refs/heads/gitbutler/workspace"
                    || stack_lineage.is_parent_of_workspace_commit
                    || stack_lineage.branches_in_lineage.contains(&full_name);
                if in_lineage {
                    new_stack_lineage
                        .branches_in_lineage
                        .extend(stack_lineage.branches_in_lineage);
                    new_stack_lineage.is_parent_of_workspace_commit |=
                        stack_lineage.is_parent_of_workspace_commit;
                    for to_repoint in to_repoints {
                        nodes[to_repoint.node_index].parents[to_repoint.parent_index] = node_index;
                    }
                } else {
                    new_map.insert(stack_lineage, to_repoints);
                }
            }
            if full_name.as_bstr().starts_with(b"refs/remotes/") {
                new_stack_lineage
                    .branches_in_lineage
                    .insert(FullName::try_from(format!(
                        "refs/heads/{}",
                        last_segment(full_name.as_ref())
                    ))?);
            } else {
                new_stack_lineage
                    .branches_in_lineage
                    .extend(stack_of_branch(full_name.as_ref()));
            }
            new_map
                .entry(new_stack_lineage)
                .or_default()
                .push(ToRepoint {
                    node_index,
                    parent_index: 0,
                });
            unassigned_oid_map.insert(commit_id, new_map);

            nodes.push(Node {
                weight: NodeWeight::Ref { full_name },
                parents: vec![PLACEHOLDER],
            });
        }

        dbg!(&commit_id);
        dbg!(&unassigned_oid_map);
        let mut branches_in_lineage = BTreeSet::<gix::refs::FullName>::new();
        let node_index = nodes.len();
        for (stack_lineage, to_repoints) in
            unassigned_oid_map.remove(&commit_id).unwrap_or_default()
        {
            branches_in_lineage.extend(stack_lineage.branches_in_lineage);
            for to_repoint in to_repoints {
                nodes[to_repoint.node_index].parents[to_repoint.parent_index] = node_index;
            }
        }
        nodes.push(Node {
            weight: NodeWeight::Pick { oid: commit_id },
            parents: vec![PLACEHOLDER; commit.parent_ids().count()],
        });

        let stack_lineage = StackLineage {
            branches_in_lineage,
            is_parent_of_workspace_commit: is_workspace_commit,
        };
        for (parent_index, parent_id) in commit.parent_ids().enumerate() {
            unassigned_oid_map
                .entry(parent_id.detach())
                .or_default()
                .entry(stack_lineage.clone())
                .or_default()
                .push(ToRepoint {
                    node_index: nodes.len() - 1,
                    parent_index,
                });
        }
    }
    for (oid, stack_lineage_to_to_repoints) in unassigned_oid_map {
        let node_index = nodes.len();
        for (_, to_repoints) in stack_lineage_to_to_repoints {
            for to_repoint in to_repoints {
                nodes[to_repoint.node_index].parents[to_repoint.parent_index] = node_index;
            }
        }
        nodes.push(Node {
            weight: NodeWeight::ShallowPoint { oid },
            parents: Vec::new(),
        });
    }

    Ok(WorkspaceGraph { nodes })
}

impl WorkspaceGraph {
    #[allow(unused)]
    fn dot_graph(&self) -> String {
        let mut out = "digraph {\nnode [shape=\"rectangle\"]".to_string();
        for (i, node) in self.nodes.iter().enumerate() {
            out.push_str(&format!(
                " n{} [label=\"{} {}\"]\n",
                i,
                i,
                match &node.weight {
                    NodeWeight::Pick { oid } => format!("pick {}", oid.to_hex_with_len(7)),
                    NodeWeight::Ref { full_name } => format!(
                        "ref {}",
                        full_name
                            .to_string()
                            .strip_prefix("refs/heads/")
                            .unwrap_or(&full_name.to_string()),
                    ),
                    NodeWeight::ShallowPoint { oid } =>
                        format!("shallow {}", oid.to_hex_with_len(7)),
                }
            ));
        }
        for (i, node) in self.nodes.iter().enumerate() {
            for j in &node.parents {
                out.push_str(&format!(" n{} -> n{}\n", i, j));
            }
        }
        out.push_str("}\n");
        out
    }

    #[allow(unused)]
    pub fn open_as_svg(&self) {
        use bstr::ByteSlice as _;
        use std::{io::Write, process::Stdio, sync::atomic::AtomicUsize};

        static SUFFIX: AtomicUsize = AtomicUsize::new(0);
        let suffix = SUFFIX.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let svg_name = format!("debug-nosegment-{suffix:02}.svg");
        let svg_path = std::env::var_os("CARGO_MANIFEST_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_default()
            .join(svg_name);
        let mut dot = std::process::Command::new("dot")
            .args(["-Tsvg", "-o"])
            .arg(&svg_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("'dot' (graphviz) must be installed on the system");
        dot.stdin
            .as_mut()
            .unwrap()
            .write_all(self.dot_graph().as_bytes())
            .ok();
        let mut out = dot.wait_with_output().unwrap();
        out.stdout.extend(out.stderr);
        assert!(
            out.status.success(),
            "dot failed: {out}",
            out = out.stdout.as_bstr()
        );

        assert!(
            std::process::Command::new("xdg-open")
                .arg(&svg_path)
                .status()
                .unwrap()
                .success(),
            "Opening of {svg_path} failed",
            svg_path = svg_path.display()
        );
    }
}

#[derive(Debug, Default)]
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
        // self.open_as_svg();
        let mut inner_graph = crate::init::PetGraph::default();
        let mut node_index_to_predecessor_segment_info =
            HashMap::<NodeIndex, PredecessorSegmentInfo>::new();
        let mut entrypoint = None;
        let worktree_by_branch = overlay_repo.worktree_branches(None)?;

        #[derive(Debug)]
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
                && !matches!(node.weight, NodeWeight::Ref { .. })
            {
                segment_index
            } else {
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
            worktree_tips: Vec::new(),
            project_meta,
            symbolic_remote_names: Vec::new(),
        })
    }
}

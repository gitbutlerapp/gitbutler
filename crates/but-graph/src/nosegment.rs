use std::collections::{BTreeSet, HashMap, VecDeque};

use anyhow::Result;
use but_core::{RefMetadata, is_workspace_ref_name};
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

const UNKNOWN_PARENT_NODE_INDEX: NodeIndex = usize::MAX;

#[expect(clippy::too_many_arguments)]
pub fn graph(
    repo: &gix::Repository,
    overlay_meta: &crate::init::OverlayMetadata<'_, impl RefMetadata>,
    remote_names: &gix::remote::Names,
    worktree_by_branch: &crate::init::walk::WorktreeByBranch,
    head_ref: Option<&FullNameRef>,
    collect_tags: bool,
    mut heads: Vec<gix::ObjectId>,
    target_ref: &FullNameRef,
    target_upstream_ref: Option<&FullNameRef>,
    target_commit_id: Option<gix::ObjectId>,
) -> Result<WorkspaceGraph> {
    let mut workspace_ref_name = None;
    let mut oid_to_refs = HashMap::<gix::ObjectId, Vec<FullName>>::new();
    let mut ref_to_oid = HashMap::<FullName, gix::ObjectId>::new();
    let mut add_reference = |full_name: FullName, id: gix::ObjectId| {
        ref_to_oid.insert(full_name.clone(), id);
        oid_to_refs.entry(id).or_default().push(full_name);
    };
    for reference in repo
        .references()?
        .local_branches()?
        .chain(repo.references()?.remote_branches()?)
    {
        let reference = reference.map_err(|err| anyhow::anyhow!(err))?;
        let Some(id) = reference.try_id() else {
            continue;
        };
        let full_name = reference.name().to_owned();
        // Every remote-tracking branch is a head of the walk.
        if full_name.category() == Some(gix::refs::Category::RemoteBranch) {
            heads.push(id.detach());
        }
        if workspace_ref_name.is_none() && is_workspace_ref_name(full_name.as_ref()) {
            workspace_ref_name = Some(full_name.clone());
        }
        add_reference(full_name, id.detach());
    }
    if collect_tags {
        for reference in repo.references()?.tags()? {
            let mut reference = reference.map_err(|err| anyhow::anyhow!(err))?;
            let Ok(commit) = reference.peel_to_commit() else {
                continue;
            };
            add_reference(reference.name().to_owned(), commit.id);
        }
    }
    // Workspace metadata may outlive the workspace reference, as in
    // single-branch mode, where it no longer says anything about the stacks.
    let segment_metadata = match &workspace_ref_name {
        Some(workspace_ref_name) => overlay_meta.workspace_opt(workspace_ref_name.as_ref())?,
        None => None,
    };
    let mut stacks = Vec::<Vec<FullName>>::new();
    // The stacks reachable from the workspace commit, in metadata order.
    let mut workspace_stacks = Vec::<Vec<FullName>>::new();
    if let Some(segment_metadata) = segment_metadata {
        for stack in segment_metadata.stacks {
            let is_in_workspace = stack.is_in_workspace();
            let branches: Vec<FullName> = stack
                .branches
                .into_iter()
                .map(|branch| branch.ref_name)
                .collect();
            if is_in_workspace {
                workspace_stacks.push(branches.clone());
            }
            stacks.push(branches);
        }
    }
    // A stack whose tip isn't a parent of the managed workspace commit was
    // stacked onto another one in Git, unless it rests on the target commit as
    // an empty lane of its own. Its branches then belong to whichever stack
    // reaches them.
    let mut managed_workspace_commit_id = None;
    if let Some(&workspace_commit_id) = workspace_ref_name
        .as_ref()
        .and_then(|name| ref_to_oid.get(name))
    {
        let workspace_commit = repo.find_commit(workspace_commit_id)?;
        if crate::workspace::commit::is_managed_workspace_by_message(
            workspace_commit.message_raw()?,
        ) {
            managed_workspace_commit_id = Some(workspace_commit_id);
            let parent_ids: Vec<gix::ObjectId> =
                workspace_commit.parent_ids().map(gix::Id::detach).collect();
            let is_stacked_onto_another = |stack: &Vec<FullName>| {
                stack
                    .iter()
                    .find_map(|name| ref_to_oid.get(name))
                    .is_some_and(|tip| !parent_ids.contains(tip) && Some(*tip) != target_commit_id)
            };
            stacks.retain(|stack| {
                !(workspace_stacks.contains(stack) && is_stacked_onto_another(stack))
            });
            workspace_stacks.retain(|stack| !is_stacked_onto_another(stack));
        }
    }
    // Branches outside the workspace, like those of a single-branch-mode
    // checkout, may still be stacked through their persisted branch stack order.
    for full_name in ref_to_oid.keys() {
        if full_name.category() != Some(gix::refs::Category::LocalBranch)
            || stacks.iter().any(|stack| stack.contains(full_name))
        {
            continue;
        }
        if let Some(stack) = overlay_meta.branch_stack_order(full_name.as_ref())? {
            stacks.push(stack);
        }
    }
    /// The branches of the stack containing `branch`, or none if no stack does.
    fn stack_of_branch<'a>(stacks: &'a [Vec<FullName>], branch: &FullNameRef) -> &'a [FullName] {
        stacks
            .iter()
            .find(|stack| stack.iter().any(|name| name.as_ref() == branch))
            .map_or(&[], Vec::as_slice)
    }

    for refs in oid_to_refs.values_mut() {
        // The checked-out branch and the workspace reference sit above the
        // branches they share a commit with, followed by remote-tracking
        // branches, branches in stack order, branches of no stack, and the
        // target branch and then tags below them.
        refs.sort_by_cached_key(|reference| {
            if Some(reference.as_ref()) == head_ref || is_workspace_ref_name(reference.as_ref()) {
                return 0;
            }
            match reference.category() {
                Some(gix::refs::Category::RemoteBranch) => return 1,
                Some(gix::refs::Category::Tag) => return usize::MAX,
                _ if reference.as_ref() == target_ref => return usize::MAX - 1,
                _ => {}
            }
            stack_of_branch(&stacks, reference.as_ref())
                .iter()
                .position(|r| r == reference)
                .map_or(usize::MAX - 2, |position| position + 2)
        });
    }

    heads.extend(
        stacks
            .iter()
            .flatten()
            .filter_map(|name| ref_to_oid.get(name)),
    );
    // History below the target commit is walked only while it leads somewhere.
    let commit_ids = crate::topowalk::walk(repo, heads, target_commit_id)?;

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
        /// lineage, along an edge no stack in the workspace metadata accounts
        /// for. Such an edge chains through all references on the commit,
        /// whichever stack they belong to.
        is_parent_of_workspace_commit: bool,
    }
    /// An element of [Node::parents] that must be updated when an unassigned
    /// OID becomes assigned to a node.
    #[derive(Debug)]
    struct ToRepoint {
        node_index: NodeIndex,
        parent_index: usize,
    }
    /// Points each of `to_repoints` at the node at `node_index`.
    fn repoint(
        nodes: &mut [Node],
        to_repoints: impl IntoIterator<Item = ToRepoint>,
        node_index: NodeIndex,
    ) {
        for to_repoint in to_repoints {
            nodes[to_repoint.node_index].parents[to_repoint.parent_index] = node_index;
        }
    }
    // Data regarding known OIDs that have not yet been assigned a `NodeIndex`.
    // Each OID is known because we have parsed at least one of its children.
    let mut unassigned_oid_map =
        HashMap::<gix::ObjectId, HashMap<StackLineage, Vec<ToRepoint>>>::new();

    let mut nodes = Vec::<Node>::new();

    for commit_id in commit_ids {
        let commit = repo.find_commit(commit_id)?;
        // Branches rest on the target commit when the target branch, or its
        // upstream, points there too.
        let rests_on_target = target_commit_id == Some(commit_id)
            && std::iter::once(target_ref)
                .chain(target_upstream_ref)
                .any(|name| ref_to_oid.get(name) == Some(&commit_id));

        for full_name in oid_to_refs.remove(&commit_id).unwrap_or_default() {
            let category = full_name.category();
            let is_checked_out_elsewhere = worktree_by_branch
                .get(&full_name)
                .is_some_and(|worktrees| worktrees.iter().any(|wt| !wt.owned_by_repo));
            // A tag, or a local branch that no stack claims, belongs to whichever
            // lineage reaches it, like a branch of a stack without metadata.
            // That's unless another worktree has the branch checked out, making
            // it its own lane, or it merely rests on the target commit next to
            // the target branch.
            let is_stackless_branch = match category {
                Some(gix::refs::Category::LocalBranch) => {
                    !full_name.as_bstr().starts_with(b"refs/heads/gitbutler/")
                        && stack_of_branch(&stacks, full_name.as_ref()).is_empty()
                        && !is_checked_out_elsewhere
                }
                Some(gix::refs::Category::Tag) => true,
                _ => false,
            } && !rests_on_target;
            // A branch or tag merely resting on the target commit isn't taken in
            // by an edge that takes in every reference either, and neither is a
            // remote-tracking branch, which is always its own lane.
            let rests_beside = (rests_on_target
                && matches!(
                    category,
                    Some(gix::refs::Category::LocalBranch | gix::refs::Category::Tag)
                )
                && full_name.as_ref() != target_ref)
                || category == Some(gix::refs::Category::RemoteBranch);
            // The target branch checked out in another worktree is a lane of its own, so that
            // it can follow the target there.
            let target_in_other_worktree =
                full_name.as_ref() == target_ref && is_checked_out_elsewhere;
            let mut new_stack_lineage = StackLineage::default();
            let mut new_map = HashMap::<StackLineage, Vec<ToRepoint>>::new();
            let node_index = nodes.len();
            for (stack_lineage, to_repoints) in
                unassigned_oid_map.remove(&commit_id).unwrap_or_default()
            {
                let in_lineage = !target_in_other_worktree
                    && (full_name.as_ref() == target_ref
                        || (stack_lineage.is_parent_of_workspace_commit && !rests_beside)
                        || is_stackless_branch
                        || stack_lineage.branches_in_lineage.contains(&full_name));
                if in_lineage {
                    new_stack_lineage
                        .branches_in_lineage
                        .extend(stack_lineage.branches_in_lineage);
                    new_stack_lineage.is_parent_of_workspace_commit |=
                        stack_lineage.is_parent_of_workspace_commit;
                    repoint(&mut nodes, to_repoints, node_index);
                } else {
                    new_map.insert(stack_lineage, to_repoints);
                }
            }
            // A remote-tracking branch leads through the local branch of the same name.
            if category == Some(gix::refs::Category::RemoteBranch) {
                if let Some((_remote, short_name)) =
                    but_core::extract_remote_name_and_short_name(full_name.as_ref(), remote_names)
                {
                    new_stack_lineage
                        .branches_in_lineage
                        .insert(gix::refs::Category::LocalBranch.to_full_name(&short_name[..])?);
                }
            } else {
                new_stack_lineage
                    .branches_in_lineage
                    .extend(stack_of_branch(&stacks, full_name.as_ref()).iter().cloned());
            }
            // Without a workspace commit, the workspace reference sits on top
            // of the branches on its commit, whichever stack they belong to.
            if is_workspace_ref_name(full_name.as_ref()) {
                new_stack_lineage.is_parent_of_workspace_commit = true;
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
                parents: vec![UNKNOWN_PARENT_NODE_INDEX],
            });
        }

        let mut branches_in_lineage = BTreeSet::<gix::refs::FullName>::new();
        let node_index = nodes.len();
        for (stack_lineage, to_repoints) in
            unassigned_oid_map.remove(&commit_id).unwrap_or_default()
        {
            branches_in_lineage.extend(stack_lineage.branches_in_lineage);
            repoint(&mut nodes, to_repoints, node_index);
        }

        // A managed workspace commit gets one parent edge per stack, in
        // metadata order, each restricted to the branches of its stack. That
        // way stacks resting on the same commit stay apart, even if they share
        // a Git parent. Only tips that are Git parents get an edge, as any
        // other tip would become a redundant parent once the workspace commit
        // is rewritten. The exception is an empty lane resting on the target
        // commit, which the workspace commit is to gain as a parent.
        //
        // The stacks take the places of the parents they cover, in metadata
        // order, while parents no stack covers keep their place. Stacks left
        // over, like those sharing a parent, follow at the end.
        let parent_ids: Vec<gix::ObjectId> = commit.parent_ids().map(gix::Id::detach).collect();
        let mut stack_edges = VecDeque::<(gix::ObjectId, StackLineage)>::new();
        let is_managed_workspace_commit = managed_workspace_commit_id == Some(commit_id);
        if is_managed_workspace_commit {
            for stack in &workspace_stacks {
                let Some(&tip) = stack.iter().find_map(|name| ref_to_oid.get(name)) else {
                    continue;
                };
                if parent_ids.contains(&tip) || Some(tip) == target_commit_id {
                    stack_edges.push_back((
                        tip,
                        StackLineage {
                            branches_in_lineage: stack.iter().cloned().collect(),
                            is_parent_of_workspace_commit: false,
                        },
                    ));
                }
            }
        }
        let stack_lineage = StackLineage {
            branches_in_lineage,
            is_parent_of_workspace_commit: is_managed_workspace_commit,
        };
        let covered_parent_ids: Vec<gix::ObjectId> =
            stack_edges.iter().map(|(tip, _)| *tip).collect();
        let mut parent_edges = Vec::<(gix::ObjectId, StackLineage)>::new();
        for parent_id in parent_ids {
            if !covered_parent_ids.contains(&parent_id) {
                parent_edges.push((parent_id, stack_lineage.clone()));
            } else if let Some(stack_edge) = stack_edges.pop_front() {
                parent_edges.push(stack_edge);
            }
        }
        parent_edges.extend(stack_edges);

        nodes.push(Node {
            weight: NodeWeight::Pick { oid: commit_id },
            parents: vec![UNKNOWN_PARENT_NODE_INDEX; parent_edges.len()],
        });
        for (parent_index, (parent_id, stack_lineage)) in parent_edges.into_iter().enumerate() {
            unassigned_oid_map
                .entry(parent_id)
                .or_default()
                .entry(stack_lineage)
                .or_default()
                .push(ToRepoint {
                    node_index,
                    parent_index,
                });
        }
    }
    for (oid, stack_lineage_to_to_repoints) in unassigned_oid_map {
        let node_index = nodes.len();
        repoint(
            &mut nodes,
            stack_lineage_to_to_repoints.into_values().flatten(),
            node_index,
        );
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
                    NodeWeight::Ref { full_name, .. } => format!("ref {}", full_name.shorten()),
                    NodeWeight::ShallowPoint { oid } =>
                        format!("shallow {}", oid.to_hex_with_len(7)),
                }
            ));
        }
        for (i, node) in self.nodes.iter().enumerate() {
            for j in &node.parents {
                out.push_str(&format!(" n{i} -> n{j}\n"));
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
    /// The segments of the child nodes, each with the position of this node
    /// among that child's parents.
    segment_indexes: Vec<(crate::SegmentIndex, u32)>,
    must_start_new_segment: bool,
    commit_flags: CommitFlags,
}

impl WorkspaceGraph {
    /// The commit node `i` is or, for a reference, points at.
    fn commit_id_of(&self, mut i: NodeIndex) -> gix::ObjectId {
        loop {
            match &self.nodes[i].weight {
                NodeWeight::Pick { oid } | NodeWeight::ShallowPoint { oid } => return *oid,
                NodeWeight::Ref { .. } => i = self.nodes[i].parents[0],
            }
        }
    }

    pub fn to_segment_graph(
        &self,
        repo: &gix::Repository,
        overlay_repo: &crate::init::OverlayRepo,
        overlay_meta: &crate::init::OverlayMetadata<'_, impl RefMetadata>,
        worktree_by_branch: &crate::init::walk::WorktreeByBranch,
        project_meta: but_core::ref_metadata::ProjectMeta,
    ) -> Result<crate::Graph> {
        // self.open_as_svg();
        let mut inner_graph = crate::init::PetGraph::default();
        let mut node_index_to_predecessor_segment_info =
            HashMap::<NodeIndex, PredecessorSegmentInfo>::new();
        let mut entrypoint = None;
        let target_upstream_ref = project_meta.remote_target_ref(repo)?;

        // Each local branch with the segment it names, to be paired with its remote-tracking branch.
        let mut local_branches = Vec::<(crate::SegmentIndex, FullName)>::new();
        // References reached by a segment without a reference of its own, by the
        // node they lead to, to be recorded on the commit they point at.
        let mut pending_commit_refs = HashMap::<NodeIndex, Vec<crate::RefInfo>>::new();
        let mut remote_tracking_branch_name_to_segment_index = HashMap::new();
        /// Moves the references pending on node `from`, followed by `reference`,
        /// to node `to`, which leads to the same commit.
        fn forward_pending_refs(
            pending_commit_refs: &mut HashMap<NodeIndex, Vec<crate::RefInfo>>,
            from: NodeIndex,
            to: NodeIndex,
            reference: Option<crate::RefInfo>,
        ) {
            let mut refs = pending_commit_refs.remove(&from).unwrap_or_default();
            refs.extend(reference);
            if !refs.is_empty() {
                pending_commit_refs.entry(to).or_default().extend(refs);
            }
        }

        for (i, node) in self.nodes.iter().enumerate() {
            let mut predecessor_segment_info = node_index_to_predecessor_segment_info
                .remove(&i)
                .unwrap_or_default();
            let reference = match &node.weight {
                NodeWeight::Ref { full_name } => Some((full_name, self.commit_id_of(i))),
                NodeWeight::Pick { .. } | NodeWeight::ShallowPoint { .. } => None,
            };
            if let Some((full_name, _)) = reference {
                if full_name.category() != Some(gix::refs::Category::RemoteBranch) {
                    predecessor_segment_info
                        .commit_flags
                        .insert(CommitFlags::NotInRemote);
                }
                if is_workspace_ref_name(full_name.as_ref()) {
                    predecessor_segment_info
                        .commit_flags
                        .insert(CommitFlags::InWorkspace);
                }
                if let Some(ref target_ref) = target_upstream_ref
                    && full_name == target_ref
                {
                    predecessor_segment_info
                        .commit_flags
                        .insert(CommitFlags::Integrated);
                }
            }
            // A branch or tag that merely rests on the target commit, with
            // nothing reaching it, is recorded on that commit like the branches
            // of the segment containing it, instead of being a lane of its own.
            if let Some((full_name, commit_id)) = reference
                && predecessor_segment_info.segment_indexes.is_empty()
                && matches!(
                    full_name.category(),
                    Some(gix::refs::Category::LocalBranch | gix::refs::Category::Tag)
                )
                && project_meta
                    .target_ref
                    .as_ref()
                    .is_none_or(|target_ref| target_ref.as_local_full_name_ref() != full_name)
                && !worktree_by_branch.contains_key(full_name)
                && project_meta.target_commit_id == Some(commit_id)
            {
                forward_pending_refs(
                    &mut pending_commit_refs,
                    i,
                    node.parents[0],
                    Some(crate::RefInfo::from_ref(
                        full_name.clone(),
                        Some(commit_id),
                        worktree_by_branch,
                    )),
                );
                continue;
            }
            // A segment without a reference of its own continues through the
            // references it reaches, which then point at its next commit. A
            // branch checked out in a worktree still names a segment of its own.
            let continued_segment = match predecessor_segment_info.segment_indexes[..] {
                [(segment_index, _)] if !predecessor_segment_info.must_start_new_segment => {
                    match &node.weight {
                        NodeWeight::Ref { full_name, .. } => {
                            (inner_graph[segment_index].ref_info.is_none()
                                && !worktree_by_branch.contains_key(full_name))
                            .then_some(segment_index)
                        }
                        NodeWeight::Pick { .. } | NodeWeight::ShallowPoint { .. } => {
                            Some(segment_index)
                        }
                    }
                }
                _ => None,
            };
            let segment_index = if let Some(segment_index) = continued_segment {
                segment_index
            } else {
                let metadata = match &node.weight {
                    NodeWeight::Ref { full_name, .. } => {
                        crate::init::walk::extract_local_branch_metadata(
                            full_name.as_ref(),
                            overlay_meta,
                        )?
                    }
                    NodeWeight::Pick { .. } | NodeWeight::ShallowPoint { .. } => None,
                };
                let segment_index = inner_graph.add_node(crate::Segment {
                    metadata,
                    ..Default::default()
                });
                inner_graph[segment_index].id = segment_index;
                for (predecessor_segment_index, parent_order) in
                    &predecessor_segment_info.segment_indexes
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
                            parent_order: *parent_order,
                            // The parents of a workspace commit are in stack
                            // order, even where they lead to an empty segment
                            // that has no commit to tell the parent order by.
                            workspace_parent_order: matches!(
                                predecessor_segment.metadata,
                                Some(SegmentMetadata::Workspace(_))
                            )
                            .then_some(*parent_order),
                        },
                    );
                }
                segment_index
            };
            match &node.weight {
                NodeWeight::Pick { oid } | NodeWeight::ShallowPoint { oid } => {
                    // A shallow point keeps its parents too, so the editor knows the walk
                    // stopped there and keeps it as it is.
                    let parent_ids: Vec<_> = repo
                        .find_commit(*oid)?
                        .parent_ids()
                        .map(|id| id.detach())
                        .collect();
                    inner_graph[segment_index].commits.push(crate::Commit {
                        id: *oid,
                        parent_ids,
                        flags: predecessor_segment_info.commit_flags,
                        refs: pending_commit_refs.remove(&i).unwrap_or_default(),
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
                NodeWeight::Ref { full_name } if continued_segment.is_some() => {
                    forward_pending_refs(
                        &mut pending_commit_refs,
                        i,
                        node.parents[0],
                        Some(crate::RefInfo::from_ref(
                            full_name.clone(),
                            Some(self.commit_id_of(i)),
                            worktree_by_branch,
                        )),
                    );
                }
                NodeWeight::Ref { full_name } => {
                    // References that reached this one still point at the same commit.
                    forward_pending_refs(&mut pending_commit_refs, i, node.parents[0], None);
                    match full_name.category() {
                        Some(gix::refs::Category::LocalBranch) => {
                            local_branches.push((segment_index, full_name.clone()));
                        }
                        Some(gix::refs::Category::RemoteBranch) => {
                            remote_tracking_branch_name_to_segment_index
                                .insert(full_name.clone(), segment_index);
                        }
                        _ => {}
                    }
                    inner_graph[segment_index].ref_info = Some(crate::RefInfo::from_ref(
                        full_name.clone(),
                        Some(self.commit_id_of(i)),
                        worktree_by_branch,
                    ));
                }
            }
            for (parent_order, parent) in node.parents.iter().enumerate() {
                let parent_predecessor_segment_info = node_index_to_predecessor_segment_info
                    .entry(*parent)
                    .or_default();
                parent_predecessor_segment_info
                    .segment_indexes
                    .push((segment_index, parent_order as u32));
                if node.parents.len() > 1 {
                    parent_predecessor_segment_info.must_start_new_segment = true;
                }
                parent_predecessor_segment_info
                    .commit_flags
                    .insert(predecessor_segment_info.commit_flags);
            }
        }

        for (segment_index, full_name) in local_branches {
            if let Some(remote_tracking_branch_name) =
                crate::init::remotes::lookup_remote_tracking_branch(
                    overlay_repo,
                    full_name.as_ref(),
                )?
                && let Some(remote_tracking_segment_index) =
                    remote_tracking_branch_name_to_segment_index
                        .remove(remote_tracking_branch_name.as_ref())
            {
                let segment = &mut inner_graph[segment_index];
                segment.remote_tracking_branch_segment_id = Some(remote_tracking_segment_index);
                segment.remote_tracking_ref_name = Some(remote_tracking_branch_name);
            }
        }

        let mut graph = crate::Graph {
            inner: inner_graph,
            entrypoint,
            entrypoint_ref: None,
            traversal_tips: Vec::new(),
            ad_hoc_branch_stack_orders: Vec::new(),
            worktree_branch_stack_orders: Vec::new(),
            hard_limit_hit: false,
            options: Default::default(),
            worktree_tips: Vec::new(),
            project_meta,
            target_upstream_ref,
            symbolic_remote_names: Vec::new(),
        };
        // A commit's parents may become segments in any order, but consumers
        // expect its outgoing edges in parent order, as after a traversal.
        graph.rebuild_edges_in_parent_order();
        Ok(graph)
    }
}

impl crate::Workspace {
    /// Produce a segment graph of all commits reachable from `heads`, along with all remote-tracking
    /// branches and workspace stack branches known to `meta`, by first building a graph of commits
    /// and references in topological order and then grouping it into segments.
    ///
    /// History below the workspace's target commit is only included while it leads to one of the
    /// tips, and ends in a commit whose parents are left out.
    ///
    /// It uses this workspace's project metadata and its own workspace metadata, which may not be
    /// persisted in `meta` yet. Branches that no stack claims are kept off the stacks if they rest
    /// on its target commit.
    pub fn nosegment_graph(
        &self,
        repo: &gix::Repository,
        meta: &impl RefMetadata,
        mut heads: Vec<gix::ObjectId>,
    ) -> Result<crate::Graph> {
        let project_meta = self.graph.project_meta.clone();
        let target_commit_id = self.target_commit.as_ref().map(|commit| commit.commit_id);
        let head_ref = self.graph.entrypoint()?.segment.ref_name();
        let overlay = Overlay::default().with_workspace_metadata_override(
            self.ref_name()
                .map(ToOwned::to_owned)
                .zip(self.metadata.clone()),
        );
        let (overlay_repo, overlay_meta, _entrypoint) = overlay.into_parts(repo, meta);
        let worktree_by_branch = overlay_repo.worktree_branches(None)?;
        // Everything the workspace's graph was traversed from is part of it,
        // even what lies above the entrypoint.
        heads.extend(self.graph.traversal_tips.iter().map(|tip| tip.id));
        // A configured target branch may have moved on to commits nothing else reaches.
        if let Some(target_ref) = &project_meta.target_ref
            && let Some(mut reference) =
                repo.try_find_reference(target_ref.as_local_full_name_ref())?
        {
            heads.push(reference.peel_to_commit()?.id);
        }
        let default_target_ref = FullName::try_from("refs/heads/main")?;
        let target_ref = project_meta
            .target_ref
            .as_ref()
            .map_or(default_target_ref.as_ref(), |target_ref| {
                target_ref.as_local_full_name_ref()
            });
        let target_upstream_ref = project_meta.remote_target_ref(repo)?;
        graph(
            repo,
            &overlay_meta,
            &overlay_repo.remote_names(),
            &worktree_by_branch,
            head_ref,
            self.graph.options.collect_tags,
            heads,
            target_ref,
            target_upstream_ref.as_ref().map(|name| name.as_ref()),
            target_commit_id,
        )?
        .to_segment_graph(
            repo,
            &overlay_repo,
            &overlay_meta,
            &worktree_by_branch,
            project_meta,
        )
    }
}

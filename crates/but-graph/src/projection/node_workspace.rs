use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use anyhow::Context as _;
use but_core::ref_metadata::{self, StackId};

use super::stack::{StackCommit, StackCommitFlags};
use crate::{
    Graph, NodeGraphEntrypoint, NodeIndex, NodeKind, RefInfo, Reference, ReferenceMetadata,
};

/// A first-parent stack projected from the node graph.
#[derive(Clone, Debug)]
pub struct Stack {
    /// Stable metadata identity, or the single-branch identity for an ad-hoc workspace.
    pub id: Option<StackId>,
    /// Reference-delimited portions from tip toward base.
    pub segments: Vec<StackSegment>,
}

impl Stack {
    /// Return the first projected commit.
    pub fn tip(&self) -> Option<gix::ObjectId> {
        self.segments
            .first()
            .and_then(|segment| segment.commits.first())
            .map(|commit| commit.id)
    }

    /// Return the first reference name.
    pub fn ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.segments.first().and_then(StackSegment::ref_name)
    }

    /// Return the first commit, skipping empty reference segments.
    pub fn tip_skip_empty(&self) -> Option<gix::ObjectId> {
        self.segments
            .iter()
            .find_map(|segment| segment.commits.first().map(|commit| commit.id))
    }

    /// Return the base of the bottom segment.
    pub fn base(&self) -> Option<gix::ObjectId> {
        self.segments.last().and_then(|segment| segment.base)
    }
}

/// A reference-delimited linear portion of a projected stack.
#[derive(Clone, Debug)]
pub struct StackSegment {
    /// Reference at the tip, if this portion is named.
    pub ref_info: Option<RefInfo>,
    /// Configured remote-tracking reference.
    pub remote_tracking_ref_name: Option<gix::refs::FullName>,
    /// Related local or remote reference node.
    pub sibling_node_id: Option<NodeIndex>,
    /// Remote-tracking reference node.
    pub remote_tracking_branch_node_id: Option<NodeIndex>,
    /// First backing node for this portion.
    pub id: NodeIndex,
    /// Commits from tip toward base along first parents.
    pub commits: Vec<StackCommit>,
    /// Commits projected from a related reference outside the workspace.
    pub commits_outside: Option<Vec<StackCommit>>,
    /// Commit directly below this portion.
    pub base: Option<gix::ObjectId>,
    /// Commits available only from the configured remote-tracking reference.
    pub commits_on_remote: Vec<StackCommit>,
    /// Read-only branch metadata.
    pub metadata: Option<ref_metadata::Branch>,
    /// Whether this portion contains the traversal entrypoint.
    pub is_entrypoint: bool,
}

impl StackSegment {
    /// Return the first commit.
    pub fn tip(&self) -> Option<gix::ObjectId> {
        self.commits.first().map(|commit| commit.id)
    }

    /// Return this portion's reference name.
    pub fn ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.ref_info.as_ref().map(|info| info.ref_name.as_ref())
    }

    /// Iterate all reference names represented by this portion.
    pub fn ref_names(&self) -> impl Iterator<Item = &gix::refs::FullNameRef> {
        self.ref_name().into_iter().chain(
            self.commits
                .iter()
                .flat_map(|commit| commit.refs.iter().map(|info| info.ref_name.as_ref())),
        )
    }
}

/// A node-backed workspace projection.
#[derive(Clone, Debug)]
pub struct Workspace {
    /// Source graph. Projection never rewrites or replaces it.
    pub graph: Graph,
    /// Node representing the workspace ref or direct traversal entrypoint, if it is born.
    pub id: Option<NodeIndex>,
    /// Workspace classification.
    pub kind: WorkspaceKind,
    /// User-visible first-parent stacks.
    pub stacks: Vec<Stack>,
    /// Lowest commit included as workspace context.
    pub lower_bound: Option<gix::ObjectId>,
    /// Backing node for [`Self::lower_bound`].
    pub lower_bound_node_id: Option<NodeIndex>,
    /// Configured or discovered integration reference.
    pub target_ref: Option<TargetRef>,
    /// Stable target commit remembered by project metadata.
    pub target_commit: Option<TargetCommit>,
    /// Read-only workspace metadata.
    pub metadata: Option<ref_metadata::Workspace>,
}

/// How a workspace is anchored in the graph.
#[derive(Debug, Clone)]
pub enum WorkspaceKind {
    /// A metadata-backed workspace reference with its managed commit.
    Managed {
        /// Workspace reference information.
        ref_info: RefInfo,
    },
    /// A metadata-backed workspace reference without a managed commit.
    ManagedMissingWorkspaceCommit {
        /// Workspace reference information.
        ref_info: RefInfo,
    },
    /// A direct branch or detached checkout.
    AdHoc,
}

impl WorkspaceKind {
    /// Return whether GitButler owns the workspace reference.
    pub fn has_managed_ref(&self) -> bool {
        matches!(
            self,
            WorkspaceKind::Managed { .. } | WorkspaceKind::ManagedMissingWorkspaceCommit { .. }
        )
    }

    /// Return whether the graph contains the managed workspace commit.
    pub fn has_managed_commit(&self) -> bool {
        matches!(self, WorkspaceKind::Managed { .. })
    }
}

/// A target reference and its node-local status.
#[derive(Debug, Clone)]
pub struct TargetRef {
    /// Full target reference name.
    pub ref_name: gix::refs::FullName,
    /// Reference node index.
    pub node_index: NodeIndex,
    /// Commits between the target tip and workspace lower bound.
    pub commits_ahead: usize,
}

/// A stable target commit and its backing node.
#[derive(Debug, Clone)]
pub struct TargetCommit {
    /// Target commit object ID.
    pub commit_id: gix::ObjectId,
    /// Commit node index.
    pub node_index: NodeIndex,
}

impl Graph {
    /// Consume this graph and project it into a node-backed workspace.
    pub fn into_workspace(self) -> anyhow::Result<Workspace> {
        Workspace::from_graph(self)
    }

    /// Redo traversal and immediately project its result.
    pub fn into_workspace_of_redone_traversal(
        self,
        repo: &gix::Repository,
        meta: &impl but_core::RefMetadata,
    ) -> anyhow::Result<Workspace> {
        self.redo_traversal_with_overlay(repo, meta, Default::default())?
            .into_workspace()
    }
}

impl Workspace {
    fn from_graph(graph: Graph) -> anyhow::Result<Self> {
        let entrypoint = match graph.entrypoint() {
            NodeGraphEntrypoint::Node(index) => Some(*index),
            NodeGraphEntrypoint::Unborn(_) => None,
        };
        let entrypoint_ref = graph
            .entrypoint_ref()
            .and_then(|name| graph.node_by_ref_name(name).map(|(index, _)| index));
        let containing_workspace = entrypoint
            .and_then(|entrypoint| commit_index_at(&graph, entrypoint).ok())
            .and_then(|entrypoint| {
                let entrypoint_id = commit_id_at(&graph, entrypoint);
                graph
                    .nodes()
                    .iter()
                    .enumerate()
                    .filter_map(|(index, node)| {
                        let NodeKind::Reference(reference) = node.kind() else {
                            return None;
                        };
                        let Some(ReferenceMetadata::Workspace(metadata)) = &reference.metadata
                        else {
                            return None;
                        };
                        let workspace_tip = commit_index_at(&graph, index).ok()?;
                        let distance = distance_to(&graph, workspace_tip, entrypoint)?;
                        let is_workspace_entrypoint =
                            graph.entrypoint_ref() == Some(reference.ref_info.ref_name.as_ref());
                        let is_workspace_tip = entrypoint_id
                            .is_some_and(|id| reference.ref_info.commit_id == Some(id));
                        let target_commit_id = graph
                            .project_meta()
                            .target_commit_id
                            .or_else(|| metadata.target_commit_id());
                        // The stored target is the stable membership boundary. The moving target
                        // ref may integrate a stack without moving that boundary.
                        let is_target_history = target_commit_id.is_some_and(|target_id| {
                            entrypoint_id == Some(target_id)
                                || graph.node_by_commit_id(target_id).is_some_and(
                                    |(target_index, _)| {
                                        distance_to(&graph, target_index, entrypoint).is_some()
                                    },
                                )
                        });
                        (is_workspace_entrypoint || is_workspace_tip || !is_target_history)
                            .then_some((distance, index))
                    })
                    .min()
                    .map(|(_, index)| index)
            });
        let workspace_id = containing_workspace.or(entrypoint_ref).or(entrypoint);

        let (kind, metadata) = containing_workspace
            .and_then(|index| match graph.nodes()[index].kind() {
                NodeKind::Reference(reference) => Some((index, reference.as_ref())),
                NodeKind::Commit { .. } | NodeKind::ShallowPoint { .. } => None,
            })
            .map(|(_index, reference)| {
                let metadata = match &reference.metadata {
                    Some(ReferenceMetadata::Workspace(metadata)) => Some(metadata.clone()),
                    Some(ReferenceMetadata::Branch(_)) | None => None,
                };
                let has_managed_commit = graph
                    .managed_workspace_commit_id()
                    .is_some_and(|id| reference.ref_info.commit_id == Some(id));
                let kind = if has_managed_commit {
                    WorkspaceKind::Managed {
                        ref_info: reference.ref_info.clone(),
                    }
                } else {
                    WorkspaceKind::ManagedMissingWorkspaceCommit {
                        ref_info: reference.ref_info.clone(),
                    }
                };
                (kind, metadata)
            })
            .unwrap_or((WorkspaceKind::AdHoc, None));

        let target_ref = graph
            .project_meta()
            .target_ref
            .clone()
            .or_else(|| {
                metadata
                    .as_ref()
                    .and_then(|metadata| metadata.target_ref().map(ToOwned::to_owned))
            })
            .and_then(|name| target_ref_from_name(&graph, name))
            .or_else(|| integrated_tip_target_ref(&graph));
        let target_commit_id = graph.project_meta().target_commit_id.or_else(|| {
            metadata
                .as_ref()
                .and_then(|metadata| metadata.target_commit_id())
        });
        let integrated_target_indices = integrated_tip_target_indices(&graph, target_ref.as_ref());
        let target_commit = target_commit_id
            .and_then(|commit_id| {
                graph
                    .node_by_commit_id(commit_id)
                    .map(|(node_index, _)| TargetCommit {
                        commit_id,
                        node_index,
                    })
            })
            .or_else(|| lowest_integrated_target(&graph, &integrated_target_indices));
        let stack_target = target_commit
            .as_ref()
            .map(|target| target.node_index)
            .or_else(|| target_ref.as_ref().map(|target| target.node_index));

        let mut starts = stack_starts(
            &graph,
            containing_workspace,
            entrypoint_ref.or(entrypoint),
            metadata.as_ref(),
            stack_target,
        );
        if let Some(target_ref) = target_ref.as_ref() {
            starts.retain(|index| *index != target_ref.node_index);
        }
        deduplicate(&mut starts);

        let mut common_starts = starts.clone();
        if matches!(kind, WorkspaceKind::ManagedMissingWorkspaceCommit { .. }) {
            common_starts.extend(
                containing_workspace
                    .and_then(|workspace| graph.nodes()[workspace].parents().last().copied()),
            );
        }
        common_starts.extend(integrated_target_indices.iter().copied());
        deduplicate(&mut common_starts);
        let lower_bound_node_id = match target_commit.as_ref() {
            Some(target)
                if common_starts
                    .iter()
                    .all(|start| distance_to(&graph, *start, target.node_index).is_some()) =>
            {
                Some(target.node_index)
            }
            Some(target) => {
                common_starts.push(target.node_index);
                deduplicate(&mut common_starts);
                common_ancestor(&graph, &common_starts)
            }
            None => {
                common_starts.extend(target_ref.as_ref().map(|target| target.node_index));
                deduplicate(&mut common_starts);
                (common_starts.len() > 1)
                    .then(|| common_ancestor(&graph, &common_starts))
                    .flatten()
            }
        };
        let lower_bound = lower_bound_node_id.and_then(|index| commit_id_at(&graph, index));

        let refs_by_commit = references_by_commit(&graph);
        let workspace_overlay_roots = containing_workspace
            .and_then(|workspace| graph.nodes()[workspace].parents().split_last())
            .map(|(_, overlay_roots)| overlay_roots)
            .unwrap_or_default();
        let mut stacks = starts
            .into_iter()
            .filter_map(|start| {
                let stack_lower_bound = stack_lower_bound(
                    &graph,
                    start,
                    stack_target,
                    stack_target.and(lower_bound_node_id),
                );
                collect_stack(
                    &graph,
                    start,
                    entrypoint_ref.or(entrypoint),
                    stack_lower_bound,
                    &refs_by_commit,
                )
                .transpose()
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        if let Some(metadata) = metadata.as_ref() {
            enrich_anonymous_stack_tips(
                &graph,
                metadata,
                &mut stacks,
                entrypoint_ref.or(entrypoint),
                &refs_by_commit,
            )?;
        }
        enrich_remotes(&graph, &mut stacks, &refs_by_commit);

        for stack in &mut stacks {
            stack.id = metadata
                .as_ref()
                .and_then(|metadata| metadata_stack_id(metadata, stack))
                .or_else(|| matches!(kind, WorkspaceKind::AdHoc).then(StackId::single_branch_id));
        }
        if !matches!(kind, WorkspaceKind::AdHoc) {
            stacks.retain(|stack| {
                stack.id.is_some()
                    || stack.tip_skip_empty().is_some()
                    || lower_bound.is_none()
                    || stack.base() != lower_bound
                    || stack
                        .segments
                        .first()
                        .is_some_and(|segment| workspace_overlay_roots.contains(&segment.id))
            });
            for stack in &mut stacks {
                while stack.segments.last().is_some_and(|segment| {
                    segment.commits.is_empty()
                        && lower_bound.is_some()
                        && segment.base == lower_bound
                        && target_ref.as_ref().is_some_and(|target| {
                            segment.ref_name() == Some(target.ref_name.as_ref())
                                || segment.remote_tracking_ref_name.as_ref()
                                    == Some(&target.ref_name)
                        })
                        && !segment.ref_name().is_some_and(|ref_name| {
                            metadata.as_ref().is_some_and(|metadata| {
                                metadata.stacks.iter().any(|stack| {
                                    stack
                                        .branches
                                        .iter()
                                        .any(|branch| branch.ref_name.as_ref() == ref_name)
                                })
                            })
                        })
                }) {
                    stack.segments.pop();
                    if let Some(bottom) = stack.segments.last_mut() {
                        bottom.base = lower_bound;
                    }
                }
            }
        }

        let mut target_ref = target_ref;
        if let Some(target) = target_ref.as_mut() {
            target.commits_ahead =
                reachable_commit_difference(&graph, target.node_index, lower_bound_node_id).len();
        }

        Ok(Workspace {
            graph,
            id: workspace_id,
            kind,
            stacks,
            lower_bound,
            lower_bound_node_id,
            target_ref,
            target_commit,
            metadata,
        })
    }

    /// Return the workspace reference name, if any.
    pub fn ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        match &self.kind {
            WorkspaceKind::Managed { ref_info }
            | WorkspaceKind::ManagedMissingWorkspaceCommit { ref_info } => {
                Some(ref_info.ref_name.as_ref())
            }
            WorkspaceKind::AdHoc => self.graph.entrypoint_ref(),
        }
    }

    /// Return the stored target commit ID.
    pub fn stored_target_commit_id(&self) -> Option<gix::ObjectId> {
        self.target_commit.as_ref().map(|target| target.commit_id)
    }

    /// Return the target reference tip commit ID.
    pub fn target_ref_tip_commit_id(&self) -> Option<gix::ObjectId> {
        self.target_ref
            .as_ref()
            .and_then(|target| commit_id_at(&self.graph, target.node_index))
    }
}

fn target_ref_from_name(graph: &Graph, ref_name: gix::refs::FullName) -> Option<TargetRef> {
    graph
        .node_by_ref_name(ref_name.as_ref())
        .map(|(node_index, _)| TargetRef {
            ref_name,
            node_index,
            commits_ahead: 0,
        })
}

fn integrated_tip_target_ref(graph: &Graph) -> Option<TargetRef> {
    if graph
        .context
        .traversal_tips
        .iter()
        .any(|tip| matches!(&tip.metadata, Some(ReferenceMetadata::Workspace(_))))
    {
        return None;
    }
    graph
        .context
        .traversal_tips
        .iter()
        .filter(|tip| tip.role.is_integrated())
        .filter_map(|tip| tip.ref_name.clone())
        .find_map(|ref_name| target_ref_from_name(graph, ref_name))
}

fn integrated_tip_target_indices(graph: &Graph, target_ref: Option<&TargetRef>) -> Vec<NodeIndex> {
    let target_ref_id = target_ref.and_then(|target| commit_id_at(graph, target.node_index));
    let mut out = graph
        .context
        .traversal_tips
        .iter()
        .filter(|tip| tip.role.is_integrated() && Some(tip.id) != target_ref_id)
        .filter_map(|tip| graph.node_by_commit_id(tip.id).map(|(index, _)| index))
        .collect::<Vec<_>>();
    deduplicate(&mut out);
    out
}

fn lowest_integrated_target(graph: &Graph, targets: &[NodeIndex]) -> Option<TargetCommit> {
    let child_counts = graph.child_counts();
    targets
        .iter()
        .copied()
        .max_by_key(|target| {
            child_counts
                .iter()
                .enumerate()
                .filter(|(_, children)| **children == 0)
                .filter_map(|(root, _)| distance_to(graph, root, *target))
                .max()
                .unwrap_or_default()
        })
        .and_then(|node_index| {
            commit_id_at(graph, node_index).map(|commit_id| TargetCommit {
                commit_id,
                node_index,
            })
        })
}

fn stack_lower_bound(
    graph: &Graph,
    start: NodeIndex,
    target: Option<NodeIndex>,
    workspace_lower_bound: Option<NodeIndex>,
) -> Option<NodeIndex> {
    let Some(target) = target else {
        return workspace_lower_bound;
    };
    let target_ancestors = reachable_commit_indices(graph, [target]);
    let mut cursor = commit_index_at(graph, start).ok()?;
    let mut seen = HashSet::new();
    while seen.insert(cursor) {
        if target_ancestors.contains(&cursor) {
            return Some(cursor);
        }
        let Some(parent) = graph.nodes()[cursor]
            .parents()
            .first()
            .and_then(|parent| commit_index_at(graph, *parent).ok())
        else {
            break;
        };
        cursor = parent;
    }
    workspace_lower_bound
}

fn stack_starts(
    graph: &Graph,
    workspace_ref: Option<NodeIndex>,
    fallback: Option<NodeIndex>,
    metadata: Option<&ref_metadata::Workspace>,
    target_commit: Option<NodeIndex>,
) -> Vec<NodeIndex> {
    let Some(workspace_ref) = workspace_ref else {
        return fallback.into_iter().collect();
    };
    let node = &graph.nodes()[workspace_ref];
    let Some((own_target, overlay_parents)) = node.parents().split_last() else {
        return Vec::new();
    };
    let managed = graph
        .managed_workspace_commit_id()
        .and_then(|id| graph.node_by_commit_id(id).map(|(index, _)| index));
    if managed == Some(*own_target) {
        let actual_parents = graph.nodes()[*own_target].parents();
        let mut starts = overlay_parents.to_vec();
        starts.extend(actual_parents);
        deduplicate(&mut starts);
        let candidates = starts.clone();
        starts.retain(|root| {
            !candidates
                .iter()
                .any(|other| other != root && distance_to(graph, *other, *root).is_some())
        });
        let actual_parents = actual_parents.iter().copied().collect::<HashSet<_>>();
        let mut roots_by_commit = HashMap::<NodeIndex, Vec<usize>>::new();
        let mut unique = Vec::new();
        for root in starts {
            let Ok(commit) = commit_index_at(graph, root) else {
                continue;
            };
            let stack_id = metadata_stack_id_for_root(graph, metadata, root);
            let positions = roots_by_commit.entry(commit).or_default();
            let same_stack = positions.iter().copied().find(|position| {
                stack_id.is_none()
                    || metadata_stack_id_for_root(graph, metadata, unique[*position]) == stack_id
            });
            if let Some(position) = same_stack {
                if actual_parents.contains(&root) && !actual_parents.contains(&unique[position]) {
                    unique[position] = root;
                }
            } else if target_commit
                .is_some_and(|target| distance_to(graph, target, commit).is_some())
                || positions.len()
                    < graph.nodes()[*own_target]
                        .parents()
                        .iter()
                        .filter(|parent| {
                            commit_index_at(graph, **parent).is_ok_and(|index| index == commit)
                        })
                        .count()
                        .max(1)
            {
                positions.push(unique.len());
                unique.push(root);
            } else if actual_parents.contains(&root)
                && let Some(position) = positions
                    .iter()
                    .copied()
                    .find(|position| !actual_parents.contains(&unique[*position]))
            {
                unique[position] = root;
            } else {
                continue;
            }
        }
        unique
    } else if !overlay_parents.is_empty() {
        overlay_parents.to_vec()
    } else {
        vec![*own_target]
    }
}

fn metadata_stack_id_for_root(
    graph: &Graph,
    metadata: Option<&ref_metadata::Workspace>,
    root: NodeIndex,
) -> Option<StackId> {
    let NodeKind::Reference(reference) = graph.nodes()[root].kind() else {
        return None;
    };
    metadata?
        .find_stack_with_branch(
            reference.ref_info.ref_name.as_ref(),
            ref_metadata::StackKind::Applied,
        )
        .map(|stack| stack.id)
}

fn is_internal_gitbutler_reference(reference: &Reference) -> bool {
    reference
        .ref_info
        .ref_name
        .as_bstr()
        .starts_with(b"refs/heads/gitbutler/")
}

fn collect_stack(
    graph: &Graph,
    start: NodeIndex,
    entrypoint: Option<NodeIndex>,
    lower_bound: Option<NodeIndex>,
    refs_by_commit: &HashMap<NodeIndex, Vec<(NodeIndex, RefInfo)>>,
) -> anyhow::Result<Option<Stack>> {
    let mut segments = Vec::new();
    let mut current = None;
    let mut cursor = start;
    let mut seen = HashSet::new();
    let mut named_reference = None;

    while seen.insert(cursor) && Some(cursor) != lower_bound {
        match graph.nodes()[cursor].kind() {
            NodeKind::Reference(reference) => {
                if matches!(reference.metadata, Some(ReferenceMetadata::Workspace(_))) {
                    break;
                }
                let Some(parent) = graph.nodes()[cursor].parents().last().copied() else {
                    break;
                };
                if is_internal_gitbutler_reference(reference) {
                    cursor = parent;
                    continue;
                }
                if current.as_ref().is_some_and(|segment: &StackSegment| {
                    segment.ref_info.is_some() || !segment.commits.is_empty()
                }) {
                    segments.push(current.take().expect("checked above"));
                }
                current = Some(segment_from_reference(graph, cursor, reference, entrypoint));
                named_reference = Some(cursor);
                if Some(parent) == lower_bound {
                    let Some(segment) = current.as_mut() else {
                        break;
                    };
                    segment.base = commit_id_at(graph, parent);
                    break;
                }
                cursor = parent;
            }
            NodeKind::Commit { id } => {
                let mut refs = refs_by_commit.get(&cursor).cloned().unwrap_or_default();
                let reference_index = (current.is_none() && Some(cursor) != entrypoint)
                    .then(|| unique_local_reference(graph, &refs))
                    .flatten();
                if let Some(reference_index) = reference_index
                    && let NodeKind::Reference(reference) = graph.nodes()[reference_index].kind()
                {
                    if current.as_ref().is_some_and(|segment: &StackSegment| {
                        segment.ref_info.is_some() || !segment.commits.is_empty()
                    }) {
                        segments.push(current.take().expect("checked above"));
                    }
                    current = Some(segment_from_reference(
                        graph,
                        reference_index,
                        reference,
                        entrypoint,
                    ));
                    named_reference = Some(reference_index);
                }
                let segment = current.get_or_insert_with(|| anonymous_segment(cursor, entrypoint));
                refs.retain(|(index, _)| Some(*index) != named_reference);
                let annotation = graph.annotations()[cursor];
                let mut flags = StackCommitFlags::from(annotation);
                if !annotation.contains(crate::CommitFlags::NotInRemote) {
                    flags |= StackCommitFlags::ReachableByRemote;
                }
                segment.commits.push(StackCommit {
                    id: *id,
                    parent_ids: graph.nodes()[cursor]
                        .parents()
                        .iter()
                        .filter_map(|parent| commit_id_at(graph, *parent))
                        .collect(),
                    flags,
                    refs: refs.into_iter().map(|(_, info)| info).collect(),
                });
                let Some(parent) = graph.nodes()[cursor].parents().first().copied() else {
                    break;
                };
                if Some(parent) == lower_bound {
                    segment.base = commit_id_at(graph, parent);
                    break;
                }
                cursor = parent;
            }
            NodeKind::ShallowPoint { id, .. } => {
                if let Some(segment) = current.as_mut() {
                    if let Some(commit) = segment.commits.last_mut() {
                        commit.flags |= StackCommitFlags::EarlyEnd;
                    }
                    segment.base = Some(*id);
                }
                break;
            }
        }
    }
    if let Some(segment) = current {
        segments.push(segment);
    }
    if segments.is_empty() {
        return Ok(None);
    }
    for index in 0..segments.len().saturating_sub(1) {
        segments[index].base = segments[index + 1].tip();
    }
    Ok(Some(Stack { id: None, segments }))
}

fn segment_from_reference(
    graph: &Graph,
    index: NodeIndex,
    reference: &Reference,
    entrypoint: Option<NodeIndex>,
) -> StackSegment {
    let remote_tracking_branch_node_id =
        reference
            .remote_tracking_ref_name
            .as_ref()
            .and_then(|name| {
                graph
                    .node_by_ref_name(name.as_ref())
                    .map(|(index, _)| index)
            });
    let sibling_node_id =
        if reference.ref_info.ref_name.category() == Some(gix::refs::Category::RemoteBranch) {
            graph
                .nodes()
                .iter()
                .enumerate()
                .find_map(|(candidate, node)| {
                    let NodeKind::Reference(local) = node.kind() else {
                        return None;
                    };
                    (local.remote_tracking_ref_name.as_ref() == Some(&reference.ref_info.ref_name))
                        .then_some(candidate)
                })
        } else {
            None
        };
    StackSegment {
        ref_info: Some(reference.ref_info.clone()),
        remote_tracking_ref_name: reference.remote_tracking_ref_name.clone(),
        sibling_node_id,
        remote_tracking_branch_node_id,
        id: index,
        commits: Vec::new(),
        commits_outside: None,
        base: None,
        commits_on_remote: Vec::new(),
        metadata: match &reference.metadata {
            Some(ReferenceMetadata::Branch(metadata)) => Some(metadata.clone()),
            Some(ReferenceMetadata::Workspace(_)) | None => None,
        },
        is_entrypoint: entrypoint == Some(index),
    }
}

fn anonymous_segment(index: NodeIndex, entrypoint: Option<NodeIndex>) -> StackSegment {
    StackSegment {
        ref_info: None,
        remote_tracking_ref_name: None,
        sibling_node_id: None,
        remote_tracking_branch_node_id: None,
        id: index,
        commits: Vec::new(),
        commits_outside: None,
        base: None,
        commits_on_remote: Vec::new(),
        metadata: None,
        is_entrypoint: entrypoint == Some(index),
    }
}

fn references_by_commit(graph: &Graph) -> HashMap<NodeIndex, Vec<(NodeIndex, RefInfo)>> {
    let mut out = HashMap::<_, Vec<_>>::new();
    for (index, node) in graph.nodes().iter().enumerate() {
        let NodeKind::Reference(reference) = node.kind() else {
            continue;
        };
        if let Ok(target) = commit_index_at(graph, index) {
            out.entry(target)
                .or_default()
                .push((index, reference.ref_info.clone()));
        }
    }
    out
}

fn unique_local_reference(graph: &Graph, references: &[(NodeIndex, RefInfo)]) -> Option<NodeIndex> {
    let candidates = references
        .iter()
        .filter_map(|(index, info)| {
            (info.ref_name.category() == Some(gix::refs::Category::LocalBranch)
                && !matches!(
                    graph.nodes()[*index].kind(),
                    NodeKind::Reference(reference)
                        if matches!(reference.metadata, Some(ReferenceMetadata::Workspace(_)))
                            || is_internal_gitbutler_reference(reference)
                ))
            .then_some(*index)
        })
        .collect::<Vec<_>>();
    if let [candidate] = candidates.as_slice() {
        return Some(*candidate);
    }
    let mut tracked = candidates.into_iter().filter(|index| {
        let NodeKind::Reference(reference) = graph.nodes()[*index].kind() else {
            return false;
        };
        reference
            .remote_tracking_ref_name
            .as_ref()
            .and_then(|name| graph.node_by_ref_name(name.as_ref()))
            .is_some_and(|(remote, _)| {
                commit_index_at(graph, remote).ok() == commit_index_at(graph, *index).ok()
            })
    });
    let candidate = tracked.next()?;
    tracked.next().is_none().then_some(candidate)
}

fn enrich_anonymous_stack_tips(
    graph: &Graph,
    metadata: &ref_metadata::Workspace,
    stacks: &mut [Stack],
    entrypoint: Option<NodeIndex>,
    refs_by_commit: &HashMap<NodeIndex, Vec<(NodeIndex, RefInfo)>>,
) -> anyhow::Result<()> {
    for stack in stacks {
        let Some(first) = stack.segments.first() else {
            continue;
        };
        if first.ref_info.is_some() {
            continue;
        }
        let names = stack
            .segments
            .iter()
            .flat_map(StackSegment::ref_names)
            .collect::<BTreeSet<_>>();
        let Some(top_ref_name) = metadata.stacks.iter().find_map(|candidate| {
            candidate
                .branches
                .iter()
                .skip(1)
                .any(|branch| names.contains(branch.ref_name.as_ref()))
                .then(|| candidate.branches.first())
                .flatten()
                .map(|branch| branch.ref_name.as_ref())
        }) else {
            continue;
        };
        let Some((reference_index, reference)) = graph.node_by_ref_name(top_ref_name) else {
            continue;
        };
        let in_workspace_tip = first.id;
        if distance_to(graph, reference_index, in_workspace_tip).is_none() {
            continue;
        }
        let Some(outside) = collect_stack(
            graph,
            reference_index,
            entrypoint,
            Some(in_workspace_tip),
            refs_by_commit,
        )?
        else {
            continue;
        };
        let Some(outside_segment) = outside.segments.into_iter().next() else {
            continue;
        };

        let first = &mut stack.segments[0];
        let mut replacement = segment_from_reference(graph, reference_index, reference, entrypoint);
        replacement.sibling_node_id = Some(first.id);
        replacement.commits = std::mem::take(&mut first.commits);
        replacement.commits_outside =
            (!outside_segment.commits.is_empty()).then_some(outside_segment.commits);
        replacement.base = first.base;
        replacement.commits_on_remote = std::mem::take(&mut first.commits_on_remote);
        *first = replacement;
    }
    Ok(())
}

fn enrich_remotes(
    graph: &Graph,
    stacks: &mut [Stack],
    refs_by_commit: &HashMap<NodeIndex, Vec<(NodeIndex, RefInfo)>>,
) {
    for stack in stacks {
        for segment_index in 0..stack.segments.len() {
            let Some(remote_index) = stack.segments[segment_index].remote_tracking_branch_node_id
            else {
                continue;
            };
            let remote_reachable = reachable_commit_indices(graph, [remote_index]);
            for segment in &mut stack.segments {
                for commit in &mut segment.commits {
                    if graph
                        .node_by_commit_id(commit.id)
                        .is_some_and(|(index, _)| remote_reachable.contains(&index))
                    {
                        commit.flags |= StackCommitFlags::ReachableByRemote;
                    }
                }
            }
            let local_starts = stack.segments[segment_index..]
                .iter()
                .flat_map(|segment| segment.commits.iter().map(|commit| commit.id))
                .chain(stack.base())
                .filter_map(|id| graph.node_by_commit_id(id).map(|(index, _)| index));
            let remote_only = reachable_commit_difference(graph, remote_index, local_starts);
            stack.segments[segment_index].commits_on_remote =
                remote_only_commits(graph, remote_index, &remote_only, refs_by_commit);
        }
    }
}

fn remote_only_commits(
    graph: &Graph,
    start: NodeIndex,
    remote_only: &HashSet<NodeIndex>,
    refs_by_commit: &HashMap<NodeIndex, Vec<(NodeIndex, RefInfo)>>,
) -> Vec<StackCommit> {
    let Ok(start) = commit_index_at(graph, start) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut pending = VecDeque::from([start]);
    let mut seen = HashSet::new();
    while let Some(index) = pending.pop_front() {
        if !seen.insert(index) {
            continue;
        }
        if !remote_only.contains(&index) {
            continue;
        }
        let NodeKind::Commit { id } = graph.nodes()[index].kind() else {
            continue;
        };
        let annotation = graph.annotations()[index];
        out.push(StackCommit {
            id: *id,
            parent_ids: graph.nodes()[index]
                .parents()
                .iter()
                .filter_map(|parent| commit_id_at(graph, *parent))
                .collect(),
            flags: StackCommitFlags::from(annotation) | StackCommitFlags::ReachableByRemote,
            refs: refs_by_commit
                .get(&index)
                .into_iter()
                .flatten()
                .map(|(_, info)| info.clone())
                .collect(),
        });
        pending.extend(
            graph.nodes()[index]
                .parents()
                .iter()
                .filter_map(|parent| commit_index_at(graph, *parent).ok()),
        );
    }
    out
}

fn metadata_stack_id(metadata: &ref_metadata::Workspace, stack: &Stack) -> Option<StackId> {
    let names = stack
        .segments
        .iter()
        .filter_map(StackSegment::ref_name)
        .collect::<BTreeSet<_>>();
    metadata.stacks.iter().find_map(|candidate| {
        candidate
            .branches
            .iter()
            .any(|branch| names.contains(branch.ref_name.as_ref()))
            .then_some(candidate.id)
    })
}

fn common_ancestor(graph: &Graph, starts: &[NodeIndex]) -> Option<NodeIndex> {
    let mut distances = starts
        .iter()
        .filter_map(|start| ancestor_distances(graph, *start))
        .collect::<Vec<_>>();
    let first = distances.pop()?;
    first
        .into_iter()
        .filter(|(candidate, _)| distances.iter().all(|set| set.contains_key(candidate)))
        .min_by_key(|(candidate, distance)| {
            let max_distance = distances
                .iter()
                .filter_map(|set| set.get(candidate))
                .copied()
                .max()
                .unwrap_or_default()
                .max(*distance);
            (max_distance, *candidate)
        })
        .map(|(index, _)| index)
}

fn ancestor_distances(graph: &Graph, start: NodeIndex) -> Option<HashMap<NodeIndex, usize>> {
    let start = commit_index_at(graph, start).ok()?;
    let mut out = HashMap::new();
    let mut queue = VecDeque::from([(start, 0)]);
    while let Some((index, distance)) = queue.pop_front() {
        if out.insert(index, distance).is_some() {
            continue;
        }
        for parent in graph.nodes()[index].parents() {
            if let Ok(parent) = commit_index_at(graph, *parent) {
                queue.push_back((parent, distance + 1));
            }
        }
    }
    Some(out)
}

fn reachable_commit_indices(
    graph: &Graph,
    starts: impl IntoIterator<Item = NodeIndex>,
) -> HashSet<NodeIndex> {
    let mut out = HashSet::new();
    let mut pending = starts.into_iter().collect::<Vec<_>>();
    while let Some(index) = pending.pop() {
        let Ok(commit) = commit_index_at(graph, index) else {
            continue;
        };
        if out.insert(commit) {
            pending.extend(graph.nodes()[commit].parents());
        }
    }
    out
}

fn reachable_commit_difference(
    graph: &Graph,
    start: NodeIndex,
    excluded_starts: impl IntoIterator<Item = NodeIndex>,
) -> HashSet<NodeIndex> {
    let excluded = reachable_commit_indices(graph, excluded_starts);
    let mut out = reachable_commit_indices(graph, [start]);
    out.retain(|index| !excluded.contains(index));
    out
}

fn commit_index_at(graph: &Graph, start: NodeIndex) -> anyhow::Result<NodeIndex> {
    let mut cursor = start;
    let mut seen = HashSet::new();
    while seen.insert(cursor) {
        match graph.nodes()[cursor].kind() {
            NodeKind::Commit { .. } => return Ok(cursor),
            NodeKind::Reference(_) => {
                cursor = *graph.nodes()[cursor]
                    .parents()
                    .last()
                    .context("reference node has no target")?;
            }
            NodeKind::ShallowPoint { .. } => anyhow::bail!("shallow point has no commit node"),
        }
    }
    anyhow::bail!("reference target cycle")
}

fn commit_id_at(graph: &Graph, index: NodeIndex) -> Option<gix::ObjectId> {
    match graph.nodes()[index].kind() {
        NodeKind::Commit { id } | NodeKind::ShallowPoint { id, .. } => Some(*id),
        NodeKind::Reference(reference) => reference.ref_info.commit_id,
    }
}

fn distance_to(graph: &Graph, start: NodeIndex, wanted: NodeIndex) -> Option<usize> {
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([(start, 0)]);
    while let Some((index, distance)) = queue.pop_front() {
        if index == wanted {
            return Some(distance);
        }
        if seen.insert(index) {
            queue.extend(
                graph.nodes()[index]
                    .parents()
                    .iter()
                    .map(|parent| (*parent, distance + 1)),
            );
        }
    }
    None
}

fn deduplicate(nodes: &mut Vec<NodeIndex>) {
    let mut seen = HashSet::new();
    nodes.retain(|index| seen.insert(*index));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CommitFlags, Node, node::ConstructionContext};

    fn oid(byte: u8) -> gix::ObjectId {
        gix::ObjectId::from_hex(format!("{byte:040x}").as_bytes()).expect("valid test object id")
    }

    fn commit(id: gix::ObjectId, parents: Vec<NodeIndex>) -> Node {
        Node {
            kind: NodeKind::Commit { id },
            parents,
        }
    }

    fn reference(
        name: &str,
        id: gix::ObjectId,
        parents: Vec<NodeIndex>,
        metadata: Option<ReferenceMetadata>,
    ) -> Node {
        Node {
            kind: NodeKind::Reference(Box::new(Reference {
                ref_info: RefInfo {
                    ref_name: name.try_into().expect("valid full ref name"),
                    commit_id: Some(id),
                    worktree: None,
                },
                metadata,
                remote_tracking_ref_name: None,
            })),
            parents,
        }
    }

    fn graph(
        nodes: Vec<Node>,
        entrypoint: NodeIndex,
        entrypoint_ref: &str,
        managed_workspace_commit_id: Option<gix::ObjectId>,
        target_commit_id: gix::ObjectId,
    ) -> Graph {
        let project_meta = but_core::ref_metadata::ProjectMeta {
            target_commit_id: Some(target_commit_id),
            ..Default::default()
        };
        Graph {
            annotations: vec![CommitFlags::default(); nodes.len()],
            nodes,
            context: ConstructionContext {
                entrypoint: NodeGraphEntrypoint::Node(entrypoint),
                entrypoint_ref: Some(
                    entrypoint_ref
                        .try_into()
                        .expect("valid entrypoint ref name"),
                ),
                managed_workspace_commit_id,
                traversal_tips: Vec::new(),
                ad_hoc_branch_stack_orders: Vec::new(),
                hard_limit_hit: false,
                options: crate::init::Options::default(),
                project_meta,
                symbolic_remote_names: Vec::new(),
            },
        }
    }

    #[test]
    fn projects_reference_delimited_ad_hoc_stack_directly_from_nodes() -> anyhow::Result<()> {
        let base = oid(1);
        let lower = oid(2);
        let upper = oid(3);
        let graph = graph(
            vec![
                commit(base, vec![]),
                commit(lower, vec![0]),
                reference("refs/heads/lower", lower, vec![1], None),
                commit(upper, vec![2]),
                reference("refs/heads/upper", upper, vec![3], None),
            ],
            4,
            "refs/heads/upper",
            None,
            base,
        )
        .validated()?;

        let workspace = graph.into_workspace()?;

        assert!(matches!(workspace.kind, WorkspaceKind::AdHoc));
        assert_eq!(workspace.lower_bound, Some(base));
        assert_eq!(workspace.lower_bound_node_id, Some(0));
        assert_eq!(workspace.stacks.len(), 1);
        assert_eq!(workspace.stacks[0].segments.len(), 2);
        assert_eq!(workspace.stacks[0].segments[0].id, 4);
        assert_eq!(workspace.stacks[0].segments[0].tip(), Some(upper));
        assert_eq!(workspace.stacks[0].segments[1].id, 2);
        assert_eq!(workspace.stacks[0].segments[1].tip(), Some(lower));
        Ok(())
    }

    #[test]
    fn nearest_workspace_contains_sibling_ref_at_same_commit() -> anyhow::Result<()> {
        let base = oid(1);
        let managed = oid(2);
        let outer = oid(3);
        let graph = graph(
            vec![
                commit(base, vec![]),
                commit(managed, vec![0]),
                commit(outer, vec![1]),
                reference(
                    "refs/heads/outer-workspace",
                    outer,
                    vec![2],
                    Some(ReferenceMetadata::Workspace(Default::default())),
                ),
                reference(
                    "refs/heads/gitbutler/workspace",
                    managed,
                    vec![1],
                    Some(ReferenceMetadata::Workspace(Default::default())),
                ),
                reference("refs/tags/entrypoint", managed, vec![1], None),
            ],
            5,
            "refs/tags/entrypoint",
            Some(managed),
            base,
        )
        .validated()?;

        let workspace = graph.into_workspace()?;

        assert_eq!(
            workspace.id,
            Some(4),
            "the nearest containing workspace wins"
        );
        let WorkspaceKind::Managed { ref_info } = workspace.kind else {
            panic!("the sibling ref peels to the managed workspace commit");
        };
        assert_eq!(
            ref_info.ref_name.as_bstr(),
            b"refs/heads/gitbutler/workspace",
            "the managed workspace ref is retained"
        );
        Ok(())
    }

    #[test]
    fn projects_managed_overlay_parents_as_independent_stacks() -> anyhow::Result<()> {
        let base = oid(1);
        let left = oid(2);
        let right = oid(3);
        let managed = oid(4);
        let graph = graph(
            vec![
                commit(base, vec![]),
                commit(left, vec![0]),
                reference("refs/heads/left", left, vec![1], None),
                commit(right, vec![0]),
                reference("refs/heads/right", right, vec![3], None),
                commit(managed, vec![1, 3]),
                reference(
                    "refs/heads/gitbutler/workspace",
                    managed,
                    vec![2, 4, 5],
                    Some(ReferenceMetadata::Workspace(Default::default())),
                ),
            ],
            6,
            "refs/heads/gitbutler/workspace",
            Some(managed),
            base,
        )
        .validated()?;

        let workspace = graph.into_workspace()?;

        assert!(matches!(workspace.kind, WorkspaceKind::Managed { .. }));
        assert_eq!(workspace.id, Some(6));
        assert_eq!(workspace.lower_bound_node_id, Some(0));
        assert_eq!(workspace.stacks.len(), 2);
        assert_eq!(
            workspace.stacks[0].ref_name().map(ToString::to_string),
            Some("refs/heads/left".into())
        );
        assert_eq!(
            workspace.stacks[1].ref_name().map(ToString::to_string),
            Some("refs/heads/right".into())
        );
        Ok(())
    }

    #[test]
    fn managed_commit_parent_above_metadata_root_remains_projected() -> anyhow::Result<()> {
        use but_core::ref_metadata::{
            Workspace as WorkspaceMetadata, WorkspaceCommitRelation, WorkspaceStack,
            WorkspaceStackBranch,
        };

        let base = oid(1);
        let lower = oid(2);
        let upper = oid(3);
        let managed = oid(4);
        let stack_id = StackId::from_number_for_testing(1);
        let metadata = WorkspaceMetadata::new(
            Default::default(),
            vec![WorkspaceStack {
                id: stack_id,
                branches: vec![WorkspaceStackBranch {
                    ref_name: "refs/heads/lower".try_into()?,
                    archived: false,
                }],
                workspacecommit_relation: WorkspaceCommitRelation::Merged,
            }],
            Default::default(),
        );
        let graph = graph(
            vec![
                commit(base, vec![]),
                commit(lower, vec![0]),
                reference("refs/heads/lower", lower, vec![1], None),
                commit(upper, vec![2]),
                reference("refs/heads/upper", upper, vec![3], None),
                commit(managed, vec![4]),
                reference(
                    "refs/heads/gitbutler/workspace",
                    managed,
                    vec![2, 5],
                    Some(ReferenceMetadata::Workspace(metadata)),
                ),
            ],
            6,
            "refs/heads/gitbutler/workspace",
            Some(managed),
            base,
        )
        .validated()?;

        let workspace = graph.into_workspace()?;

        assert_eq!(workspace.stacks.len(), 1);
        assert_eq!(workspace.stacks[0].id, Some(stack_id));
        assert_eq!(workspace.stacks[0].tip_skip_empty(), Some(upper));
        assert_eq!(
            workspace.stacks[0]
                .segments
                .iter()
                .filter_map(StackSegment::ref_name)
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["refs/heads/upper", "refs/heads/lower"]
        );
        Ok(())
    }

    #[test]
    fn managed_target_only_parent_is_not_a_stack() -> anyhow::Result<()> {
        let base = oid(1);
        let managed = oid(2);
        let graph = graph(
            vec![
                commit(base, vec![]),
                reference("refs/heads/main", base, vec![0], None),
                commit(managed, vec![1]),
                reference(
                    "refs/heads/gitbutler/workspace",
                    managed,
                    vec![2],
                    Some(ReferenceMetadata::Workspace(Default::default())),
                ),
            ],
            3,
            "refs/heads/gitbutler/workspace",
            Some(managed),
            base,
        )
        .validated()?;

        let workspace = graph.into_workspace()?;

        assert!(matches!(workspace.kind, WorkspaceKind::Managed { .. }));
        assert_eq!(workspace.lower_bound, Some(base));
        assert!(workspace.stacks.is_empty());
        Ok(())
    }

    #[test]
    fn managed_stack_omits_untracked_target_context_segment() -> anyhow::Result<()> {
        use but_core::ref_metadata::{
            Workspace as WorkspaceMetadata, WorkspaceCommitRelation, WorkspaceStack,
            WorkspaceStackBranch,
        };

        let base = oid(1);
        let work = oid(2);
        let managed = oid(3);
        let metadata = WorkspaceMetadata::new(
            Default::default(),
            vec![WorkspaceStack {
                id: StackId::from_number_for_testing(1),
                branches: vec![WorkspaceStackBranch {
                    ref_name: "refs/heads/A".try_into()?,
                    archived: false,
                }],
                workspacecommit_relation: WorkspaceCommitRelation::Merged,
            }],
            Default::default(),
        );
        let mut graph = graph(
            vec![
                commit(base, vec![]),
                reference("refs/heads/main", base, vec![0], None),
                commit(work, vec![1]),
                reference("refs/heads/A", work, vec![2], None),
                commit(managed, vec![2]),
                reference(
                    "refs/heads/gitbutler/workspace",
                    managed,
                    vec![3, 4],
                    Some(ReferenceMetadata::Workspace(metadata)),
                ),
                reference("refs/remotes/origin/main", base, vec![1], None),
            ],
            5,
            "refs/heads/gitbutler/workspace",
            Some(managed),
            base,
        );
        graph.context.project_meta.target_ref = Some("refs/remotes/origin/main".try_into()?);
        let NodeKind::Reference(main) = &mut graph.nodes[1].kind else {
            unreachable!("node 1 is the local target reference")
        };
        main.remote_tracking_ref_name = Some("refs/remotes/origin/main".try_into()?);
        let graph = graph.validated()?;

        let workspace = graph.into_workspace()?;

        assert_eq!(workspace.stacks.len(), 1);
        assert_eq!(workspace.stacks[0].segments.len(), 1);
        assert_eq!(
            workspace.stacks[0].segments[0]
                .ref_name()
                .map(ToString::to_string),
            Some("refs/heads/A".into())
        );
        assert_eq!(workspace.stacks[0].base(), Some(base));
        Ok(())
    }

    #[test]
    fn ordinary_one_parent_same_commit_roots_collapse_to_managed_parent() -> anyhow::Result<()> {
        let base = oid(1);
        let tip = oid(2);
        let managed = oid(3);
        let graph = graph(
            vec![
                commit(base, vec![]),
                commit(tip, vec![0]),
                reference("refs/heads/B", tip, vec![1], None),
                reference("refs/heads/C", tip, vec![1], None),
                commit(managed, vec![2]),
                reference(
                    "refs/heads/gitbutler/workspace",
                    managed,
                    vec![3, 2, 4],
                    Some(ReferenceMetadata::Workspace(Default::default())),
                ),
            ],
            5,
            "refs/heads/gitbutler/workspace",
            Some(managed),
            base,
        )
        .validated()?;

        let workspace = graph.into_workspace()?;

        assert_eq!(workspace.stacks.len(), 1);
        assert_eq!(
            workspace.stacks[0].ref_name().map(ToString::to_string),
            Some("refs/heads/B".into())
        );
        Ok(())
    }

    #[test]
    fn empty_same_commit_applied_roots_at_projection_delimiter_remain_independent()
    -> anyhow::Result<()> {
        use but_core::ref_metadata::{
            Workspace as WorkspaceMetadata, WorkspaceCommitRelation, WorkspaceStack,
            WorkspaceStackBranch,
        };

        let base = oid(1);
        let managed = oid(2);
        let metadata = WorkspaceMetadata::new(
            Default::default(),
            ["A", "B"]
                .into_iter()
                .enumerate()
                .map(|(index, name)| WorkspaceStack {
                    id: StackId::from_number_for_testing(index as u128 + 1),
                    branches: vec![WorkspaceStackBranch {
                        ref_name: format!("refs/heads/{name}").try_into().expect("valid ref"),
                        archived: false,
                    }],
                    workspacecommit_relation: WorkspaceCommitRelation::Merged,
                })
                .collect(),
            Default::default(),
        );
        let graph = graph(
            vec![
                commit(base, vec![]),
                reference("refs/heads/A", base, vec![0], None),
                reference("refs/heads/B", base, vec![0], None),
                commit(managed, vec![1]),
                reference(
                    "refs/heads/gitbutler/workspace",
                    managed,
                    vec![1, 2, 3],
                    Some(ReferenceMetadata::Workspace(metadata)),
                ),
            ],
            4,
            "refs/heads/gitbutler/workspace",
            Some(managed),
            base,
        )
        .validated()?;

        let workspace = graph.into_workspace()?;

        assert_eq!(workspace.stacks.len(), 2);
        assert_eq!(
            workspace.stacks[0].id,
            Some(StackId::from_number_for_testing(1))
        );
        assert_eq!(
            workspace.stacks[1].id,
            Some(StackId::from_number_for_testing(2))
        );
        Ok(())
    }

    #[test]
    fn duplicate_managed_parents_preserve_same_commit_applied_roots() -> anyhow::Result<()> {
        use but_core::ref_metadata::{
            Workspace as WorkspaceMetadata, WorkspaceCommitRelation, WorkspaceStack,
            WorkspaceStackBranch,
        };

        let base = oid(1);
        let tip = oid(2);
        let managed = oid(3);
        let metadata = WorkspaceMetadata::new(
            Default::default(),
            ["A", "B"]
                .into_iter()
                .enumerate()
                .map(|(index, name)| WorkspaceStack {
                    id: StackId::from_number_for_testing(index as u128 + 1),
                    branches: vec![WorkspaceStackBranch {
                        ref_name: format!("refs/heads/{name}").try_into().expect("valid ref"),
                        archived: false,
                    }],
                    workspacecommit_relation: WorkspaceCommitRelation::Merged,
                })
                .collect(),
            Default::default(),
        );
        let graph = graph(
            vec![
                commit(base, vec![]),
                commit(tip, vec![0]),
                reference("refs/heads/A", tip, vec![1], None),
                reference("refs/heads/B", tip, vec![1], None),
                commit(managed, vec![2, 3]),
                reference(
                    "refs/heads/gitbutler/workspace",
                    managed,
                    vec![2, 3, 4],
                    Some(ReferenceMetadata::Workspace(metadata)),
                ),
            ],
            5,
            "refs/heads/gitbutler/workspace",
            Some(managed),
            base,
        )
        .validated()?;

        let workspace = graph.into_workspace()?;

        assert_eq!(workspace.stacks.len(), 2);
        assert_eq!(
            workspace.stacks[0].id,
            Some(StackId::from_number_for_testing(1))
        );
        assert_eq!(
            workspace.stacks[1].id,
            Some(StackId::from_number_for_testing(2))
        );
        Ok(())
    }

    #[test]
    fn empty_reference_at_lower_bound_retains_its_base() -> anyhow::Result<()> {
        let base = oid(1);
        let graph = graph(
            vec![
                commit(base, vec![]),
                reference("refs/heads/independent", base, vec![0], None),
            ],
            1,
            "refs/heads/independent",
            None,
            base,
        )
        .validated()?;

        let workspace = graph.into_workspace()?;

        assert_eq!(workspace.lower_bound, Some(base));
        assert_eq!(workspace.lower_bound_node_id, Some(0));
        assert_eq!(workspace.stacks.len(), 1);
        assert_eq!(workspace.stacks[0].tip_skip_empty(), None);
        assert_eq!(workspace.stacks[0].base(), Some(base));
        Ok(())
    }

    #[test]
    fn same_commit_reference_chain_reaches_its_lower_bound() -> anyhow::Result<()> {
        let base = oid(1);
        let graph = graph(
            vec![
                commit(base, vec![]),
                reference("refs/heads/bottom", base, vec![0], None),
                reference("refs/heads/top", base, vec![1], None),
            ],
            2,
            "refs/heads/top",
            None,
            base,
        )
        .validated()?;

        let workspace = graph.into_workspace()?;

        assert_eq!(workspace.stacks.len(), 1);
        assert_eq!(workspace.stacks[0].segments.len(), 2);
        assert_eq!(
            workspace.stacks[0]
                .segments
                .iter()
                .filter_map(StackSegment::ref_name)
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["refs/heads/top", "refs/heads/bottom"]
        );
        assert_eq!(workspace.stacks[0].base(), Some(base));
        Ok(())
    }

    #[test]
    fn target_descendant_uses_common_ancestor_as_projection_delimiter() -> anyhow::Result<()> {
        let base = oid(1);
        let target = oid(2);
        let managed = oid(3);
        let graph = graph(
            vec![
                commit(base, vec![]),
                commit(target, vec![0]),
                reference("refs/heads/bottom", base, vec![0], None),
                reference("refs/heads/top", base, vec![2], None),
                commit(managed, vec![3]),
                reference(
                    "refs/heads/gitbutler/workspace",
                    managed,
                    vec![3, 4],
                    Some(ReferenceMetadata::Workspace(Default::default())),
                ),
            ],
            5,
            "refs/heads/gitbutler/workspace",
            Some(managed),
            target,
        )
        .validated()?;

        let workspace = graph.into_workspace()?;

        assert_eq!(workspace.stored_target_commit_id(), Some(target));
        assert_eq!(workspace.lower_bound, Some(base));
        assert_eq!(workspace.lower_bound_node_id, Some(0));
        assert_eq!(workspace.stacks.len(), 1);
        assert_eq!(workspace.stacks[0].segments.len(), 2);
        assert!(
            workspace.stacks[0]
                .segments
                .iter()
                .all(|segment| segment.commits.is_empty())
        );
        Ok(())
    }

    #[test]
    fn ad_hoc_single_stack_without_target_includes_its_tip() -> anyhow::Result<()> {
        let tip = oid(1);
        let mut graph = graph(
            vec![
                commit(tip, vec![]),
                reference("refs/heads/main", tip, vec![0], None),
            ],
            1,
            "refs/heads/main",
            None,
            tip,
        );
        graph.context.project_meta.target_commit_id = None;
        let graph = graph.validated()?;

        let workspace = graph.into_workspace()?;

        assert_eq!(workspace.lower_bound, None);
        assert_eq!(workspace.lower_bound_node_id, None);
        assert_eq!(workspace.stacks.len(), 1);
        assert_eq!(workspace.stacks[0].tip_skip_empty(), Some(tip));
        assert_eq!(workspace.stacks[0].base(), None);
        Ok(())
    }

    #[test]
    fn advanced_workspace_reference_is_missing_its_managed_commit() -> anyhow::Result<()> {
        let base = oid(1);
        let managed = oid(2);
        let advanced = oid(3);
        let graph = graph(
            vec![
                commit(base, vec![]),
                commit(managed, vec![0]),
                commit(advanced, vec![1]),
                reference(
                    "refs/heads/gitbutler/workspace",
                    advanced,
                    vec![2],
                    Some(ReferenceMetadata::Workspace(Default::default())),
                ),
            ],
            3,
            "refs/heads/gitbutler/workspace",
            Some(managed),
            base,
        )
        .validated()?;

        let workspace = graph.into_workspace()?;

        assert!(matches!(
            workspace.kind,
            WorkspaceKind::ManagedMissingWorkspaceCommit { .. }
        ));
        Ok(())
    }

    #[test]
    fn workspace_metadata_target_commit_remains_the_lower_bound() -> anyhow::Result<()> {
        let base = oid(1);
        let metadata = ref_metadata::Workspace::new(
            Default::default(),
            Vec::new(),
            but_core::ref_metadata::ProjectMeta {
                target_commit_id: Some(base),
                ..Default::default()
            },
        );
        let mut graph = graph(
            vec![
                commit(base, vec![]),
                reference(
                    "refs/heads/gitbutler/workspace",
                    base,
                    vec![0],
                    Some(ReferenceMetadata::Workspace(metadata)),
                ),
            ],
            1,
            "refs/heads/gitbutler/workspace",
            None,
            base,
        );
        graph.context.project_meta.target_commit_id = None;
        let graph = graph.validated()?;

        let workspace = graph.into_workspace()?;

        assert_eq!(workspace.stored_target_commit_id(), Some(base));
        assert_eq!(workspace.lower_bound, Some(base));
        assert_eq!(workspace.lower_bound_node_id, Some(0));
        assert!(workspace.stacks.is_empty());
        Ok(())
    }
}

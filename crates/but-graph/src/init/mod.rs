use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

use anyhow::{Context as _, Result, bail};
use but_core::{RefMetadata, ref_metadata::ProjectMeta};
use gix::refs::Category;

use crate::{
    CommitFlags, NodeGraph, NodeGraphEntrypoint, NodeIndex, NodeKind, Reference,
    node::ConstructionContext,
};

mod node_traversal;
mod overlay;
mod reference_discovery;
mod reference_groups;
mod remotes;
mod walk;

use overlay::{OverlayMetadata, OverlayRepo};
use walk::{obtain_workspace_infos, try_refname_to_id};

pub(crate) type Entrypoint = Option<(gix::ObjectId, Option<gix::refs::FullName>)>;

/// A way to define information served from memory instead of the underlying
/// repository while rebuilding a graph.
#[derive(Debug, Default, Clone)]
pub struct Overlay {
    entrypoint: Entrypoint,
    nonoverriding_references: Vec<gix::refs::Reference>,
    overriding_references: Vec<gix::refs::Reference>,
    dropped_references: Vec<gix::refs::FullName>,
    meta_branches: Vec<(gix::refs::FullName, but_core::ref_metadata::Branch)>,
    branch_stack_orders: Vec<Vec<gix::refs::FullName>>,
    workspace: Option<(gix::refs::FullName, but_core::ref_metadata::Workspace)>,
}

#[derive(Default)]
struct SeedPlan {
    tips: BTreeSet<gix::ObjectId>,
    entrypoint_roots: BTreeSet<gix::ObjectId>,
    workspace_roots: BTreeSet<gix::ObjectId>,
    target_roots: BTreeSet<gix::ObjectId>,
}

/// Node-native construction.
impl NodeGraph {
    /// Construct a graph from the repository and optional in-memory overrides.
    ///
    /// With an empty overlay, traversal starts at `HEAD`. An overlay entrypoint
    /// provides an explicit commit and optional symbolic identity.
    pub fn from_repo(
        repo: &gix::Repository,
        meta: &impl RefMetadata,
        project_meta: ProjectMeta,
        overlay: Overlay,
    ) -> Result<Self> {
        let (overlay_repo, meta, overlay_entrypoint) = overlay.into_parts(repo, meta);
        let (entrypoint, ref_name) = match overlay_entrypoint {
            Some(entrypoint) => entrypoint,
            None => match repo.head()?.kind {
                gix::head::Kind::Unborn(ref_name) => {
                    let wt_by_branch = BTreeMap::from([(
                        ref_name.clone(),
                        vec![crate::Worktree {
                            kind: crate::WorktreeKind::Main,
                            owned_by_repo: true,
                        }],
                    )]);
                    let reference = Reference {
                        ref_info: crate::RefInfo::from_ref(ref_name.clone(), None, &wt_by_branch),
                        metadata: reference_discovery::metadata_for_ref(&meta, ref_name.as_ref())?,
                        remote_tracking_ref_name: None,
                    };
                    return NodeGraph {
                        nodes: Vec::new(),
                        annotations: Vec::new(),
                        context: ConstructionContext {
                            entrypoint: NodeGraphEntrypoint::Unborn(Box::new(reference)),
                            entrypoint_ref: Some(ref_name),
                            managed_workspace_commit_id: None,
                            project_meta,
                        },
                    }
                    .validated();
                }
                gix::head::Kind::Detached { target, peeled } => (peeled.unwrap_or(target), None),
                gix::head::Kind::Symbolic(existing_reference) => {
                    let name = existing_reference.name;
                    let id = overlay_repo
                        .try_find_reference(name.as_ref())?
                        .with_context(|| format!("HEAD reference {name} does not exist"))?
                        .peel_to_id()?
                        .detach();
                    (id, Some(name))
                }
            },
        };
        Self::build(&overlay_repo, entrypoint, ref_name, &meta, project_meta)
    }

    fn build<T: RefMetadata>(
        repo: &OverlayRepo<'_>,
        entrypoint: gix::ObjectId,
        entrypoint_ref: Option<gix::refs::FullName>,
        meta: &OverlayMetadata<'_, T>,
        project_meta: ProjectMeta,
    ) -> Result<Self> {
        if entrypoint_ref
            .as_ref()
            .is_some_and(|name| name.category() == Some(Category::RemoteBranch))
        {
            bail!("Cannot currently handle remotes as start position");
        }

        let mut plan = seed_plan(
            repo,
            meta,
            entrypoint,
            entrypoint_ref.as_ref(),
            &project_meta,
        )?;
        let nodes = node_traversal::traverse(repo, std::mem::take(&mut plan.tips))?;
        let entrypoint_index = nodes
            .iter()
            .position(|node| matches!(node.kind(), NodeKind::Commit { id } if *id == entrypoint))
            .context("BUG: traversal omitted its entrypoint")?;
        let ad_hoc_branch_stack_orders: Vec<Vec<gix::refs::FullName>> = entrypoint_ref
            .as_ref()
            .map(|name| meta.branch_stack_order(name.as_ref()))
            .transpose()?
            .flatten()
            .into_iter()
            .collect();
        let mut graph = NodeGraph {
            annotations: vec![CommitFlags::empty(); nodes.len()],
            nodes,
            context: ConstructionContext {
                entrypoint: NodeGraphEntrypoint::Node(entrypoint_index),
                entrypoint_ref,
                managed_workspace_commit_id: None,
                project_meta,
            },
        };
        apply_annotations(&mut graph, &plan);
        let mut workspace_roots = plan.workspace_roots.clone();
        if graph
            .context
            .entrypoint_ref
            .as_ref()
            .is_some_and(|name| but_core::is_workspace_ref_name(name.as_ref()))
        {
            workspace_roots.insert(entrypoint);
        }
        graph.context.managed_workspace_commit_id =
            managed_workspace_commit_id(repo, &graph, &workspace_roots)?;
        reference_discovery::discover_and_apply_reference_groups(
            graph.validated()?,
            repo,
            meta,
            &ad_hoc_branch_stack_orders,
        )
    }
}

fn seed_plan<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    project_meta: &ProjectMeta,
) -> Result<SeedPlan> {
    let mut plan = SeedPlan::default();
    plan.tips.insert(entrypoint);
    plan.entrypoint_roots.insert(entrypoint);

    for (workspace_id, _workspace_ref, workspace) in
        obtain_workspace_infos(repo, entrypoint_ref.map(|name| name.as_ref()), meta)?
    {
        plan.tips.insert(workspace_id);
        plan.entrypoint_roots.insert(workspace_id);
        plan.workspace_roots.insert(workspace_id);
        for branch in workspace
            .stacks
            .into_iter()
            .filter(|stack| stack.is_in_workspace())
            .flat_map(|stack| stack.branches)
        {
            if let Some(id) = try_refname_to_id(repo, branch.ref_name.as_ref())? {
                plan.tips.insert(id);
                plan.entrypoint_roots.insert(id);
            }
        }
    }

    if let Some(target_ref) = project_meta.target_ref.as_ref()
        && let Some(target_id) = try_refname_to_id(repo, target_ref.as_ref())?
    {
        plan.tips.insert(target_id);
        plan.target_roots.insert(target_id);
        if let Some((local_ref, _remote)) =
            repo.upstream_branch_and_remote_for_tracking_branch(target_ref.as_ref())?
            && let Some(local_id) = try_refname_to_id(repo, local_ref.as_ref())?
        {
            plan.tips.insert(local_id);
            plan.entrypoint_roots.insert(local_id);
        }
    }

    if let Some(target_id) = project_meta.target_commit_id {
        if repo.find_commit(target_id).is_ok() {
            plan.tips.insert(target_id);
            plan.target_roots.insert(target_id);
        } else {
            tracing::warn!(%target_id, "ignoring stale target commit");
        }
    }
    Ok(plan)
}

fn apply_annotations(graph: &mut NodeGraph, plan: &SeedPlan) {
    let commit_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| match node.kind {
            NodeKind::Commit { id } => Some((id, index)),
            NodeKind::Reference(_) | NodeKind::Boundary { .. } | NodeKind::None => None,
        })
        .collect::<HashMap<_, _>>();
    mark_reachable(
        graph,
        &commit_by_id,
        &plan.entrypoint_roots,
        CommitFlags::EntrypointSide,
    );
    mark_reachable(
        graph,
        &commit_by_id,
        &plan.target_roots,
        CommitFlags::TargetSide,
    );
}

fn mark_reachable(
    graph: &mut NodeGraph,
    commit_by_id: &HashMap<gix::ObjectId, NodeIndex>,
    roots: &BTreeSet<gix::ObjectId>,
    flag: CommitFlags,
) {
    let mut pending = roots
        .iter()
        .filter_map(|id| commit_by_id.get(id).copied())
        .collect::<Vec<_>>();
    let mut seen = HashSet::new();
    while let Some(index) = pending.pop() {
        if !seen.insert(index) || !matches!(graph.nodes[index].kind, NodeKind::Commit { .. }) {
            continue;
        }
        graph.annotations[index] |= flag;
        pending.extend(graph.nodes[index].parents.iter().copied());
    }
}

fn managed_workspace_commit_id(
    repo: &OverlayRepo<'_>,
    graph: &NodeGraph,
    roots: &BTreeSet<gix::ObjectId>,
) -> Result<Option<gix::ObjectId>> {
    let commit_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| match node.kind {
            NodeKind::Commit { id } => Some((id, index)),
            NodeKind::Reference(_) | NodeKind::Boundary { .. } | NodeKind::None => None,
        })
        .collect::<HashMap<_, _>>();
    let mut pending = roots
        .iter()
        .filter_map(|id| commit_by_id.get(id).copied())
        .collect::<VecDeque<_>>();
    let mut seen = HashSet::new();
    while let Some(index) = pending.pop_front() {
        if !seen.insert(index) {
            continue;
        }
        let NodeKind::Commit { id } = graph.nodes[index].kind else {
            continue;
        };
        let commit = repo.find_commit(id)?;
        if crate::workspace::commit::is_managed_workspace_by_message(commit.message_raw()?) {
            return Ok(Some(id));
        }
        pending.extend(graph.nodes[index].parents.iter().copied());
    }
    Ok(None)
}

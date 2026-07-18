use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use anyhow::{Context as _, Result, bail};
use but_core::{RefMetadata, ref_metadata::ProjectMeta};
use gix::refs::Category;

use crate::{
    CommitFlags, Node, NodeGraph, NodeGraphEntrypoint, NodeIndex, NodeKind, StopCondition,
    node::ConstructionContext,
};

use super::{
    InitialTips, Options, OverlayMetadata, OverlayRepo, Tip, TipRole, initial_tips_from_tips,
    queue_should_frontload_tip, remotes,
    types::{Goals, Limit, Queue},
    validate_explicit_tips,
    walk::{TraverseInfo, find, try_refname_to_id},
};

type CommitQueue = Queue<QueueSource>;
type TraversalState = (u32, Option<usize>, u32);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum QueueSource {
    Tip,
    Parent,
}

/// Traverse commits directly into the vector-backed construction graph.
///
/// References are used only to discover remote tips in this phase. They are
/// placed into the graph by the later reference-group phase.
pub(crate) fn traverse_tips<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    tips: Vec<Tip>,
    meta: &OverlayMetadata<'_, T>,
    project_meta: ProjectMeta,
    options: Options,
    entrypoint_ref_override: Option<gix::refs::FullName>,
) -> Result<NodeGraph> {
    let entrypoint = validate_explicit_tips(repo, &tips, entrypoint_ref_override.as_ref())?;
    let entrypoint_id = entrypoint.id;
    let entrypoint_ref = if entrypoint.is_detached {
        None
    } else {
        entrypoint_ref_override.or_else(|| entrypoint.ref_name.clone())
    };
    if entrypoint_ref
        .as_ref()
        .is_some_and(|name| name.category() == Some(Category::RemoteBranch))
    {
        bail!("Cannot currently handle remotes as start position");
    }

    let initial_tips =
        initial_tips_from_tips(repo, tips, &project_meta, options.extra_target_commit_id);
    let commit_graph = repo.commit_graph_if_enabled()?;
    let shallow_commits = repo.shallow_commits()?;
    let configured_remote_tracking_branches = remotes::configured_remote_tracking_branches(repo)?;
    let refs_by_id = repo.collect_ref_mapping_by_prefix(
        ["refs/heads/", "refs/remotes/"]
            .into_iter()
            .chain(options.collect_tags.then_some("refs/tags/")),
        &initial_tips
            .workspace_ref_names
            .iter()
            .map(|name| name.as_ref())
            .collect::<Vec<_>>(),
    )?;

    let mut goals = Goals::default();
    let max_limit = Limit::new(options.commits_limit_hint);
    let target_limit = max_limit
        .with_indirect_goal(entrypoint_id, &mut goals)
        .without_allowance();
    let entrypoint_flags = CommitFlags::NotInRemote
        | goals
            .flag_for(entrypoint_id)
            .context("more than one goal bit is available")?;
    let mut queue = CommitQueue::new_with_limit(options.hard_limit);
    let mut builder = Builder::default();
    let traversal_tips = queue_initial_tips(
        repo,
        &initial_tips,
        entrypoint_id,
        entrypoint_flags,
        max_limit,
        target_limit,
        &mut goals,
        &mut queue,
        commit_graph.as_ref(),
    )?;
    if !traversal_tips
        .iter()
        .any(|tip| tip.is_entrypoint && tip.id == entrypoint_id)
    {
        bail!("hard limit rejected the traversal entrypoint {entrypoint_id}");
    }
    let mut scheduled_remote_refs = traversal_tips
        .iter()
        .filter_map(|tip| tip.ref_name.clone())
        .filter(|name| name.category() == Some(Category::RemoteBranch))
        .collect::<BTreeSet<_>>();
    let target_refs = initial_tips.target_refs.iter().cloned().collect();
    let mut recharge_locations = options.commits_limit_recharge_location.clone();
    recharge_locations.sort();
    let mut buf = Vec::new();
    let mut initial_items_left = queue.iter().count();
    let mut entrypoint_index = None;

    while let Some((info, queued_flags, source, mut limit)) = queue.pop_front() {
        initial_items_left = initial_items_left.saturating_sub(1);
        if recharge_locations.binary_search(&info.id).is_ok() {
            limit.set_but_keep_goal(max_limit);
        }

        let index = builder.ensure_commit(info.id)?;
        if info.id == entrypoint_id {
            entrypoint_index.get_or_insert(index);
        }
        let is_shallow_boundary = shallow_commits
            .as_ref()
            .is_some_and(|boundaries| boundaries.binary_search(&info.id).is_ok());
        let mut flags = builder.flags[index] | queued_flags;
        if is_shallow_boundary {
            flags |= CommitFlags::ShallowBoundary;
        }

        let refs = refs_by_id.get(&info.id).cloned().unwrap_or_default();
        let mut remote_items = Vec::new();
        for (remote_ref, remote_tip) in discover_remote_tips(
            repo,
            &refs,
            &initial_tips.symbolic_remote_names,
            &configured_remote_tracking_branches,
            &target_refs,
            &mut scheduled_remote_refs,
        )? {
            let remote_limit = limit.with_indirect_goal(info.id, &mut goals);
            let remote_flags = goals.flag_for(remote_tip).unwrap_or_default();
            flags |= remote_limit.goal_flags();
            limit = limit.additional_goal(remote_flags);
            remote_items.push((
                find(
                    commit_graph.as_ref(),
                    repo.for_find_only(),
                    remote_tip,
                    &mut buf,
                )?,
                remote_flags,
                QueueSource::Tip,
                remote_limit,
            ));
            tracing::trace!(%remote_ref, %remote_tip, "queued remote tip for commit traversal");
        }
        builder.flags[index] = flags;
        builder.annotations[index] = flags & CommitFlags::all();

        let is_convergence = !builder.processed[index].is_empty();
        if is_convergence {
            builder.merge_converged_state(index, flags, limit, &mut queue);
        }

        let (allowance, goal) = limit.traversal_state();
        let state = (flags.bits(), allowance, goal);
        if builder.processed[index].insert(state) && !(is_convergence && source == QueueSource::Tip)
        {
            queue_parents(
                &mut builder,
                &mut queue,
                index,
                &info,
                flags,
                limit,
                is_shallow_boundary,
                commit_graph.as_ref(),
                repo,
                &mut buf,
            )?;
        }

        for item in remote_items {
            if queue.push_back_exhausted(item) {
                break;
            }
        }

        if let Some(entrypoint_index) = entrypoint_index {
            prune_integrated_tips(&builder, entrypoint_index, &mut queue);
        }
        if initial_items_left == 0 {
            queue.sort();
        }
    }

    let ad_hoc_branch_stack_orders = entrypoint_ref
        .as_ref()
        .map(|name| meta.branch_stack_order(name.as_ref()))
        .transpose()?
        .flatten()
        .into_iter()
        .collect();
    let entrypoint_index =
        entrypoint_index.context("BUG: accepted traversal entrypoint was not processed")?;
    let (nodes, annotations, entrypoint_index) = builder.finish(entrypoint_index);
    let traversed_commit_ids = nodes
        .iter()
        .filter_map(|node| match node.kind {
            NodeKind::Commit { id } => Some(id),
            NodeKind::Reference(_) | NodeKind::ShallowPoint { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    let managed_workspace_commit_id = managed_workspace_commit_id(
        repo,
        &traversal_tips,
        &traversed_commit_ids,
        entrypoint_ref.as_ref().map(|name| name.as_ref()),
    )?;
    NodeGraph {
        nodes,
        annotations,
        context: ConstructionContext {
            entrypoint: NodeGraphEntrypoint::Node(entrypoint_index),
            entrypoint_ref,
            managed_workspace_commit_id,
            traversal_tips,
            ad_hoc_branch_stack_orders,
            hard_limit_hit: queue.hard_limit_hit(),
            options,
            project_meta,
            symbolic_remote_names: initial_tips.symbolic_remote_names,
        },
    }
    .validated()
}

fn managed_workspace_commit_id(
    repo: &OverlayRepo<'_>,
    traversal_tips: &[Tip],
    traversed_commit_ids: &BTreeSet<gix::ObjectId>,
    entrypoint_ref: Option<&gix::refs::FullNameRef>,
) -> Result<Option<gix::ObjectId>> {
    let entrypoint_is_workspace = entrypoint_ref.is_some_and(but_core::is_workspace_ref_name);
    for tip in traversal_tips.iter().filter(|tip| {
        matches!(tip.role, TipRole::Workspace)
            || tip
                .ref_name
                .as_ref()
                .is_some_and(|name| but_core::is_workspace_ref_name(name.as_ref()))
            || (entrypoint_is_workspace && tip.is_entrypoint)
    }) {
        // Breadth-first traversal selects the nearest match, with Git parent order as the tie-breaker.
        let mut candidates = VecDeque::from([tip.id]);
        let mut seen = HashSet::new();
        while let Some(id) = candidates.pop_front() {
            if !traversed_commit_ids.contains(&id) || !seen.insert(id) {
                continue;
            }
            let commit = repo.find_commit(id)?;
            if crate::workspace::commit::is_managed_workspace_by_message(commit.message_raw()?) {
                return Ok(Some(id));
            }
            candidates.extend(commit.parent_ids().map(|id| id.detach()));
        }
    }
    Ok(None)
}

#[expect(clippy::too_many_arguments)]
fn queue_initial_tips(
    repo: &OverlayRepo<'_>,
    initial_tips: &InitialTips,
    entrypoint: gix::ObjectId,
    entrypoint_flags: CommitFlags,
    max_limit: Limit,
    target_limit: Limit,
    goals: &mut Goals,
    queue: &mut CommitQueue,
    commit_graph: Option<&gix::commitgraph::Graph>,
) -> Result<Vec<Tip>> {
    let mut local_goals = HashMap::new();
    let mut entrypoint_extra_goals = CommitFlags::empty();
    for tip in &initial_tips.tips {
        if let TipRole::TargetLocal { local_ref_name } = &tip.role {
            let goal = goals.flag_for(tip.id).unwrap_or_default();
            local_goals.insert(local_ref_name.clone(), goal);
            entrypoint_extra_goals |= goal;
        }
    }

    let mut buf = Vec::new();
    let mut effective_tips = Vec::new();
    let mut entrypoint_queued = false;
    for tip in &initial_tips.tips {
        let queued = queue.iter_mut().find(|(info, _, _, _)| info.id == tip.id);
        let merged_into_queued = match (&tip.role, queued) {
            (TipRole::WorkspaceStackBranch { .. }, Some((_, flags, _, limit))) => {
                *flags |= CommitFlags::NotInRemote;
                *limit = limit.additional_goal(goals.flag_for(entrypoint).unwrap_or_default());
                true
            }
            (TipRole::TargetRemote, Some((_, flags, _, _)))
                if tip.is_auxiliary_integrated_tip(&initial_tips.auxiliary_integrated_tip_ids) =>
            {
                *flags |= CommitFlags::Integrated;
                true
            }
            _ => false,
        };
        if merged_into_queued {
            effective_tips.push(tip.clone());
            continue;
        }
        // Leave one hard-limit slot for the entrypoint until it is queued.
        // This keeps admission authoritative without changing the normalized
        // processing order of the accepted initial tips.
        let required_capacity = if entrypoint_queued || tip.is_entrypoint {
            1
        } else {
            2
        };
        if !queue.can_accept(required_capacity) {
            continue;
        }
        let (flags, limit) = match &tip.role {
            TipRole::Reachable if tip.is_entrypoint => (
                entrypoint_flags,
                max_limit.additional_goal(entrypoint_extra_goals),
            ),
            TipRole::Reachable => (
                CommitFlags::NotInRemote,
                max_limit.with_indirect_goal(entrypoint, goals),
            ),
            TipRole::Workspace => {
                let extra = if tip.is_entrypoint {
                    entrypoint_flags
                } else {
                    CommitFlags::empty()
                };
                let limit = if tip.is_entrypoint {
                    max_limit.additional_goal(entrypoint_extra_goals)
                } else {
                    max_limit.with_indirect_goal(entrypoint, goals)
                };
                (
                    CommitFlags::InWorkspace | CommitFlags::NotInRemote | extra,
                    limit,
                )
            }
            TipRole::WorkspaceStackBranch { .. } => (
                CommitFlags::NotInRemote,
                max_limit.with_indirect_goal(entrypoint, goals),
            ),
            TipRole::TargetLocal { local_ref_name } => (
                CommitFlags::NotInRemote
                    | local_goals.get(local_ref_name).copied().unwrap_or_default(),
                target_limit,
            ),
            TipRole::TargetRemote => {
                let local_goal = tip
                    .ref_name
                    .as_ref()
                    .and_then(|target| initial_tips.target_local_links.local_by_target.get(target))
                    .and_then(|local| local_goals.get(local))
                    .copied()
                    .unwrap_or_default();
                (
                    CommitFlags::Integrated,
                    target_limit.additional_goal(local_goal),
                )
            }
        };
        let item = (
            find(commit_graph, repo.for_find_only(), tip.id, &mut buf)?,
            flags,
            QueueSource::Tip,
            limit,
        );
        if queue_should_frontload_tip(
            tip,
            initial_tips.frontload_workspace_related_tips,
            &initial_tips.auxiliary_integrated_tip_ids,
        ) {
            _ = queue.push_front_exhausted(item);
        } else {
            _ = queue.push_back_exhausted(item);
        }
        entrypoint_queued |= tip.is_entrypoint;
        effective_tips.push(tip.clone());
    }
    Ok(effective_tips)
}

#[expect(clippy::too_many_arguments)]
fn queue_parents(
    builder: &mut Builder,
    queue: &mut CommitQueue,
    child: NodeIndex,
    info: &TraverseInfo,
    flags: CommitFlags,
    mut limit: Limit,
    is_shallow_boundary: bool,
    commit_graph: Option<&gix::commitgraph::Graph>,
    repo: &OverlayRepo<'_>,
    buf: &mut Vec<u8>,
) -> Result<()> {
    let parent_ids = info.parent_ids.iter().copied().collect::<Vec<_>>();
    let stop_reason = if is_shallow_boundary {
        Some(StopCondition::ShallowBoundary)
    } else if queue.is_exhausted() || limit.is_exhausted_or_decrement(flags, queue) {
        Some(StopCondition::Limit)
    } else {
        None
    };
    let mut parents = Vec::with_capacity(parent_ids.len());
    if let Some(reason) = stop_reason {
        for id in parent_ids {
            parents.push(builder.ensure_shallow(id, reason));
        }
    } else {
        let per_parent = limit.per_parent(parent_ids.len().max(1));
        for id in parent_ids {
            let parent = builder.ensure_commit(id)?;
            parents.push(parent);
            let item = (
                find(commit_graph, repo.for_find_only(), id, buf)?,
                flags,
                QueueSource::Parent,
                per_parent,
            );
            if info.parent_ids.len() > 1 {
                _ = queue.push_back_even_if_exhausted(item);
            } else {
                _ = queue.push_back_exhausted(item);
            }
        }
    }
    builder.merge_parents(child, parents)?;
    builder.propagate_durable_flags(child, flags);
    Ok(())
}

fn discover_remote_tips(
    repo: &OverlayRepo<'_>,
    refs: &[gix::refs::FullName],
    symbolic_remote_names: &[String],
    configured_remote_tracking_branches: &BTreeSet<gix::refs::FullName>,
    target_refs: &BTreeSet<gix::refs::FullName>,
    scheduled_remote_refs: &mut BTreeSet<gix::refs::FullName>,
) -> Result<Vec<(gix::refs::FullName, gix::ObjectId)>> {
    let mut out = Vec::new();
    for name in refs {
        let Some(remote) = remotes::lookup_remote_tracking_branch_or_deduce_it(
            repo,
            name.as_ref(),
            symbolic_remote_names,
            configured_remote_tracking_branches,
        )?
        else {
            continue;
        };
        if target_refs.contains(&remote) || !scheduled_remote_refs.insert(remote.clone()) {
            continue;
        }
        if let Some(id) = try_refname_to_id(repo, remote.as_ref())? {
            out.push((remote, id));
        }
    }
    Ok(out)
}

fn prune_integrated_tips(builder: &Builder, entrypoint: NodeIndex, queue: &mut CommitQueue) {
    if queue.is_exhausted()
        || !queue.iter().all(|(_, flags, _, limit)| {
            flags.contains(CommitFlags::Integrated) && limit.goal_reached()
        })
        || builder.annotations[entrypoint].contains(CommitFlags::Integrated)
    {
        return;
    }
    queue.exhaust();
}

#[derive(Default)]
struct Builder {
    nodes: Vec<Node>,
    annotations: Vec<CommitFlags>,
    flags: Vec<CommitFlags>,
    processed: Vec<HashSet<TraversalState>>,
    commits_by_id: HashMap<gix::ObjectId, NodeIndex>,
    shallow_by_key: HashMap<(gix::ObjectId, u8), NodeIndex>,
}

impl Builder {
    fn ensure_commit(&mut self, id: gix::ObjectId) -> Result<NodeIndex> {
        if let Some(&index) = self.commits_by_id.get(&id) {
            return Ok(index);
        }
        let index = self.nodes.len();
        self.commits_by_id.insert(id, index);
        self.nodes.push(Node {
            kind: NodeKind::Commit { id },
            parents: Vec::new(),
        });
        self.annotations.push(CommitFlags::empty());
        self.flags.push(CommitFlags::empty());
        self.processed.push(HashSet::new());
        Ok(index)
    }

    fn ensure_shallow(&mut self, id: gix::ObjectId, reason: StopCondition) -> NodeIndex {
        let key = (id, reason.bits());
        if let Some(&index) = self.shallow_by_key.get(&key) {
            return index;
        }
        let index = self.nodes.len();
        self.shallow_by_key.insert(key, index);
        self.nodes.push(Node {
            kind: NodeKind::ShallowPoint { id, reason },
            parents: Vec::new(),
        });
        self.annotations.push(CommitFlags::empty());
        self.flags.push(CommitFlags::empty());
        self.processed.push(HashSet::new());
        index
    }

    fn merge_parents(&mut self, child: NodeIndex, parents: Vec<NodeIndex>) -> Result<()> {
        if self.nodes[child].parents.is_empty() {
            self.nodes[child].parents = parents;
            return Ok(());
        }
        if self.nodes[child].parents.len() != parents.len() {
            bail!(
                "BUG: commit parent count changed from {} to {} between traversal states",
                self.nodes[child].parents.len(),
                parents.len()
            );
        }

        for (parent_order, new_parent) in parents.into_iter().enumerate() {
            let old_parent = self.nodes[child].parents[parent_order];
            let old_kind = &self.nodes[old_parent].kind;
            let new_kind = &self.nodes[new_parent].kind;
            let old_id = match old_kind {
                NodeKind::Commit { id } | NodeKind::ShallowPoint { id, .. } => *id,
                NodeKind::Reference(_) => unreachable!("references are added after traversal"),
            };
            let new_id = match new_kind {
                NodeKind::Commit { id } | NodeKind::ShallowPoint { id, .. } => *id,
                NodeKind::Reference(_) => unreachable!("references are added after traversal"),
            };
            if old_id != new_id {
                bail!(
                    "BUG: parent {parent_order} changed from {old_id} to {new_id} between traversal states"
                );
            }

            let replace = match (old_kind, new_kind) {
                (NodeKind::ShallowPoint { reason, .. }, NodeKind::Commit { .. })
                    if *reason == StopCondition::Limit =>
                {
                    true
                }
                (NodeKind::Commit { .. }, NodeKind::ShallowPoint { reason, .. })
                    if *reason == StopCondition::Limit =>
                {
                    false
                }
                (
                    NodeKind::ShallowPoint {
                        reason: old_reason, ..
                    },
                    NodeKind::ShallowPoint {
                        reason: new_reason, ..
                    },
                ) => {
                    new_reason.contains(StopCondition::ShallowBoundary)
                        && !old_reason.contains(StopCondition::ShallowBoundary)
                }
                (NodeKind::Commit { .. }, NodeKind::ShallowPoint { reason, .. }) => {
                    reason.contains(StopCondition::ShallowBoundary)
                }
                _ => false,
            };
            if replace {
                self.nodes[child].parents[parent_order] = new_parent;
            }
        }
        Ok(())
    }

    fn merge_converged_state(
        &mut self,
        commit: NodeIndex,
        flags: CommitFlags,
        limit: Limit,
        queue: &mut CommitQueue,
    ) {
        let reachable = self.propagate_flags(commit, flags);
        let incoming_goals = limit.goal_flags();
        if !incoming_goals.is_empty() {
            for (info, _, _, queued_limit) in queue.iter_mut() {
                if self
                    .commits_by_id
                    .get(&info.id)
                    .is_some_and(|index| reachable.contains(index))
                {
                    *queued_limit = queued_limit.additional_goal(incoming_goals);
                }
            }
        }

        let reachable_goals = Limit::new(None).additional_goal(flags).goal_flags();
        if !reachable_goals.is_empty() {
            for (_, _, _, queued_limit) in queue.iter_mut() {
                if queued_limit.goal_flags().intersects(reachable_goals) {
                    queued_limit.adjust_limit_if_bigger(limit);
                }
            }
        }
    }

    fn propagate_durable_flags(&mut self, child: NodeIndex, flags: CommitFlags) {
        self.propagate_flags(
            child,
            flags.intersection(
                CommitFlags::NotInRemote | CommitFlags::InWorkspace | CommitFlags::Integrated,
            ),
        );
    }

    fn propagate_flags(&mut self, child: NodeIndex, mut flags: CommitFlags) -> HashSet<NodeIndex> {
        flags.remove(CommitFlags::ShallowBoundary);
        if flags.is_empty() {
            return HashSet::new();
        }

        let mut seen = HashSet::new();
        let mut pending = vec![child];
        while let Some(commit) = pending.pop() {
            if !seen.insert(commit) {
                continue;
            }
            if !matches!(self.nodes[commit].kind, NodeKind::Commit { .. }) {
                continue;
            }
            self.flags[commit] |= flags;
            self.annotations[commit] |= flags & CommitFlags::all();
            pending.extend(self.nodes[commit].parents.iter().copied());
        }
        seen
    }

    fn finish(self, entrypoint: NodeIndex) -> (Vec<Node>, Vec<CommitFlags>, NodeIndex) {
        let mut child_counts = vec![0; self.nodes.len()];
        for node in &self.nodes {
            for &parent in &node.parents {
                child_counts[parent] += 1;
            }
        }
        let mut remap = vec![usize::MAX; self.nodes.len()];
        let mut nodes = Vec::with_capacity(self.nodes.len());
        let mut annotations = Vec::with_capacity(self.annotations.len());
        for (old, (node, annotation)) in self.nodes.into_iter().zip(self.annotations).enumerate() {
            if matches!(node.kind, NodeKind::ShallowPoint { .. }) && child_counts[old] == 0 {
                continue;
            }
            remap[old] = nodes.len();
            nodes.push(node);
            annotations.push(annotation);
        }
        for node in &mut nodes {
            for parent in &mut node.parents {
                *parent = remap[*parent];
            }
        }
        (nodes, annotations, remap[entrypoint])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use but_meta::VirtualBranchesTomlMetadata;

    fn metadata(repo: &gix::Repository) -> Result<VirtualBranchesTomlMetadata> {
        VirtualBranchesTomlMetadata::from_path(
            repo.path()
                .join("node-traversal-metadata-never-written.toml"),
        )
    }

    fn scenario(script: &str, name: &str) -> Result<gix::Repository> {
        let root =
            but_testsupport::gix_testtools::scripted_fixture_read_only(format!("{script}.sh"))
                .map_err(anyhow::Error::from_boxed)?;
        Ok(gix::open_opts(root.join(name), gix::open::Options::isolated())?.with_object_memory())
    }

    fn tip(repo: &gix::Repository, spec: &str) -> Result<gix::ObjectId> {
        Ok(repo.rev_parse_single(spec)?.object()?.peel_to_commit()?.id)
    }

    fn traverse(repo: &gix::Repository, tips: Vec<Tip>, options: Options) -> Result<NodeGraph> {
        let meta = metadata(repo)?;
        let (repo, meta, _) = super::super::Overlay::default().into_parts(repo, &meta);
        super::traverse_tips(&repo, tips, &meta, ProjectMeta::default(), options, None)
    }

    fn node_by_id(graph: &NodeGraph, id: gix::ObjectId) -> (NodeIndex, &Node) {
        graph
            .nodes()
            .iter()
            .enumerate()
            .find(
                |(_, node)| matches!(node.kind(), NodeKind::Commit { id: actual } if *actual == id),
            )
            .expect("commit is in graph")
    }

    #[test]
    fn records_managed_workspace_commit_from_entrypoint_ref_without_metadata() -> Result<()> {
        let repo = scenario("scenarios", "ws/two-segments-one-integrated-without-remote")?;
        let workspace_id = tip(&repo, "gitbutler/workspace")?;
        let meta = metadata(&repo)?;
        let (overlay_repo, overlay_meta, _) =
            super::super::Overlay::default().into_parts(&repo, &meta);
        let graph = super::traverse_tips(
            &overlay_repo,
            vec![Tip::entrypoint(workspace_id, None)],
            &overlay_meta,
            ProjectMeta::default(),
            Options::default(),
            Some(but_core::WORKSPACE_REF_NAME.try_into()?),
        )?;

        assert_eq!(
            graph.context.managed_workspace_commit_id,
            Some(workspace_id)
        );
        Ok(())
    }

    #[test]
    fn leaves_managed_workspace_commit_unset_when_workspace_tip_is_not_managed() -> Result<()> {
        let repo = scenario("scenarios", "triple-merge")?;
        let workspace_id = tip(&repo, "C")?;
        let graph = traverse(
            &repo,
            vec![
                Tip::new(workspace_id)
                    .with_role(TipRole::Workspace)
                    .with_is_entrypoint(true),
            ],
            Options::default(),
        )?;

        assert_eq!(graph.context.managed_workspace_commit_id, None);
        Ok(())
    }

    #[test]
    fn finds_managed_workspace_commit_through_a_second_parent() -> Result<()> {
        let root = but_testsupport::gix_testtools::scripted_fixture_writable("scenarios.sh")
            .map_err(anyhow::Error::from_boxed)?;
        let mut repo = gix::open_opts(
            root.path()
                .join("ws/two-segments-one-integrated-without-remote"),
            gix::open::Options::isolated(),
        )?;
        {
            let mut config = repo.config_snapshot_mut();
            config.set_raw_value("user.name", "Test")?;
            config.set_raw_value("user.email", "test@example.com")?;
        }
        let managed_workspace_id = tip(&repo, "gitbutler/workspace")?;
        let first_parent = tip(&repo, "main")?;
        let tree = repo.find_commit(managed_workspace_id)?.tree_id()?;
        let merge_id = repo.commit(
            "refs/heads/workspace-merge",
            "ordinary merge",
            tree,
            [first_parent, managed_workspace_id],
        )?;
        let graph = traverse(
            &repo,
            vec![
                Tip::new(merge_id.detach())
                    .with_role(TipRole::Workspace)
                    .with_is_entrypoint(true),
            ],
            Options::default(),
        )?;

        assert_eq!(
            graph.context.managed_workspace_commit_id,
            Some(managed_workspace_id),
            "managed workspace discovery follows the merge's represented second parent"
        );
        Ok(())
    }

    #[test]
    fn preserves_parent_order_and_deduplicates_shared_commits() -> Result<()> {
        let repo = scenario("scenarios", "triple-merge")?;
        let head = tip(&repo, "HEAD")?;
        let graph = traverse(&repo, vec![Tip::entrypoint(head, None)], Options::default())?;
        let (_, head_node) = node_by_id(&graph, head);
        let expected = repo
            .find_commit(head)?
            .parent_ids()
            .map(|id| id.detach())
            .collect::<Vec<_>>();
        let actual = head_node
            .parents()
            .iter()
            .map(|&index| match graph.nodes()[index].kind() {
                NodeKind::Commit { id } => *id,
                NodeKind::Reference(_) | NodeKind::ShallowPoint { .. } => {
                    panic!("unlimited traversal reaches commit parents")
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);

        let unique = graph
            .nodes()
            .iter()
            .filter_map(|node| match node.kind() {
                NodeKind::Commit { id } => Some(*id),
                NodeKind::Reference(_) | NodeKind::ShallowPoint { .. } => None,
            })
            .collect::<HashSet<_>>();
        assert_eq!(unique.len(), graph.nodes().len());
        Ok(())
    }

    #[test]
    fn materializes_shallow_and_limit_parent_sentinels() -> Result<()> {
        let repo = scenario("special-conditions", "shallow-clone-depth-2")?;
        let head = tip(&repo, "HEAD")?;
        let shallow = traverse(&repo, vec![Tip::entrypoint(head, None)], Options::default())?;
        assert!(shallow.nodes().iter().any(|node| matches!(
            node.kind(),
            NodeKind::ShallowPoint { reason, .. }
                if reason.contains(StopCondition::ShallowBoundary)
        )));

        let repo = scenario("scenarios", "triple-merge")?;
        let head = tip(&repo, "HEAD")?;
        let limited = traverse(
            &repo,
            vec![Tip::entrypoint(head, None)],
            Options::default().with_limit_hint(0),
        )?;
        let (_, head_node) = node_by_id(&limited, head);
        assert!(!head_node.parents().is_empty());
        assert!(head_node.parents().iter().all(|&parent| matches!(
            limited.nodes()[parent].kind(),
            NodeKind::ShallowPoint { reason, .. } if reason.contains(StopCondition::Limit)
        )));
        Ok(())
    }

    #[test]
    fn convergence_merges_durable_annotations() -> Result<()> {
        let repo = scenario("scenarios", "triple-merge")?;
        let head = tip(&repo, "C")?;
        let integrated = tip(&repo, "A")?;
        let base = tip(&repo, "main")?;
        let graph = traverse(
            &repo,
            vec![
                Tip::entrypoint(head, None),
                Tip::integrated(integrated, None),
            ],
            Options::default(),
        )?;
        let (base_index, _) = node_by_id(&graph, base);
        assert!(
            graph.annotations()[base_index]
                .contains(CommitFlags::NotInRemote | CommitFlags::Integrated)
        );
        assert_eq!(
            graph.annotations()[base_index].bits() & !CommitFlags::all().bits(),
            0
        );
        Ok(())
    }

    #[test]
    fn same_commit_merges_permissive_and_restrictive_states_in_both_tip_orders() -> Result<()> {
        let repo = scenario("scenarios", "triple-merge")?;
        let entrypoint = tip(&repo, "C")?;
        let target = tip(&repo, "main")?;

        for tips in [
            vec![
                Tip::entrypoint(entrypoint, None),
                Tip::integrated(target, None),
            ],
            vec![
                Tip::integrated(target, None),
                Tip::entrypoint(entrypoint, None),
            ],
        ] {
            let graph = traverse(&repo, tips, Options::default())?;
            let mut id = target;
            loop {
                let (index, node) = node_by_id(&graph, id);
                assert!(
                    graph.annotations()[index]
                        .contains(CommitFlags::NotInRemote | CommitFlags::Integrated),
                    "the shared target ancestry must receive both durable states at {id}"
                );

                let parent_ids = repo
                    .find_commit(id)?
                    .parent_ids()
                    .map(|id| id.detach())
                    .collect::<Vec<_>>();
                if parent_ids.is_empty() {
                    assert!(node.parents().is_empty());
                    break;
                }
                assert_eq!(parent_ids.len(), 1, "the target history is linear");
                assert!(matches!(
                    node.parents(),
                    [parent]
                        if matches!(graph.nodes()[*parent].kind(), NodeKind::Commit { id } if *id == parent_ids[0])
                ));
                id = parent_ids[0];
            }
        }
        Ok(())
    }

    #[test]
    fn later_same_commit_initial_tip_does_not_restart_parent_traversal() -> Result<()> {
        let repo = scenario("scenarios", "triple-merge")?;
        let shared = tip(&repo, "main")?;
        let omitted_parent = repo
            .find_commit(shared)?
            .parent_ids()
            .next()
            .expect("main has a parent")
            .detach();
        let local_ref = "refs/heads/main".try_into()?;
        let workspace = Tip::new(shared)
            .with_role(TipRole::Workspace)
            .with_is_entrypoint(true);
        let target = Tip::integrated(shared, None);
        let local = Tip::new(shared).with_role(TipRole::TargetLocal {
            local_ref_name: local_ref,
        });

        for tips in [
            vec![workspace.clone(), target.clone(), local.clone()],
            vec![local.clone(), target.clone(), workspace.clone()],
        ] {
            let graph = traverse(&repo, tips, Options::default())?;
            let (_, shared) = node_by_id(&graph, shared);
            assert!(matches!(
                shared.parents(),
                [parent]
                    if matches!(
                        graph.nodes()[*parent].kind(),
                        NodeKind::ShallowPoint { id, reason }
                            if *id == omitted_parent && *reason == StopCondition::Limit
                    )
            ));
        }
        Ok(())
    }

    #[test]
    fn hard_limit_only_materializes_accepted_initial_tips() -> Result<()> {
        let repo = scenario("scenarios", "triple-merge")?;
        let head = tip(&repo, "C")?;
        let rejected = tip(&repo, "A")?;
        let graph = traverse(
            &repo,
            vec![Tip::entrypoint(head, None), Tip::reachable(rejected, None)],
            Options::default().with_hard_limit(1),
        )?;

        assert_eq!(
            graph
                .context
                .traversal_tips
                .iter()
                .map(|tip| tip.id)
                .collect::<Vec<_>>(),
            vec![head]
        );
        assert!(graph.context.hard_limit_hit);
        assert!(
            graph
                .nodes()
                .iter()
                .all(|node| !matches!(node.kind(), NodeKind::Commit { id } if *id == rejected))
        );
        assert!(graph.nodes().iter().any(|node| matches!(
            node.kind(),
            NodeKind::ShallowPoint { id, reason }
                if *id == rejected && *reason == StopCondition::Limit
        )));
        Ok(())
    }

    #[test]
    fn same_id_auxiliary_tips_merge_or_skip_without_spending_hard_limit() -> Result<()> {
        let repo = scenario("scenarios", "triple-merge")?;
        let head = tip(&repo, "C")?;
        let other = tip(&repo, "A")?;
        let stack_ref = "refs/heads/stack".try_into()?;
        let target_stack_ref = "refs/heads/target-stack".try_into()?;
        let tips = vec![
            Tip::entrypoint(head, None),
            Tip::new(head).with_role(TipRole::WorkspaceStackBranch {
                desired_ref_name: stack_ref,
            }),
            Tip::integrated(other, None),
            Tip::new(other).with_role(TipRole::WorkspaceStackBranch {
                desired_ref_name: target_stack_ref,
            }),
        ];
        let metadata = metadata(&repo)?;
        let (overlay_repo, _, _) = super::super::Overlay::default().into_parts(&repo, &metadata);
        let initial_tips = initial_tips_from_tips(
            &overlay_repo,
            tips.clone(),
            &ProjectMeta::default(),
            Some(head),
        );
        let max_limit = Limit::new(None);
        let mut goals = Goals::default();
        let target_limit = max_limit
            .with_indirect_goal(head, &mut goals)
            .without_allowance();
        let entrypoint_goal = goals.flag_for(head).expect("one goal bit is available");
        let mut queue = CommitQueue::new_with_limit(Some(2));
        let effective_tips = queue_initial_tips(
            &overlay_repo,
            &initial_tips,
            head,
            CommitFlags::NotInRemote | entrypoint_goal,
            max_limit,
            target_limit,
            &mut goals,
            &mut queue,
            overlay_repo.commit_graph_if_enabled()?.as_ref(),
        )?;

        assert_eq!(effective_tips.len(), 5);
        assert!(
            effective_tips
                .iter()
                .any(|tip| tip.is_entrypoint && tip.id == head)
        );
        assert!(
            effective_tips
                .iter()
                .any(|tip| tip.id == other && matches!(tip.role, TipRole::TargetRemote))
        );
        assert!(
            effective_tips
                .iter()
                .any(|tip| matches!(tip.role, TipRole::WorkspaceStackBranch { .. }))
        );
        assert!(
            effective_tips
                .iter()
                .any(|tip| matches!(tip.role, TipRole::TargetRemote))
        );
        assert_eq!(queue.iter().count(), 2);
        assert!(queue.hard_limit_hit());
        let head_item = queue
            .iter()
            .find(|(info, _, _, _)| info.id == head)
            .expect("entrypoint is queued");
        assert_eq!(
            head_item.1 & CommitFlags::all(),
            CommitFlags::NotInRemote | CommitFlags::Integrated,
            "the same-ID auxiliary target contributes its durable role"
        );
        assert!(head_item.3.goal_flags().contains(entrypoint_goal));
        let target_item = queue
            .iter()
            .find(|(info, _, _, _)| info.id == other)
            .expect("target is queued");
        assert_eq!(
            target_item.1 & CommitFlags::all(),
            CommitFlags::NotInRemote | CommitFlags::Integrated,
            "the same-ID stack contributes its durable role to the target"
        );

        let graph = traverse(
            &repo,
            tips,
            Options::default()
                .with_extra_target_commit_id(head)
                .with_hard_limit(2),
        )?;
        assert_eq!(graph.context.traversal_tips.len(), effective_tips.len());
        assert!(
            graph
                .context
                .traversal_tips
                .iter()
                .any(|tip| matches!(tip.role, TipRole::WorkspaceStackBranch { .. }))
        );
        assert!(
            graph
                .context
                .traversal_tips
                .iter()
                .any(|tip| matches!(tip.role, TipRole::TargetRemote))
        );
        for id in [head, other] {
            let (index, _) = node_by_id(&graph, id);
            assert_eq!(
                graph.annotations()[index],
                CommitFlags::NotInRemote | CommitFlags::Integrated,
                "same-ID roles must survive as exact durable annotations at {id}"
            );
        }

        let graph = traverse(
            &repo,
            vec![Tip::entrypoint(head, None)],
            Options::default().with_extra_target_commit_id(head),
        )?;
        let (_, entrypoint) = node_by_id(&graph, head);
        let parent = *entrypoint
            .parents()
            .first()
            .expect("the entrypoint has ancestry");
        assert_eq!(
            graph.annotations()[parent],
            CommitFlags::NotInRemote | CommitFlags::Integrated,
            "the merged auxiliary target role propagates into entrypoint ancestry"
        );
        Ok(())
    }

    #[test]
    fn integrated_tip_processing_order_drives_node_insertion() -> Result<()> {
        let repo = scenario("scenarios", "four-diamond")?;
        let entrypoint = tip(&repo, "merged")?;
        let reachable = tip(&repo, "A")?;
        let integrated = tip(&repo, "main")?;

        for tips in [
            vec![
                Tip::entrypoint(entrypoint, None),
                Tip::reachable(reachable, None),
                Tip::integrated(integrated, None),
            ],
            vec![
                Tip::integrated(integrated, None),
                Tip::entrypoint(entrypoint, None),
                Tip::reachable(reachable, None),
            ],
        ] {
            let graph = traverse(&repo, tips, Options::default())?;
            assert!(matches!(
                graph.nodes().first().map(Node::kind),
                Some(NodeKind::Commit { id }) if *id == integrated
            ));
        }
        Ok(())
    }

    #[test]
    fn dynamically_discovered_remote_tips_keep_traversing_to_their_goal() -> Result<()> {
        let repo = scenario("scenarios", "remote-includes-another-remote")?;
        let head = tip(&repo, "B")?;
        let remote_a = tip(&repo, "refs/remotes/origin/A")?;
        let remote_b = tip(&repo, "refs/remotes/origin/B")?;
        let base = tip(&repo, "main")?;
        let graph = traverse(
            &repo,
            vec![Tip::entrypoint(head, None)],
            Options::default().with_limit_hint(1),
        )?;

        node_by_id(&graph, remote_a);
        node_by_id(&graph, remote_b);
        node_by_id(&graph, base);
        assert!(
            graph
                .annotations()
                .iter()
                .all(|flags| flags.bits() & !CommitFlags::all().bits() == 0)
        );
        Ok(())
    }

    #[test]
    fn cutoff_reason_is_per_edge_even_when_the_commit_was_traversed_elsewhere() -> Result<()> {
        let parent = gix::ObjectId::from_hex(b"1111111111111111111111111111111111111111")?;
        let limit_child = gix::ObjectId::from_hex(b"2222222222222222222222222222222222222222")?;
        let shallow_child = gix::ObjectId::from_hex(b"3333333333333333333333333333333333333333")?;
        let mut builder = Builder::default();
        builder.ensure_commit(parent)?;
        let limit_parent = builder.ensure_shallow(parent, StopCondition::Limit);
        let shallow_parent = builder.ensure_shallow(parent, StopCondition::ShallowBoundary);
        let limit_child_index = builder.ensure_commit(limit_child)?;
        let shallow_child_index = builder.ensure_commit(shallow_child)?;
        builder.nodes[limit_child_index].parents = vec![limit_parent];
        builder.nodes[shallow_child_index].parents = vec![shallow_parent];

        let (nodes, _, _) = builder.finish(limit_child_index);
        let find_commit = |id| {
            nodes
                .iter()
                .position(
                    |node| matches!(node.kind, NodeKind::Commit { id: actual } if actual == id),
                )
                .expect("commit exists")
        };
        find_commit(parent);
        let limit_child_index = find_commit(limit_child);
        let shallow_child_index = find_commit(shallow_child);
        assert!(matches!(
            nodes[nodes[limit_child_index].parents[0]].kind,
            NodeKind::ShallowPoint { id, reason }
                if id == parent && reason == StopCondition::Limit
        ));
        assert!(matches!(
            nodes[nodes[shallow_child_index].parents[0]].kind,
            NodeKind::ShallowPoint { id, reason }
                if id == parent && reason == StopCondition::ShallowBoundary
        ));
        Ok(())
    }

    #[test]
    fn concrete_parent_materialization_wins_in_both_state_orders() -> Result<()> {
        let child_id = gix::ObjectId::from_hex(b"1111111111111111111111111111111111111111")?;
        let parent_id = gix::ObjectId::from_hex(b"2222222222222222222222222222222222222222")?;

        for restrictive_first in [true, false] {
            let mut builder = Builder::default();
            let child = builder.ensure_commit(child_id)?;
            let parent = builder.ensure_commit(parent_id)?;
            let limit = builder.ensure_shallow(parent_id, StopCondition::Limit);
            let (first, second) = if restrictive_first {
                (limit, parent)
            } else {
                (parent, limit)
            };

            builder.merge_parents(child, vec![first, first])?;
            builder.merge_parents(child, vec![second, second])?;
            assert_eq!(builder.nodes[child].parents, vec![parent, parent]);
        }
        Ok(())
    }

    #[test]
    fn convergence_updates_only_reachable_frontier_in_both_queue_orders() -> Result<()> {
        let repo = scenario("scenarios", "triple-merge")?;
        let shared_id = tip(&repo, "A")?;
        let parent_id = repo
            .find_commit(shared_id)?
            .parent_ids()
            .next()
            .expect("A has a parent")
            .detach();
        let unrelated_id = tip(&repo, "B")?;
        let goal_id = tip(&repo, "C")?;
        let metadata = metadata(&repo)?;
        let (overlay_repo, _, _) = super::super::Overlay::default().into_parts(&repo, &metadata);
        let commit_graph = overlay_repo.commit_graph_if_enabled()?;

        for reachable_first in [true, false] {
            let mut goals = Goals::default();
            let goal = goals.flag_for(goal_id).expect("one goal bit is available");
            let incoming_limit = Limit::new(Some(3)).with_indirect_goal(goal_id, &mut goals);
            let mut builder = Builder::default();
            let shared = builder.ensure_commit(shared_id)?;
            let parent = builder.ensure_commit(parent_id)?;
            builder.nodes[shared].parents = vec![parent];
            builder.ensure_commit(unrelated_id)?;

            let mut queue = CommitQueue::new_with_limit(None);
            let mut buf = Vec::new();
            let reachable = (
                find(
                    commit_graph.as_ref(),
                    overlay_repo.for_find_only(),
                    parent_id,
                    &mut buf,
                )?,
                CommitFlags::Integrated,
                QueueSource::Parent,
                Limit::new(Some(0)),
            );
            let unrelated = (
                find(
                    commit_graph.as_ref(),
                    overlay_repo.for_find_only(),
                    unrelated_id,
                    &mut buf,
                )?,
                CommitFlags::NotInRemote,
                QueueSource::Parent,
                Limit::new(Some(0)),
            );
            for item in if reachable_first {
                [reachable, unrelated]
            } else {
                [unrelated, reachable]
            } {
                _ = queue.push_back_exhausted(item);
            }

            builder.merge_converged_state(
                shared,
                CommitFlags::NotInRemote
                    | CommitFlags::InWorkspace
                    | CommitFlags::Integrated
                    | goal,
                incoming_limit,
                &mut queue,
            );

            let reachable_limit = queue
                .iter()
                .find_map(|(info, _, _, limit)| (info.id == parent_id).then_some(limit))
                .expect("reachable frontier remains queued");
            assert!(reachable_limit.goal_flags().contains(goal));
            assert_eq!(reachable_limit.traversal_state().0, Some(3));
            let unrelated_limit = queue
                .iter()
                .find_map(|(info, _, _, limit)| (info.id == unrelated_id).then_some(limit))
                .expect("unrelated frontier remains queued");
            assert!(unrelated_limit.goal_flags().is_empty());
            assert_eq!(unrelated_limit.traversal_state().0, Some(0));
            assert!(builder.flags[parent].contains(goal));
            assert_eq!(
                builder.annotations[parent],
                CommitFlags::NotInRemote | CommitFlags::InWorkspace | CommitFlags::Integrated
            );
        }
        Ok(())
    }
}

//! Segment graphs built from a topological walk of commits and references, rather than from
//! the traversal of [`Graph::from_head()`](crate::Graph::from_head()).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{Context as _, Result};
use but_core::{RefMetadata, WORKSPACE_REF_NAME, is_workspace_ref_name};
use gix::refs::{FullName, FullNameRef};
use petgraph::visit::EdgeRef as _;

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
pub(crate) struct WorkspaceGraph {
    nodes: Vec<Node>,
    /// The local and remote-tracking branches that no stack claims, which belong to whichever
    /// lineage reaches them.
    unclaimed_branches: BTreeSet<FullName>,
    /// The managed workspace commit the workspace reference points at, if any.
    workspace_commit: Option<gix::ObjectId>,
    /// Whether the workspace reference, like after a traversal, is an ordinary branch: it has no
    /// workspace metadata, and the graph isn't seen from it.
    workspace_ref_is_plain: bool,
}

const UNKNOWN_PARENT_NODE_INDEX: NodeIndex = usize::MAX;

#[expect(clippy::too_many_arguments)]
pub(crate) fn graph(
    repo: &gix::Repository,
    overlay_repo: &crate::init::OverlayRepo<'_>,
    overlay_meta: &crate::init::OverlayMetadata<'_, impl RefMetadata>,
    remote_names: &gix::remote::Names,
    forked_branches: &BTreeSet<FullName>,
    head_ref: Option<&FullNameRef>,
    collect_tags: bool,
    mut heads: Vec<gix::ObjectId>,
    target_ref: &FullNameRef,
    target_ref_is_configured: bool,
    target_upstream_ref: Option<&FullNameRef>,
    target_commit_id: Option<gix::ObjectId>,
    commits_limit: Option<usize>,
) -> Result<WorkspaceGraph> {
    let (
        workspace_ref_name,
        workspace_commit,
        workspace_ref_is_plain,
        heads_on_reaching,
        mut oid_to_refs,
        ref_to_oid,
    ) = {
        let mut workspace_ref_name = None;
        let mut workspace_commit = None;
        let mut workspace_ref_is_plain = false;
        let mut heads_on_reaching = HashMap::<gix::ObjectId, Vec<gix::ObjectId>>::new();
        let mut oid_to_refs = HashMap::<gix::ObjectId, Vec<FullName>>::new();
        let mut ref_to_oid = BTreeMap::<FullName, gix::ObjectId>::new();
        // The references are those the overlay sees, which may amend the repository's.
        for reference in overlay_repo.references_with_prefix("refs/heads/")? {
            let Some(id) = reference.try_id() else {
                continue;
            };
            let full_name = reference.name().to_owned();
            if is_workspace_ref_name(full_name.as_ref())
                && head_ref != Some(full_name.as_ref())
                && overlay_meta.workspace_opt(full_name.as_ref())?.is_none()
            {
                workspace_ref_is_plain = true;
            } else if is_workspace_ref_name(full_name.as_ref()) {
                workspace_ref_name = Some(full_name.clone());
                let commit = repo.find_commit(id)?;
                workspace_commit = crate::workspace::commit::is_managed_workspace_by_message(
                    commit.message_raw()?,
                )
                .then_some(commit);
            }
            ref_to_oid.insert(full_name.clone(), id.detach());
            oid_to_refs.entry(id.detach()).or_default().push(full_name);
        }
        for reference in overlay_repo.references_with_prefix("refs/remotes/")? {
            let Some(id) = reference.try_id() else {
                continue;
            };
            let full_name = reference.name().to_owned();
            // Like a traversal, the walk leaves out the remote-tracking branch of a workspace
            // reference, as it would bring in a stale workspace commit.
            if let Some((_remote, short_name)) =
                but_core::extract_remote_name_and_short_name(full_name.as_ref(), remote_names)
                && let local_name =
                    gix::refs::Category::LocalBranch.to_full_name(&short_name[..])?
                && !is_workspace_ref_name(local_name.as_ref())
                && let Some(local_id) = ref_to_oid.get(&local_name)
            {
                heads_on_reaching
                    .entry(*local_id)
                    .or_default()
                    .push(id.detach());
            }
            ref_to_oid.insert(full_name.clone(), id.detach());
            oid_to_refs.entry(id.detach()).or_default().push(full_name);
        }
        if collect_tags {
            for mut reference in overlay_repo.references_with_prefix("refs/tags/")? {
                let Ok(commit) = reference.peel_to_commit() else {
                    continue;
                };
                let full_name = reference.name().to_owned();
                ref_to_oid.insert(full_name.clone(), commit.id);
                oid_to_refs.entry(commit.id).or_default().push(full_name);
            }
        }
        (
            workspace_ref_name,
            workspace_commit,
            workspace_ref_is_plain,
            heads_on_reaching,
            oid_to_refs,
            ref_to_oid,
        )
    };
    // Like after a traversal, a workspace reference without workspace metadata is an ordinary
    // branch, unless the graph is seen from it.
    let is_workspace_ref_name = |name: &FullNameRef| -> bool {
        but_core::is_workspace_ref_name(name) && !workspace_ref_is_plain
    };

    /// The branches of a stack that exist, top first.
    struct Stack {
        branches: Vec<FullName>,
        /// The commit the top branch points at.
        tip: gix::ObjectId,
        /// Whether the stack is in the workspace, making it a parent of the
        /// workspace commit.
        in_workspace: bool,
    }
    // Whether the target has the commit `id`, as the target commit or in the history of the
    // target branch or its upstream.
    let target_tips: Vec<gix::ObjectId> = std::iter::once(target_ref)
        .chain(target_upstream_ref)
        .filter_map(|name| ref_to_oid.get(name).copied())
        .collect();
    let cache = repo.commit_graph_if_enabled().ok();
    let mut revision_graph = cache
        .as_ref()
        .map(|cache| repo.revision_graph(cache.as_ref()));
    let is_ancestor = |id, tip, graph: &mut Option<_>| {
        graph
            .as_mut()
            .is_some_and(|graph| repo.is_ancestor_with_graph(id, tip, graph).unwrap_or(false))
    };
    let is_in_target = |id, graph: &mut _| {
        Some(id) == target_commit_id || target_tips.iter().any(|&tip| is_ancestor(id, tip, graph))
    };
    // Whether the workspace's lanes meet the target at the commit `id`: it's the target commit,
    // or the target and the managed workspace commit both have it.
    let is_workspace_base = |id, graph: &mut _| {
        Some(id) == target_commit_id
            || (is_in_target(id, graph)
                && workspace_commit
                    .as_ref()
                    .is_some_and(|workspace_commit| is_ancestor(id, workspace_commit.id, graph)))
    };
    let (stacks, unstacked_workspace_branches) = {
        let live_stack = |branches: Vec<FullName>, in_workspace: bool| {
            let branches: Vec<FullName> = branches
                .into_iter()
                .filter(|name| ref_to_oid.contains_key(name))
                .collect();
            let tip = *ref_to_oid.get(branches.first()?)?;
            Some(Stack {
                branches,
                tip,
                in_workspace,
            })
        };

        let mut stacks = Vec::<Stack>::new();
        let mut unstacked_workspace_branches = BTreeSet::<FullName>::new();
        if let Some(workspace_ref_name) = &workspace_ref_name
            && let Some(workspace_metadata) =
                overlay_meta.workspace_opt(workspace_ref_name.as_ref())?
        {
            let workspace_commit_parent_ids = workspace_commit
                .as_ref()
                .map(|commit| commit.parent_ids().map(gix::Id::detach).collect::<Vec<_>>());
            for stack in workspace_metadata.stacks {
                let in_workspace = stack.is_in_workspace();
                let branches = stack
                    .branches
                    .into_iter()
                    .map(|branch| branch.ref_name)
                    .collect();
                let Some(stack) = live_stack(branches, in_workspace) else {
                    continue;
                };
                // A stack whose tip isn't a parent of the managed workspace commit
                // has had another one stacked onto it in Git, unless it rests where the
                // workspace's lanes meet the target as an empty lane of its own. Its
                // branches then belong to whichever stack reaches them.
                // LEGACY
                let is_stack_underneath_another = workspace_commit_parent_ids
                    .as_ref()
                    .is_some_and(|parent_ids| !parent_ids.contains(&stack.tip))
                    && !is_workspace_base(stack.tip, &mut revision_graph);
                // Like after a traversal, a parent of the managed workspace commit is one lane
                // however many stacks rest on it, that of the first of them, unless the target
                // already has it, where each is an empty lane. The branches of the others then
                // belong to that lane too.
                let rests_on_another_stack = workspace_commit_parent_ids.is_some()
                    && stacks
                        .iter()
                        .any(|other| other.in_workspace && other.tip == stack.tip)
                    && !is_in_target(stack.tip, &mut revision_graph);
                // Seen from the workspace, a stack outside of it is no lane, so like after a
                // traversal its branches belong to whichever lane reaches them.
                let is_outside_seen_from_workspace =
                    !stack.in_workspace && head_ref.is_some_and(is_workspace_ref_name);
                // Seen from anything but the workspace, a stack in it is no lane either, as a
                // traversal only splits the workspace into its stacks when seen from the workspace.
                let is_inside_seen_from_elsewhere =
                    stack.in_workspace && !head_ref.is_some_and(is_workspace_ref_name);
                if !(stack.in_workspace && (is_stack_underneath_another || rests_on_another_stack))
                    && !is_outside_seen_from_workspace
                    && !is_inside_seen_from_elsewhere
                {
                    stacks.push(stack);
                } else {
                    unstacked_workspace_branches.extend(stack.branches);
                }
            }
        }
        // Branches outside the workspace, like those of a single-branch-mode
        // checkout, may still be stacked through their persisted branch stack order.
        // Seen from the workspace, those of a workspace stack that was left out above
        // aren't, so they stay with whichever stack reaches them. Seen from elsewhere, a
        // traversal ignores the workspace's stacks, so their branches are stacked like
        // any other.
        for full_name in ref_to_oid.keys() {
            if full_name.category() != Some(gix::refs::Category::LocalBranch)
                || (unstacked_workspace_branches.contains(full_name)
                    && head_ref.is_some_and(is_workspace_ref_name))
                || stacks
                    .iter()
                    .any(|stack| stack.branches.contains(full_name))
            {
                continue;
            }
            if let Some(branches) = overlay_meta.branch_stack_order(full_name.as_ref())? {
                stacks.extend(live_stack(branches, false));
            }
        }
        (stacks, unstacked_workspace_branches)
    };

    /// The branches of the stack containing `branch`, or none if no stack does.
    fn stack_of_branch<'a>(stacks: &'a [Stack], branch: &FullNameRef) -> &'a [FullName] {
        stacks
            .iter()
            .find(|stack| stack.branches.iter().any(|name| name.as_ref() == branch))
            .map_or(&[], |stack| stack.branches.as_slice())
    }

    // Sort the values of oid_to_refs for consistency and compatibility with the old implementation.
    let head_stack = head_ref.map_or(&[][..], |head_ref| stack_of_branch(&stacks, head_ref));
    for refs in oid_to_refs.values_mut() {
        // Like after a traversal, of the branches no stack claims on a commit, one with metadata
        // names its segment, unless another branch does anyway.
        let metadata_names_segment = !refs.iter().any(|reference| {
            is_workspace_ref_name(reference.as_ref())
                || Some(reference.as_ref()) == head_ref
                || reference.as_ref() == target_ref
                || !stack_of_branch(&stacks, reference.as_ref()).is_empty()
        });
        refs.sort_by_cached_key(|reference| {
            if is_workspace_ref_name(reference.as_ref()) {
                return (0, 0);
            }
            // The branch the graph is seen from comes first, but with the branches of its stack
            // in their order, as a traversal keeps the branches above it out of its lane.
            if let Some(position) = head_stack.iter().position(|r| r == reference) {
                return (0, position + 1);
            }
            // Without a stack it still comes right after the workspace ref, whatever its name, so
            // the workspace ref leads to it like to the tip a traversal makes of it.
            if Some(reference.as_ref()) == head_ref {
                return (0, 1);
            }
            (
                match reference.category() {
                    // A remote-tracking branch without a local branch joins the lane that
                    // reaches it like a branch no stack claims, and so comes after the local
                    // branches on its commit, which lead that lane.
                    Some(gix::refs::Category::RemoteBranch)
                        if but_core::extract_remote_name_and_short_name(
                            reference.as_ref(),
                            remote_names,
                        )
                        .and_then(|(_remote, short_name)| {
                            gix::refs::Category::LocalBranch
                                .to_full_name(&short_name[..])
                                .ok()
                        })
                        .is_some_and(|local_name| !ref_to_oid.contains_key(&local_name)) =>
                    {
                        usize::MAX - 2
                    }
                    Some(gix::refs::Category::RemoteBranch) => 1,
                    Some(gix::refs::Category::Tag) => usize::MAX,
                    // Like the tip a traversal makes of it, a configured target branch comes before
                    // the branches no stack claims, so it names the segment of a commit they share,
                    // unless it's a branch of a stack, which keeps its place in the stack.
                    _ if reference.as_ref() == target_ref
                        && stack_of_branch(&stacks, reference.as_ref()).is_empty() =>
                    {
                        if target_ref_is_configured {
                            usize::MAX - 3
                        } else {
                            usize::MAX - 1
                        }
                    }
                    _ => stack_of_branch(&stacks, reference.as_ref())
                        .iter()
                        .position(|r| r == reference)
                        .map_or(usize::MAX - 2, |position| position + 2),
                },
                usize::from(
                    metadata_names_segment
                        && reference.category() == Some(gix::refs::Category::LocalBranch)
                        && stack_of_branch(&stacks, reference.as_ref()).is_empty()
                        && crate::init::walk::extract_local_branch_metadata(
                            reference.as_ref(),
                            overlay_meta,
                        )
                        .ok()
                        .flatten()
                        .is_none(),
                ),
            )
        });
    }

    // Like a traversal, the branches of every stack the workspace has are walked, even those of
    // a stack left out above, so they're in the graph however far from the workspace commit.
    heads.extend(
        stacks
            .iter()
            .flat_map(|stack| &stack.branches)
            .chain(&unstacked_workspace_branches)
            .filter_map(|name| ref_to_oid.get(name)),
    );

    let shallow_commits = repo.shallow_commits()?;
    let commit_ids = crate::topowalk::walk(
        repo,
        heads,
        &heads_on_reaching,
        target_commit_id,
        commits_limit,
        shallow_commits.as_ref(),
    )?;

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
        /// This lineage comes directly from the workspace, either along a parent
        /// edge of the managed workspace commit that no stack covers, or from the
        /// workspace reference itself when there's no workspace commit, down to the
        /// first commit with a branch it takes in. It takes in every reference on
        /// the commit it reaches, whichever stack the reference belongs to.
        is_unclaimed_workspace_edge: bool,
    }
    /// An element of [Node::parents] that must be updated when an unassigned
    /// OID becomes assigned to a node.
    #[derive(Debug)]
    struct ToRepoint {
        node_index: NodeIndex,
        parent_index: usize,
    }
    /// Updates some elements of [Node::parents] in `nodes` to `node_index`.
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
    let mut unclaimed_branches = BTreeSet::new();

    for commit_id in commit_ids {
        let commit = repo.find_commit(commit_id)?;
        // Branches rest on the target commit when the target branch, or its
        // upstream, points there too.
        // LEGACY why check 3 things? could there be a simpler rule (or no rule)?
        let rests_on_target = target_commit_id == Some(commit_id)
            && std::iter::once(target_ref)
                .chain(target_upstream_ref)
                .any(|name| ref_to_oid.get(name) == Some(&commit_id));
        let mut workspace_took_in_branch = false;
        // The branches of the stack whose branch on this commit a lane ran into, if any.
        let mut stack_run_into: Option<&[FullName]> = None;

        for full_name in oid_to_refs.remove(&commit_id).unwrap_or_default() {
            // 1. Calculate some flags. LEGACY this seems overly complicated

            let category = full_name.category();
            // Like a traversal, only a linked worktree the graph is walked from makes its branch a
            // lane of its own.
            let is_checked_out_elsewhere = forked_branches.contains(&full_name);
            // A tag, or a local branch that no stack claims, belongs to whichever
            // lineage reaches it, like a branch of a stack without metadata, and so
            // does a remote-tracking branch without a local branch to lead through.
            // That's unless another worktree has the branch checked out, making
            // it its own lane, or it merely rests on the target commit next to
            // the target branch.
            let is_remote_only_branch = category == Some(gix::refs::Category::RemoteBranch)
                && but_core::extract_remote_name_and_short_name(full_name.as_ref(), remote_names)
                    .map(|(_remote, short_name)| {
                        gix::refs::Category::LocalBranch.to_full_name(&short_name[..])
                    })
                    .transpose()?
                    .is_some_and(|local_name| !ref_to_oid.contains_key(&local_name));
            let is_stackless_branch = match category {
                Some(gix::refs::Category::LocalBranch) => {
                    (!full_name.as_bstr().starts_with(b"refs/heads/gitbutler/")
                        || (workspace_ref_is_plain
                            && but_core::is_workspace_ref_name(full_name.as_ref())))
                        && stack_of_branch(&stacks, full_name.as_ref()).is_empty()
                        && !is_checked_out_elsewhere
                }
                Some(gix::refs::Category::RemoteBranch) => is_remote_only_branch,
                Some(gix::refs::Category::Tag) => true,
                _ => false,
            } && !rests_on_target;
            // A branch or tag merely resting on the target commit isn't taken in
            // by an edge that takes in every reference either, and neither is the
            // remote-tracking branch of a local branch, which is always its own lane.
            // Without a workspace commit to give it an edge of its own, a branch of a
            // stack in the workspace is still a lane of the workspace there.
            let is_workspace_lane_without_commit = workspace_commit.is_none()
                && stacks
                    .iter()
                    .any(|stack| stack.in_workspace && stack.branches.contains(&full_name));
            let rests_beside = (rests_on_target
                && matches!(
                    category,
                    Some(gix::refs::Category::LocalBranch | gix::refs::Category::Tag)
                )
                && full_name.as_ref() != target_ref
                && !is_workspace_lane_without_commit)
                || (category == Some(gix::refs::Category::RemoteBranch)
                    && (!is_remote_only_branch || rests_on_target));
            // The target branch checked out in another worktree is a lane of its own, so that
            // it can follow the target there.
            let target_in_other_worktree =
                full_name.as_ref() == target_ref && is_checked_out_elsewhere;
            if is_stackless_branch
                && matches!(
                    category,
                    Some(gix::refs::Category::LocalBranch | gix::refs::Category::RemoteBranch)
                )
            {
                unclaimed_branches.insert(full_name.clone());
            }

            // 2. For this ref, process (remove and reinsert) the `unassigned_oid_map` entry for
            // this commit. Some ToRepoint elements are executed, and others are reinserted into
            // `unassigned_oid_map`.

            let mut new_stack_lineage = StackLineage::default();
            let mut new_map = HashMap::<StackLineage, Vec<ToRepoint>>::new();
            let node_index = nodes.len();
            // Like after a traversal, where the branch naming the segment of a commit has it for
            // every lane reaching it, the lanes of the workspace that reach a branch of a stack in
            // the workspace whose own lane reaches it too run into that branch.
            let is_workspace_lane = |stack_lineage: &StackLineage| {
                stacks.iter().any(|stack| {
                    stack.in_workspace
                        && stack
                            .branches
                            .iter()
                            .any(|name| stack_lineage.branches_in_lineage.contains(name))
                })
            };
            let own_lane_reaches_it = workspace_commit.is_some()
                && !is_in_target(commit_id, &mut revision_graph)
                && stacks
                    .iter()
                    .any(|stack| stack.in_workspace && stack.branches.contains(&full_name))
                && unassigned_oid_map.get(&commit_id).is_some_and(|lineages| {
                    lineages
                        .keys()
                        .any(|stack_lineage| stack_lineage.branches_in_lineage.contains(&full_name))
                });
            for (stack_lineage, to_repoints) in
                unassigned_oid_map.remove(&commit_id).unwrap_or_default()
            {
                // Like after a traversal, the branch a linked worktree has checked out is a
                // fork of its own onto its commit, which stays in the lanes reaching it.
                let in_lineage = !target_in_other_worktree
                    && !forked_branches.contains(&full_name)
                    && (full_name.as_ref() == target_ref
                        || (stack_lineage.is_unclaimed_workspace_edge && !rests_beside)
                        || is_stackless_branch
                        || stack_lineage.branches_in_lineage.contains(&full_name)
                        || (own_lane_reaches_it && is_workspace_lane(&stack_lineage)));
                if in_lineage
                    && own_lane_reaches_it
                    && !stack_lineage.branches_in_lineage.contains(&full_name)
                {
                    stack_run_into = Some(stack_of_branch(&stacks, full_name.as_ref()));
                }
                // Like after a traversal, a branch of a lane that ran into the branch of another
                // stack on this commit is recorded on that commit, like one no stack claims.
                if in_lineage
                    && category == Some(gix::refs::Category::LocalBranch)
                    && stack_run_into.is_some_and(|branches| !branches.contains(&full_name))
                    && !is_checked_out_elsewhere
                {
                    unclaimed_branches.insert(full_name.clone());
                }
                if in_lineage {
                    workspace_took_in_branch |= stack_lineage.is_unclaimed_workspace_edge
                        && category == Some(gix::refs::Category::LocalBranch)
                        && !is_workspace_ref_name(full_name.as_ref());
                    new_stack_lineage
                        .branches_in_lineage
                        .extend(stack_lineage.branches_in_lineage);
                    new_stack_lineage.is_unclaimed_workspace_edge |=
                        stack_lineage.is_unclaimed_workspace_edge;
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
            // of the branches on its commit, whichever stack they belong to,
            // unless several stacks of the workspace rest on that commit: then,
            // like the parents of a workspace commit, it leads to each of them.
            let stacks_resting_here: Vec<&Stack> =
                if is_workspace_ref_name(full_name.as_ref()) && workspace_commit.is_none() {
                    stacks
                        .iter()
                        .filter(|stack| stack.in_workspace && stack.tip == commit_id)
                        .collect()
                } else {
                    Vec::new()
                };
            let parent_count = if stacks_resting_here.len() > 1 {
                for (parent_index, stack) in stacks_resting_here.iter().enumerate() {
                    new_map
                        .entry(StackLineage {
                            branches_in_lineage: stack.branches.iter().cloned().collect(),
                            is_unclaimed_workspace_edge: false,
                        })
                        .or_default()
                        .push(ToRepoint {
                            node_index,
                            parent_index,
                        });
                }
                stacks_resting_here.len()
            } else {
                if is_workspace_ref_name(full_name.as_ref()) {
                    new_stack_lineage.is_unclaimed_workspace_edge = true;
                }
                new_map
                    .entry(new_stack_lineage)
                    .or_default()
                    .push(ToRepoint {
                        node_index,
                        parent_index: 0,
                    });
                1
            };
            // Like the edges of a workspace commit, it also leads to each stack of the workspace
            // resting where the workspace's lanes meet the target, which without a workspace
            // commit is the target commit, as an empty lane of its own.
            let stacks_on_workspace_base: Vec<&Stack> =
                if is_workspace_ref_name(full_name.as_ref()) && workspace_commit.is_none() {
                    stacks
                        .iter()
                        .filter(|stack| {
                            stack.in_workspace
                                && stack.tip != commit_id
                                && is_workspace_base(stack.tip, &mut revision_graph)
                        })
                        .collect()
                } else {
                    Vec::new()
                };
            for (offset, stack) in stacks_on_workspace_base.iter().enumerate() {
                unassigned_oid_map
                    .entry(stack.tip)
                    .or_default()
                    .entry(StackLineage {
                        branches_in_lineage: stack.branches.iter().cloned().collect(),
                        is_unclaimed_workspace_edge: false,
                    })
                    .or_default()
                    .push(ToRepoint {
                        node_index,
                        parent_index: parent_count + offset,
                    });
            }
            let parent_count = parent_count + stacks_on_workspace_base.len();
            unassigned_oid_map.insert(commit_id, new_map);

            // 3. Push the node for this ref.

            nodes.push(Node {
                weight: NodeWeight::Ref { full_name },
                parents: vec![UNKNOWN_PARENT_NODE_INDEX; parent_count],
            });
        }

        let mut branches_in_lineage = BTreeSet::<gix::refs::FullName>::new();
        let mut reached_by_unclaimed_workspace_edge = false;
        let node_index = nodes.len();
        for (stack_lineage, to_repoints) in
            unassigned_oid_map.remove(&commit_id).unwrap_or_default()
        {
            branches_in_lineage.extend(stack_lineage.branches_in_lineage);
            reached_by_unclaimed_workspace_edge |= stack_lineage.is_unclaimed_workspace_edge;
            repoint(&mut nodes, to_repoints, node_index);
        }

        // The workspace commit gets one parent edge per stack in the workspace whose tip is one of
        // its parents or the target commit, plus one per parent that is not a tip of any such
        // stack. These stacks in metadata order take the place of parents that correspond to
        // stacks, and any left over are placed at the end. For example, if the commit has parents
        // [1, 2, 3] and the stacks in metadata order are [3, 4, 1], the parents that correspond to
        // stacks are 1 and 3. Its edges will therefore be [3, 2, 4, 1] (notice that the 3-4-1 order
        // is preserved and parent 2 keeps its position).
        // The parents of a commit at the boundary of a shallow clone aren't there.
        let parent_ids: Vec<gix::ObjectId> = if shallow_commits
            .as_ref()
            .is_some_and(|boundary| boundary.binary_search(&commit_id).is_ok())
        {
            Vec::new()
        } else {
            commit.parent_ids().map(gix::Id::detach).collect()
        };
        let is_managed_workspace_commit = workspace_commit
            .as_ref()
            .is_some_and(|commit| commit.id == commit_id);
        let mut stack_edges = Vec::<(gix::ObjectId, StackLineage)>::new();
        if is_managed_workspace_commit {
            for stack in stacks.iter().filter(|stack| stack.in_workspace) {
                if parent_ids.contains(&stack.tip)
                    || is_workspace_base(stack.tip, &mut revision_graph)
                {
                    stack_edges.push((
                        stack.tip,
                        StackLineage {
                            branches_in_lineage: stack.branches.iter().cloned().collect(),
                            is_unclaimed_workspace_edge: false,
                        },
                    ));
                }
            }
        }
        let stack_lineage = StackLineage {
            branches_in_lineage,
            // Without a workspace commit, the commits below the workspace reference are a lane of
            // the workspace, like after a traversal, until it reaches a branch.
            is_unclaimed_workspace_edge: is_managed_workspace_commit
                || (workspace_commit.is_none()
                    && reached_by_unclaimed_workspace_edge
                    && !workspace_took_in_branch),
        };
        let covered_parent_ids: Vec<gix::ObjectId> =
            stack_edges.iter().map(|(tip, _)| *tip).collect();
        let mut stack_edges = stack_edges.into_iter();
        let mut lanes_of_parents = Vec::<(gix::ObjectId, StackLineage)>::new();
        let mut parent_edges: Vec<_> = parent_ids
            .into_iter()
            .filter_map(|parent_id| {
                if covered_parent_ids.contains(&parent_id) {
                    // Like after a traversal, a parent the workspace commit has more often than
                    // stacks have a lane on it is one more lane into the same commit.
                    let edge = stack_edges.next().or_else(|| {
                        lanes_of_parents
                            .iter()
                            .find(|(tip, _)| *tip == parent_id)
                            .cloned()
                    });
                    lanes_of_parents.extend(edge.clone());
                    edge
                } else {
                    Some((parent_id, stack_lineage.clone()))
                }
            })
            .collect();
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

    Ok(WorkspaceGraph {
        nodes,
        unclaimed_branches,
        workspace_commit: workspace_commit.map(|commit| commit.id),
        workspace_ref_is_plain,
    })
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

    #[expect(clippy::too_many_arguments)]
    pub fn to_segment_graph(
        &self,
        repo: &gix::Repository,
        overlay_repo: &crate::init::OverlayRepo,
        overlay_meta: &crate::init::OverlayMetadata<'_, impl RefMetadata>,
        worktree_by_branch: &crate::init::walk::WorktreeByBranch,
        project_meta: but_core::ref_metadata::ProjectMeta,
        seen_from_ref: Option<&FullNameRef>,
        seen_from_commit: Option<gix::ObjectId>,
        worktree_tips: &[crate::init::WorktreeTip],
    ) -> Result<crate::Graph> {
        // Like after a traversal, a workspace reference without workspace metadata is an ordinary
        // branch, unless the graph is seen from it.
        let is_workspace_ref_name = |name: &FullNameRef| -> bool {
            but_core::is_workspace_ref_name(name) && !self.workspace_ref_is_plain
        };
        let mut inner_graph = crate::init::PetGraph::default();
        let shallow_commits = repo.shallow_commits()?;
        let mut node_index_to_predecessor_segment_info =
            HashMap::<NodeIndex, PredecessorSegmentInfo>::new();
        let mut entrypoint = None;
        let target_upstream_ref = project_meta.remote_target_ref(repo)?;
        // Like a traversal, the workspace ref the graph is seen from is a workspace in a project
        // with metadata, even if none is stored for it yet.
        let seen_from_workspace_ref = seen_from_ref
            .filter(|name| name.as_bstr() == WORKSPACE_REF_NAME)
            .filter(|_| project_meta != but_core::ref_metadata::ProjectMeta::default());
        // Like after a traversal, the commit a graph without a branch is seen from starts a
        // segment without a name of its own.
        let detached_commit = seen_from_ref
            .is_none()
            .then_some(seen_from_commit)
            .flatten();
        // Like the tip a traversal makes of it, the commit a linked worktree has checked out
        // without a branch starts a segment, so its lane starts there, unless the segment of a
        // branch with metadata pointing at it has it.
        let is_detached_worktree_commit = |id: gix::ObjectId| {
            worktree_tips
                .iter()
                .any(|tip| tip.ref_name.is_none() && tip.id == id)
        };

        // Each local branch with the segment it names, to be paired with its remote-tracking branch.
        let mut local_branches = Vec::<(crate::SegmentIndex, FullName)>::new();
        // References that will end up in the [crate::Commit::refs] of a commit that has not been
        // encountered yet, keyed by the next node towards that commit. If the node turns out to be
        // another reference, use `forward_awaiting_refs()` to move the refs forward.
        let mut refs_awaiting_commit = HashMap::<NodeIndex, Vec<crate::RefInfo>>::new();
        let mut remote_tracking_branch_name_to_segment_index = HashMap::new();
        /// Moves the references awaiting a commit on node `from`, followed by `reference`,
        /// to node `to`, which leads to the same commit.
        fn forward_awaiting_refs(
            refs_awaiting_commit: &mut HashMap<NodeIndex, Vec<crate::RefInfo>>,
            from: NodeIndex,
            to: NodeIndex,
            reference: Option<crate::RefInfo>,
        ) {
            let mut refs = refs_awaiting_commit.remove(&from).unwrap_or_default();
            refs.extend(reference);
            if !refs.is_empty() {
                refs_awaiting_commit.entry(to).or_default().extend(refs);
            }
        }
        // The branches no stack claims and no metadata names on each commit, which can't tell
        // where a segment starts when there are several of them.
        let mut ambiguous_branches_by_commit = HashMap::<gix::ObjectId, usize>::new();
        // How many nodes lead to each node.
        let mut leading_nodes = vec![0usize; self.nodes.len()];
        for node in &self.nodes {
            for &parent in &node.parents {
                if let Some(count) = leading_nodes.get_mut(parent) {
                    *count += 1;
                }
            }
        }
        // The node of the commit that the reference at node `i` points at.
        let commit_node_of = |mut i: NodeIndex| {
            while let NodeWeight::Ref { .. } = &self.nodes[i].weight {
                i = self.nodes[i].parents[0];
            }
            i
        };
        // The local branches with metadata on each commit, other than the workspace ref, which a
        // traversal leaves out when it names a segment.
        let mut branches_with_metadata_by_commit = HashMap::<gix::ObjectId, usize>::new();
        for (i, node) in self.nodes.iter().enumerate() {
            if let NodeWeight::Ref { full_name } = &node.weight
                && full_name.category() == Some(gix::refs::Category::LocalBranch)
                && !is_workspace_ref_name(full_name.as_ref())
            {
                let counts = if crate::init::walk::extract_local_branch_metadata(
                    full_name.as_ref(),
                    overlay_meta,
                )?
                .is_some()
                {
                    Some(&mut branches_with_metadata_by_commit)
                } else {
                    self.unclaimed_branches
                        .contains(full_name)
                        .then_some(&mut ambiguous_branches_by_commit)
                };
                if let Some(counts) = counts {
                    *counts.entry(self.commit_id_of(i)).or_default() += 1;
                }
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
            // Like the tip of a traversal, the commit the graph is seen from isn't only in a
            // remote, even without a branch.
            if let NodeWeight::Pick { oid } | NodeWeight::ShallowPoint { oid } = &node.weight
                && seen_from_commit == Some(*oid)
            {
                predecessor_segment_info
                    .commit_flags
                    .insert(CommitFlags::NotInRemote);
            }
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
            // A plain local branch on that commit is recorded on it, and whatever reaches
            // the branch reaches the commit, while workspace refs, the target branch and
            // branches with metadata still name segments of their own. Like after a
            // traversal, the managed workspace commit stays with the workspace reference,
            // and every other reference on it is recorded on it the same way, unless the
            // graph is seen from it. So is a branch no stack claims that the empty segments of
            // several stack branches on its commit reach, which a traversal leaves on their
            // anonymous commit.
            if let Some((full_name, commit_id)) = reference
                && !is_workspace_ref_name(full_name.as_ref())
                && ((self.workspace_commit == Some(commit_id)
                    && seen_from_ref != Some(full_name.as_ref()))
                    // Like after a traversal, where only local branches name segments, a tag is
                    // recorded on its commit, unless the graph is seen from it.
                    || (full_name.category() == Some(gix::refs::Category::Tag)
                        && seen_from_ref != Some(full_name.as_ref()))
                    // Like after a traversal, branches that nothing tells apart on a commit that
                    // several segments reach are recorded on it, leaving the commit unnamed.
                    || (predecessor_segment_info.segment_indexes.len() > 1
                        && seen_from_ref != Some(full_name.as_ref())
                        && project_meta.target_ref.as_ref().is_none_or(|target_ref| {
                            target_ref.as_local_full_name_ref() != full_name
                        })
                        && self.unclaimed_branches.contains(full_name)
                        && !worktree_by_branch.contains_key(full_name)
                        && ambiguous_branches_by_commit
                            .get(&commit_id)
                            .is_some_and(|count| *count > 1))
                    || (detached_commit == Some(commit_id)
                        && full_name.category() == Some(gix::refs::Category::LocalBranch)
                        && project_meta.target_ref.as_ref().is_none_or(|target_ref| {
                            target_ref.as_local_full_name_ref() != full_name
                        })
                        && crate::init::walk::extract_local_branch_metadata(
                            full_name.as_ref(),
                            overlay_meta,
                        )?
                        .is_none())
                    || (predecessor_segment_info.segment_indexes.len() > 1
                        && !predecessor_segment_info.must_start_new_segment
                        && seen_from_ref != Some(full_name.as_ref())
                        && full_name.category() == Some(gix::refs::Category::LocalBranch)
                        && project_meta.target_ref.as_ref().is_none_or(|target_ref| {
                            target_ref.as_local_full_name_ref() != full_name
                        })
                        && self.unclaimed_branches.contains(full_name)
                        && !worktree_by_branch.contains_key(full_name)
                        && predecessor_segment_info.segment_indexes.iter().any(
                            |(segment_index, _)| {
                                let segment = &inner_graph[*segment_index];
                                segment.commits.is_empty()
                                    && segment.ref_info.as_ref().is_some_and(|ref_info| {
                                        ref_info.ref_name.category()
                                            == Some(gix::refs::Category::LocalBranch)
                                    })
                            },
                        )
                        && predecessor_segment_info.segment_indexes.iter().all(
                            |(segment_index, _)| {
                                let segment = &inner_graph[*segment_index];
                                segment.ref_info.as_ref().is_some_and(|ref_info| {
                                    let category = ref_info.ref_name.category();
                                    if segment.commits.is_empty() {
                                        category == Some(gix::refs::Category::RemoteBranch)
                                            || (category == full_name.category()
                                                && (self
                                                    .unclaimed_branches
                                                    .contains(&ref_info.ref_name)
                                                    || !is_workspace_ref_name(
                                                        ref_info.ref_name.as_ref(),
                                                    )))
                                    } else {
                                        // A branch outside the workspace and the target, and not
                                        // checked out elsewhere, doesn't take it either.
                                        category == Some(gix::refs::Category::LocalBranch)
                                            && !worktree_by_branch
                                                .get(&ref_info.ref_name)
                                                .is_some_and(|worktrees| {
                                                    worktrees.iter().any(|wt| !wt.owned_by_repo)
                                                })
                                            && !segment.commits.last().is_some_and(|commit| {
                                                commit.flags.intersects(
                                                    CommitFlags::InWorkspace
                                                        | CommitFlags::Integrated,
                                                )
                                            })
                                    }
                                })
                            },
                        )))
            {
                forward_awaiting_refs(
                    &mut refs_awaiting_commit,
                    i,
                    node.parents[0],
                    Some(crate::RefInfo::from_ref(
                        full_name.clone(),
                        Some(commit_id),
                        worktree_by_branch,
                    )),
                );
                let parent_info = node_index_to_predecessor_segment_info
                    .entry(node.parents[0])
                    .or_default();
                parent_info
                    .segment_indexes
                    .extend(predecessor_segment_info.segment_indexes);
                parent_info.must_start_new_segment |=
                    predecessor_segment_info.must_start_new_segment;
                parent_info
                    .commit_flags
                    .insert(predecessor_segment_info.commit_flags);
                continue;
            }
            // A branch that merely rests on the target commit, with nothing
            // reaching it, is recorded on that commit like the branches of the
            // segment containing it, instead of being a lane of its own, unless
            // the graph is seen from it.
            if let Some((full_name, commit_id)) = reference
                && predecessor_segment_info.segment_indexes.is_empty()
                && seen_from_ref != Some(full_name.as_ref())
                && full_name.category() == Some(gix::refs::Category::LocalBranch)
                && project_meta
                    .target_ref
                    .as_ref()
                    .is_none_or(|target_ref| target_ref.as_local_full_name_ref() != full_name)
                && !worktree_by_branch.contains_key(full_name)
                && project_meta.target_commit_id == Some(commit_id)
            {
                forward_awaiting_refs(
                    &mut refs_awaiting_commit,
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
            let commit = match &node.weight {
                NodeWeight::Pick { oid } | NodeWeight::ShallowPoint { oid } => {
                    Some(repo.find_commit(*oid)?)
                }
                NodeWeight::Ref { .. } => None,
            };
            let is_workspace_commit = commit
                .as_ref()
                .map(|commit| {
                    anyhow::Ok(crate::workspace::commit::is_managed_workspace_by_message(
                        commit.message_raw()?,
                    ))
                })
                .transpose()?
                .unwrap_or(false);
            // A segment without a reference of its own continues through the
            // references it reaches, which then point at its next commit. A
            // branch checked out in a worktree still names a segment of its own,
            // and so does a branch with metadata, like a stack's branch below an
            // anonymous commit that took its place as a parent of the workspace commit.
            let continued_segment = match predecessor_segment_info.segment_indexes[..] {
                [(segment_index, _)] if !predecessor_segment_info.must_start_new_segment => {
                    match &node.weight {
                        NodeWeight::Ref { full_name } => {
                            let segment = &inner_graph[segment_index];
                            // Like after a traversal, of the local or remote branches on a
                            // commit that no stack claims, the first of a kind names a
                            // segment, and the ones that follow are recorded on its commit.
                            // Such local branches are recorded the same way when a stack's
                            // branch on the commit names the segment.
                            let shares_segment = segment.commits.is_empty()
                                && self.unclaimed_branches.contains(full_name)
                                && segment.ref_info.as_ref().is_some_and(|ref_info| {
                                    ref_info.ref_name.category() == full_name.category()
                                        && (self.unclaimed_branches.contains(&ref_info.ref_name)
                                            || (full_name.category()
                                                == Some(gix::refs::Category::LocalBranch)
                                                && !is_workspace_ref_name(
                                                    ref_info.ref_name.as_ref(),
                                                )))
                                });
                            let in_workspace = segment.commits.last().is_some_and(|commit| {
                                commit.flags.contains(CommitFlags::InWorkspace)
                            });
                            // Like after a traversal, a branch on a commit that other segments
                            // reach too names a segment of its own, as the commit may end up in
                            // another one than the segment it continues.
                            // Like the tip a traversal makes of it, the target branch names a
                            // segment of its own there too.
                            let continues_anonymous = segment.ref_info.is_none()
                                && segment.commits.is_empty()
                                && leading_nodes[commit_node_of(i)] < 2
                                && project_meta.target_ref.as_ref().is_none_or(|target_ref| {
                                    target_ref.as_local_full_name_ref() != full_name
                                })
                                && !(in_workspace
                                    && full_name.category()
                                        == Some(gix::refs::Category::LocalBranch))
                                && crate::init::walk::extract_local_branch_metadata(
                                    full_name.as_ref(),
                                    overlay_meta,
                                )?
                                .is_none();
                            // Like after a traversal, a lane goes on through several branches
                            // that nothing tells apart, recording them on its commit.
                            let continues_through_ambiguous = !segment.commits.is_empty()
                                && segment.workspace_metadata().is_none()
                                && !segment.ref_info.as_ref().is_some_and(|ref_info| {
                                    is_workspace_ref_name(ref_info.ref_name.as_ref())
                                })
                                && self.unclaimed_branches.contains(full_name)
                                && ambiguous_branches_by_commit
                                    .get(&self.commit_id_of(i))
                                    .is_some_and(|count| *count > 1);
                            // Like after a traversal, only a branch another worktree has checked out
                            // names a segment for that alone, as the traversal walks it as a tip,
                            // and so does the branch the graph is seen from, its entrypoint.
                            ((continues_anonymous || shares_segment || continues_through_ambiguous)
                                && seen_from_ref != Some(full_name.as_ref())
                                && !worktree_by_branch.get(full_name).is_some_and(|worktrees| {
                                    worktrees.iter().any(|wt| !wt.owned_by_repo)
                                }))
                            .then_some(segment_index)
                        }
                        // Like after a traversal, a workspace only owns its workspace commit,
                        // and a workspace commit always starts a segment of its own. Any other
                        // commit starts a segment of its own below the workspace's, which
                        // stays empty.
                        NodeWeight::Pick { oid } | NodeWeight::ShallowPoint { oid } => {
                            let segment = &inner_graph[segment_index];
                            let continues = if segment.workspace_metadata().is_some() {
                                is_workspace_commit
                            } else {
                                // The workspace ref keeps its workspace commit even without
                                // workspace metadata.
                                !is_workspace_commit
                                    || segment.ref_info.as_ref().is_some_and(|ref_info| {
                                        is_workspace_ref_name(ref_info.ref_name.as_ref())
                                    })
                            };
                            // Like the tip a traversal makes of it, the target commit starts a
                            // segment, unless the target branch or the branch the graph is seen
                            // from, both tips of their own, names it without commits of its own.
                            let starts_at_target = project_meta.target_commit_id == Some(*oid)
                                && !(segment.commits.is_empty()
                                    && segment.ref_info.as_ref().is_some_and(|ref_info| {
                                        seen_from_ref == Some(ref_info.ref_name.as_ref())
                                            || project_meta.target_ref.as_ref().is_some_and(
                                                |target_ref| {
                                                    target_ref.as_local_full_name_ref()
                                                        == ref_info.ref_name.as_ref()
                                                },
                                            )
                                    }));
                            // Like after a traversal, a merge commit below commits of a segment
                            // starts a segment of its own.
                            let starts_at_merge =
                                node.parents.len() > 1 && !segment.commits.is_empty();
                            let starts_at_detached_worktree = is_detached_worktree_commit(*oid)
                                && (!segment.commits.is_empty() || segment.metadata.is_none());
                            (continues
                                && detached_commit != Some(*oid)
                                && !starts_at_detached_worktree
                                && !starts_at_target
                                && !starts_at_merge)
                                .then_some(segment_index)
                        }
                    }
                }
                // Like after a traversal, a commit a local branch points at stays with that
                // branch even if remote-tracking branches without commits of their own point
                // at it as well. Remote-tracking branches with commits keep the commit to
                // themselves, so in the editor they don't pass through the local branch, and so
                // does a branch checked out in another worktree, which is a lane of its own.
                // Unless the graph is seen from the workspace, where a stack resting on a commit
                // that other lanes of the workspace or the target reach stays an empty lane, the
                // commit also stays with the branch if other segments with commits of their own
                // reach it. Without such a local branch, the commit stays with a remote-tracking
                // branch without commits of its own that points at it if only remote-tracking
                // branches with commits reach it too, so each remote history keeps the commits
                // its branch points at.
                _ => match &node.weight {
                    NodeWeight::Pick { oid } | NodeWeight::ShallowPoint { oid }
                        if !is_workspace_commit && detached_commit != Some(*oid) =>
                    {
                        let mut local_branch_segment = None;
                        let mut others_leave_the_commit = true;
                        let mut remote_branch_segment = None;
                        let mut only_remotes_with_commits_reach = true;
                        // Like after a traversal, a branch right below commits of its own lane
                        // keeps its commit where the target's lane reaches it too, as that commit
                        // is its own work merged into the target. Only a stack resting on the
                        // commit stays an empty lane.
                        let branch_is_below_commits_of_its_lane = predecessor_segment_info
                            .segment_indexes
                            .iter()
                            .any(|(segment_index, _)| {
                                let segment = &inner_graph[*segment_index];
                                segment.commits.is_empty()
                                    && segment.ref_info.as_ref().is_some_and(|ref_info| {
                                        ref_info.ref_name.category()
                                            == Some(gix::refs::Category::LocalBranch)
                                    })
                                    && inner_graph
                                        .neighbors_directed(
                                            *segment_index,
                                            petgraph::Direction::Incoming,
                                        )
                                        .any(|above| {
                                            let above = &inner_graph[above];
                                            above.workspace_metadata().is_none()
                                                && above.commits.iter().any(|commit| {
                                                    commit.flags.contains(CommitFlags::InWorkspace)
                                                })
                                        })
                            });
                        for (segment_index, _) in &predecessor_segment_info.segment_indexes {
                            let segment = &inner_graph[*segment_index];
                            let category = segment
                                .ref_info
                                .as_ref()
                                .and_then(|ref_info| ref_info.ref_name.category());
                            let is_target_lane = segment.commits.last().is_some_and(|commit| {
                                commit.flags.contains(CommitFlags::Integrated)
                                    && !commit.flags.contains(CommitFlags::InWorkspace)
                            });
                            // A branch outside the workspace and the target, and not checked
                            // out elsewhere, reaching the commit doesn't take it from its branch.
                            let is_branch_outside_lanes = category
                                == Some(gix::refs::Category::LocalBranch)
                                && !segment.ref_info.as_ref().is_some_and(|ref_info| {
                                    worktree_by_branch.get(&ref_info.ref_name).is_some_and(
                                        |worktrees| worktrees.iter().any(|wt| !wt.owned_by_repo),
                                    )
                                })
                                && !segment.commits.last().is_some_and(|commit| {
                                    commit.flags.intersects(
                                        CommitFlags::InWorkspace | CommitFlags::Integrated,
                                    )
                                });
                            match category {
                                Some(gix::refs::Category::RemoteBranch)
                                    if segment.commits.is_empty() =>
                                {
                                    if remote_branch_segment
                                        .is_some_and(|remote| remote != *segment_index)
                                    {
                                        only_remotes_with_commits_reach = false;
                                    }
                                    remote_branch_segment = Some(*segment_index);
                                }
                                Some(gix::refs::Category::LocalBranch)
                                    if segment.commits.is_empty()
                                        && segment.workspace_metadata().is_none()
                                        && !segment.ref_info.as_ref().is_some_and(|ref_info| {
                                            worktree_by_branch.get(&ref_info.ref_name).is_some_and(
                                                |worktrees| {
                                                    worktrees.iter().any(|wt| !wt.owned_by_repo)
                                                },
                                            )
                                        })
                                        && local_branch_segment
                                            .is_none_or(|local| local == *segment_index) =>
                                {
                                    local_branch_segment = Some(*segment_index);
                                    only_remotes_with_commits_reach = false;
                                }
                                Some(gix::refs::Category::RemoteBranch) => {
                                    others_leave_the_commit = false
                                }
                                _ if !segment.commits.is_empty()
                                    && (!seen_from_ref.is_some_and(is_workspace_ref_name)
                                        || is_branch_outside_lanes
                                        || (is_target_lane
                                            && branch_is_below_commits_of_its_lane)) =>
                                {
                                    only_remotes_with_commits_reach = false;
                                }
                                _ => {
                                    others_leave_the_commit = false;
                                    only_remotes_with_commits_reach = false;
                                }
                            }
                        }
                        local_branch_segment
                            .filter(|_| others_leave_the_commit)
                            .or(remote_branch_segment.filter(|_| only_remotes_with_commits_reach))
                    }
                    // Like after a traversal, the workspace commit stays with the workspace
                    // reference even where other lanes reach it too, like those of linked
                    // worktrees started on it.
                    NodeWeight::Pick { oid } | NodeWeight::ShallowPoint { oid }
                        if is_workspace_commit && detached_commit != Some(*oid) =>
                    {
                        predecessor_segment_info
                            .segment_indexes
                            .iter()
                            .map(|(segment_index, _)| *segment_index)
                            .find(|&segment_index| {
                                let segment = &inner_graph[segment_index];
                                segment.commits.is_empty()
                                    && segment.ref_info.as_ref().is_some_and(|ref_info| {
                                        is_workspace_ref_name(ref_info.ref_name.as_ref())
                                    })
                            })
                    }
                    _ => None,
                },
            };
            /// Connects `from`, reaching the node as its parent at `parent_order`, to the
            /// segment `to` the node is in.
            fn connect(
                inner_graph: &mut crate::init::PetGraph,
                from: crate::SegmentIndex,
                parent_order: u32,
                to: crate::SegmentIndex,
            ) {
                let predecessor_segment = &inner_graph[from];
                let src = predecessor_segment.last_commit_index();
                inner_graph.add_edge(
                    from,
                    to,
                    crate::Edge {
                        src,
                        src_id: predecessor_segment.commit_id_by_index(src),
                        dst: None,
                        dst_id: None,
                        parent_order,
                        // The parents of a workspace commit are in stack
                        // order, even where they lead to an empty segment
                        // that has no commit to tell the parent order by.
                        workspace_parent_order: matches!(
                            predecessor_segment.metadata,
                            Some(SegmentMetadata::Workspace(_))
                        )
                        .then_some(parent_order),
                    },
                );
            }
            let segment_index = if let Some(segment_index) = continued_segment {
                // The other segments reaching the continued one point into it.
                for (predecessor_segment_index, parent_order) in
                    &predecessor_segment_info.segment_indexes
                {
                    if *predecessor_segment_index != segment_index {
                        connect(
                            &mut inner_graph,
                            *predecessor_segment_index,
                            *parent_order,
                            segment_index,
                        );
                    }
                }
                segment_index
            } else {
                let metadata = match &node.weight {
                    NodeWeight::Ref { full_name, .. } => {
                        crate::init::walk::extract_local_branch_metadata(
                            full_name.as_ref(),
                            overlay_meta,
                        )?
                        // Like the tip a traversal makes of it, the branch the graph is seen
                        // from takes its metadata along only if no other branch on its commit
                        // has any, as otherwise the traversal only names its segment after it.
                        .filter(|_| {
                            seen_from_ref != Some(full_name.as_ref())
                                || is_workspace_ref_name(full_name.as_ref())
                                || branches_with_metadata_by_commit
                                    .get(&self.commit_id_of(i))
                                    .is_none_or(|count| *count < 2)
                        })
                        .or_else(|| {
                            (seen_from_workspace_ref == Some(full_name.as_ref()))
                                .then(|| SegmentMetadata::Workspace(Default::default()))
                        })
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
                    connect(
                        &mut inner_graph,
                        *predecessor_segment_index,
                        *parent_order,
                        segment_index,
                    );
                }
                segment_index
            };
            match &node.weight {
                NodeWeight::Pick { oid } | NodeWeight::ShallowPoint { oid } => {
                    // A shallow point keeps its parents too, so the editor knows the walk
                    // stopped there and keeps it as it is.
                    let parent_ids: Vec<_> = commit
                        .as_ref()
                        .expect("read for every commit node")
                        .parent_ids()
                        .map(|id| id.detach())
                        .collect();
                    let mut flags = predecessor_segment_info.commit_flags;
                    // Like after a traversal, a commit at the boundary of a shallow clone says so.
                    flags.set(
                        CommitFlags::ShallowBoundary,
                        shallow_commits
                            .as_ref()
                            .is_some_and(|boundary| boundary.binary_search(oid).is_ok()),
                    );
                    inner_graph[segment_index].commits.push(crate::Commit {
                        id: *oid,
                        parent_ids,
                        flags,
                        refs: refs_awaiting_commit.remove(&i).unwrap_or_default(),
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
                    forward_awaiting_refs(
                        &mut refs_awaiting_commit,
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
                    forward_awaiting_refs(&mut refs_awaiting_commit, i, node.parents[0], None);
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

        // Like a traversal, a local branch without a configured upstream pairs with the
        // remote-tracking branch of the same name on the push remote or the target's remote.
        let remote_names = overlay_repo.remote_names();
        let mut symbolic_remote_names: Vec<String> =
            project_meta.push_remote.iter().cloned().collect();
        if let Some((target_remote, _short_name)) = target_upstream_ref
            .as_ref()
            .and_then(|name| {
                but_core::extract_remote_name_and_short_name(name.as_ref(), &remote_names)
            })
            .filter(|(remote, _)| !symbolic_remote_names.contains(remote))
        {
            symbolic_remote_names.push(target_remote);
        }
        let configured_remote_tracking_branches =
            crate::init::remotes::configured_remote_tracking_branches(overlay_repo)?;
        for (segment_index, full_name) in local_branches {
            if let Some(remote_tracking_branch_name) =
                crate::init::remotes::lookup_remote_tracking_branch_or_deduce_it(
                    overlay_repo,
                    full_name.as_ref(),
                    &symbolic_remote_names,
                    &configured_remote_tracking_branches,
                )?
                && let Some(remote_tracking_segment_index) =
                    remote_tracking_branch_name_to_segment_index
                        .remove(remote_tracking_branch_name.as_ref())
            {
                let segment = &mut inner_graph[segment_index];
                segment.remote_tracking_branch_segment_id = Some(remote_tracking_segment_index);
                segment.remote_tracking_ref_name = Some(remote_tracking_branch_name);
                // Like after a traversal, the remote-tracking branch links back to the local one.
                inner_graph[remote_tracking_segment_index].sibling_segment_id = Some(segment_index);
            }
        }

        // Like after a traversal, the persisted stack order of the local branch the graph is
        // seen from keeps that stack's empty branches in the workspace, and that of a branch
        // checked out in a linked worktree keeps them in its lane, as long as at least two of
        // its local branches exist.
        let mut ad_hoc_branch_stack_orders = Vec::new();
        let mut worktree_branch_stack_orders = Vec::new();
        let seen_from_branch =
            seen_from_ref.filter(|name| name.category() == Some(gix::refs::Category::LocalBranch));
        let linked_worktree_branches = worktree_by_branch
            .iter()
            .filter(|(name, worktrees)| {
                worktrees.iter().any(|wt| !wt.owned_by_repo) && Some(name.as_ref()) != seen_from_ref
            })
            .map(|(name, _)| name.as_ref());
        for (is_worktree, branch) in seen_from_branch
            .map(|name| (false, name))
            .into_iter()
            .chain(linked_worktree_branches.map(|name| (true, name)))
        {
            let Some(branch_order) = overlay_meta.branch_stack_order(branch)? else {
                continue;
            };
            let mut existing_branches = Vec::new();
            for branch in branch_order {
                if branch.category() == Some(gix::refs::Category::LocalBranch)
                    && overlay_repo.try_find_reference(branch.as_ref())?.is_some()
                {
                    existing_branches.push(branch);
                }
            }
            if existing_branches.len() > 1 {
                if is_worktree {
                    worktree_branch_stack_orders.push(existing_branches);
                } else {
                    ad_hoc_branch_stack_orders.push(existing_branches);
                }
            }
        }

        let mut graph = crate::Graph {
            inner: inner_graph,
            entrypoint,
            entrypoint_ref: None,
            traversal_tips: Vec::new(),
            ad_hoc_branch_stack_orders,
            worktree_branch_stack_orders,
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
        // Like after a traversal, each segment knows how far down the graph it is.
        graph.compute_generation_numbers();
        // Like after a traversal, the lanes of a workspace are in the order of its stacks in
        // metadata, which the parents of a rewritten workspace commit follow. A lane is found by
        // its stack's top branch, which is further down the lane if the metadata lacks branches
        // stacked onto it in Git, but not below the target commit. Other lanes are placed by
        // their parent order, which also breaks ties.
        for workspace_sidx in graph.segments().collect::<Vec<_>>() {
            let Some(SegmentMetadata::Workspace(workspace_metadata)) =
                &graph[workspace_sidx].metadata
            else {
                continue;
            };
            let stack_position = |name: &FullNameRef| {
                workspace_metadata.stacks.iter().position(|stack| {
                    stack.is_in_workspace()
                        && stack
                            .branches
                            .first()
                            .is_some_and(|branch| branch.ref_name.as_ref() == name)
                })
            };
            let mut lanes = Vec::new();
            for edge in graph
                .inner
                .edges_directed(workspace_sidx, petgraph::Direction::Outgoing)
            {
                let parent_order = edge.weight().parent_order;
                let mut lane = Some(edge.target());
                let mut position = None;
                while let Some(sidx) = lane
                    && position.is_none()
                {
                    let segment = &graph[sidx];
                    if segment
                        .commits
                        .iter()
                        .any(|commit| Some(commit.id) == graph.project_meta.target_commit_id)
                    {
                        break;
                    }
                    position = segment.ref_name().and_then(stack_position);
                    lane = graph
                        .segments_below_in_order(sidx)
                        .next()
                        .map(|(_, below)| below);
                }
                let position = position.map_or(parent_order, |position| position as u32);
                lanes.push(((position, parent_order), edge.id()));
            }
            lanes.sort();
            for (rank, (_, edge_id)) in lanes.into_iter().enumerate() {
                graph.inner[edge_id].workspace_parent_order = Some(rank as u32);
            }
        }
        // The entrypoint so far is the first commit of the walk, which may lie above what the
        // graph is seen from, like a remote-tracking branch that is ahead.
        let entrypoint = match seen_from_ref {
            // An empty segment leading to several lanes, like a workspace reference without
            // its workspace commit, is seen from the commit it's on.
            Some(name) => graph
                .segment_and_commit_by_ref_name(name)
                .map(|(segment, commit)| (segment.id, commit.id))
                .or_else(|| Some((graph.segment_by_ref_name(name)?.id, seen_from_commit?))),
            None => seen_from_commit.and_then(|commit_id| {
                let segment_index = graph.segment_id_by_commit_id(commit_id).ok()?;
                Some((segment_index, commit_id))
            }),
        };
        if let Some((segment_index, commit_id)) = entrypoint {
            graph.entrypoint = Some((segment_index, crate::EntryPointCommit::AtCommit(commit_id)));
        }
        Ok(graph)
    }
}

/// Produce the [segment graph](segment_graph()) of `workspace`: walked from `heads` and everything
/// the workspace's graph was traversed from, and ending below its target commit.
///
/// It uses the workspace's project metadata and its own workspace metadata, which may not be
/// persisted in `meta` yet.
///
/// Its entrypoint is the one of the workspace as long as the graph contains it: the branch the
/// workspace is seen from, or the commit if there is no such branch.
pub fn workspace_segment_graph(
    workspace: &crate::Workspace,
    repo: &gix::Repository,
    meta: &impl RefMetadata,
    mut heads: Vec<gix::ObjectId>,
) -> Result<crate::Graph> {
    let entrypoint = workspace.graph.entrypoint()?;
    let overlay = Overlay::default().with_workspace_metadata_override(
        workspace
            .ref_name()
            .map(ToOwned::to_owned)
            .zip(workspace.metadata.clone()),
    );
    // Everything the workspace's graph was traversed from is part of it,
    // even what lies above the entrypoint.
    heads.extend(workspace.graph.traversal_tips.iter().map(|tip| tip.id));
    segment_graph(
        repo,
        meta,
        overlay,
        workspace.graph.project_meta.clone(),
        entrypoint.segment.ref_name(),
        entrypoint.commit().map(|commit| commit.id),
        heads,
        workspace.graph.worktree_tips.clone(),
        workspace
            .target_commit
            .as_ref()
            .map(|commit| commit.commit_id),
        workspace.graph.options.collect_tags,
        None,
    )
}

/// Project the workspace seen from the branch `seen_from_ref`, or from `seen_from_commit` if
/// there's no such branch, for callers that have no workspace yet. It's projected from the
/// [segment graph](segment_graph()) walked from `seen_from_commit` and `extra_heads`, with
/// `overlay` amending the references of `repo` and `meta`.
///
/// Without `seen_from_commit`, `seen_from_ref` is an unborn branch, and like a traversal, the
/// workspace is that branch alone, as there's nothing to walk.
///
/// Like a traversal, a workspace not seen from a workspace is also walked from the workspaces
/// known to `meta`, so a branch below one is seen within it, and from the linked worktrees in
/// `worktree_tips`, which are recorded on the graph so each is projected as a lane of its own.
/// The walk ends below the target commit of `project_meta`, or covers all history if that commit
/// isn't available, unless `commits_limit` is set: then it only covers that many commits, the
/// most recent first, along with each head.
#[expect(clippy::too_many_arguments)]
pub fn workspace(
    repo: &gix::Repository,
    meta: &impl RefMetadata,
    overlay: Overlay,
    project_meta: but_core::ref_metadata::ProjectMeta,
    seen_from_ref: Option<&FullNameRef>,
    seen_from_commit: Option<gix::ObjectId>,
    extra_heads: impl IntoIterator<Item = gix::ObjectId>,
    worktree_tips: Vec<crate::init::WorktreeTip>,
    collect_tags: bool,
    commits_limit: Option<usize>,
) -> Result<crate::Workspace> {
    let Some(seen_from_commit) = seen_from_commit else {
        // The unborn branch gets an empty segment of its own, checked out in the main worktree.
        let ref_name = seen_from_ref
            .context("an unborn workspace is seen from the branch HEAD points to")?
            .to_owned();
        let (_overlay_repo, overlay_meta, _entrypoint) = overlay.into_parts(repo, meta);
        let worktree_by_branch = BTreeMap::from([(
            ref_name.clone(),
            vec![crate::Worktree {
                kind: crate::WorktreeKind::Main,
                owned_by_repo: true,
            }],
        )]);
        let mut graph = crate::Graph {
            target_upstream_ref: project_meta.remote_target_ref(repo)?,
            project_meta,
            ..Default::default()
        };
        graph.insert_segment_set_entrypoint(crate::init::walk::branch_segment_from_name_and_meta(
            Some((ref_name, None)),
            &overlay_meta,
            None,
            &worktree_by_branch,
        )?);
        return graph.into_workspace();
    };
    // An unavailable target commit bounds nothing, instead of failing the walk.
    let target_commit_id = project_meta
        .resolve_target_commit(repo)
        .ok()
        .filter(|id| repo.find_commit(*id).is_ok());
    let mut heads = vec![seen_from_commit];
    heads.extend(extra_heads);
    {
        let (overlay_repo, overlay_meta, _entrypoint) = overlay.clone().into_parts(repo, meta);
        let seen_from_workspace = seen_from_ref
            .map(|name| overlay_meta.workspace_opt(name))
            .transpose()?
            .flatten()
            .is_some();
        if !seen_from_workspace {
            for (ref_name, _workspace) in overlay_meta.iter_workspaces() {
                if ref_name.category() == Some(gix::refs::Category::LocalBranch)
                    && let Some(mut reference) =
                        overlay_repo.try_find_reference(ref_name.as_ref())?
                {
                    heads.push(reference.peel_to_commit()?.id);
                }
            }
        }
    }
    segment_graph(
        repo,
        meta,
        overlay,
        project_meta,
        seen_from_ref,
        Some(seen_from_commit),
        heads,
        worktree_tips,
        target_commit_id,
        collect_tags,
        commits_limit,
    )?
    .into_workspace()
}

/// Produce a segment graph of all commits reachable from `heads`, the linked worktrees in
/// `worktree_tips` and the local target branch of `project_meta`, along with the workspace stack
/// branches known to `meta` and the remote-tracking branches of the local branches reached, by
/// first building a graph of commits and references in topological order and then grouping it
/// into segments. It needs no [workspace](crate::Workspace), so it can be
/// [projected](crate::Graph::into_workspace()) into one.
///
/// The `worktree_tips` are recorded on the graph, so each is projected as a lane of its own, and
/// like after a traversal, the branch each has checked out forks off onto its commit instead of
/// naming a segment of the lanes reaching it.
///
/// History below `target_commit_id` is only included while it leads to one of the tips, and
/// ends in a commit whose parents are left out. Branches that no stack claims are kept off the
/// stacks if they rest on it. Without it, all history is included, or if `commits_limit` is
/// set, only that many commits, the most recent first, along with each head.
///
/// `overlay` amends the references of `repo` and `meta`, and tags are part of the graph if
/// `collect_tags` is set.
///
/// It's seen from the branch `seen_from_ref`, which is placed above the other branches on its
/// commit except those above it in its stack, or if there is none, from `seen_from_commit`, as
/// long as the graph contains it.
/// Otherwise it's seen from the first commit of the walk. Seen from the workspace ref of a
/// project with metadata, that ref is a workspace even if `meta` has no metadata for it.
#[expect(clippy::too_many_arguments)]
pub fn segment_graph(
    repo: &gix::Repository,
    meta: &impl RefMetadata,
    overlay: Overlay,
    project_meta: but_core::ref_metadata::ProjectMeta,
    seen_from_ref: Option<&FullNameRef>,
    seen_from_commit: Option<gix::ObjectId>,
    mut heads: Vec<gix::ObjectId>,
    worktree_tips: Vec<crate::init::WorktreeTip>,
    target_commit_id: Option<gix::ObjectId>,
    collect_tags: bool,
    commits_limit: Option<usize>,
) -> Result<crate::Graph> {
    let (overlay_repo, overlay_meta, entrypoint) = overlay.into_parts(repo, meta);
    // Like a traversal, the branch the overlay puts `HEAD` on is checked out in this worktree.
    let worktree_by_branch = overlay_repo.worktree_branches(
        entrypoint
            .as_ref()
            .and_then(|(_id, ref_name)| ref_name.as_ref())
            .map(|ref_name| ref_name.as_ref()),
    )?;
    heads.extend(worktree_tips.iter().map(|tip| tip.id));
    // Like a traversal, the branches the linked worktrees have checked out fork off, unless
    // this repository has it checked out or the graph is seen from it.
    let forked_branches: BTreeSet<FullName> = worktree_tips
        .iter()
        .filter_map(|tip| tip.ref_name.clone())
        .filter(|name| {
            !worktree_by_branch
                .get(name)
                .is_some_and(|worktrees| worktrees.iter().any(|wt| wt.owned_by_repo))
                && Some(name.as_ref()) != seen_from_ref
        })
        .collect();
    // A configured target branch may have moved on to commits nothing else reaches.
    if let Some(target_ref) = &project_meta.target_ref
        && let Some(mut reference) =
            overlay_repo.try_find_reference(target_ref.as_local_full_name_ref())?
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
    let mut graph = graph(
        repo,
        &overlay_repo,
        &overlay_meta,
        &overlay_repo.remote_names(),
        &forked_branches,
        seen_from_ref,
        collect_tags,
        heads,
        target_ref,
        project_meta.target_ref.is_some(),
        target_upstream_ref.as_ref().map(|name| name.as_ref()),
        target_commit_id,
        // Cutting short a walk the target commit bounds could leave out that commit, and
        // where the lanes meet its history.
        commits_limit.filter(|_| target_commit_id.is_none()),
    )?
    .to_segment_graph(
        repo,
        &overlay_repo,
        &overlay_meta,
        &worktree_by_branch,
        project_meta,
        seen_from_ref,
        seen_from_commit,
        &worktree_tips,
    )?;
    graph.worktree_tips = worktree_tips;
    // Like after a traversal, a linked worktree's lane lands on the commit another checkout's
    // branch names there, rather than on that branch.
    graph.land_worktree_lanes_on_commits(&overlay_meta, &worktree_by_branch)?;
    graph.compute_generation_numbers();
    Ok(graph)
}

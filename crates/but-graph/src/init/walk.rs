//! Utilities for graph-walking specifically.
use crate::init::overlay::{OverlayMetadata, OverlayRepo};
use crate::init::types::{EdgeOwned, Instruction, Limit, Queue, QueueItem, TopoWalk};
use crate::init::{Goals, PetGraph, remotes};
use crate::{
    Commit, CommitFlags, CommitIndex, Edge, Graph, Segment, SegmentIndex, SegmentMetadata,
    is_workspace_ref_name,
};
use anyhow::{Context, bail};
use but_core::{RefMetadata, ref_metadata};
use gix::hashtable::hash_map::Entry;
use gix::reference::Category;
use gix::traverse::commit::Either;
use petgraph::Direction;
use std::collections::BTreeSet;
use std::ops::Deref;

pub(crate) type RefsById = gix::hashtable::HashMap<gix::ObjectId, Vec<gix::refs::FullName>>;

/// Assure that the first tips most important to us in `next` actually get to own commits.
/// `graph` is used to lookup segments and their names.
///
/// The third argument is used to assure the workspace commit is always owned by the workspace segment,
/// and that otherwise the workspace segment won't own commits.
/// Note that these workspaces are identified by having metadata attached, it doesn't say anything about
/// the reference name.
pub fn prioritize_initial_tips_and_assure_ws_commit_ownership<T: RefMetadata>(
    graph: &mut Graph,
    next: &mut Queue,
    (ws_tips, repo, meta): (
        Vec<gix::ObjectId>,
        &OverlayRepo<'_>,
        &OverlayMetadata<'_, T>,
    ),
) -> anyhow::Result<Vec<SegmentIndex>> {
    next.inner
        .make_contiguous()
        .sort_by_key(|(_id, _flags, instruction, _limit)| {
            // put local branches first, everything else later.
            graph[instruction.segment_idx()]
                .ref_name
                .as_ref()
                .map(|rn| match rn.category() {
                    Some(Category::LocalBranch) => {
                        if is_workspace_ref_name(rn.as_ref()) {
                            Kind::Workspace
                        } else {
                            Kind::Local
                        }
                    }
                    _ => Kind::NonLocal,
                })
        });

    #[derive(Ord, PartialOrd, PartialEq, Eq)]
    enum Kind {
        Local,
        /// Must sort after `Local` so workspaces don't capture commits by default,
        /// code that follows relies on this.
        Workspace,
        NonLocal,
    }

    let mut out = Vec::new();
    'next_ws_tip: for ws_tip in ws_tips {
        if crate::projection::commit::is_managed_workspace_by_message(
            repo.find_commit(ws_tip)?.message_raw()?,
        ) {
            if next.iter().filter(|(tip, ..)| *tip == ws_tip).count() <= 1 {
                // Assume it's the workspace tip, and it's uniquely assigned to a workspace segment.
                continue 'next_ws_tip;
            }
            let mut segments_with_ws_tip =
                next.iter()
                    .enumerate()
                    .filter_map(|(idx, (tip, _, instruction, _))| {
                        (*tip == ws_tip).then_some((idx, instruction.segment_idx()))
                    });
            let (first, second) = (
                segments_with_ws_tip.next().expect("at least two"),
                segments_with_ws_tip.next().expect("at least two"),
            );
            if graph[first.1].workspace_metadata().is_some() {
                continue 'next_ws_tip;
            }
            // Assure that the workspace comes first.
            drop(segments_with_ws_tip);
            next.inner.swap(first.0, second.0);
        } else if next.iter().filter(|(tip, ..)| *tip == ws_tip).count() >= 2 {
            // There are multiple tips pointing to the unmanaged workspace commit.
            let mut segments_with_ws_tip =
                next.iter()
                    .enumerate()
                    .filter_map(|(idx, (tip, _, instruction, _))| {
                        (*tip == ws_tip).then_some((idx, instruction.segment_idx()))
                    });
            let (first, second) = (
                segments_with_ws_tip.next().expect("at least two"),
                segments_with_ws_tip.next().expect("at least two"),
            );
            if graph[first.1].workspace_metadata().is_none() {
                continue 'next_ws_tip;
            }

            // Assure that the non-workspace comes first.
            drop(segments_with_ws_tip);
            next.inner.swap(first.0, second.0);
        } else {
            // Otherwise, assure there is an owner that isn't the workspace branch.
            // To keep it simple, just create anon segments that are fixed up later.

            let (_, flags, _instruction, limit) = next
                .iter()
                .find(|t| t.0 == ws_tip)
                .cloned()
                .expect("each ws-tip has one entry on queue");
            let new_anon_segment =
                graph.insert_root(branch_segment_from_name_and_meta(None, meta, None)?);
            // This segment acts as stand-in - always process it even if the queue says it's done.
            _ = next.push_front_exhausted((
                ws_tip,
                flags,
                Instruction::CollectCommit {
                    into: new_anon_segment,
                },
                limit,
            ));
            out.push(new_anon_segment);
        }
    }
    Ok(out)
}

/// Split `sidx[commit..]` into its own segment and connect the parts. Move all connections in `commit..`
/// from `sidx` to the new segment, and return that.
pub fn split_commit_into_segment(
    graph: &mut Graph,
    next: &mut Queue,
    seen: &mut gix::revwalk::graph::IdMap<SegmentIndex>,
    sidx: SegmentIndex,
    commit: CommitIndex,
) -> anyhow::Result<SegmentIndex> {
    let mut bottom_segment = Segment {
        commits: graph[sidx].commits.clone(),
        ..Default::default()
    };
    // keep only the commits before `commit`.
    let commits_in_top_segment = commit;
    graph[sidx].commits.truncate(commits_in_top_segment);
    bottom_segment.commits = bottom_segment
        .commits
        .into_iter()
        .skip(commits_in_top_segment)
        .collect();
    let top_commit = graph[sidx].last_commit_index();
    let bottom_id = bottom_segment.commits[0].id;
    let bottom_segment = graph.connect_new_segment(sidx, top_commit, bottom_segment, 0, bottom_id);

    // Res-assign ownership to assure future queries will find the right segment.
    for commit_id in graph[bottom_segment].commits.iter().map(|c| c.id) {
        seen.entry(commit_id).insert(bottom_segment);
    }

    // All in-flight commits now go into the new segment.
    replace_queued_segments(next, sidx, bottom_segment);
    split_connections(&mut graph.inner, (sidx, commit), bottom_segment)?;
    Ok(bottom_segment)
}

/// Assumes that `dst.commits == `src[src_commit..]` and will move connections down, updating their
/// indices accordingly.
fn split_connections(
    graph: &mut PetGraph,
    from: (SegmentIndex, CommitIndex),
    dst: SegmentIndex,
) -> anyhow::Result<()> {
    let (sidx, cidx) = from;
    if !collect_edges_from_commit(graph, from, Direction::Incoming).is_empty() {
        bail!(
            "Segment {sidx:?} had incoming connections from commit {cidx}, even though these should cause splits"
        );
    }
    let edges = collect_edges_from_commit(graph, from, Direction::Outgoing);
    for edge in &edges {
        graph.remove_edge(edge.id);
    }

    for edge in edges {
        let edge_src_sidx = if edge
            .weight
            .src_id
            .is_none_or(|src_id| graph[sidx].commit_index_of(src_id).is_some())
        {
            if edge.source != sidx {
                bail!(
                    "BUG: {sidx:?} contained {src_id:?}, but the edge source was {:?}",
                    edge.source,
                    src_id = edge.weight.src_id,
                );
            }
            sidx
        } else {
            // assume that the commit is now contained in the destination edge, so connect that instead.
            dst
        };
        let edge_dst_sidx = if edge_src_sidx == sidx {
            dst
        } else {
            edge.target
        };
        graph.add_edge(
            edge_src_sidx,
            edge_dst_sidx,
            Edge {
                src: edge
                    .weight
                    .src_id
                    .map(|id| {
                        graph[edge_src_sidx].commit_index_of(id).with_context(|| {
                            format!(
                                "BUG: source edge {edge_src_sidx:?} was supposed to contain {:?}",
                                edge.weight.src_id
                            )
                        })
                    })
                    .transpose()?,
                src_id: edge.weight.src_id,
                dst: edge
                    .weight
                    .dst_id
                    .map(|id| {
                        graph[edge_dst_sidx].commit_index_of(id).with_context(|| {
                            format!(
                                "BUG: destination edge {edge_dst_sidx:?} was supposed to contain {:?}",
                                edge.weight.dst_id
                            )
                        })
                    })
                    .transpose()?,
                dst_id: edge.weight.dst_id,
            },
        );
    }
    Ok(())
}

fn collect_edges_from_commit(
    graph: &PetGraph,
    (segment, commit): (SegmentIndex, CommitIndex),
    direction: Direction,
) -> Vec<EdgeOwned> {
    graph
        .edges_directed(segment, direction)
        .filter(|&e| match direction {
            Direction::Incoming => e.weight().dst >= Some(commit),
            Direction::Outgoing => e.weight().src >= Some(commit),
        })
        .map(Into::into)
        .collect()
}

pub fn replace_queued_segments(queue: &mut Queue, find: SegmentIndex, replace: SegmentIndex) {
    for instruction_to_replace in queue.iter_mut().map(|(_, _, instruction, _)| instruction) {
        let cmp = instruction_to_replace.segment_idx();
        if cmp == find {
            *instruction_to_replace = instruction_to_replace.with_replaced_sidx(replace);
        }
    }
}

pub fn swap_queued_segments(queue: &mut Queue, a: SegmentIndex, b: SegmentIndex) {
    for instruction_to_replace in queue.iter_mut().map(|(_, _, instruction, _)| instruction) {
        let cmp = instruction_to_replace.segment_idx();
        if cmp == a {
            *instruction_to_replace = instruction_to_replace.with_replaced_sidx(b);
        } else if cmp == b {
            *instruction_to_replace = instruction_to_replace.with_replaced_sidx(a);
        }
    }
}

pub fn swap_commits_and_connections(graph: &mut PetGraph, a: SegmentIndex, b: SegmentIndex) {
    {
        let (a, b) = graph.index_twice_mut(a, b);
        std::mem::swap(&mut a.commits, &mut b.commits);
    }
    if graph.edges(a).next().is_some() || graph.edges(b).next().is_some() {
        todo!("swap connections of nodes as well")
    }
}

fn local_branches_by_id(
    refs_by_id: &RefsById,
    id: gix::ObjectId,
) -> Option<impl Iterator<Item = &gix::refs::FullName> + '_> {
    refs_by_id.get(&id).map(|refs| {
        refs.iter()
            .filter(|rn| rn.category() == Some(Category::LocalBranch))
    })
}

/// Split `src_sidx` into a new segment (to receive the commit at `info`) and connect it with the new segment
/// whose id will be returned, if…
///
/// * …there is exactly one eligible branch to name it.
/// * …it is a merge commit.
pub fn try_split_non_empty_segment_at_branch<T: RefMetadata>(
    graph: &mut Graph,
    src_sidx: SegmentIndex,
    info: &TraverseInfo,
    refs_by_id: &RefsById,
    meta: &OverlayMetadata<'_, T>,
) -> anyhow::Result<Option<SegmentIndex>> {
    let src_segment = &graph[src_sidx];
    if src_segment.commits.is_empty() {
        return Ok(None);
    }
    let maybe_segment_name_from_unabigous_refs =
        disambiguate_refs_by_branch_metadata_with_lookup((refs_by_id, info.id), meta);
    let Some(maybe_segment_name) = maybe_segment_name_from_unabigous_refs
        .map(Some)
        .or_else(|| {
            let want_segment_without_name = Some(None);
            if info.parent_ids.len() < 2 {
                return None;
            }
            want_segment_without_name
        })
    else {
        return Ok(None);
    };

    let segment_below = branch_segment_from_name_and_meta(maybe_segment_name, meta, None)?;
    let segment_below = graph.connect_new_segment(
        src_sidx,
        src_segment
            .last_commit_index()
            .context("BUG: we are here because the segment above has commits")?,
        segment_below,
        0,
        info.id,
    );
    Ok(Some(segment_below))
}

/// Queue the `parent_ids` of the current commit, whose additional information like `current_kind` and `current_index`
/// are used.
/// `limit` is used to determine if the tip is NOT supposed to be dropped, with `0` meaning it's depleted.
#[must_use]
pub fn queue_parents(
    next: &mut Queue,
    parent_ids: &[gix::ObjectId],
    flags: CommitFlags,
    current_sidx: SegmentIndex,
    current_cidx: CommitIndex,
    mut limit: Limit,
) -> bool {
    if limit.is_exhausted_or_decrement(flags, next) {
        return false;
    }
    if parent_ids.len() > 1 {
        let instruction = Instruction::ConnectNewSegment {
            parent_above: current_sidx,
            at_commit: current_cidx,
        };
        let limit_per_parent = limit.per_parent(parent_ids.len());
        for pid in parent_ids {
            if next.push_back_exhausted((*pid, flags, instruction, limit_per_parent)) {
                return true;
            }
        }
    } else if !parent_ids.is_empty()
        && next.push_back_exhausted((
            parent_ids[0],
            flags,
            Instruction::CollectCommit { into: current_sidx },
            limit,
        ))
    {
        return true;
    }

    false
}

/// As convenience, if `ref_name` is `Some` and the metadata is not set, it will look it up for you.
/// If `ref_name` is `None`, and `refs_by_id_lookup` is `Some`, it will try to look up unambiguous
/// references on that object.
pub fn branch_segment_from_name_and_meta<T: RefMetadata>(
    ref_name: Option<(gix::refs::FullName, Option<SegmentMetadata>)>,
    meta: &OverlayMetadata<'_, T>,
    refs_by_id_lookup: Option<(&RefsById, gix::ObjectId)>,
) -> anyhow::Result<Segment> {
    branch_segment_from_name_and_meta_sibling(ref_name, None, meta, refs_by_id_lookup)
}

/// Like `branch_segment_from_name_and_meta`, but allows to set `sibling_sidx` as well to link
/// a new remote segment to a local tracking branch.
pub fn branch_segment_from_name_and_meta_sibling<T: RefMetadata>(
    ref_name: Option<(gix::refs::FullName, Option<SegmentMetadata>)>,
    sibling_sidx: Option<SegmentIndex>,
    meta: &OverlayMetadata<'_, T>,
    refs_by_id_lookup: Option<(&RefsById, gix::ObjectId)>,
) -> anyhow::Result<Segment> {
    let (ref_name, metadata) =
        unambiguous_local_branch_and_segment_data(ref_name, meta, refs_by_id_lookup)?;
    Ok(Segment {
        metadata,
        ref_name,
        sibling_segment_id: sibling_sidx,
        ..Default::default()
    })
}

fn unambiguous_local_branch_and_segment_data<T: RefMetadata>(
    ref_name: Option<(gix::refs::FullName, Option<SegmentMetadata>)>,
    meta: &OverlayMetadata<'_, T>,
    refs_by_id_lookup: Option<(&RefsById, gix::ObjectId)>,
) -> anyhow::Result<(Option<gix::refs::FullName>, Option<SegmentMetadata>)> {
    Ok(match ref_name {
        None => {
            let Some(lookup) = refs_by_id_lookup else {
                return Ok(Default::default());
            };
            disambiguate_refs_by_branch_metadata_with_lookup(lookup, meta)
                .map(|(rn, md)| (Some(rn), md))
                .unwrap_or_default()
        }
        Some((ref_name, maybe_metadata)) => {
            let metadata = maybe_metadata
                .map(Ok)
                .or_else(|| extract_local_branch_metadata(ref_name.as_ref(), meta).transpose())
                .transpose()?;
            (Some(ref_name), metadata)
        }
    })
}

fn disambiguate_refs_by_branch_metadata_with_lookup<T: RefMetadata>(
    refs_by_id_lookup: (&RefsById, gix::ObjectId),
    meta: &OverlayMetadata<'_, T>,
) -> Option<(gix::refs::FullName, Option<SegmentMetadata>)> {
    let (refs_by_id, id) = refs_by_id_lookup;
    let branches = local_branches_by_id(refs_by_id, id)?;
    disambiguate_refs_by_branch_metadata(branches, meta)
}

pub fn disambiguate_refs_by_branch_metadata<'a, T: RefMetadata>(
    branches: impl Iterator<Item = &'a gix::refs::FullName>,
    meta: &OverlayMetadata<'_, T>,
) -> Option<(gix::refs::FullName, Option<SegmentMetadata>)> {
    let branches = branches
        .map(|rn| {
            (
                rn,
                extract_local_branch_metadata(rn.as_ref(), meta)
                    .ok()
                    .flatten(),
            )
        })
        .collect::<Vec<_>>();
    let mut branches_with_metadata = branches
        .iter()
        .filter_map(|(rn, md)| md.is_some().then_some((*rn, md.as_ref())));
    // Take an unambiguous branch *with* metadata, or fallback to one without metadata.
    branches_with_metadata
        .next()
        .filter(|_| branches_with_metadata.next().is_none())
        .or_else(|| {
            let mut iter = branches.iter();
            iter.next()
                .filter(|_| iter.next().is_none())
                .map(|(rn, md)| (*rn, md.as_ref()))
        })
        .map(|(rn, md)| (rn.clone(), md.cloned()))
}

fn extract_local_branch_metadata<T: RefMetadata>(
    ref_name: &gix::refs::FullNameRef,
    meta: &OverlayMetadata<'_, T>,
) -> anyhow::Result<Option<SegmentMetadata>> {
    if ref_name.category() != Some(Category::LocalBranch) {
        return Ok(None);
    }
    meta.branch_opt(ref_name)
        .map(|res| res.map(SegmentMetadata::Branch))
        .transpose()
        // Also check for workspace data so we always correctly classify segments.
        // This could happen if we run over another workspace commit which is reachable
        // through the current tip.
        .or_else(|| {
            meta.workspace_opt(ref_name)
                .map(|res| res.map(|md| SegmentMetadata::Workspace(md.clone())))
                .transpose()
        })
        .transpose()
}

// Like the plumbing type, but will keep information that was already accessible for us.
#[derive(Debug)]
pub struct TraverseInfo {
    inner: gix::traverse::commit::Info,
    /// The pre-parsed commit if available.
    commit: Option<Commit>,
}

impl TraverseInfo {
    pub fn into_commit(
        self,
        flags: CommitFlags,
        refs: Vec<gix::refs::FullName>,
    ) -> anyhow::Result<Commit> {
        Ok(match self.commit {
            Some(commit) => Commit {
                refs,
                flags,
                ..commit
            },
            None => Commit {
                id: self.inner.id,
                parent_ids: self.inner.parent_ids.into_iter().collect(),
                flags,
                refs,
            },
        })
    }
}

impl Deref for TraverseInfo {
    type Target = gix::traverse::commit::Info;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub fn find(
    cache: Option<&gix::commitgraph::Graph>,
    objects: &impl gix::objs::Find,
    id: gix::ObjectId,
    buf: &mut Vec<u8>,
) -> anyhow::Result<TraverseInfo> {
    let mut parent_ids = gix::traverse::commit::ParentIds::new();
    let commit = match gix::traverse::commit::find(cache, objects, &id, buf)? {
        Either::CachedCommit(c) => {
            let cache = cache.expect("cache is available if a cached commit is returned");
            for parent_id in c.iter_parents() {
                match parent_id {
                    Ok(pos) => parent_ids.push({
                        let parent = cache.commit_at(pos);
                        parent.id().to_owned()
                    }),
                    Err(_err) => {
                        // retry without cache
                        return find(None, objects, id, buf);
                    }
                }
            }
            None
        }
        Either::CommitRefIter(iter) => {
            for token in iter {
                use gix::objs::commit::ref_iter::Token;
                match token {
                    Ok(Token::Tree { .. }) => continue,
                    Ok(Token::Parent { id }) => {
                        parent_ids.push(id);
                    }
                    Ok(_other_tokens) => break,
                    Err(err) => return Err(err.into()),
                };
            }
            Some(Commit {
                id,
                parent_ids: parent_ids.iter().cloned().collect(),
                refs: Vec::new(),
                flags: CommitFlags::empty(),
            })
        }
    };

    Ok(TraverseInfo {
        inner: gix::traverse::commit::Info {
            id,
            parent_ids,
            commit_time: None,
        },
        commit,
    })
}

/// Returns `([(workspace_tip, workspace_ref_name, workspace_info)], target_refs, desired_refs)` for all available workspace,
/// or exactly one workspace if `maybe_ref_name`.
/// already points to a workspace. That way we can discover the workspace containing any starting point, but only if needed.
///
/// This means we process all workspaces if we aren't currently and clearly looking at a workspace.
///
/// Also prune all non-standard workspaces early, or those that don't have a tip.
#[expect(clippy::type_complexity)]
pub fn obtain_workspace_infos<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    maybe_ref_name: Option<&gix::refs::FullNameRef>,
    meta: &OverlayMetadata<'_, T>,
) -> anyhow::Result<(
    Vec<(gix::ObjectId, gix::refs::FullName, ref_metadata::Workspace)>,
    Vec<gix::refs::FullName>,
)> {
    let workspaces = if let Some((ref_name, ws_data)) = maybe_ref_name
        .and_then(|ref_name| {
            meta.workspace_opt(ref_name)
                .transpose()
                .map(|res| res.map(|ws_data| (ref_name, ws_data)))
        })
        .transpose()?
    {
        vec![(ref_name.to_owned(), ws_data)]
    } else {
        meta.iter_workspaces().collect()
    };

    let (mut out, mut target_refs) = (Vec::new(), Vec::new());
    for (rn, data) in workspaces {
        if rn.category() != Some(Category::LocalBranch) {
            tracing::warn!(
                "Skipped workspace at ref {} as workspaces can only ever be on normal branches",
                rn.as_bstr()
            );
            continue;
        }
        if target_refs.contains(&rn) {
            tracing::warn!(
                "Skipped workspace at ref {} as it was also a target ref for another workspace (or for itself)",
                rn.as_bstr()
            );
            continue;
        }
        if let Some(invalid_target_ref) = data
            .target_ref
            .as_ref()
            .filter(|trn| trn.category() != Some(Category::RemoteBranch))
        {
            tracing::warn!(
                "Skipped workspace at ref {} as its target reference {target} was not a remote tracking branch",
                rn.as_bstr(),
                target = invalid_target_ref.as_bstr(),
            );
            continue;
        }
        let Some(ws_tip) = try_refname_to_id(repo, rn.as_ref())? else {
            tracing::warn!(
                "Ignoring stale workspace ref '{ws_ref}', which didn't exist in Git but still had workspace data",
                ws_ref = rn.as_bstr()
            );
            continue;
        };

        target_refs.extend(data.target_ref.clone());
        out.push((ws_tip, rn, data))
    }

    Ok((out, target_refs))
}

pub fn try_refname_to_id(
    repo: &OverlayRepo<'_>,
    refname: &gix::refs::FullNameRef,
) -> anyhow::Result<Option<gix::ObjectId>> {
    Ok(repo
        .try_find_reference(refname)?
        .map(|mut r| r.peel_to_id_in_place())
        .transpose()?
        .map(|id| id.detach()))
}

/// Propagation is always called if one segment reaches another one, which is when the flag
/// among the shared commit are send downward, towards the base.
pub fn propagate_flags_downward(
    graph: &mut PetGraph,
    flags_to_add: CommitFlags,
    dst_sidx: SegmentIndex,
    dst_commit: Option<CommitIndex>,
) {
    let mut topo = TopoWalk::start_from(dst_sidx, dst_commit, petgraph::Direction::Outgoing);
    while let Some((segment, commit_range)) = topo.next(graph) {
        for commit in &mut graph[segment].commits[commit_range] {
            commit.flags |= flags_to_add;
        }
    }
}

/// Check `refs` for refs with remote tracking branches, and return a queue for them for traversal after creating a segment
/// named after the tracking branch.
/// This eager queuing makes sure that the post-processing doesn't have to traverse again when it creates segments
/// that were previously ambiguous.
/// If a remote tracking branch is in `target_refs`, we assume it was already scheduled and won't schedule it again.
/// Note that remotes fully obey the limit.
/// If the created remote segment belongs to the segment of `local_tracking_sidx`, return its Segment index along with its name.
#[expect(clippy::too_many_arguments)]
pub fn try_queue_remote_tracking_branches<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    refs: &[gix::refs::FullName],
    graph: &mut Graph,
    target_symbolic_remote_names: &[String],
    configured_remote_tracking_branches: &BTreeSet<gix::refs::FullName>,
    target_refs: &[gix::refs::FullName],
    meta: &OverlayMetadata<'_, T>,
    id: gix::ObjectId,
    limit: Limit,
    goals: &mut Goals,
) -> anyhow::Result<(Vec<QueueItem>, CommitFlags)> {
    let mut goal_flags = CommitFlags::empty();
    let mut queue = Vec::new();
    for rn in refs {
        let Some(remote_tracking_branch) = remotes::lookup_remote_tracking_branch_or_deduce_it(
            repo,
            rn.as_ref(),
            target_symbolic_remote_names,
            configured_remote_tracking_branches,
        )?
        else {
            continue;
        };
        if target_refs.contains(&remote_tracking_branch) {
            continue;
        }
        // Note that we don't connect the remote segment yet as it still has to reach
        // any segment really. It could be anywhere and point to anything.
        let Some(remote_tip) = try_refname_to_id(repo, remote_tracking_branch.as_ref())? else {
            continue;
        };
        let remote_segment = graph.insert_root(branch_segment_from_name_and_meta(
            Some((remote_tracking_branch.clone(), None)),
            meta,
            None,
        )?);

        let remote_limit = limit.with_indirect_goal(id, goals);
        // These flags are to be attached to `id` so it can propagate itself later.
        // The remote limit is for searching `id`.
        goal_flags |= remote_limit.goal_flags();
        queue.push((
            remote_tip,
            CommitFlags::empty(),
            Instruction::CollectCommit {
                into: remote_segment,
            },
            remote_limit,
        ));
    }
    Ok((queue, goal_flags))
}

pub fn possibly_split_occupied_segment(
    graph: &mut Graph,
    seen: &mut gix::revwalk::graph::IdMap<SegmentIndex>,
    next: &mut Queue,
    id: gix::ObjectId,
    propagated_flags: CommitFlags,
    src_sidx: SegmentIndex,
    limit: Limit,
) -> anyhow::Result<()> {
    let Entry::Occupied(mut existing_sidx) = seen.entry(id) else {
        bail!("BUG: Can only work with occupied entries")
    };
    let dst_sidx = *existing_sidx.get();
    let (top_sidx, mut bottom_sidx) =
        // If a normal branch walks into a workspace branch, put the workspace branch on top
        // so it doesn't own the existing commit.
        if graph[dst_sidx].workspace_metadata().is_some() &&
            graph[src_sidx].ref_name.as_ref()
                .and_then(|rn| rn.category()).is_some_and(|c| matches!(c, Category::LocalBranch)) {
            // `dst` is basically swapping with `src`, so must swap commits and connections.
            swap_commits_and_connections(&mut graph.inner, dst_sidx, src_sidx);
            swap_queued_segments(next, dst_sidx, src_sidx);

            // Assure the first commit doesn't name the new owner segment.
            {
                let s = &mut graph[src_sidx];
                if let Some(c) = s.commits.first_mut() {
                    c.refs.retain(|rn| Some(rn) != s.ref_name.as_ref())
                }
                // Update the commit-ownership of the connecting commit, but also
                // of all other commits in the segment.
                existing_sidx.insert(src_sidx);
                for commit_id in s.commits.iter().skip(1).map(|c| c.id) {
                    seen.entry(commit_id).insert(src_sidx);
                }
            }
            (dst_sidx, src_sidx)
        } else {
            // `src` naturally runs into destination, so nothing needs to be done
            // except for connecting both. Commit ownership doesn't change.
            (src_sidx, dst_sidx)
        };
    let top_cidx = graph[top_sidx].last_commit_index();
    let mut bottom_cidx = graph[bottom_sidx].commit_index_of(id).with_context(|| {
        format!(
            "BUG: Didn't find commit {id} in segment {bottom_sidx}",
            bottom_sidx = dst_sidx.index(),
        )
    })?;

    if bottom_cidx != 0 {
        let new_bottom_sidx =
            split_commit_into_segment(graph, next, seen, bottom_sidx, bottom_cidx)?;
        bottom_sidx = new_bottom_sidx;
        bottom_cidx = 0;
    }
    graph.connect_segments(top_sidx, top_cidx, bottom_sidx, bottom_cidx);
    let top_flags = top_cidx
        .map(|cidx| graph[top_sidx].commits[cidx].flags)
        .unwrap_or_default();
    let bottom_flags = graph[bottom_sidx].commits[bottom_cidx].flags;
    let new_flags = propagated_flags | top_flags | bottom_flags;

    // Only propagate if there is something new as propagation is slow
    if new_flags != bottom_flags {
        propagate_flags_downward(&mut graph.inner, new_flags, bottom_sidx, Some(bottom_cidx));
    }

    // Find the tips that saw this commit, and adjust their limit it that would extend it.
    // The commit is the one we hit, but seen from the newly split segment which should never be empty.
    let bottom_commit_goals = Limit::new(None)
        .additional_goal(
            graph[bottom_sidx]
                .commits
                .first()
                .expect("we just split it out into its own segment")
                .flags,
        )
        .goal_flags();
    for queued_tip_limit in next.iter_mut().filter_map(|(_, _, _, limit)| {
        limit
            .goal_flags()
            .intersects(bottom_commit_goals)
            .then_some(limit)
    }) {
        queued_tip_limit.adjust_limit_if_bigger(limit);
    }
    Ok(())
}

/// Remove if there are only tips with integrated commits and delete empty segments of pruned tips,
/// as these are uninteresting.
/// However, do so only if our entrypoint isn't integrated itself and is not in a workspace. The reason for this is that we
/// always also traverse workspaces and their targets, even if the traversal starts outside a workspace.
pub fn prune_integrated_tips(graph: &mut Graph, next: &mut Queue) {
    let all_integated_and_done = next.iter().all(|(_id, flags, _instruction, tip_limit)| {
        flags.contains(CommitFlags::Integrated) && tip_limit.goal_reached()
    });
    if !all_integated_and_done {
        return;
    }
    if graph
        .lookup_entrypoint()
        .ok()
        .and_then(|ep| ep.segment.non_empty_flags_of_first_commit())
        .is_some_and(|flags| flags.contains(CommitFlags::Integrated))
    {
        return;
    }

    next.inner
        .retain_mut(|(_id, _flags, instruction, _tip_limit)| {
            let sidx = instruction.segment_idx();
            let s = &graph[sidx];
            if s.commits.is_empty() {
                graph.inner.remove_node(sidx);
            }
            false
        });
}

//! Utilities for graph-walking specifically.
use crate::{
    Commit, CommitFlags, CommitIndex, Edge, Graph, Segment, SegmentIndex, SegmentMetadata,
    Worktree,
    init::{
        Goals, PetGraph,
        overlay::{OverlayMetadata, OverlayRepo},
        remotes,
        types::{EdgeOwned, Instruction, Limit, Queue, QueueItem},
    },
};
use anyhow::{Context as _, bail};
use but_core::{RefMetadata, is_workspace_ref_name, ref_metadata};
use gix::{hashtable::hash_map::Entry, reference::Category, traverse::commit::Either};
use petgraph::Direction;
use petgraph::prelude::EdgeRef;
use std::cmp::Ordering;
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Deref,
};

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
    worktree_by_branch: &WorktreeByBranch,
) -> anyhow::Result<Vec<SegmentIndex>> {
    next.inner
        .make_contiguous()
        .sort_by_key(|(_info, _flags, instruction, _limit)| {
            // put local branches first, everything else later.
            graph[instruction.segment_idx()]
                .ref_name()
                .map(|rn| match rn.category() {
                    Some(Category::LocalBranch) => {
                        if is_workspace_ref_name(rn) {
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
            if next.iter().filter(|(info, ..)| info.id == ws_tip).count() <= 1 {
                // Assume it's the workspace tip, and it's uniquely assigned to a workspace segment.
                continue 'next_ws_tip;
            }
            let mut segments_with_ws_tip =
                next.iter()
                    .enumerate()
                    .filter_map(|(idx, (info, _, instruction, _))| {
                        (info.id == ws_tip).then_some((idx, instruction.segment_idx()))
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
        } else if next.iter().filter(|(info, ..)| info.id == ws_tip).count() >= 2 {
            // There are multiple tips pointing to the unmanaged workspace commit.
            let mut segments_with_ws_tip =
                next.iter()
                    .enumerate()
                    .filter_map(|(idx, (info, _, instruction, _))| {
                        (info.id == ws_tip).then_some((idx, instruction.segment_idx()))
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

            let (info, flags, _instruction, limit) = next
                .iter()
                .find(|t| t.0.id == ws_tip)
                .cloned()
                .expect("each ws-tip has one entry on queue");
            let new_anon_segment = graph.insert_segment_set_entrypoint(
                branch_segment_from_name_and_meta(None, meta, None, worktree_by_branch)?,
            );
            // This segment acts as stand-in - always process it even if the queue says it's done.
            _ = next.push_front_exhausted((
                info,
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
/// Note that `standin` is used instead of creating a new bottom segment, and all its outgoing connections will be removed.
pub fn split_commit_into_segment(
    graph: &mut Graph,
    next: &mut Queue,
    seen: &mut gix::revwalk::graph::IdMap<SegmentIndex>,
    sidx: SegmentIndex,
    commit: CommitIndex,
    standin: Option<SegmentIndex>,
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
    let top_commit_index = graph[sidx].last_commit_index();
    let bottom_commit_id = bottom_segment.commits[0].id;
    let bottom_segment = match standin {
        None => {
            graph.connect_new_segment(sidx, top_commit_index, bottom_segment, 0, bottom_commit_id)
        }
        Some(standin_sidx) => {
            let outgoing_edges: Vec<_> = graph
                .edges_directed(standin_sidx, Direction::Outgoing)
                .map(|e| e.id())
                .collect();
            for edge_id in outgoing_edges {
                graph.remove_edge(edge_id);
            }

            let top_commit_id = top_commit_index.map(|idx| graph[sidx].commits[idx].id);
            graph.connect_segments_with_ids(
                sidx,
                top_commit_index,
                top_commit_id,
                standin_sidx,
                0,
                Some(bottom_commit_id),
            );
            let s = &mut graph[standin_sidx];
            s.commits = bottom_segment.commits;
            if !s.commits[0].refs.is_empty()
                && let Some(ref_info) = s.ref_info.take()
            {
                let first = &mut s.commits[0];
                if let Some(pos) = first
                    .refs
                    .iter()
                    .position(|ri| ri.ref_name == ref_info.ref_name)
                {
                    // The standin segment name is already mentioned there (as duplicate),
                    // So remove it.
                    first.refs.remove(pos);
                    // But if there is no name left, put our name back as it's not ambiguous.
                    if first.refs.is_empty() {
                        s.ref_info = Some(ref_info);
                    }
                } else {
                    // Since there are more than one refs here, it's ambiguous,
                    // leave it to post-processing to resolve
                    s.commits[0].refs.push(ref_info);
                }
            }
            standin_sidx
        }
    };

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
    worktree_by_branch: &WorktreeByBranch,
) -> anyhow::Result<Option<SegmentIndex>> {
    let src_segment = &graph[src_sidx];
    if src_segment.commits.is_empty() {
        return Ok(None);
    }
    let maybe_segment_name_from_unambiguous_refs =
        disambiguate_refs_by_branch_metadata_with_lookup((refs_by_id, info.id), meta);
    let Some(maybe_segment_name) =
        maybe_segment_name_from_unambiguous_refs
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

    let segment_below =
        branch_segment_from_name_and_meta(maybe_segment_name, meta, None, worktree_by_branch)?;
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
/// TODO(1.0): remove `no_duplicate_parents` once no old code is running anymore.
/// `no_duplicate_parents` is used to avoid 'broken' merge commits that have multiple parents - our own (probably old) code
/// seems to do that which breaks invariants. Admittedly, this code runs for all merge commits, not just our workspace commits,
/// but as a matter of fact under many situations we already miss such duplicate connection due to the way the traversal is run,
/// so this at least makes this apply to the whole graph uniformly.
#[expect(clippy::too_many_arguments)]
pub fn queue_parents(
    next: &mut Queue,
    no_duplicate_parents: &mut gix::hashtable::HashSet,
    parent_ids: &[gix::ObjectId],
    flags: CommitFlags,
    current_sidx: SegmentIndex,
    current_cidx: CommitIndex,
    mut limit: Limit,
    commit_graph: Option<&gix::commitgraph::Graph>,
    objects: &impl gix::objs::Find,
    buf: &mut Vec<u8>,
) -> anyhow::Result<bool> {
    if limit.is_exhausted_or_decrement(flags, next) {
        return Ok(false);
    }
    if parent_ids.len() > 1 {
        let instruction = Instruction::ConnectNewSegment {
            parent_above: current_sidx,
            at_commit: current_cidx,
        };
        no_duplicate_parents.clear();
        let limit_per_parent = limit.per_parent(parent_ids.len());
        for pid in parent_ids {
            if !no_duplicate_parents.insert(*pid) {
                continue;
            };
            let info = find(commit_graph, objects, *pid, buf)?;
            if next.push_back_exhausted((info, flags, instruction, limit_per_parent)) {
                return Ok(true);
            }
        }
    } else if !parent_ids.is_empty() {
        let info = find(commit_graph, objects, parent_ids[0], buf)?;
        if next.push_back_exhausted((
            info,
            flags,
            Instruction::CollectCommit { into: current_sidx },
            limit,
        )) {
            return Ok(true);
        }
    }

    Ok(false)
}

/// As convenience, if `ref_name` is `Some` and the metadata is not set, it will look it up for you.
/// If `ref_name` is `None`, and `refs_by_id_lookup` is `Some`, it will try to look up unambiguous
/// references on that object.
/// Note that `ref_name` should only be set if you are sure that it is unambiguous, and otherwise won't interfere with
/// the post-processing or the workspace projection later.
pub fn branch_segment_from_name_and_meta<T: RefMetadata>(
    ref_name: Option<(gix::refs::FullName, Option<SegmentMetadata>)>,
    meta: &OverlayMetadata<'_, T>,
    refs_by_id_lookup: Option<(&RefsById, gix::ObjectId)>,
    worktree_by_branch: &WorktreeByBranch,
) -> anyhow::Result<Segment> {
    branch_segment_from_name_and_meta_sibling(
        ref_name,
        None,
        meta,
        refs_by_id_lookup,
        worktree_by_branch,
    )
}

/// Like `branch_segment_from_name_and_meta`, but allows to set `sibling_sidx` as well to link
/// a new remote segment to a local tracking branch.
pub fn branch_segment_from_name_and_meta_sibling<T: RefMetadata>(
    ref_name: Option<(gix::refs::FullName, Option<SegmentMetadata>)>,
    sibling_sidx: Option<SegmentIndex>,
    meta: &OverlayMetadata<'_, T>,
    refs_by_id_lookup: Option<(&RefsById, gix::ObjectId)>,
    worktree_by_branch: &WorktreeByBranch,
) -> anyhow::Result<Segment> {
    let (ref_name, metadata) =
        unambiguous_local_branch_and_segment_data(ref_name, meta, refs_by_id_lookup)?;
    Ok(Segment {
        metadata,
        ref_info: ref_name.map(|rn| crate::RefInfo::from_ref(rn, worktree_by_branch)),
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
#[derive(Debug, Clone)]
pub struct TraverseInfo {
    inner: gix::traverse::commit::Info,
    /// The pre-parsed commit if available.
    commit: Option<Commit>,
    /// A means of sorting the entry on the queue.
    pub(crate) gen_then_time: GenThenTime,
}

#[derive(Debug, Clone)]
pub(crate) struct GenThenTime {
    /// The generation number from the commit-graph cache, if there was one.
    generation: Option<u32>,
    /// The committer timestamp, either from the commit-graph cache, or as parsed from the commit.
    committer_time: u64,
}

impl Eq for GenThenTime {}

impl PartialEq<Self> for GenThenTime {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl PartialOrd<Self> for GenThenTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.cmp(other).into()
    }
}

/// Sort it so younger generations sort first, and lacking two generations, so that more recent times (i.e. higher)
/// sort first.
impl Ord for GenThenTime {
    fn cmp(&self, other: &Self) -> Ordering {
        let time_cmp = self.committer_time.cmp(&other.committer_time).reverse();
        match (self.generation, other.generation) {
            (Some(a), Some(b)) => a.cmp(&b).reverse().then(time_cmp),
            _ => time_cmp,
        }
    }
}

impl TraverseInfo {
    pub fn into_commit(
        self,
        flags: CommitFlags,
        refs: Vec<gix::refs::FullName>,
        worktree_by_branch: &WorktreeByBranch,
    ) -> anyhow::Result<Commit> {
        let refs: Vec<_> = refs
            .into_iter()
            .map(|rn| crate::RefInfo::from_ref(rn, worktree_by_branch))
            .collect();
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
    let (commit, gen_then_time) = match gix::traverse::commit::find(cache, objects, &id, buf)? {
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
            (
                None,
                GenThenTime {
                    generation: c.generation().into(),
                    committer_time: c.committer_timestamp(),
                },
            )
        }
        Either::CommitRefIter(iter) => {
            let mut committer_time = None;
            for token in iter {
                use gix::objs::commit::ref_iter::Token;
                match token {
                    Ok(Token::Tree { .. }) => continue,
                    Ok(Token::Parent { id }) => {
                        parent_ids.push(id);
                    }
                    Ok(Token::Author { .. }) => continue,
                    Ok(Token::Committer { signature }) => {
                        committer_time = Some(
                            signature
                                .time()
                                .map(|t| t.seconds as u64)
                                .unwrap_or_default(),
                        )
                    }
                    Ok(_other_tokens) => break,
                    Err(err) => return Err(err.into()),
                };
            }
            (
                Some(Commit {
                    id,
                    parent_ids: parent_ids.iter().cloned().collect(),
                    refs: Vec::new(),
                    flags: CommitFlags::empty(),
                }),
                GenThenTime {
                    generation: None,
                    committer_time: committer_time.unwrap_or_default(),
                },
            )
        }
    };

    Ok(TraverseInfo {
        inner: gix::traverse::commit::Info {
            id,
            parent_ids,
            commit_time: None,
        },
        commit,
        gen_then_time,
    })
}

/// Returns `([(workspace_tip, workspace_ref_name, workspace_info)], target_refs, desired_refs)` for all available workspace,
/// or exactly one workspace if `maybe_ref_name` has workspace metadata (and only then).
///
/// That way we can discover the workspace containing any starting point, but only if needed.
/// This means we process all workspaces if we aren't currently and clearly looking at a workspace.
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
                "Skipped workspace at ref {rn} as workspaces can only ever be on normal branches",
            );
            continue;
        }
        if target_refs.contains(&rn) {
            tracing::warn!(
                "Skipped workspace at ref {rn} as it was also a target ref for another workspace (or for itself)",
            );
            continue;
        }
        if let Some(invalid_target_ref) = data
            .target_ref
            .as_ref()
            .filter(|trn| trn.category() != Some(Category::RemoteBranch))
        {
            tracing::warn!(
                "Skipped workspace at ref {rn} as its target reference {invalid_target_ref} was not a remote tracking branch",
            );
            continue;
        }
        let Some(ws_tip) = try_refname_to_id(repo, rn.as_ref())? else {
            tracing::warn!(
                "Ignoring stale workspace ref '{rn}', which didn't exist in Git but still had workspace data",
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
        .map(|mut r| r.peel_to_id())
        .transpose()?
        .map(|id| id.detach()))
}

/// Propagation is always called if one segment reaches another one, which is when the flag
/// among the shared commit are send downward, towards the base.
///
/// # Performance Warning
///
/// This function is critical to performance, and ideally is called less. But when it is called,
/// it must be fast, hence the manual implementation of the TopoWalk, which is about 60% faster.
///
/// To validate your changes, clone https://@github.com/schacon/homebrew-cask.git, and run
/// `cargo run --release --bin but-testing -- -dd -C /path/to/homebrew-cask graph -s -l 300 --no-debug-workspace`
///
/// If this gets slower, the change isn't good.
pub fn propagate_flags_downward(
    graph: &mut PetGraph,
    flags_to_add: CommitFlags,
    dst_sidx: SegmentIndex,
    dst_commit: Option<CommitIndex>,
    needs_leafs: bool,
) -> Option<Vec<SegmentIndex>> {
    let mut visited = gix::hashtable::HashSet::default();
    let mut leafs = needs_leafs.then(Vec::new);
    let mut stack = vec![(dst_sidx, dst_commit)];

    while let Some((segment, first_commit_index)) = stack.pop() {
        // Select the range of commits to process in this segment
        let commit_range = if let Some(start_commit_idx) = first_commit_index {
            let segment_ref = &graph[segment];
            start_commit_idx..segment_ref.commits.len()
        } else {
            // Empty segment, no commits to process
            0..0
        };

        // Mark all commits in range with flags
        if !commit_range.is_empty() {
            for commit in &mut graph[segment].commits[commit_range.clone()] {
                commit.flags |= flags_to_add;
            }
        }

        // Process outgoing edges
        let mut neighbors = graph
            .neighbors_directed(segment, petgraph::Direction::Outgoing)
            .detach();

        // Track edges for leaf detection
        let mut edge_count = 0;
        while let Some((edge_idx, target_segment)) = neighbors.next(graph) {
            edge_count += 1;
            let edge = &graph[edge_idx];

            // Skip edges that don't originate from our commit range
            if let Some(src_cidx) = edge.src
                && !commit_range.contains(&src_cidx)
            {
                continue;
            }

            // For DAG, we can visit each node multiple times from different parents,
            // but we want to process each commit-id only once in this walk.
            let next_commit = edge.dst_id;
            if let Some(commit_id) = next_commit
                && visited.insert(commit_id)
            {
                stack.push((target_segment, edge.dst));
            }
        }

        // Track leaf segments (those with no outgoing edges from the processed range)
        if edge_count == 0
            && let Some(ref mut leafs) = leafs
        {
            leafs.push(segment);
        }
    }

    leafs.filter(|v| !v.is_empty())
}

pub(crate) struct RemoteQueueOutcome {
    /// The new tips to queue officially later.
    pub items_to_queue_later: Vec<QueueItem>,
    /// A way for the remote to find the local tracking branch.
    pub maybe_make_id_a_goal_so_remote_can_find_local: CommitFlags,
    /// A way for the local tracking branch to find the remote.
    /// Only set if `items_to_queue_later` is also set.
    pub limit_to_let_local_find_remote: CommitFlags,
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
    next: &Queue,
    worktree_by_branch: &WorktreeByBranch,
    commit_graph: Option<&gix::commitgraph::Graph>,
    objects: &impl gix::objs::Find,
    buf: &mut Vec<u8>,
) -> anyhow::Result<RemoteQueueOutcome> {
    let mut goal_flags = CommitFlags::empty();
    let mut limit_flags = CommitFlags::empty();
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

        // It can happen a remote is in the workspace and was already queued as workspace tip.
        // Don't double-queue.
        if next.iter().any(|t| {
            t.0.id == remote_tip
                && graph[t.2.segment_idx()]
                    .ref_name()
                    .is_some_and(|rn| rn == remote_tracking_branch.as_ref())
        }) {
            continue;
        };
        let remote_segment =
            graph.insert_segment_set_entrypoint(branch_segment_from_name_and_meta(
                Some((remote_tracking_branch.clone(), None)),
                meta,
                None,
                worktree_by_branch,
            )?);

        let remote_limit = limit.with_indirect_goal(id, goals);
        let self_flags = goals.flag_for(remote_tip).unwrap_or_default();
        limit_flags |= self_flags;
        // These flags are to be attached to `id` so it can propagate itself later.
        // The remote limit is for searching `id`.
        // Also, this makes the local tracking branch look for its remote, which is important
        // if the remote is far away as the local branch was rebased somewhere far ahead of the remote.
        goal_flags |= remote_limit.goal_flags();
        let remote_tip_info = find(commit_graph, objects, remote_tip, buf)?;
        queue.push((
            remote_tip_info,
            self_flags,
            Instruction::CollectCommit {
                into: remote_segment,
            },
            remote_limit,
        ));
    }
    Ok(RemoteQueueOutcome {
        items_to_queue_later: queue,
        maybe_make_id_a_goal_so_remote_can_find_local: goal_flags,
        limit_to_let_local_find_remote: limit_flags,
    })
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
            graph[src_sidx].ref_name()
                .and_then(|rn| rn.category()).is_some_and(|c| matches!(c, Category::LocalBranch)) {
            // `dst` is basically swapping with `src`, so must swap commits and connections.
            swap_commits_and_connections(&mut graph.inner, dst_sidx, src_sidx);
            swap_queued_segments(next, dst_sidx, src_sidx);

            // Assure the first commit doesn't name the new owner segment.
            {
                let s: &mut Segment = &mut graph[src_sidx];
                if let Some(c) = s.commits.first_mut() {
                    c.refs.retain(|ri| Some(&ri.ref_name) != s.ref_info.as_ref().map(|rn| &rn.ref_name))
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
        // Re-use an existing empty segment to better integrate them into the graph, and to prevent loose segments
        // just 'hanging around'. This can happen particularly for remote tracking branches which get to their commit
        // too late, but might also be possible for other nodes we have created with a name pre-emptively.
        // And even though there is a good reason to do what we do with remote tracking segments,
        // maybe all this can be simpler?
        let standin = {
            let s = &graph[src_sidx];
            (top_sidx == src_sidx
                && s.commits.is_empty()
                && s.ref_info.is_some()
                && graph
                    .neighbors_directed(src_sidx, Direction::Incoming)
                    .next()
                    .is_none()
                && graph[bottom_sidx]
                    .commits
                    .first()
                    .is_some_and(|c| c.flags.is_remote()))
            .then_some(src_sidx)
        };
        let new_bottom_sidx =
            split_commit_into_segment(graph, next, seen, bottom_sidx, bottom_cidx, standin)?;
        bottom_sidx = new_bottom_sidx;
        bottom_cidx = 0;
    }

    // Standins will cause this, avoid self-connection.
    if top_sidx != bottom_sidx {
        graph.connect_segments(top_sidx, top_cidx, bottom_sidx, bottom_cidx);
    }
    let top_flags = top_cidx
        .map(|cidx| graph[top_sidx].commits[cidx].flags)
        .unwrap_or_default();
    let bottom_flags = graph[bottom_sidx].commits[bottom_cidx].flags;
    let new_flags = propagated_flags | top_flags | bottom_flags;

    // Only propagate if there is something new as propagation is slow
    // Note that we currently assume that the flags are different also to get leafs, even though these
    // depend on flags to propagate. This will be an issue, but seems to work well for all known cases.
    let needs_leafs = !limit.goal_reached();
    let leafs = if new_flags != bottom_flags
        || (needs_leafs
            && next
                .iter()
                .any(|(_, _, _, tip_limit)| !tip_limit.goal_flags().contains(limit.goal_flags())))
    {
        propagate_flags_downward(
            &mut graph.inner,
            new_flags,
            bottom_sidx,
            Some(bottom_cidx),
            needs_leafs,
        )
    } else {
        None
    };

    if let Some(leafs) = leafs {
        propagate_goals_of_reachable_tips(next, leafs, limit.goal_flags());
    }

    // Find the tips that saw this commit, and adjust their limit it that would extend it.
    // The commit is the one we hit, but seen from the newly split segment which should never be empty.
    // TODO: merge this into the new propagation based one.
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

// Find all tips that are scheduled to be put into one of the `reachable_segments`
// (reachable from the commit we just ran into)
fn propagate_goals_of_reachable_tips(
    next: &mut Queue,
    reachable_segments: Vec<SegmentIndex>,
    goal_flags: CommitFlags,
) {
    for (_, _, instruction, limit) in next.iter_mut() {
        if reachable_segments.contains(&instruction.segment_idx()) {
            *limit = limit.additional_goal(goal_flags);
        }
    }
}

/// Remove tips with only integrated commits and delete empty segments of pruned tips,
/// as these are uninteresting.
/// However, do so only if our entrypoint isn't integrated itself and is not in a workspace. The reason for this is that we
/// always also traverse workspaces and their targets, even if the traversal starts outside a workspace.
pub fn prune_integrated_tips(graph: &mut Graph, next: &mut Queue) -> anyhow::Result<()> {
    let all_integated_and_done = next.iter().all(|(_id, flags, _instruction, tip_limit)| {
        flags.contains(CommitFlags::Integrated) && tip_limit.goal_reached()
    });
    if !all_integated_and_done {
        return Ok(());
    }
    let ep = graph.lookup_entrypoint()?;
    if ep
        .segment
        .non_empty_flags_of_first_commit()
        .is_some_and(|flags| flags.contains(CommitFlags::Integrated))
    {
        return Ok(());
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
    Ok(())
}

pub(crate) type WorktreeByBranch = BTreeMap<gix::refs::FullName, Vec<Worktree>>;

impl crate::RefInfo {
    pub(crate) fn from_ref(
        ref_name: gix::refs::FullName,
        worktree_by_branch: &WorktreeByBranch,
    ) -> Self {
        let worktree = worktree_by_branch
            .get(&ref_name)
            .and_then(|worktrees| worktrees.first().cloned());
        Self { ref_name, worktree }
    }
}

use crate::{
    Commit, CommitIndex, Graph, LocalCommit, Segment, SegmentIndex, SegmentMetadata,
    is_workspace_ref_name,
};
use crate::{CommitFlags, Edge};
use anyhow::Context;
use bstr::BString;
use but_core::{RefMetadata, ref_metadata};
use gix::ObjectId;
use gix::hashtable::hash_map::Entry;
use gix::prelude::{ObjectIdExt, ReferenceExt};
use gix::refs::Category;
use gix::traverse::commit::Either;
use std::collections::VecDeque;
use std::ops::Deref;

mod post;

mod walk;
use walk::TopoWalk;

pub(super) type PetGraph = petgraph::Graph<Segment, Edge>;

/// Options for use in [`Graph::from_head()`] and [`Graph::from_commit_traversal()`].
#[derive(Default, Debug, Copy, Clone)]
pub struct Options {
    /// Associate tag references with commits.
    ///
    /// If `false`, tags are not collected.
    pub collect_tags: bool,
    /// Determine how to segment the graph.
    pub segmentation: Segmentation,
}

/// Define how the graph is segmented.
#[derive(Default, Debug, Copy, Clone)]
pub enum Segmentation {
    /// Whenever a merge is encountered, the current segment, including the merge commit, will stop
    /// and start new segments for each of parents.
    #[default]
    AtMergeCommits,
    /// When encountering a merge commit, keep traversing the current segment along the first parent,
    /// and start new segments along the remaining parents.
    /// This creates longer segments along the first parent, giving it greater significance which
    /// seems more appropriate given a user's merge behaviour.
    FirstParentPriority,
}

/// Lifecycle
impl Graph {
    /// Read the `HEAD` of `repo` and represent whatever is visible as a graph.
    ///
    /// See [`Self::from_commit_traversal()`] for details.
    pub fn from_head(
        repo: &gix::Repository,
        meta: &impl RefMetadata,
        options: Options,
    ) -> anyhow::Result<Self> {
        let head = repo.head()?;
        let (tip, maybe_name) = match head.kind {
            gix::head::Kind::Unborn(ref_name) => {
                let mut graph = Graph::default();
                graph.insert_root(branch_segment_from_name_and_meta(
                    Some(ref_name),
                    meta,
                    None,
                )?);
                return Ok(graph);
            }
            gix::head::Kind::Detached { target, peeled } => {
                (peeled.unwrap_or(target).attach(repo), None)
            }
            gix::head::Kind::Symbolic(existing_reference) => {
                let mut existing_reference = existing_reference.attach(repo);
                let tip = existing_reference.peel_to_id_in_place()?;
                (tip, Some(existing_reference.inner.name))
            }
        };
        Self::from_commit_traversal(tip, maybe_name, meta, options)
    }
    /// Produce a minimal-effort representation of the commit-graph reachable from the commit at `tip` such the returned instance
    /// can represent everything that's observed, without loosing information.
    /// `ref_name` is assumed to point to `tip` if given.
    ///
    /// `meta` is used to learn more about the encountered references.
    ///
    /// ### Features
    ///
    /// * discover a Workspace on the fly based on `meta`-data.
    /// * support the notion of a branch to integrate with, the *target*
    ///     - *target* branches consist of a local and remote tracking branch, and one can be ahead of the other.
    ///     - workspaces are relative to the local tracking branch of the target.
    /// * remote tracking branches are seen in relation to their branches.
    /// * the graph of segment assigns each reachable commit
    ///
    /// ### (Arbitrary) Rules
    ///
    /// These rules should help to create graphs and segmentations that feel natural and are desirable to the user.
    /// Change the rules as you see fit to accomplish this.
    ///
    /// * a commit can be governed by multiple workspaces
    /// * as workspaces and entrypoints "grow" together, we don't know anything about workspaces until the every end,
    ///   or when two streams touch. This means we can't make decisions based on [flags](CommitFlags) until the traversal
    ///   is finished.
    /// * an entrypoint is always the start of a segment.
    /// * Segments are always named if their first commit has a single local branch pointing to it.
    /// * Segments stored in the workspace are used/relevant only if they are backed by an existing branch.
    pub fn from_commit_traversal(
        tip: gix::Id<'_>,
        ref_name: impl Into<Option<gix::refs::FullName>>,
        meta: &impl RefMetadata,
        Options {
            collect_tags,
            segmentation,
        }: Options,
    ) -> anyhow::Result<Self> {
        // TODO: also traverse (outside)-branches that ought to be in the workspace. That way we have the desired ones
        //       automatically and just have to find a way to prune the undesired ones.
        // TODO: pickup ref-names and see if some simple logic can avoid messes, like lot's of refs pointing to a single commit.
        //       while at it: make tags work.
        let repo = tip.repo;
        let ref_name = ref_name.into();
        let commit_graph = repo.commit_graph_if_enabled()?;
        let mut buf = Vec::new();
        let mut graph = Graph::default();

        let mut refs_by_id = collect_ref_mapping_by_prefix(
            repo,
            std::iter::once("refs/heads/").chain(if collect_tags {
                Some("refs/tags/")
            } else {
                None
            }),
        )?;
        let mut workspaces = obtain_workspace_infos(ref_name.as_ref().map(|rn| rn.as_ref()), meta)?;
        let current = graph.insert_root(branch_segment_from_name_and_meta(
            ref_name.clone(),
            meta,
            Some((&refs_by_id, tip.detach())),
        )?);
        let mut seen = gix::revwalk::graph::IdMap::<SegmentIndex>::default();
        let mut flags = CommitFlags::empty();

        if let Some(branch_ref) = ref_name {
            // Transfer workspace data to our current ref if it has some.
            workspaces.retain(|(workspace_ref, workspace_info)| {
                if workspace_ref != &branch_ref {
                    return true
                }

                // Turn this segment into a workspace segment.
                let current = &mut graph[current];
                if let Some(md) = &current.metadata {
                    tracing::warn!("BUG(?): Segment '{branch_ref}' had branch metadata {md:?} and workspace metadata - this is unexpected, workspace data takes precedence");
                }
                current.metadata = Some(SegmentMetadata::Workspace(workspace_info.clone()));
                flags |= CommitFlags::InWorkspace;
                false
            })
        }

        let mut next = VecDeque::<QueueItem>::new();
        next.push_back((
            tip.detach(),
            flags,
            Instruction::CollectCommit { into: current },
        ));
        for (ws_ref, workspace_info) in workspaces {
            let Some(ws_tip) = try_refname_to_id(repo, ws_ref.as_ref())? else {
                tracing::warn!(
                    "Ignoring stale workspace ref '{ws_ref}', which didn't exist in Git but still had workspace data",
                    ws_ref = ws_ref.as_bstr()
                );
                continue;
            };
            let mut ws_segment = branch_segment_from_name_and_meta(Some(ws_ref), meta, None)?;
            ws_segment.metadata = Some(SegmentMetadata::Workspace(workspace_info));
            let ws_segment = graph.insert_root(ws_segment);
            // As workspaces typically have integration branches which can help us to stop the traversal,
            // pick these up first.
            next.push_front((
                ws_tip,
                CommitFlags::InWorkspace,
                Instruction::CollectCommit { into: ws_segment },
            ));
        }

        while let Some((id, mut propagated_flags, instruction)) = next.pop_front() {
            let info = find(commit_graph.as_ref(), repo, id, &mut buf)?;
            let src_flags = graph[instruction.segment_idx()]
                .commits
                .last()
                .map(|c| c.flags)
                .unwrap_or_default();

            // These flags might be outdated as they have been queued, meanwhile we may have propagated flags.
            // So be sure this gets picked up.
            propagated_flags |= src_flags;
            let segment_idx_for_id = match instruction {
                Instruction::CollectCommit { into: src_sidx } => match seen.entry(id) {
                    Entry::Occupied(existing_sidx) => {
                        let maybe_src_commit_idx = graph[src_sidx].commits.len().checked_sub(1);
                        let dst_sidx = existing_sidx.get();
                        let dst_commit_idx =
                            graph[*dst_sidx].commit_index_of(id).with_context(|| {
                                format!(
                                    "BUG: Didn't find commit {id} in segment {ex_sidx}",
                                    ex_sidx = dst_sidx.index(),
                                )
                            })?;
                        let (top_sidx, top_cidx, bottom_sidx, bottom_cidx) =
                            if graph[*dst_sidx].workspace_metadata().is_some() {
                                // `dst` is basically swapping with `src`, so must swap commits and connections.
                                swap_commits_and_connections(&mut graph.inner, *dst_sidx, src_sidx);
                                swap_queued_segments(&mut next, *dst_sidx, src_sidx);
                                (*dst_sidx, maybe_src_commit_idx, src_sidx, dst_commit_idx)
                            } else {
                                // `src` naturally runs into destination, so nothing needs to be done.
                                (src_sidx, maybe_src_commit_idx, *dst_sidx, dst_commit_idx)
                            };
                        graph.connect_segments(top_sidx, top_cidx, bottom_sidx, bottom_cidx);
                        let top_flags = top_cidx
                            .map(|cidx| graph[top_sidx].commits[cidx].flags)
                            .unwrap_or_default();
                        let bottom_flags = graph[bottom_sidx].commits[bottom_cidx].flags;
                        propagate_flags_downward(
                            &mut graph.inner,
                            propagated_flags | top_flags | bottom_flags,
                            top_sidx,
                            top_cidx,
                        );

                        continue;
                    }
                    Entry::Vacant(e) => {
                        let src_sidx = try_split_segment_at_singular_branch(
                            &mut graph,
                            src_sidx,
                            id,
                            &refs_by_id,
                            meta,
                        )?
                        .unwrap_or(src_sidx);
                        e.insert(src_sidx);
                        src_sidx
                    }
                },
                Instruction::ConnectNewSegment {
                    parent_above,
                    at_commit,
                } => match seen.entry(id) {
                    Entry::Occupied(_) => {
                        todo!("handle previously existing segment when connecting a new one")
                    }
                    Entry::Vacant(e) => {
                        let segment_below =
                            branch_segment_from_name_and_meta(None, meta, Some((&refs_by_id, id)))?;
                        let segment_below =
                            graph.connect_new_segment(parent_above, at_commit, segment_below, 0);
                        e.insert(segment_below);
                        segment_below
                    }
                },
            };

            let segment = &mut graph[segment_idx_for_id];
            let commit_idx_for_possible_fork = segment.commits.len();
            queue_parents(
                &mut next,
                &info.parent_ids,
                propagated_flags,
                segment_idx_for_id,
                commit_idx_for_possible_fork,
                segmentation,
            );

            segment.commits.push(
                info.into_local_commit(
                    repo,
                    segment
                        .commits
                        // Flags are additive, and meanwhile something may have dumped flags on us
                        // so there is more compared to when the 'flags' value was put onto the queue.
                        .last()
                        .map_or(propagated_flags, |last| last.flags | propagated_flags),
                    refs_by_id
                        .remove(&id)
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|rn| segment.ref_name.as_ref() != Some(rn)),
                )?,
            );
        }

        Ok(graph.post_processed(meta, tip.detach()))
    }
}

fn swap_queued_segments(queue: &mut VecDeque<QueueItem>, a: SegmentIndex, b: SegmentIndex) {
    for instruction_to_replace in queue.iter_mut().map(|(_, _, instruction)| instruction) {
        let cmp = instruction_to_replace.segment_idx();
        if cmp == a {
            *instruction_to_replace = instruction_to_replace.with_replaced_sidx(b);
        } else if cmp == b {
            *instruction_to_replace = instruction_to_replace.with_replaced_sidx(a);
        }
    }
}

fn swap_commits_and_connections(graph: &mut PetGraph, a: SegmentIndex, b: SegmentIndex) {
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

fn try_split_segment_at_singular_branch(
    graph: &mut Graph,
    src_sidx: SegmentIndex,
    id: gix::ObjectId,
    refs_by_id: &RefsById,
    meta: &impl RefMetadata,
) -> anyhow::Result<Option<SegmentIndex>> {
    let src_segment = &graph[src_sidx];
    let Some(mut branch_refs) =
        local_branches_by_id(refs_by_id, id).filter(|_| !src_segment.commits.is_empty())
    else {
        return Ok(None);
    };

    let Some(first_ref) = branch_refs.next() else {
        return Ok(None);
    };
    Ok(if branch_refs.next().is_none() {
        // There is exactly one branch ref in a linear history, start a new segment.
        let segment_below = branch_segment_from_name_and_meta(Some(first_ref.clone()), meta, None)?;
        let segment_below = graph.connect_new_segment(
            src_sidx,
            src_segment
                .commits
                .len()
                .checked_sub(1)
                .context("BUG: we are here because the segment above has commits")?,
            segment_below,
            0,
        );
        Some(segment_below)
    } else {
        // There are more than one branches so we can't infer the name of the segment.
        // This will be handled later during post-processing.
        None
    })
}

#[derive(Debug, Copy, Clone)]
enum Instruction {
    /// Contains the segment into which to place this commit.
    CollectCommit { into: SegmentIndex },
    /// This is the first commit in a new segment which is below `parent_above` and which should be placed
    /// at the last commit (at the time) via `at_commit`.
    ConnectNewSegment {
        parent_above: SegmentIndex,
        at_commit: CommitIndex,
    },
}

impl Instruction {
    /// Returns any segment index we may be referring to.
    fn segment_idx(&self) -> SegmentIndex {
        match self {
            Instruction::CollectCommit { into } => *into,
            Instruction::ConnectNewSegment { parent_above, .. } => *parent_above,
        }
    }

    fn with_replaced_sidx(self, sidx: SegmentIndex) -> Self {
        match self {
            Instruction::CollectCommit { into: _ } => Instruction::CollectCommit { into: sidx },
            Instruction::ConnectNewSegment {
                parent_above: _,
                at_commit,
            } => Instruction::ConnectNewSegment {
                parent_above: sidx,
                at_commit,
            },
        }
    }
}

type QueueItem = (ObjectId, CommitFlags, Instruction);

/// Like the plumbing type, but will keep information that was already accessible for us.
#[derive(Debug)]
struct TraverseInfo {
    inner: gix::traverse::commit::Info,
    /// The pre-parsed commit if available.
    commit: Option<Commit>,
}

/// Queue the `parent_ids` of the current commit, whose additional information like `current_kind` and `current_index`
/// are used.
fn queue_parents(
    next: &mut VecDeque<QueueItem>,
    parent_ids: &[gix::ObjectId],
    flags: CommitFlags,
    current_sidx: SegmentIndex,
    current_cidx: CommitIndex,
    segmentation: Segmentation,
) {
    if parent_ids.len() > 1 {
        match segmentation {
            Segmentation::AtMergeCommits => {
                let instruction = Instruction::ConnectNewSegment {
                    parent_above: current_sidx,
                    at_commit: current_cidx,
                };
                for pid in parent_ids {
                    next.push_back((*pid, flags, instruction))
                }
            }
            Segmentation::FirstParentPriority => {
                let mut parent_ids = parent_ids.iter().cloned();
                // Keep following the first parent in this segment.
                next.push_back((
                    parent_ids.next().expect("more than 1"),
                    flags,
                    Instruction::CollectCommit { into: current_sidx },
                ));
                // Collect all other parents into their own segments.
                let instruction = Instruction::ConnectNewSegment {
                    parent_above: current_sidx,
                    at_commit: current_cidx,
                };
                for pid in parent_ids {
                    next.push_back((pid, flags, instruction))
                }
            }
        }
    } else if !parent_ids.is_empty() {
        next.push_back((
            parent_ids[0],
            flags,
            Instruction::CollectCommit { into: current_sidx },
        ));
    } else {
        return;
    };
}

fn branch_segment_from_name_and_meta(
    mut ref_name: Option<gix::refs::FullName>,
    meta: &impl RefMetadata,
    refs_by_id_lookup: Option<(&RefsById, gix::ObjectId)>,
) -> anyhow::Result<Segment> {
    if let Some((refs_by_id, id)) = refs_by_id_lookup.filter(|_| ref_name.is_none()) {
        if let Some(unambiguous_local_branch) = local_branches_by_id(refs_by_id, id)
            .and_then(|mut branches| branches.next().filter(|_| branches.next().is_none()))
        {
            ref_name = Some(unambiguous_local_branch.clone());
        }
    }
    Ok(Segment {
        metadata: ref_name
            .as_ref()
            .and_then(|rn| {
                meta.branch_opt(rn.as_ref())
                    .map(|res| res.map(|md| SegmentMetadata::Branch(md.clone())))
                    .transpose()
            })
            // Also check for workspace data so we always correctly classify segments.
            // This could happen if we run over another workspace commit which is reachable
            // through the current tip.
            .or_else(|| {
                let rn = ref_name.as_ref()?;
                meta.workspace_opt(rn.as_ref())
                    .map(|res| res.map(|md| SegmentMetadata::Workspace(md.clone())))
                    .transpose()
            })
            .transpose()?,
        ref_name,
        ..Default::default()
    })
}

impl TraverseInfo {
    fn into_local_commit(
        self,
        repo: &gix::Repository,
        flags: CommitFlags,
        refs: impl Iterator<Item = gix::refs::FullName>,
    ) -> anyhow::Result<LocalCommit> {
        let commit = but_core::Commit::from_id(self.id.attach(repo))?;
        let has_conflicts = commit.is_conflicted();
        let refs = refs.collect();
        let commit = match self.commit {
            Some(commit) => Commit {
                refs,
                flags,
                ..commit
            },
            None => Commit {
                id: self.inner.id,
                parent_ids: self.inner.parent_ids.into_iter().collect(),
                message: commit.message.clone(),
                author: commit.author.clone(),
                flags,
                refs,
            },
        };

        Ok(LocalCommit {
            inner: commit,
            relation: Default::default(),
            has_conflicts,
        })
    }
}

impl Deref for TraverseInfo {
    type Target = gix::traverse::commit::Info;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

fn find(
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
            let mut message = None::<BString>;
            let mut author = None;
            for token in iter {
                use gix::objs::commit::ref_iter::Token;
                match token {
                    Ok(Token::Parent { id }) => {
                        parent_ids.push(id);
                    }
                    Ok(Token::Author { signature }) => author = Some(signature.to_owned()?),
                    Ok(Token::Message(msg)) => message = Some(msg.into()),
                    Ok(_other_tokens) => {}
                    Err(err) => return Err(err.into()),
                };
            }
            Some(Commit {
                id,
                parent_ids: parent_ids.iter().cloned().collect(),
                message: message.context("Every valid commit must have a message")?,
                author: author.context("Every valid commit must have an author signature")?,
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

type RefsById = gix::hashtable::HashMap<gix::ObjectId, Vec<gix::refs::FullName>>;

// Create a mapping of all heads to the object ids they point to.
fn collect_ref_mapping_by_prefix<'a>(
    repo: &gix::Repository,
    prefixes: impl Iterator<Item = &'a str>,
) -> anyhow::Result<RefsById> {
    let mut all_refs_by_id = gix::hashtable::HashMap::<_, Vec<_>>::default();
    for prefix in prefixes {
        for (commit_id, git_reference) in repo
            .references()?
            .prefixed(prefix)?
            .filter_map(Result::ok)
            .filter_map(|r| {
                if is_workspace_ref_name(r.name()) {
                    return None;
                }
                let id = r.try_id()?;
                if matches!(r.name().category(), Some(gix::reference::Category::Tag)) {
                    // TODO: also make use of the tag name (the tag object has its own name)
                    (id.object().ok()?.peel_tags_to_end().ok()?.id, r.inner.name)
                } else {
                    (id.detach(), r.inner.name)
                }
                .into()
            })
        {
            all_refs_by_id
                .entry(commit_id)
                .or_default()
                .push(git_reference);
        }
    }
    all_refs_by_id.values_mut().for_each(|v| v.sort());
    Ok(all_refs_by_id)
}

/// Returns `[(workspace_ref_name, workspace_info)]` for all available workspace, or exactly one workspace if `maybe_ref_name`
/// already points to a workspace.
///
/// This means we process all workspaces if we aren't currently and clearly looking at a workspace.
#[allow(clippy::type_complexity)]
fn obtain_workspace_infos(
    maybe_ref_name: Option<&gix::refs::FullNameRef>,
    meta: &impl RefMetadata,
) -> anyhow::Result<Vec<(gix::refs::FullName, ref_metadata::Workspace)>> {
    let workspaces = if let Some((ref_name, ws_data)) = maybe_ref_name
        .and_then(|ref_name| {
            meta.workspace_opt(ref_name)
                .transpose()
                .map(|res| res.map(|ws_data| (ref_name, ws_data)))
        })
        .transpose()?
    {
        vec![(ref_name.to_owned(), ws_data.clone())]
    } else {
        meta.iter()
            .filter_map(Result::ok)
            .filter_map(|(ref_name, item)| {
                item.downcast::<ref_metadata::Workspace>()
                    .ok()
                    .map(|ws| (ref_name, ws))
            })
            .map(|(ref_name, ws)| (ref_name, (*ws).clone()))
            .collect()
    };
    Ok(workspaces)
}

fn try_refname_to_id(
    repo: &gix::Repository,
    refname: &gix::refs::FullNameRef,
) -> anyhow::Result<Option<gix::ObjectId>> {
    Ok(repo
        .try_find_reference(refname)?
        .map(|mut r| r.peel_to_id_in_place())
        .transpose()?
        .map(|id| id.detach()))
}

fn propagate_flags_downward(
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

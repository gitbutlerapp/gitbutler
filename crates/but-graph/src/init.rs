use crate::{
    Commit, CommitIndex, Graph, LocalCommit, RefLocation, Segment, SegmentIndex,
    is_workspace_ref_name,
};
use anyhow::Context;
use bstr::BString;
use gix::ObjectId;
use gix::hashtable::hash_map::Entry;
use gix::prelude::{ObjectIdExt, ReferenceExt};
use gix::traverse::commit::Either;
use std::collections::VecDeque;
use std::ops::Deref;

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
        meta: &impl but_core::RefMetadata,
        options: Options,
    ) -> anyhow::Result<Self> {
        let head = repo.head()?;
        let (tip, maybe_name) = match head.kind {
            gix::head::Kind::Unborn(ref_name) => {
                let empty_segment = Segment {
                    ref_location: Some(RefLocation::OutsideOfWorkspace),
                    ..segment_from_name_and_meta(Some(ref_name), meta)?
                };
                let mut graph = Graph::default();
                graph.insert_root(empty_segment);
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
    pub fn from_commit_traversal(
        tip: gix::Id<'_>,
        ref_name: Option<gix::refs::FullName>,
        meta: &impl but_core::RefMetadata,
        Options {
            collect_tags,
            segmentation,
        }: Options,
    ) -> anyhow::Result<Self> {
        // TODO: also traverse (outside)-branches that ought to be in the workspace. That way we have the desired ones
        //       automatically and just have to find a way to prune the undesired ones.
        // TODO: pickup ref-names and see if some simple logic can avoid messes, like lot's of refs pointing to a single commit.
        //       while at it: make tags work.
        // TODO: We probably want to use a prio-queue walk the first parent faster (or even first) for more stable and probably
        //       better results.
        let repo = tip.repo;
        let commit_graph = repo.commit_graph_if_enabled()?;
        let mut buf = Vec::new();
        let mut graph = Graph::default();
        let current = graph.insert_root(segment_from_name_and_meta(ref_name, meta)?);
        let mut seen = gix::revwalk::graph::IdMap::<SegmentIndex>::default();

        let mut next = VecDeque::<QueueItem>::new();
        next.push_back((
            tip.detach(),
            CommitKind::Unclassified,
            Instruction::CollectCommit { into: current },
        ));
        let mut refs_by_id = collect_ref_mapping_by_prefix(
            repo,
            std::iter::once("refs/heads/").chain(if collect_tags {
                Some("refs/tags/")
            } else {
                None
            }),
        )?;

        while let Some((id, kind, instruction)) = next.pop_front() {
            let info = find(commit_graph.as_ref(), repo, id, &mut buf)?;
            let segment_idx_for_id = match instruction {
                Instruction::CollectCommit { into: src_sidx } => match seen.entry(id) {
                    Entry::Occupied(existing_sidx) => {
                        let src_segment = &graph[src_sidx];
                        let Some(src_commit) =
                            src_segment.commits_unique_from_tip.len().checked_sub(1)
                        else {
                            // Cannot assign this ID as it's already in `existing_segment`
                            // Or one could just connect the segment itself, logically
                            todo!("Probably it's OK to let the segment disappear")
                        };
                        let dst_sidx = existing_sidx.get();
                        let dst_segment = &graph[*dst_sidx];
                        let dst_commit = dst_segment.commit_index_of(id).with_context(|| {
                            format!(
                                "BUG: Didn't find commit {id} in segment {ex_sidx}",
                                ex_sidx = dst_sidx.index(),
                            )
                        })?;
                        graph.connect_segments(src_sidx, src_commit, *dst_sidx, dst_commit);
                        continue;
                    }
                    Entry::Vacant(e) => {
                        e.insert(src_sidx);
                        src_sidx
                    }
                },
                Instruction::ConnectNewSegment {
                    parent_above,
                    at_commit,
                } => match seen.entry(id) {
                    Entry::Occupied(_) => {
                        todo!("handle previously existing segment")
                    }
                    Entry::Vacant(e) => {
                        let segment_below = segment_from_name_and_meta(None, meta)?;
                        let segment_below =
                            graph.connect_new_segment(parent_above, at_commit, segment_below, 0);
                        e.insert(segment_below);
                        segment_below
                    }
                },
            };

            let segment = &mut graph[segment_idx_for_id];
            let commit_idx_for_possible_fork = segment.commits_unique_from_tip.len();
            queue_parents(
                &mut next,
                &info.parent_ids,
                kind,
                segment_idx_for_id,
                commit_idx_for_possible_fork,
                segmentation,
            );

            segment.commits_unique_from_tip.push(
                info.into_local_commit(
                    repo,
                    refs_by_id
                        .remove(&id)
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|rn| segment.ref_name.as_ref() != Some(rn)),
                )?,
            );
        }

        Ok(graph)
    }
}

#[derive(Copy, Clone)]
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

type QueueItem = (ObjectId, CommitKind, Instruction);

#[derive(Debug, Copy, Clone)]
enum CommitKind {
    Unclassified,
}

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
    current_kind: CommitKind,
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
                    next.push_back((*pid, current_kind, instruction))
                }
            }
            Segmentation::FirstParentPriority => {
                let mut parent_ids = parent_ids.iter().cloned();
                // Keep following the first parent in this segment.
                next.push_back((
                    parent_ids.next().expect("more than 1"),
                    current_kind,
                    Instruction::CollectCommit { into: current_sidx },
                ));
                // Collect all other parents into their own segments.
                let instruction = Instruction::ConnectNewSegment {
                    parent_above: current_sidx,
                    at_commit: current_cidx,
                };
                for pid in parent_ids {
                    next.push_back((pid, current_kind, instruction))
                }
            }
        }
    } else if !parent_ids.is_empty() {
        next.push_back((
            parent_ids[0],
            current_kind,
            Instruction::CollectCommit { into: current_sidx },
        ));
    } else {
        return;
    };
}

fn segment_from_name_and_meta(
    ref_name: Option<gix::refs::FullName>,
    meta: &impl but_core::RefMetadata,
) -> anyhow::Result<Segment> {
    Ok(Segment {
        metadata: ref_name
            .as_ref()
            .and_then(|rn| meta.branch_opt(rn.as_ref()).transpose())
            .transpose()?
            .map(|md| md.clone()),
        ref_name,
        ..Default::default()
    })
}

impl TraverseInfo {
    fn into_local_commit(
        self,
        repo: &gix::Repository,
        refs: impl Iterator<Item = gix::refs::FullName>,
    ) -> anyhow::Result<LocalCommit> {
        let commit = but_core::Commit::from_id(self.id.attach(repo))?;
        let has_conflicts = commit.is_conflicted();
        let refs = refs.collect();
        let commit = match self.commit {
            Some(commit) => Commit { refs, ..commit },
            None => Commit {
                id: self.inner.id,
                parent_ids: self.inner.parent_ids.into_iter().collect(),
                message: commit.message.clone(),
                author: commit.author.clone(),
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

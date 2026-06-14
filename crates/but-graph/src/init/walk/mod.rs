//! Utilities for graph-walking specifically.
use std::{cmp::Ordering, collections::BTreeMap, ops::Deref};

use but_core::{RefMetadata, is_workspace_ref_name, ref_metadata};
use gix::{reference::Category, traverse::commit::Either};

use crate::{
    SegmentMetadata, Worktree,
    init::{
        overlay::{OverlayMetadata, OverlayRepo},
        types::{Queue, Step},
    },
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
    state: &mut super::commit_walk::State,
    next: &mut Queue,
    (ws_tips, repo, meta): (
        Vec<gix::ObjectId>,
        &OverlayRepo<'_>,
        &OverlayMetadata<'_, T>,
    ),
    worktree_by_branch: &WorktreeByBranch,
) -> anyhow::Result<Vec<usize>> {
    next.inner
        .make_contiguous()
        .sort_by_key(|(_info, _flags, step, _limit)| {
            // put local branches first, everything else later.
            state
                .seg_ref_name(
                    step.seed_segment()
                        .expect("initial items are all tip seeds"),
                )
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
        if crate::workspace::commit::is_managed_workspace_by_message(
            repo.find_commit(ws_tip)?.message_raw()?,
        ) {
            if next.iter().filter(|(info, ..)| info.id == ws_tip).count() <= 1 {
                // Assume it's the workspace tip, and it's uniquely assigned to a workspace segment.
                continue 'next_ws_tip;
            }
            let mut segments_with_ws_tip =
                next.iter()
                    .enumerate()
                    .filter_map(|(idx, (info, _, step, _))| {
                        Some((idx, step.seed_segment()?)).filter(|_| info.id == ws_tip)
                    });
            let (first, second) = (
                segments_with_ws_tip.next().expect("at least two"),
                segments_with_ws_tip.next().expect("at least two"),
            );
            if state.seg_workspace_metadata(first.1).is_some() {
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
                    .filter_map(|(idx, (info, _, step, _))| {
                        Some((idx, step.seed_segment()?)).filter(|_| info.id == ws_tip)
                    });
            let (first, second) = (
                segments_with_ws_tip.next().expect("at least two"),
                segments_with_ws_tip.next().expect("at least two"),
            );
            if state.seg_workspace_metadata(first.1).is_none() {
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
            let new_anon_segment = state.insert_recording_set_entrypoint(
                branch_segment_from_name_and_meta(None, meta, None, worktree_by_branch)?,
            );
            // This segment acts as stand-in - always process it even if the queue says it's done.
            _ = next.push_front_exhausted((
                info,
                flags,
                Step::SeedTip {
                    into: new_anon_segment,
                },
                limit,
            ));
            out.push(new_anon_segment);
        }
    }
    Ok(out)
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

/// As convenience, if `ref_name` is `Some` and the metadata is not set, it will look it up for you.
/// If `ref_name` is `None`, and `refs_by_id_lookup` is `Some`, it will try to look up unambiguous
/// references on that object.
/// Note that `ref_name` should only be set if you are sure that it is unambiguous, and otherwise won't interfere with
/// the workspace projection later.
pub fn branch_segment_from_name_and_meta<T: RefMetadata>(
    ref_name: Option<(gix::refs::FullName, Option<SegmentMetadata>)>,
    meta: &OverlayMetadata<'_, T>,
    refs_by_id_lookup: Option<(&RefsById, gix::ObjectId)>,
    worktree_by_branch: &WorktreeByBranch,
) -> anyhow::Result<(Option<crate::RefInfo>, Option<SegmentMetadata>)> {
    let commit_id = refs_by_id_lookup.map(|(_, id)| id);
    let (ref_name, metadata) =
        unambiguous_local_branch_and_segment_data(ref_name, meta, refs_by_id_lookup)?;
    Ok((
        ref_name.map(|rn| crate::RefInfo::from_ref(rn, commit_id, worktree_by_branch)),
        metadata,
    ))
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

pub fn disambiguate_refs_by_branch_metadata_with_lookup<T: RefMetadata>(
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

/// Sort it so younger generations sort first, with more recent times (i.e. higher) as tiebreaker.
/// When the generation is unknown (`None`), treat it as `u32::MAX` (youngest possible) to match
/// git's `GENERATION_NUMBER_INFINITY` convention, ensuring unknown-generation commits are processed
/// first during traversal.
impl Ord for GenThenTime {
    fn cmp(&self, other: &Self) -> Ordering {
        // Using a fixed sentinel for `None` is necessary to maintain a total order
        // — the previous approach of falling back to time-only comparison when generations were mixed
        // violated transitivity.
        let gen_a = self.generation.unwrap_or(u32::MAX);
        let gen_b = other.generation.unwrap_or(u32::MAX);
        gen_a
            .cmp(&gen_b)
            .reverse()
            .then_with(|| self.committer_time.cmp(&other.committer_time).reverse())
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
    let gen_then_time = match gix::traverse::commit::find(cache, objects, &id, buf)? {
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
            GenThenTime {
                generation: c.generation().into(),
                committer_time: c.committer_timestamp(),
            }
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
            GenThenTime {
                generation: None,
                committer_time: committer_time.unwrap_or_default(),
            }
        }
    };

    Ok(TraverseInfo {
        inner: gix::traverse::commit::Info {
            id,
            parent_ids,
            commit_time: None,
        },
        gen_then_time,
    })
}

/// Returns `[(workspace_tip, workspace_ref_name, workspace_info)]` for all available workspace,
/// or exactly one workspace if `maybe_ref_name` has workspace metadata (and only then).
///
/// That way we can discover the workspace containing any starting point, but only if needed.
/// This means we process all workspaces if we aren't currently and clearly looking at a workspace.
/// Also prune all non-standard workspaces early, or those that don't have a tip.
pub fn obtain_workspace_infos<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    maybe_ref_name: Option<&gix::refs::FullNameRef>,
    meta: &OverlayMetadata<'_, T>,
) -> anyhow::Result<Vec<(gix::ObjectId, gix::refs::FullName, ref_metadata::Workspace)>> {
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

    let mut out = Vec::new();
    for (rn, data) in workspaces {
        if rn.category() != Some(Category::LocalBranch) {
            tracing::warn!(
                "Skipped workspace at ref {rn} as workspaces can only ever be on normal branches",
            );
            continue;
        }
        let Some(ws_tip) = try_refname_to_id(repo, rn.as_ref())? else {
            tracing::warn!(
                "Ignoring stale workspace ref '{rn}', which didn't exist in Git but still had workspace data",
            );
            continue;
        };

        out.push((ws_tip, rn, data))
    }

    Ok(out)
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

pub(crate) type WorktreeByBranch = BTreeMap<gix::refs::FullName, Vec<Worktree>>;

impl crate::RefInfo {
    pub(crate) fn from_ref(
        ref_name: gix::refs::FullName,
        commit_id: impl Into<Option<gix::ObjectId>>,
        worktree_by_branch: &WorktreeByBranch,
    ) -> Self {
        let worktree = worktree_by_branch
            .get(&ref_name)
            .and_then(|worktrees| worktrees.first().cloned());
        Self {
            ref_name,
            commit_id: commit_id.into(),
            worktree,
        }
    }
}

#[cfg(test)]
mod tests;

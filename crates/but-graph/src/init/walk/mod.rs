use std::collections::BTreeMap;

use but_core::RefMetadata;
use gix::reference::Category;

use crate::{Segment, SegmentIndex, SegmentMetadata, Worktree, init::overlay::OverlayMetadata};

pub(crate) type RefsById = gix::hashtable::HashMap<gix::ObjectId, Vec<gix::refs::FullName>>;

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
/// a new remote tracking segment to a local tracking segment.
pub fn branch_segment_from_name_and_meta_sibling<T: RefMetadata>(
    ref_name: Option<(gix::refs::FullName, Option<SegmentMetadata>)>,
    sibling_sidx: Option<SegmentIndex>,
    meta: &OverlayMetadata<'_, T>,
    refs_by_id_lookup: Option<(&RefsById, gix::ObjectId)>,
    worktree_by_branch: &WorktreeByBranch,
) -> anyhow::Result<Segment> {
    let commit_id = refs_by_id_lookup.map(|(_, id)| id);
    let (ref_name, metadata) =
        unambiguous_local_branch_and_segment_data(ref_name, meta, refs_by_id_lookup)?;
    Ok(Segment {
        metadata,
        ref_info: ref_name.map(|rn| crate::RefInfo::from_ref(rn, commit_id, worktree_by_branch)),
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

pub(crate) fn extract_local_branch_metadata<T: RefMetadata>(
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
                .map(|res| res.map(SegmentMetadata::Workspace))
                .transpose()
        })
        .transpose()
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

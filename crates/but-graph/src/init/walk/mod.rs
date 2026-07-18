//! Utilities shared by commit traversal and reference discovery.

use std::collections::BTreeMap;

use but_core::{RefMetadata, ref_metadata};
use gix::reference::Category;

use crate::{
    Worktree,
    init::overlay::{OverlayMetadata, OverlayRepo},
};

pub(crate) type RefsById = gix::hashtable::HashMap<gix::ObjectId, Vec<gix::refs::FullName>>;
pub(crate) type WorktreeByBranch = BTreeMap<gix::refs::FullName, Vec<Worktree>>;

/// Return all applicable workspace tips and their metadata.
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
        out.push((ws_tip, rn, data));
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

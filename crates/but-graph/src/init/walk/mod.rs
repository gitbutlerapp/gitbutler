//! Utilities shared by commit traversal and reference discovery.

use std::{cmp::Ordering, collections::BTreeMap, ops::Deref};

use but_core::{RefMetadata, ref_metadata};
use gix::{reference::Category, traverse::commit::Either};

use crate::{
    Worktree,
    init::overlay::{OverlayMetadata, OverlayRepo},
};

pub(crate) type RefsById = gix::hashtable::HashMap<gix::ObjectId, Vec<gix::refs::FullName>>;
pub(crate) type WorktreeByBranch = BTreeMap<gix::refs::FullName, Vec<Worktree>>;

/// Commit data needed while traversing, plus its stable queue sort key.
#[derive(Debug, Clone)]
pub struct TraverseInfo {
    inner: gix::traverse::commit::Info,
    pub(crate) gen_then_time: GenThenTime,
}

#[derive(Debug, Clone)]
pub(crate) struct GenThenTime {
    generation: Option<u32>,
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

/// Sort younger generations first, using newer commit times as the tiebreaker.
impl Ord for GenThenTime {
    fn cmp(&self, other: &Self) -> Ordering {
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
                    Ok(pos) => parent_ids.push(cache.commit_at(pos).id().to_owned()),
                    Err(_err) => return find(None, objects, id, buf),
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
                    Ok(Token::Tree { .. } | Token::Author { .. }) => continue,
                    Ok(Token::Parent { id }) => parent_ids.push(id),
                    Ok(Token::Committer { signature }) => {
                        committer_time = Some(
                            signature
                                .time()
                                .map(|t| t.seconds as u64)
                                .unwrap_or_default(),
                        )
                    }
                    Ok(_) => break,
                    Err(err) => return Err(err.into()),
                }
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

#[cfg(test)]
mod tests;

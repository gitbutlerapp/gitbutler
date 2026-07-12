//! Remote-region decisions: the AHEAD-region shape, its segment tips, and which
//! remote names are in play.

use std::collections::HashMap;

use super::{IdMap, IdSet, is_plain_local_branch};
use crate::CommitGraph;

/// A region's AHEAD set (commits reachable from its tip that are not in-set) with the shape
/// data it is segmented by: merges' first parents and the child fan-out.
pub(super) struct AheadRegion {
    pub(super) set: IdSet,
    merge_first_parents: IdSet,
    children: IdMap<Vec<gix::ObjectId>>,
}

impl AheadRegion {
    pub(super) fn compute(cg: &CommitGraph, tip: gix::ObjectId, in_set: &IdSet) -> Self {
        let mut set = IdSet::default();
        let mut stack = vec![tip];
        while let Some(id) = stack.pop() {
            if in_set.contains(&id) || !set.insert(id) {
                continue;
            }
            stack.extend(cg.all_parent_ids(id));
        }
        let mut children: IdMap<Vec<gix::ObjectId>> = IdMap::default();
        for &c in &set {
            for p in cg.all_parent_ids(c) {
                if set.contains(&p) {
                    children.entry(p).or_default().push(c);
                }
            }
        }
        let merge_first_parents: IdSet = set
            .iter()
            .filter(|&&c| cg.all_parent_ids(c).len() > 1)
            .filter_map(|&c| cg.first_parent(c))
            .filter(|p| set.contains(p))
            .collect();
        AheadRegion {
            set,
            merge_first_parents,
            children,
        }
    }

    /// The region's SHAPE boundaries, mirroring local segmentation: the tip, pinned commits,
    /// merges and their first parents, plain-local-branch carriers, and fan-out/second-parent
    /// joints.
    pub(super) fn is_shape_boundary(
        &self,
        cg: &CommitGraph,
        tip: gix::ObjectId,
        pinned_commits: &IdSet,
        c: gix::ObjectId,
    ) -> bool {
        c == tip
            || pinned_commits.contains(&c)
            || cg.all_parent_ids(c).len() > 1
            || self.merge_first_parents.contains(&c)
            || cg.refs_at(c).iter().any(is_plain_local_branch)
            || {
                let kids = self.children.get(&c).map(Vec::as_slice).unwrap_or_default();
                kids.len() > 1
                    || kids
                        .iter()
                        .any(|&k| cg.first_parent(k) != Some(c) && self.set.contains(&k))
            }
    }
}

/// The region's segment tips in minting order: the region tip first, then descending
/// generation, then id — deterministic even though the ahead set is a hash set.
pub(super) fn region_tips(
    cg: &CommitGraph,
    region: &AheadRegion,
    region_tip: gix::ObjectId,
    is_boundary: &impl Fn(gix::ObjectId) -> bool,
) -> Vec<gix::ObjectId> {
    let mut tips: Vec<gix::ObjectId> = region
        .set
        .iter()
        .copied()
        .filter(|&c| is_boundary(c))
        .collect();
    tips.sort_by_cached_key(|&t| {
        (
            t != region_tip,
            std::cmp::Reverse(cg.generation_of(t).unwrap_or(0)),
            t,
        )
    });
    tips
}

/// The unique plain local branch at `c`, if any — the name fallback for region roots and
/// interior boundaries; ambiguity yields `None`.
pub(super) fn unique_plain_local(
    cg: &CommitGraph,
    c: gix::ObjectId,
) -> Option<gix::refs::FullName> {
    let mut it = cg.refs_at(c).into_iter().filter(is_plain_local_branch);
    it.next().filter(|_| it.next().is_none())
}

/// Is `remote_ref` on a remote the workspace configuration implies (target/push remote, or a
/// git-configured tracking branch)? Only such remotes' ahead regions are traversed.
pub(super) fn remote_name_in_play(
    remote_ref: &gix::refs::FullName,
    symbolic_remotes: &[String],
) -> bool {
    remote_ref
        .as_bstr()
        .strip_prefix(b"refs/remotes/".as_ref())
        .is_some_and(|rest| {
            symbolic_remotes.iter().any(|r| {
                rest.strip_prefix(r.as_bytes())
                    .is_some_and(|s| s.first() == Some(&b'/'))
            })
        })
}

/// Local branch -> its remote-tracking branch, mirroring the walk's
/// `lookup_remote_tracking_branch_or_deduce_it`, plus the SYMBOLIC remote names in play:
/// 1. A branch CONFIGURED in git (`branch.<name>.remote`/`merge`) tracks that remote branch.
/// 2. Otherwise the relationship is deduced by name (`refs/remotes/<remote>/<X>` for `refs/heads/<X>`),
///    but ONLY against remotes the workspace configuration implies — the `push_remote` (highest
///    priority: "the push-remote overrides the remote we use for listing, even if a fetch remote is
///    available"), then the remote of the configured `target_ref`. A workspace with neither deduces
///    NO name-based relationships at all.
///
/// The returned symbolic names also gate which remotes' AHEAD regions the graph traverses — a
/// config-only tracking link keeps its name, but its remote's own commits stay out of the graph,
/// matching what the walk's traversal reaches.
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn remote_tracking_from_repository(
    repo: &gix::Repository,
    overlay_repo: &crate::walk::overlay::OverlayRepo<'_>,
    project_meta: &but_core::ref_metadata::ProjectMeta,
) -> anyhow::Result<(
    HashMap<gix::refs::FullName, gix::refs::FullName>,
    Vec<String>,
)> {
    let mut remotes: Vec<String> = Vec::new();
    if let Some(push_remote) = project_meta.push_remote.as_deref() {
        remotes.push(push_remote.to_string());
    }
    if let Some(target_ref) = project_meta.target_ref.as_ref()
        && let Some((remote, _short)) =
            but_core::extract_remote_name_and_short_name(target_ref.as_ref(), &repo.remote_names())
        && !remotes.contains(&remote)
    {
        remotes.push(remote);
    }

    // Only the remotes namespace, through the per-build scan cache — the walker's ref
    // mapping consumes the same expensive iteration.
    let remote_refs = overlay_repo.raw_refs_prefixed("refs/remotes/")?;
    let mut map = HashMap::new();
    // Name-deduction against the symbolic remotes.
    for remote in &remotes {
        let prefix = format!("refs/remotes/{remote}/");
        for (_id, name) in remote_refs.iter() {
            if let Some(short) = name.as_bstr().strip_prefix(prefix.as_bytes()) {
                let local = format!("refs/heads/{}", String::from_utf8_lossy(short));
                if let Ok(local_ref) = gix::refs::FullName::try_from(local) {
                    // The first (highest-priority) remote to claim a local branch wins.
                    map.entry(local_ref).or_insert_with(|| name.clone());
                }
            }
        }
    }
    // Git-configured tracking branches win over name-deduction.
    let mut config_bound: HashMap<gix::refs::FullName, gix::refs::FullName> = HashMap::new();
    for reference in repo.references()?.local_branches()?.filter_map(Result::ok) {
        let local = reference.name().to_owned();
        // The configured NAME counts even when the remote ref does not exist (yet) — the link is
        // name-only then, and passes that need the remote's commits skip unresolvable refs anyway.
        if let Some(Ok(rt)) =
            repo.branch_remote_tracking_ref_name(local.as_ref(), gix::remote::Direction::Fetch)
        {
            let rt = rt.into_owned();
            // The walk also traverses the remotes of git-configured tracking branches — their remote
            // names join the eligibility set. Read the configured name (never split the tracking
            // ref at a slash — remote names may contain slashes).
            if let Some(remote) = repo
                .branch_remote_name(local.shorten(), gix::remote::Direction::Fetch)
                .and_then(|name| name.as_symbol().map(ToOwned::to_owned))
                && !remotes.contains(&remote)
            {
                remotes.push(remote);
            }
            config_bound.insert(rt.clone(), local.clone());
            map.insert(local, rt);
        }
    }
    // A remote tracks ONE local: a git-CONFIGURED binding evicts a name-deduced pair for the same
    // remote (e.g. `base-of-A` configured to track `origin/A` after `A` was rebased away from it —
    // `A` no longer tracks anything).
    map.retain(|local, rt| {
        config_bound
            .get(rt)
            .is_none_or(|config_local| config_local == local)
    });
    Ok((map, remotes))
}

//! Shared reading of the stored ref layout: how a view ANCHORS ([`ViewAnchor`],
//! [`resolve_view_anchor`]) and the per-commit naming index the projection derives from
//! ([`index_layout`]). This is layout VOCABULARY, shared by the two readers of it — the
//! partition engine (`super::partition`), which is the sole producer of stacks, and the frame.
//! No stack is derived here.

use std::collections::{HashMap, HashSet};

use crate::ref_layout::in_gitbutler_namespace;
use crate::workspace::{GraphContext, StackCommit, StackSegment};
use crate::{CommitGraph, RefInfo};
use but_core::ref_metadata;

/// How the view anchors; only these three states are legal (a managed COMMIT implies a
/// managed ref).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ViewAnchor {
    /// The anchor is a managed workspace merge commit (with metadata).
    ManagedCommit,
    /// A managed workspace REF exists, but its commit is missing or unmanaged.
    ManagedRefOnly,
    /// A plain checkout: no managed ref at all.
    AdHoc,
}

impl ViewAnchor {
    pub(super) fn new(managed_commit: bool, has_managed_ref: bool) -> Self {
        if managed_commit {
            debug_assert!(has_managed_ref, "a managed commit implies a managed ref");
            Self::ManagedCommit
        } else if has_managed_ref {
            Self::ManagedRefOnly
        } else {
            Self::AdHoc
        }
    }
    /// The anchor IS a managed workspace merge commit.
    fn managed_commit(self) -> bool {
        matches!(self, Self::ManagedCommit)
    }
}

/// The commit the workspace view anchors on, plus the materialized parents.
///
/// Without materialized parents, a positioned gitbutler/* ref still pointing at
/// a managed workspace commit defines the workspace view (a branch checkout
/// inside a workspace); otherwise the entrypoint commit is the single run and
/// the ad-hoc discriminator does the rest.
pub(super) fn resolve_view_anchor(
    cg: &CommitGraph,
    layout: &crate::ref_layout::RefLayout,
    ws_meta: Option<&ref_metadata::Workspace>,
    entry_ref: Option<&gix::refs::FullName>,
) -> Option<(gix::ObjectId, Vec<gix::ObjectId>)> {
    layout
        .amended_ws_parents
        .clone()
        .map(|m| (m.commit, m.parents))
        .or_else(|| {
            // ... and like everywhere else, workspace semantics require the
            // metadata to exist; without it the entrypoint's view wins.
            ws_meta?;
            let on = layout.placements().find_map(|(name, on)| {
                (in_gitbutler_namespace(name.as_ref()) && cg.is_managed_ws_commit(on)).then_some(on)
            })?;
            Some((on, Vec::new()))
        })
        .or_else(|| {
            // The ws ref advanced past its merge: the managed commit sits buried on
            // the first-parent line below, and stacks anchor on the merge itself —
            // the commits above it belong to no lane (they await user relocation).
            ws_meta?;
            let pos = layout
                .placements()
                .find_map(|(name, on)| in_gitbutler_namespace(name.as_ref()).then_some(on))?;
            let on = cg.first_parent_managed_ws_commit_below(pos)?;
            Some((on, Vec::new()))
        })
        .or_else(|| {
            // A redone traversal can remember a stale entrypoint commit; the
            // entry REF's live position anchors the run instead.
            let entry = entry_ref
                .map(|r| r.as_ref())
                .or_else(|| cg.entrypoint_ref().map(|r| r.as_ref()))?;
            let on = layout.positioned_on(entry)?;
            Some((on, Vec::new()))
        })
        .or_else(|| Some((cg.entrypoint()?, Vec::new())))
}

/// The layout read three ways: positioned refs by name, run-naming refs by commit,
/// empty-segment names, and out-of-workspace name projections.
pub(super) struct LayoutIndexes<'a> {
    pub(super) naming_at: HashMap<gix::ObjectId, &'a gix::refs::FullName>,
    pub(super) names_empty: HashSet<&'a gix::refs::FullNameRef>,
}

/// Implementation refs never shape user-visible stacks: they neither name segments
/// nor ride on commits.
fn is_implementation_ref(cg: &CommitGraph, name: &gix::refs::FullNameRef) -> bool {
    in_gitbutler_namespace(name) && cg.entrypoint_ref().map(|r| r.as_ref()) != Some(name)
}

pub(super) fn index_layout<'a>(
    cg: &CommitGraph,
    layout: &'a crate::ref_layout::RefLayout,
    anchor: ViewAnchor,
) -> LayoutIndexes<'a> {
    let placed: HashMap<&gix::refs::FullNameRef, gix::ObjectId> = layout
        .placements()
        .map(|(name, on)| (name.as_ref(), on))
        .collect();
    let mut naming_at = HashMap::<gix::ObjectId, &gix::refs::FullName>::new();
    let mut names_empty = HashSet::<&gix::refs::FullNameRef>::new();
    for (name, facts) in layout
        .facts
        .iter()
        .filter(|(name, facts)| facts.names_segment && !is_implementation_ref(cg, name.as_ref()))
    {
        if facts.names_empty_segment {
            names_empty.insert(name.as_ref());
            continue;
        }
        let Some(pos_on) = placed.get(name.as_ref()).copied() else {
            continue;
        };
        if name.category() == Some(gix::reference::Category::RemoteBranch)
            && anchor.managed_commit()
        {
            // Remote positions never carve a WORKSPACE stack's runs — locals name
            // segments, remotes only pair up as sidebands. An ad-hoc stack without
            // local names does read them.
            continue;
        }
        // A ref positioned OUTSIDE the workspace is legitimately outside: its name does NOT
        // project onto an in-workspace ancestor (product ruling 2026-07-24 — don't misrepresent
        // an advanced ref as being in the workspace). It names only its own (outside) position;
        // the walk collects in-workspace commits only, so that position is never a cut point.
        naming_at.insert(pos_on, name);
    }
    LayoutIndexes {
        naming_at,
        names_empty,
    }
}

/// Ad-hoc entries keep only order-mates at or below the entry in a persisted
/// ad-hoc stack order, sorted to that order; without one, ambiguous same-commit
/// peers don't materialize — only the entry itself does.
pub(super) fn retain_ordered_after_entry(
    names: &mut Vec<&gix::refs::FullName>,
    ctx: &GraphContext,
    entry_ref: Option<&gix::refs::FullName>,
) {
    if let Some((order, ei)) = entry_ref.and_then(|entry| {
        ctx.ad_hoc_branch_stack_orders.iter().find_map(|order| {
            order
                .iter()
                .position(|n| n.as_ref() == entry.as_ref())
                .map(|ei| (order, ei))
        })
    }) {
        let pos_of = |n: &gix::refs::FullNameRef| order.iter().position(|m| m.as_ref() == n);
        names.retain(|n| pos_of(n.as_ref()).is_some_and(|i| i >= ei));
        names.sort_by_key(|n| pos_of(n.as_ref()).unwrap_or(usize::MAX));
    } else {
        names.retain(|n| Some(n.as_ref()) == entry_ref.map(|e| e.as_ref()));
    }
}

/// Refs consumed as structure stay off the commit: the segment's own naming ref,
/// refs naming empty segments, remote-category refs, and gitbutler/* refs.
pub(super) fn strip_structural_refs(
    commit: &mut StackCommit,
    own_name: Option<&gix::refs::FullNameRef>,
    names_empty: &HashSet<&gix::refs::FullNameRef>,
) {
    commit.refs.retain(|ri| {
        let name = ri.ref_name.as_ref();
        use gix::reference::Category;
        name.category() != Some(Category::RemoteBranch)
            && !in_gitbutler_namespace(name)
            && Some(name) != own_name
            && !names_empty.contains(name)
    });
}

pub(crate) fn named_segment(
    name: gix::refs::FullName,
    commit_id: Option<gix::ObjectId>,
    ctx: &GraphContext,
) -> StackSegment {
    StackSegment {
        remote_tracking_ref_name: ctx.remote_tracking.get(&name).cloned(),
        ref_info: Some(RefInfo {
            ref_name: name,
            commit_id,
            worktree: None,
        }),
        ..Default::default()
    }
}

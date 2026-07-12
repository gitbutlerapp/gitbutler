//! Ad-hoc/single-branch mode: decide which persisted GitButler-created branch ordering
//! applies to the checked-out ref. The order is a CHAIN — it threads through the same
//! plan machinery as workspace metadata stacks; nothing is rewritten after the fact.

use anyhow::Context as _;
use but_core::RefMetadata;
use gix::reference::Category;

use crate::walk::overlay::{OverlayMetadata, OverlayRepo};

/// The persisted ad-hoc branch ordering, split by consumer.
#[derive(Default)]
pub(crate) struct AdHocOrders {
    /// The complete persisted order — the projection walks it to keep one stack across
    /// commit-owning branch boundaries.
    pub full_orders: Vec<Vec<gix::refs::FullName>>,
    /// Contiguous same-tip runs (two or more refs on one commit) — the chains whose
    /// earlier members become empty segments above the run's bottom owner.
    pub same_tip_chains: Vec<Vec<gix::refs::FullName>>,
}

/// In ad-hoc/single-branch mode, return the persisted GitButler-created branch ordering
/// that applies to the checked-out ref. Empty when no ordering applies.
pub(crate) fn ad_hoc_branch_stack_upgrades<T: RefMetadata>(
    entrypoint_ref: Option<&gix::refs::FullName>,
    entrypoint_present: bool,
    repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
) -> anyhow::Result<AdHocOrders> {
    let Some(entrypoint_ref) = entrypoint_ref.cloned() else {
        return Ok(AdHocOrders::default());
    };
    if entrypoint_ref.category() != Some(Category::LocalBranch) {
        return Ok(AdHocOrders::default());
    }
    if !entrypoint_present {
        return Ok(AdHocOrders::default());
    }
    let Some(branch_order) = meta.branch_stack_order(entrypoint_ref.as_ref())? else {
        return Ok(AdHocOrders::default());
    };
    let mut existing_ordered_refs = Vec::new();
    for branch in branch_order {
        if branch.category() != Some(Category::LocalBranch) {
            continue;
        }
        let Some(mut reference) = repo.try_find_reference(branch.as_ref()).with_context(|| {
            format!(
                "failed to find ordered ad-hoc branch '{}'",
                branch.shorten()
            )
        })?
        else {
            continue;
        };
        let commit_id = reference
            .peel_to_id()
            .with_context(|| {
                format!(
                    "failed to peel ordered ad-hoc branch '{}'",
                    branch.shorten()
                )
            })?
            .detach();
        existing_ordered_refs.push((branch, commit_id));
    }
    if existing_ordered_refs.len() < 2 {
        return Ok(AdHocOrders::default());
    }

    // A persisted stack can cross commit-owning branch boundaries: an empty branch
    // ordered above a commit-owning one must stay an empty segment above it, not claim
    // its commit. So the chains are the CONTIGUOUS same-tip runs, each rebuilt
    // independently — while the projection gets the complete order to thread the runs
    // into one stack.
    let full_order: Vec<_> = existing_ordered_refs
        .iter()
        .map(|(branch, _)| branch.clone())
        .collect();
    let mut same_tip_groups: Vec<(gix::ObjectId, Vec<gix::refs::FullName>)> = Vec::new();
    for (branch, commit_id) in existing_ordered_refs {
        if let Some((group_commit_id, branches)) = same_tip_groups.last_mut()
            && *group_commit_id == commit_id
        {
            branches.push(branch);
        } else {
            same_tip_groups.push((commit_id, vec![branch]));
        }
    }
    Ok(AdHocOrders {
        full_orders: vec![full_order],
        same_tip_chains: same_tip_groups
            .into_iter()
            .filter(|(_, refs)| refs.len() >= 2)
            .map(|(_, refs)| refs)
            .collect(),
    })
}

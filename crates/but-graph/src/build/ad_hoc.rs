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
    /// The same order as ONE chain for the plan: threaded like a metadata stack list, so
    /// every member gets a position — an empty member above a commit-owning one, or one
    /// resting on a commit another ref names (the target tip, say), splices in as an empty
    /// segment instead of riding that commit.
    pub chains: Vec<Vec<gix::refs::FullName>>,
}

/// In ad-hoc/single-branch mode, return the persisted GitButler-created branch ordering
/// that applies to the checked-out ref. Empty when no ordering applies.
pub(crate) fn ad_hoc_branch_orders<T: RefMetadata>(
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

    let full_order: Vec<_> = existing_ordered_refs
        .into_iter()
        .map(|(branch, _)| branch)
        .collect();
    Ok(AdHocOrders {
        full_orders: vec![full_order.clone()],
        chains: vec![full_order],
    })
}

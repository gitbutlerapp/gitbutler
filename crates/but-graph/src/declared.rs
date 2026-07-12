//! Metadata-only DECLARATION lookups, kept apart from the graph that derives its own answers.
//!
//! Two sources can say which stack a branch belongs to: the workspace metadata DECLARES it, and
//! the graph DERIVES it. This module is the declared side alone — no graph, no re-derivation —
//! so a caller that means "what does the recording say" cannot accidentally get the graph's
//! answer instead.
//!
//! It also holds the two places where the two sources must agree, as debug assertions: every
//! declared in-workspace branch is visible to the graph, and a declared branch's resting commit
//! is its ref's own target.

use but_core::ref_metadata::WorkspaceStack;

/// The declared in-workspace stack listing `name` among its (non-archived) branches.
pub fn declared_stack_of<'a>(
    ws: &'a crate::Workspace,
    name: &gix::refs::FullNameRef,
) -> Option<&'a WorkspaceStack> {
    ws.metadata.as_ref()?.stacks.iter().find(|stack| {
        stack.is_in_workspace()
            && stack
                .branches
                .iter()
                .any(|branch| !branch.archived && branch.ref_name.as_ref() == name)
    })
}

/// INVARIANT: every declared in-workspace branch is visible to the graph. Measured across
/// ~1,600 probes before being asserted, with no counterexample.
///
/// `graph_stack_id` is the id of the stack the graph homed `name` in — `Some(None)` when the
/// graph found it but the stack is anonymous, and `None` only when the graph did not find
/// `name` at all. That last case is the one that must not happen.
#[cfg(debug_assertions)]
pub fn debug_assert_declared_branch_is_visible(
    ws: &crate::Workspace,
    name: &gix::refs::FullNameRef,
    graph_stack_id: Option<Option<but_core::ref_metadata::StackId>>,
) {
    debug_assert!(
        !(graph_stack_id.is_none() && declared_stack_of(ws, name).is_some()),
        "declared in-workspace branch {name} is invisible to the graph"
    );
}

/// INVARIANT: a declared branch's resting commit IS its ref's own target. Measured across 364
/// probes before being asserted, with no counterexample. Undeclared branches, and either side
/// being absent, are outside it.
#[cfg(debug_assertions)]
pub fn debug_assert_resting_matches_ref_target(
    ws: &crate::Workspace,
    name: &gix::refs::FullNameRef,
    graph_resting: Option<gix::ObjectId>,
) {
    if declared_stack_of(ws, name).is_none() {
        return;
    }
    let (Some(ref_target), Some(resting)) = (ws.commit_graph().commit_by_ref(name), graph_resting)
    else {
        return;
    };
    debug_assert!(
        ref_target == resting,
        "resting of {name} diverged from its ref target \
         (ref {ref_target}, graph {resting})"
    );
}

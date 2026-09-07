use but_core::ref_metadata::StackId;
use but_meta::virtual_branches_legacy_types::{Stack, StackBranch};
use but_testsupport::StackState;

mod change_id;
mod cherry_pick;
mod conflictable_restriction;
mod disconnect;
mod edge;
mod editor_creation;
mod graph_workspace;
mod insert;
mod insert_segment;
mod materialize;
mod merge_commit_changes;
mod multiple_operations;
mod order_commit_selectors_by_parentage;
mod rebase_identities;
mod replace;
mod sha256;
mod signing_preferences;
mod workspace_commit_behaviour;

// Add parameters as needed.
pub fn add_stack_with_segments(
    meta: &mut but_db::DbHandle,
    stack_id: usize,
    stack_name: &str,
    state: StackState,
    segments: &[&str],
) -> StackId {
    let mut stack = Stack::new_with_just_heads(
        segments
            .iter()
            .rev()
            .map(|stack_name| {
                StackBranch::new_with_zero_head((*stack_name).into(), None, None, false)
            })
            .chain(std::iter::once(StackBranch::new_with_zero_head(
                stack_name.into(),
                None,
                None,
                false,
            )))
            .collect(),
        but_testsupport::legacy_metadata(meta)
            .unwrap()
            .branches
            .len(),
        match state {
            StackState::InWorkspace => true,
            StackState::Inactive => false,
        },
    );
    stack.order = stack_id;
    let stack_id = StackId::from_number_for_testing(stack_id as u128);
    stack.id = stack_id;
    but_testsupport::edit_legacy_metadata(meta, |data| data.branches.insert(stack_id, stack))
        .unwrap();
    stack_id
}

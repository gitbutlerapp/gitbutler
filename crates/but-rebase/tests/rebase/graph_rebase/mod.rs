use but_core::ref_metadata::StackId;
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
    but_testsupport::add_stack_with_segments(meta, stack_id as u128, stack_name, state, segments)
}

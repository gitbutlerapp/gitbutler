use but_core::ref_metadata::StackId;
use but_meta::{
    VirtualBranchesTomlMetadata,
    virtual_branches_legacy_types::{Stack, StackBranch, Target},
};
use but_testsupport::StackState;

mod cherry_pick;
mod conflictable_restriction;
mod editor_creation;
mod insert;
mod materialize;
mod multiple_operations;
mod parents_must_be_references_restriction;
mod rebase_identities;
mod replace;
mod signing_preferences;
mod workspace_commit_behaviour;

// Add parameters as needed.
pub fn add_stack_with_segments(
    meta: &mut VirtualBranchesTomlMetadata,
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
        meta.data().branches.len(),
        match state {
            StackState::InWorkspace => true,
            StackState::Inactive => false,
        },
    );
    stack.order = stack_id;
    let stack_id = StackId::from_number_for_testing(stack_id as u128);
    stack.id = stack_id;
    meta.data_mut().branches.insert(stack_id, stack);
    // Assure we have a target set.
    if meta.data_mut().default_target.is_none() {
        meta.data_mut().default_target = Some(Target {
            branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
            remote_url: "does not matter".to_string(),
            sha: gix::hash::Kind::Sha1.null(),
            push_remote_name: None,
        });
    }
    stack_id
}

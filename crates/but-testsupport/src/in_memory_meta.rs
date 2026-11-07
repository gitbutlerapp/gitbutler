use but_core::ref_metadata::{
    StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
};

/// Whether a fixture stack belongs to the workspace.
pub enum StackState {
    /// The stack is applied and included in the workspace.
    InWorkspace,
    /// The stack is remembered outside the workspace.
    Inactive,
}

/// Add a deterministic stack to the fixture database, with branches ordered from tip to base.
pub fn add_stack_with_segments(
    db: &mut but_db::DbHandle,
    stack_id: u128,
    stack_name: &str,
    state: StackState,
    segments: &[&str],
) -> StackId {
    let name = but_core::WORKSPACE_REF_NAME
        .try_into()
        .expect("valid workspace ref");
    let mut workspace = db.meta().workspace(name).expect("read fixture workspace");
    let id = StackId::from_number_for_testing(stack_id);
    workspace.stacks.retain(|stack| stack.id != id);
    workspace.stacks.push(WorkspaceStack {
        id,
        branches: std::iter::once(stack_name)
            .chain(segments.iter().copied())
            .map(|name| WorkspaceStackBranch {
                ref_name: format!("refs/heads/{name}")
                    .try_into()
                    .expect("valid fixture branch"),
                archived: false,
            })
            .collect(),
        workspacecommit_relation: match state {
            StackState::InWorkspace => WorkspaceCommitRelation::Merged,
            StackState::Inactive => WorkspaceCommitRelation::Outside,
        },
    });
    workspace.stacks.sort_by_key(|stack| stack.id.to_string());
    db.meta_mut()
        .expect("write fixture metadata")
        .set_workspace(&workspace)
        .expect("store fixture workspace");
    id
}

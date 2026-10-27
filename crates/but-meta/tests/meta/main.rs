use but_core::ref_metadata::{
    StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
};

mod garbage_collect;
mod ref_metadata;

fn stack(
    id: u128,
    names: &[&str],
    workspacecommit_relation: WorkspaceCommitRelation,
) -> anyhow::Result<WorkspaceStack> {
    Ok(WorkspaceStack {
        id: StackId::from_number_for_testing(id),
        branches: names
            .iter()
            .map(|name| {
                Ok(WorkspaceStackBranch {
                    ref_name: format!("refs/heads/{name}").try_into()?,
                    archived: false,
                })
            })
            .collect::<anyhow::Result<_>>()?,
        workspacecommit_relation,
    })
}

#[cfg(feature = "legacy")]
mod legacy;
mod merge_base_with_target_branch;
mod remote_name;
mod resolved_target_commit_id;

fn project_meta(meta: &impl but_core::RefMetadata) -> but_core::ref_metadata::ProjectMeta {
    let workspace = meta
        .workspace(
            but_core::WORKSPACE_REF_NAME
                .try_into()
                .expect("valid workspace ref"),
        )
        .expect("workspace metadata is readable");
    let project_meta = workspace.project_meta();
    if project_meta != but_core::ref_metadata::ProjectMeta::default() || workspace.stacks.is_empty()
    {
        project_meta
    } else {
        crate::init::utils::default_project_meta()
    }
}

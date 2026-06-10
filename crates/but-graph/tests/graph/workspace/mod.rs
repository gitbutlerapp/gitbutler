#[cfg(feature = "legacy")]
mod legacy;
mod merge_base_with_target_branch;
mod remote_name;
mod resolved_target_commit_id;

fn project_meta(meta: &impl but_core::RefMetadata) -> but_core::ref_metadata::ProjectMeta {
    meta.workspace(
        but_core::WORKSPACE_REF_NAME
            .try_into()
            .expect("valid workspace ref"),
    )
    .map(|workspace| workspace.project_meta())
    .unwrap_or_default()
}

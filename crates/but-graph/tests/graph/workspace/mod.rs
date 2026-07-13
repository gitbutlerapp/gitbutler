#[cfg(feature = "legacy")]
mod legacy;
mod merge_base_with_target_branch;
mod remote_name;
mod resolved_target_commit_id;

fn target_meta() -> but_core::ref_metadata::ProjectMeta {
    crate::init::utils::default_project_meta()
}

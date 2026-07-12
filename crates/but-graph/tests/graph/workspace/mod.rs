mod declared_dag;
mod decoration_is_the_displays_job;
#[cfg(feature = "legacy")]
mod merge_base_with_target_branch;
mod missing_target_ref;
mod remote_name;
mod resolved_target_commit_id;

fn target_meta() -> but_core::ref_metadata::ProjectMeta {
    crate::walk::utils::default_project_meta()
}

/// For the tests about a workspace that has no target configured at all.
fn no_target_meta() -> but_core::ref_metadata::ProjectMeta {
    crate::walk::utils::no_target_project_meta()
}

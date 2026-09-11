mod merge_base_with_target_branch;
mod remote_name;
mod target_commit;

fn target_meta(repo: &gix::Repository) -> but_core::ref_metadata::ProjectMeta {
    crate::init::utils::default_project_meta(repo)
}

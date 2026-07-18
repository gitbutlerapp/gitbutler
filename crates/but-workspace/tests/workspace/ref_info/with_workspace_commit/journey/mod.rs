mod exhaustive_with_squash_merges;
mod integrate_with_merges;
mod integrate_with_rebase;

mod utils {
    use crate::ref_info::utils::standard_options;
    pub fn standard_options_with_extra_target(
        repo: &gix::Repository,
        revspec: &str,
    ) -> but_workspace::ref_info::Options<'static> {
        but_workspace::ref_info::Options {
            project_meta: but_core::ref_metadata::ProjectMeta {
                target_commit_id: repo.rev_parse_single(revspec).unwrap().detach().into(),
                ..Default::default()
            },
            ..standard_options()
        }
    }
}

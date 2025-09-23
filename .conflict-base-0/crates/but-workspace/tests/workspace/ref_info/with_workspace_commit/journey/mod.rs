mod exhaustive_with_squash_merges;
mod integrate_with_merges;
mod integrate_with_rebase;

mod utils {
    use crate::ref_info::utils::standard_options;
    pub fn standard_options_with_extra_target(
        repo: &gix::Repository,
        revspec: &str,
    ) -> but_workspace::ref_info::Options {
        but_workspace::ref_info::Options {
            traversal: but_graph::init::Options {
                extra_target_commit_id: repo.rev_parse_single(revspec).unwrap().detach().into(),
                ..Default::default()
            },
            ..standard_options()
        }
    }
}

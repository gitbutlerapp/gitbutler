use but_ctx::Context;
use but_oxidize::OidExt;
use but_workspace::{legacy::StacksFilter, ui::StackDetails};
use gitbutler_stack::StackId;
use gix::prelude::ObjectIdExt;

pub const VAR_NO_CLEANUP: &str = "GITBUTLER_TESTS_NO_CLEANUP";

mod test_project;
pub use test_project::TestProject;

mod suite;
pub use suite::*;

pub mod testing_repository;

pub mod paths {
    use tempfile::TempDir;

    use super::temp_dir;

    pub fn data_dir() -> TempDir {
        temp_dir()
    }
}

pub mod virtual_branches {
    use but_ctx::Context;
    use but_oxidize::OidExt;
    use gitbutler_stack::{Target, VirtualBranchesHandle};

    use super::empty_bare_repository;

    pub fn set_test_target(ctx: &Context) -> anyhow::Result<()> {
        let mut vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
        let (remote_repo, _tmp) = empty_bare_repository();
        #[expect(
            deprecated,
            reason = "legacy fixture coverage for transport/setup compatibility"
        )]
        let git2_repo = &*ctx.git2_repo.get()?;
        let mut remote = git2_repo
            .remote("origin", remote_repo.path().to_str().unwrap())
            .expect("failed to add remote");
        remote.push(&["refs/heads/master:refs/heads/master"], None)?;

        vb_state
            .set_default_target(Target {
                branch: "refs/remotes/origin/master".parse().unwrap(),
                remote_url: remote_repo.path().to_str().unwrap().parse().unwrap(),
                sha: remote_repo.head().unwrap().target().unwrap().to_gix(),
                push_remote_name: None,
            })
            .expect("failed to write target");

        gitbutler_branch_actions::update_workspace_commit_with_vb_state(&vb_state, ctx, false)
            .expect("failed to update workspace");

        Ok(())
    }
}

pub fn init_opts() -> git2::RepositoryInitOptions {
    let mut opts = git2::RepositoryInitOptions::new();
    opts.initial_head("master");
    opts
}

pub fn init_opts_bare() -> git2::RepositoryInitOptions {
    let mut opts = init_opts();
    opts.bare(true);
    opts
}

pub fn visualize_git2_tree(tree_id: git2::Oid, repo: &git2::Repository) -> termtree::Tree<String> {
    let repo = gix::open_opts(repo.path(), gix::open::Options::isolated()).unwrap();
    crate::visualize_tree(tree_id.to_gix().attach(&repo))
}

pub fn stack_details(ctx: &Context) -> Vec<(StackId, StackDetails)> {
    let repo = ctx.clone_repo_for_merging_non_persisting().unwrap();
    let stacks = {
        let meta = ctx.legacy_meta().unwrap();
        let mut cache = ctx.cache.get_cache_mut().unwrap();
        but_workspace::legacy::stacks_v3(&repo, &meta, StacksFilter::default(), None, &mut cache)
    }
    .unwrap();
    let mut details = vec![];
    for stack in stacks {
        let stack_id = stack
            .id
            .expect("BUG(opt-stack-id): test code shouldn't trigger this");
        details.push((
            stack_id,
            {
                let meta = ctx.legacy_meta().unwrap();
                let mut cache = ctx.cache.get_cache_mut().unwrap();
                but_workspace::legacy::stack_details_v3(stack_id.into(), &repo, &meta, &mut cache)
            }
            .unwrap(),
        ));
    }
    details
}

pub mod secrets;

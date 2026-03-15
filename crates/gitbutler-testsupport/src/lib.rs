use but_ctx::Context;
use but_oxidize::OidExt;
use but_workspace::{legacy::StacksFilter, ui::StackDetails};
use gitbutler_stack::StackId;
use gix::bstr::BStr;

pub const VAR_NO_CLEANUP: &str = "GITBUTLER_TESTS_NO_CLEANUP";

/// Direct access to lower-level utilities for cases where this is enough.
pub use gix_testtools;

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

    use crate::empty_bare_repository;

    pub fn set_test_target(ctx: &Context) -> anyhow::Result<()> {
        let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
        let (remote_repo, _tmp) = empty_bare_repository();
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

        gitbutler_branch_actions::update_workspace_commit(&vb_state, ctx, false)
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

/// Display a Git tree in the style of the `tree` CLI program, but include blob contents and usful Git metadata.
pub fn visualize_gix_tree(tree_id: gix::Id<'_>) -> termtree::Tree<String> {
    fn visualize_tree(
        id: gix::Id<'_>,
        name_and_mode: Option<(&BStr, gix::object::tree::EntryMode)>,
    ) -> anyhow::Result<termtree::Tree<String>> {
        fn short_id(id: &gix::hash::oid) -> String {
            id.to_hex_with_len(7).to_string()
        }
        let repo = id.repo;
        let entry_name =
            |id: &gix::hash::oid, name: Option<(&BStr, gix::object::tree::EntryMode)>| -> String {
                match name {
                    None => short_id(id),
                    Some((name, mode)) => {
                        format!(
                            "{name}:{mode}{} {}",
                            short_id(id),
                            match repo.find_blob(id) {
                                Ok(blob) => format!("{:?}", blob.data.as_bstr()),
                                Err(_) => "".into(),
                            },
                            mode = if mode.is_tree() {
                                "".into()
                            } else {
                                format!("{:o}:", mode.value())
                            }
                        )
                    }
                }
            };

        let mut tree = termtree::Tree::new(entry_name(&id, name_and_mode));
        for entry in repo.find_tree(id)?.iter() {
            let entry = entry?;
            if entry.mode().is_tree() {
                tree.push(visualize_tree(
                    entry.id(),
                    Some((entry.filename(), entry.mode())),
                )?);
            } else {
                tree.push(entry_name(
                    entry.oid(),
                    Some((entry.filename(), entry.mode())),
                ));
            }
        }
        Ok(tree)
    }
    visualize_tree(tree_id.object().unwrap().peel_to_tree().unwrap().id(), None).unwrap()
}

/// Visualize a git2 tree, otherwise just like [`visualize_gix_tree()`].
pub fn visualize_git2_tree(tree_id: git2::Oid, repo: &git2::Repository) -> termtree::Tree<String> {
    let repo = gix::open_opts(repo.path(), gix::open::Options::isolated()).unwrap();
    visualize_gix_tree(tree_id.to_gix().attach(&repo))
}

pub fn stack_details(ctx: &Context) -> Vec<(StackId, StackDetails)> {
    let repo = ctx.clone_repo_for_merging_non_persisting().unwrap();
    let stacks = {
        let meta = ctx.legacy_meta().unwrap();
        but_workspace::legacy::stacks_v3(&repo, &meta, StacksFilter::default(), None)
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
                but_workspace::legacy::stack_details_v3(stack_id.into(), &repo, &meta)
            }
            .unwrap(),
        ));
    }
    details
}

use gix::{bstr::ByteSlice, prelude::ObjectIdExt};

/// A secrets store to prevent secrets to be written into the systems own store.
///
/// Note that this can't be used if secrets themselves are under test as it' doesn't act
/// like a real store, i.e. stored secrets can't be retrieved anymore.
pub mod secrets;

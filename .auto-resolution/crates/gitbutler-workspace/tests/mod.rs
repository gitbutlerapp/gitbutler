#[allow(deprecated)]
mod branch_trees;

mod checkout_branch_trees {
    use std::fs;

    use but_settings::AppSettings;
    use gitbutler_branch::BranchCreateRequest;
    use gitbutler_branch_actions as branch_actions;
    use gitbutler_command_context::CommandContext;
    use gitbutler_project::AUTO_TRACK_LIMIT_BYTES;
    use gitbutler_repo::RepositoryExt as _;
    use gitbutler_testsupport::{paths, testing_repository::assert_tree_matches, TestProject};
    #[allow(deprecated)]
    use gitbutler_workspace::checkout_branch_trees;

    #[test]
    fn checkout_with_two_branches() {
        let test_project = &TestProject::default();

        let data_dir = paths::data_dir();
        let projects = gitbutler_project::Controller::from_path(data_dir.path());

        let project = projects
            .add(test_project.path(), None, None)
            .expect("failed to add project");

        let ctx = CommandContext::open(&project, AppSettings::default()).unwrap();

        branch_actions::set_base_branch(&ctx, &"refs/remotes/origin/master".parse().unwrap())
            .unwrap();

        let stach_entry_1 =
            branch_actions::create_virtual_branch(&ctx, &BranchCreateRequest::default()).unwrap();

        fs::write(test_project.path().join("foo.txt"), "content").unwrap();

        branch_actions::create_commit(&ctx, stach_entry_1.id, "commit one", None).unwrap();

        let stack_entry_2 =
            branch_actions::create_virtual_branch(&ctx, &BranchCreateRequest::default()).unwrap();

        fs::write(test_project.path().join("bar.txt"), "content").unwrap();

        branch_actions::create_commit(&ctx, stack_entry_2.id, "commit two", None).unwrap();

        let tree = test_project
            .local_repo
            .create_wd_tree(AUTO_TRACK_LIMIT_BYTES)
            .unwrap();

        // Assert original state
        assert_tree_matches(
            &test_project.local_repo,
            &tree,
            &[("foo.txt", b"content"), ("bar.txt", b"content")],
        );
        assert_eq!(tree.len(), 2);

        // Checkout an empty tree
        {
            let tree_oid = test_project
                .local_repo
                .treebuilder(None)
                .unwrap()
                .write()
                .unwrap();
            let tree = test_project.local_repo.find_tree(tree_oid).unwrap();
            test_project
                .local_repo
                .checkout_tree_builder(&tree)
                .force()
                .remove_untracked()
                .checkout()
                .unwrap();
        }

        // Assert tree is indeed empty
        {
            let tree: git2::Tree = test_project
                .local_repo
                .create_wd_tree(AUTO_TRACK_LIMIT_BYTES)
                .unwrap();

            // Tree should be empty
            assert_eq!(
                tree.len(),
                0,
                "Should be empty after checking out an empty tree"
            );
        }

        let mut guard = project.exclusive_worktree_access();

        #[allow(deprecated)]
        checkout_branch_trees(&ctx, guard.write_permission()).unwrap();

        let tree = test_project
            .local_repo
            .create_wd_tree(AUTO_TRACK_LIMIT_BYTES)
            .unwrap();

        // Should be back to original state
        assert_tree_matches(
            &test_project.local_repo,
            &tree,
            &[("foo.txt", b"content"), ("bar.txt", b"content")],
        );
        assert_eq!(tree.len(), 2, "Should match original state");
    }
}

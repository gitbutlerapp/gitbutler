mod branch_trees;

mod checkout_branch_trees {
    use std::fs;

    use gitbutler_branch::BranchCreateRequest;
    use gitbutler_branch_actions as branch_actions;
    use gitbutler_command_context::CommandContext;
    use gitbutler_repo::RepositoryExt as _;
    use gitbutler_testsupport::{paths, testing_repository::assert_tree_matches, TestProject};
    use gitbutler_workspace::checkout_branch_trees;

    #[test]
    fn checkout_with_two_branches() {
        let test_project = &TestProject::default();

        let data_dir = paths::data_dir();
        let projects = gitbutler_project::Controller::from_path(data_dir.path());

        let project = projects
            .add(test_project.path())
            .expect("failed to add project");

        branch_actions::set_base_branch(&project, &"refs/remotes/origin/master".parse().unwrap())
            .unwrap();

        let branch_1 =
            branch_actions::create_virtual_branch(&project, &BranchCreateRequest::default())
                .unwrap();

        fs::write(test_project.path().join("foo.txt"), "content").unwrap();

        branch_actions::create_commit(&project, branch_1, "commit one", None, false).unwrap();

        let branch_2 =
            branch_actions::create_virtual_branch(&project, &BranchCreateRequest::default())
                .unwrap();

        fs::write(test_project.path().join("bar.txt"), "content").unwrap();

        branch_actions::create_commit(&project, branch_2, "commit two", None, false).unwrap();

        let tree = test_project.local_repository.create_wd_tree().unwrap();

        // Assert original state
        assert_tree_matches(
            &test_project.local_repository,
            &tree,
            &[("foo.txt", b"content"), ("bar.txt", b"content")],
        );
        assert_eq!(tree.len(), 2);

        // Checkout an empty tree
        {
            let tree_oid = test_project
                .local_repository
                .treebuilder(None)
                .unwrap()
                .write()
                .unwrap();
            let tree = test_project.local_repository.find_tree(tree_oid).unwrap();
            test_project
                .local_repository
                .checkout_tree_builder(&tree)
                .force()
                .remove_untracked()
                .checkout()
                .unwrap();
        }

        // Assert tree is indeed empty
        {
            let tree: git2::Tree = test_project.local_repository.create_wd_tree().unwrap();

            // Tree should be empty
            assert_eq!(
                tree.len(),
                0,
                "Should be empty after checking out an empty tree"
            );
        }

        let ctx = CommandContext::open(&project).unwrap();
        let mut guard = project.exclusive_worktree_access();

        checkout_branch_trees(&ctx, guard.write_permission()).unwrap();

        let tree = test_project.local_repository.create_wd_tree().unwrap();

        // Should be back to original state
        assert_tree_matches(
            &test_project.local_repository,
            &tree,
            &[("foo.txt", b"content"), ("bar.txt", b"content")],
        );
        assert_eq!(tree.len(), 2, "Should match original state");
    }
}

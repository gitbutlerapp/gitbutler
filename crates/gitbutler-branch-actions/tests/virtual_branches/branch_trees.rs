use gitbutler_stack::Stack;

/// Makes a Branch struct with a bunch of default values.
///
/// This assumes that the only relevant properties for your test are the head
/// and tree Oids.
fn make_branch(head: git2::Oid, tree: git2::Oid) -> Stack {
    #[allow(deprecated)] // this is a test
    let mut branch = Stack::new(
        "branchy branch".into(),
        None,
        None,
        None,
        tree,
        head,
        0,
        None,
        true,
    );
    branch.created_timestamp_ms = 69420;
    branch.updated_timestamp_ms = 69420;
    branch.notes = "bla bla bla".into();
    branch
}

#[cfg(test)]
mod compute_updated_branch_head {
    use super::*;
    use gitbutler_branch_actions::branch_trees::{compute_updated_branch_head, BranchHeadAndTree};
    use gitbutler_cherry_pick::RepositoryExt as _;
    use gitbutler_commit::commit_ext::CommitExt;
    use gitbutler_testsupport::testing_repository::{
        assert_commit_tree_matches, assert_tree_matches, TestingRepository,
    };

    /// When the head ID is the same as the branch ID, we should return the same Oids.
    #[test]
    fn head_id_is_the_same() {
        let test_repository = TestingRepository::open();

        let base_commit = test_repository.commit_tree(None, &[("foo.txt", "foo")]);
        let head = test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "bar")]);
        let tree = test_repository.commit_tree(Some(&head), &[("foo.txt", "baz")]);

        let branch = make_branch(head.id(), tree.tree_id());

        let BranchHeadAndTree { head, tree } =
            compute_updated_branch_head(&test_repository.repository, &branch, head.id(), true)
                .unwrap();

        assert_eq!(head, branch.head());
        assert_eq!(tree, branch.tree);
    }

    /// When the head ID is different from the branch ID, we should rebase the
    /// tree on top of it.
    ///
    /// This test is set up such that the tree won't be conflicted.
    ///
    /// We expect to see the head commit match what we passed in as the new
    /// head, and the tree should rebased on top of that new head.
    #[test]
    fn head_id_is_different() {
        let test_repository = TestingRepository::open();

        let base_commit = test_repository.commit_tree(None, &[("foo.txt", "foo")]);
        let head = test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "bar")]);
        let tree =
            test_repository.commit_tree(Some(&head), &[("foo.txt", "bar"), ("bar.txt", "baz")]);

        let branch = make_branch(head.id(), tree.tree_id());

        let new_head = test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "new")]);

        let BranchHeadAndTree { head, tree } =
            compute_updated_branch_head(&test_repository.repository, &branch, new_head.id(), true)
                .unwrap();

        assert_eq!(head, new_head.id());
        assert_tree_matches(
            &test_repository.repository,
            &test_repository.repository.find_tree(tree).unwrap(),
            &[("foo.txt", b"new"), ("bar.txt", b"baz")],
        );
    }

    /// When the head ID is different from the branch ID and the new head will
    /// conflict with the tree.
    ///
    /// In this case we should expect to receive a new head commit that is the
    /// conflicted result of the rebase, and the tree will the the
    /// auto-resolved tree of that new head commit.
    #[test]
    fn tree_conflicts() {
        let test_repository = TestingRepository::open();

        let base_commit = test_repository.commit_tree(None, &[("foo.txt", "foo")]);
        let head = test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "bar")]);
        let tree = test_repository.commit_tree(Some(&head), &[("foo.txt", "baz")]);

        let branch = make_branch(head.id(), tree.tree_id());

        let new_head = test_repository.commit_tree(Some(&base_commit), &[("foo.txt", "new")]);

        let BranchHeadAndTree { head, tree } =
            compute_updated_branch_head(&test_repository.repository, &branch, new_head.id(), true)
                .unwrap();

        let new_new_head = test_repository.repository.find_commit(head).unwrap();
        assert!(new_new_head.is_conflicted());
        assert_eq!(new_new_head.parent(0).unwrap().id(), new_head.id());

        assert_commit_tree_matches(
            &test_repository.repository,
            &new_new_head,
            &[
                (".auto-resolution/foo.txt", b"new"), // Auto-resolves to new_head
                (".conflict-base-0/foo.txt", b"bar"), // head is the base
                (".conflict-side-0/foo.txt", b"new"), // new_head is the ours side
                (".conflict-side-1/foo.txt", b"baz"), // tree is the theris side
            ],
        );

        // Tree should be the auto-resolved tree.
        assert_eq!(
            tree,
            test_repository
                .repository
                .find_real_tree(&new_new_head, Default::default())
                .unwrap()
                .id()
        );
    }
}

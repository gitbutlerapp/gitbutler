mod cherry_rebase_group {
    use gitbutler_commit::commit_ext::CommitExt;
    use gitbutler_repo::logging::RepositoryExt as _;
    use gitbutler_testsupport::testing_repository::{
        assert_commit_tree_matches, TestingRepository,
    };

    use gitbutler_repo::{logging::LogUntil, rebase::cherry_rebase_group};

    #[test]
    fn unconflicting_rebase() {
        let test_repository = TestingRepository::open();

        // Make some commits
        let a = test_repository.commit_tree(None, &[("foo.txt", "a"), ("bar.txt", "a")]);
        let b = test_repository.commit_tree(Some(&a), &[("foo.txt", "b"), ("bar.txt", "a")]);
        let c = test_repository.commit_tree(Some(&b), &[("foo.txt", "c"), ("bar.txt", "a")]);
        let d = test_repository.commit_tree(Some(&a), &[("foo.txt", "a"), ("bar.txt", "x")]);

        let result = cherry_rebase_group(
            &test_repository.repository,
            d.id(),
            &[c.id(), b.id()],
            false,
        )
        .unwrap();

        let commit: git2::Commit = test_repository.repository.find_commit(result).unwrap();

        let commits: Vec<git2::Commit> = test_repository
            .repository
            .log(commit.id(), LogUntil::End, false)
            .unwrap();

        assert!(commits.into_iter().all(|commit| !commit.is_conflicted()));

        assert_commit_tree_matches(
            &test_repository.repository,
            &commit,
            &[("foo.txt", b"c"), ("bar.txt", b"x")],
        );
    }

    #[test]
    fn single_commit_ends_up_conflicted() {
        let test_repository = TestingRepository::open();

        let a = test_repository.commit_tree(None, &[("foo.txt", "a")]);
        let b = test_repository.commit_tree(Some(&a), &[("foo.txt", "b")]);
        let c = test_repository.commit_tree(Some(&a), &[("foo.txt", "c")]);

        // Rebase C on top of B
        let result =
            cherry_rebase_group(&test_repository.repository, b.id(), &[c.id()], false).unwrap();

        let commit: git2::Commit = test_repository.repository.find_commit(result).unwrap();

        assert!(commit.is_conflicted());

        assert_commit_tree_matches(
            &test_repository.repository,
            &commit,
            &[
                (".auto-resolution/foo.txt", b"b"), // Prefer the commit we're rebasing onto
                (".conflict-base-0/foo.txt", b"a"), // The content of A
                (".conflict-side-0/foo.txt", b"b"), // "Our" side, content of B
                (".conflict-side-1/foo.txt", b"c"), // "Their" side, content of C
            ],
        );
    }

    #[test]
    fn rebase_single_conflicted_commit() {
        let test_repository = TestingRepository::open();

        let a = test_repository.commit_tree(None, &[("foo.txt", "a")]);
        let b = test_repository.commit_tree(Some(&a), &[("foo.txt", "b")]);
        let c = test_repository.commit_tree(Some(&a), &[("foo.txt", "c")]);
        let d = test_repository.commit_tree(Some(&a), &[("foo.txt", "d")]);

        // Rebase C on top of B => C'
        let result =
            cherry_rebase_group(&test_repository.repository, b.id(), &[c.id()], false).unwrap();

        // Rebase C' on top of D => C''
        let result =
            cherry_rebase_group(&test_repository.repository, d.id(), &[result], false).unwrap();

        let commit: git2::Commit = test_repository.repository.find_commit(result).unwrap();

        assert!(commit.is_conflicted());

        assert_commit_tree_matches(
            &test_repository.repository,
            &commit,
            &[
                (".auto-resolution/foo.txt", b"d"), // Prefer the commit we're rebasing onto
                (".conflict-base-0/foo.txt", b"a"), // The content of A
                (".conflict-side-0/foo.txt", b"d"), // "Our" side, content of B
                (".conflict-side-1/foo.txt", b"c"), // "Their" side, content of C
            ],
        );
    }

    /// Test what happens if you were to keep rebasing a branch on top of origin/master
    #[test]
    fn rebase_onto_series_multiple_times() {
        let test_repository = TestingRepository::open();

        let a = test_repository.commit_tree(None, &[("foo.txt", "a")]);
        let b = test_repository.commit_tree(Some(&a), &[("foo.txt", "b")]);
        let c = test_repository.commit_tree(Some(&b), &[("foo.txt", "c")]);
        let d = test_repository.commit_tree(Some(&a), &[("foo.txt", "d")]);

        // Rebase D on top of B => D'
        let result =
            cherry_rebase_group(&test_repository.repository, b.id(), &[d.id()], false).unwrap();

        let commit: git2::Commit = test_repository.repository.find_commit(result).unwrap();
        assert!(commit.is_conflicted());

        assert_commit_tree_matches(
            &test_repository.repository,
            &commit,
            &[
                (".auto-resolution/foo.txt", b"b"), // Prefer the commit we're rebasing onto
                (".conflict-base-0/foo.txt", b"a"), // The content of A
                (".conflict-side-0/foo.txt", b"b"), // "Our" side, content of B
                (".conflict-side-1/foo.txt", b"d"), // "Their" side, content of D
            ],
        );

        // Rebase D' on top of C => D''
        let result =
            cherry_rebase_group(&test_repository.repository, c.id(), &[result], false).unwrap();

        let commit: git2::Commit = test_repository.repository.find_commit(result).unwrap();
        assert!(commit.is_conflicted());

        assert_commit_tree_matches(
            &test_repository.repository,
            &commit,
            &[
                (".auto-resolution/foo.txt", b"c"), // Prefer the commit we're rebasing onto
                (".conflict-base-0/foo.txt", b"a"), // The content of A
                (".conflict-side-0/foo.txt", b"c"), // "Our" side, content of C
                (".conflict-side-1/foo.txt", b"d"), // "Their" side, content of D
            ],
        );
    }

    #[test]
    fn multiple_commit_ends_up_conflicted() {
        let test_repository = TestingRepository::open();

        let a = test_repository.commit_tree(None, &[("foo.txt", "a"), ("bar.txt", "a")]);
        let b = test_repository.commit_tree(Some(&a), &[("foo.txt", "b"), ("bar.txt", "a")]);
        let c = test_repository.commit_tree(Some(&b), &[("foo.txt", "b"), ("bar.txt", "b")]);
        let d = test_repository.commit_tree(Some(&a), &[("foo.txt", "c"), ("bar.txt", "c")]);

        // Rebase C on top of B
        let result = cherry_rebase_group(
            &test_repository.repository,
            d.id(),
            &[c.id(), b.id()],
            false,
        )
        .unwrap();

        let commit: git2::Commit = test_repository.repository.find_commit(result).unwrap();

        let commits: Vec<git2::Commit> = test_repository
            .repository
            .log(commit.id(), LogUntil::Commit(d.id()), false)
            .unwrap();

        assert!(commits.iter().all(|commit| commit.is_conflicted()));

        // Rebased version of B (B')
        assert_commit_tree_matches(
            &test_repository.repository,
            &commits[1],
            &[
                (".auto-resolution/foo.txt", b"c"),
                (".auto-resolution/bar.txt", b"c"),
                (".conflict-base-0/foo.txt", b"a"), // Commit A contents
                (".conflict-base-0/bar.txt", b"a"),
                (".conflict-side-0/foo.txt", b"c"), // (ours) Commit D contents
                (".conflict-side-0/bar.txt", b"c"),
                (".conflict-side-1/foo.txt", b"b"), // (theirs) Commit B contents
                (".conflict-side-1/bar.txt", b"a"),
            ],
        );

        // Rebased version of C
        assert_commit_tree_matches(
            &test_repository.repository,
            &commits[0],
            &[
                (".auto-resolution/foo.txt", b"c"),
                (".auto-resolution/bar.txt", b"c"),
                (".conflict-base-0/foo.txt", b"b"), // Commit B contents
                (".conflict-base-0/bar.txt", b"a"),
                (".conflict-side-0/foo.txt", b"c"), // (ours) Commit B' contents
                (".conflict-side-0/bar.txt", b"c"),
                (".conflict-side-1/foo.txt", b"b"), // (theirs) Commit C contents
                (".conflict-side-1/bar.txt", b"b"),
            ],
        );
    }
}

mod gitbutler_merge_commits {
    use gitbutler_commit::commit_ext::CommitExt as _;
    use gitbutler_repo::rebase::gitbutler_merge_commits;
    use gitbutler_testsupport::testing_repository::{
        assert_commit_tree_matches, TestingRepository,
    };

    #[test]
    fn unconflicting_merge() {
        let test_repository = TestingRepository::open();

        // Make some commits
        let a = test_repository.commit_tree(None, &[("foo.txt", "a")]);
        let b = test_repository.commit_tree(Some(&a), &[("foo.txt", "b")]);
        let c = test_repository.commit_tree(Some(&a), &[("foo.txt", "a"), ("bar.txt", "a")]);

        let result =
            gitbutler_merge_commits(&test_repository.repository, b, c, "master", "feature")
                .unwrap();

        assert!(!result.is_conflicted());

        assert_commit_tree_matches(
            &test_repository.repository,
            &result,
            &[("foo.txt", b"b"), ("bar.txt", b"a")],
        );
    }

    #[test]
    fn conflicting_merge() {
        let test_repository = TestingRepository::open();

        // Make some commits
        let a = test_repository.commit_tree(None, &[("foo.txt", "a")]);
        let b = test_repository.commit_tree(Some(&a), &[("foo.txt", "b")]);
        let c = test_repository.commit_tree(Some(&a), &[("foo.txt", "c")]);

        let result =
            gitbutler_merge_commits(&test_repository.repository, b, c, "master", "feature")
                .unwrap();

        assert!(result.is_conflicted());

        assert_commit_tree_matches(
            &test_repository.repository,
            &result,
            &[
                (".auto-resolution/foo.txt", b"c"), // Prefer the "Our" side, C
                (".conflict-base-0/foo.txt", b"a"), // The content of A
                (".conflict-side-0/foo.txt", b"c"), // "Our" side, content of B
                (".conflict-side-1/foo.txt", b"b"), // "Their" side, content of C
            ],
        );
    }

    #[test]
    fn merging_conflicted_commit_with_unconflicted_incoming() {
        let test_repository = TestingRepository::open();

        // Make some commits
        let a = test_repository.commit_tree(None, &[("foo.txt", "a")]);
        let b = test_repository.commit_tree(Some(&a), &[("foo.txt", "b")]);
        let c = test_repository.commit_tree(Some(&a), &[("foo.txt", "c")]);
        let d = test_repository.commit_tree(Some(&a), &[("foo.txt", "a"), ("bar.txt", "a")]);

        let bc_result =
            gitbutler_merge_commits(&test_repository.repository, b, c, "master", "feature")
                .unwrap();

        let result = gitbutler_merge_commits(
            &test_repository.repository,
            bc_result,
            d,
            "master",
            "feature",
        )
        .unwrap();

        // While its based on a conflicted commit, merging `bc_result` and `d`
        // should not conflict, because the auto-resolution of `bc_result`,
        // and `a` can be cleanly merged when `a` is the base.
        //
        // bc_result auto-resoultion tree:
        // foo.txt: c

        assert!(!result.is_conflicted());

        assert_commit_tree_matches(
            &test_repository.repository,
            &result,
            &[("foo.txt", b"c"), ("bar.txt", b"a")],
        );
    }

    #[test]
    fn merging_conflicted_commit_with_conflicted_incoming() {
        let test_repository = TestingRepository::open();

        // Make some commits
        let a = test_repository.commit_tree(None, &[("foo.txt", "a"), ("bar.txt", "a")]);
        let b = test_repository.commit_tree(Some(&a), &[("foo.txt", "b"), ("bar.txt", "a")]);
        let c = test_repository.commit_tree(Some(&a), &[("foo.txt", "c"), ("bar.txt", "a")]);
        let d = test_repository.commit_tree(Some(&a), &[("foo.txt", "a"), ("bar.txt", "b")]);
        let e = test_repository.commit_tree(Some(&a), &[("foo.txt", "a"), ("bar.txt", "c")]);

        let bc_result =
            gitbutler_merge_commits(&test_repository.repository, b, c, "master", "feature")
                .unwrap();

        let de_result =
            gitbutler_merge_commits(&test_repository.repository, d, e, "master", "feature")
                .unwrap();

        let result = gitbutler_merge_commits(
            &test_repository.repository,
            bc_result,
            de_result,
            "master",
            "feature",
        )
        .unwrap();

        // We don't expect result to be conflicted, because we've chosen the
        // setup such that the auto-resolution of `bc_result` and `de_result`
        // don't conflict when merged themselves.
        //
        // bc_result auto-resolution tree:
        // foo.txt: c
        // bar.txt: a
        //
        // bc_result auto-resolution tree:
        // foo.txt: a
        // bar.txt: c

        assert!(!result.is_conflicted());

        assert_commit_tree_matches(
            &test_repository.repository,
            &result,
            &[("foo.txt", b"c"), ("bar.txt", b"c")],
        );
    }

    #[test]
    fn merging_conflicted_commit_with_conflicted_incoming_and_results_in_conflicted() {
        let test_repository = TestingRepository::open();

        // Make some commits
        let a = test_repository.commit_tree(None, &[("foo.txt", "a")]);
        let b = test_repository.commit_tree(Some(&a), &[("foo.txt", "b")]);
        let c = test_repository.commit_tree(Some(&a), &[("foo.txt", "c")]);
        let d = test_repository.commit_tree(Some(&a), &[("foo.txt", "d")]);
        let e = test_repository.commit_tree(Some(&a), &[("foo.txt", "f")]);

        let bc_result =
            gitbutler_merge_commits(&test_repository.repository, b, c, "master", "feature")
                .unwrap();

        let de_result =
            gitbutler_merge_commits(&test_repository.repository, d, e, "master", "feature")
                .unwrap();

        let result = gitbutler_merge_commits(
            &test_repository.repository,
            bc_result,
            de_result,
            "master",
            "feature",
        )
        .unwrap();

        // bc_result auto-resoultion tree:
        // foo.txt: c
        //
        // bc_result auto-resoultion tree:
        // foo.txt: f
        //
        // This conflicts and results in auto-resolution f
        //
        // We however expect the theirs side to be "b" and the ours side to
        // be "f"

        assert!(result.is_conflicted());

        assert_commit_tree_matches(
            &test_repository.repository,
            &result,
            &[
                (".auto-resolution/foo.txt", b"f"), // Incoming change preferred
                (".conflict-base-0/foo.txt", b"a"), // Base should match A
                (".conflict-side-0/foo.txt", b"f"), // Side 0 should be incoming change
                (".conflict-side-1/foo.txt", b"b"), // Side 1 should be target change
            ],
        );
    }
}

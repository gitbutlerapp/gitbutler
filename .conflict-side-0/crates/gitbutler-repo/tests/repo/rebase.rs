mod gitbutler_merge_commits {
    use crate::support::repository;
    use gitbutler_repo::rebase::merge_commits;
    use tempfile::TempDir;

    fn test_repository(name: &str) -> (gix::Repository, TempDir) {
        repository(name)
    }

    fn commit_id(repo: &gix::Repository, revspec: &str) -> anyhow::Result<gix::ObjectId> {
        Ok(repo.rev_parse_single(revspec)?.detach())
    }

    fn gitbutler_merge_commits(
        repo: &gix::Repository,
        target_commit: &str,
        incoming_commit: &str,
        target_branch_name: &str,
        incoming_branch_name: &str,
    ) -> anyhow::Result<gix::ObjectId> {
        merge_commits(
            repo,
            commit_id(repo, target_commit)?,
            commit_id(repo, incoming_commit)?,
            &format!("Merge `{incoming_branch_name}` into `{target_branch_name}`"),
        )
    }

    #[test]
    fn unconflicting_merge() -> anyhow::Result<()> {
        let (repo, _tmp) = test_repository("rebase-merge-unconflicting");

        let result = gitbutler_merge_commits(&repo, "target", "incoming", "master", "feature")?;

        assert_commit_tree_matches(&repo, result, &[("foo.txt", b"b"), ("bar.txt", b"a")])?;
        Ok(())
    }

    #[test]
    fn conflicting_merge() -> anyhow::Result<()> {
        let (repo, _tmp) = test_repository("rebase-merge-conflicting");

        let result = gitbutler_merge_commits(&repo, "target", "incoming", "master", "feature")?;

        assert_commit_tree_matches(
            &repo,
            result,
            &[
                (".auto-resolution/foo.txt", b"c"), // Prefer the "Our" side, C
                (".conflict-base-0/foo.txt", b"a"), // The content of A
                (".conflict-side-0/foo.txt", b"c"), // "Our" side, content of B
                (".conflict-side-1/foo.txt", b"b"), // "Their" side, content of C
            ],
        )?;
        Ok(())
    }

    #[test]
    fn merging_conflicted_commit_with_unconflicted_incoming() -> anyhow::Result<()> {
        let (repo, _tmp) = test_repository("rebase-merge-conflicted-with-unconflicted");

        let conflicted = gitbutler_merge_commits(&repo, "target", "incoming", "master", "feature")?;
        let result = merge_commits(
            &repo,
            conflicted,
            commit_id(&repo, "unconflicted")?,
            "Merge `feature` into `master`",
        )?;

        // While its based on a conflicted commit, merging `bc_result` and `d`
        // should not conflict, because the auto-resolution of `bc_result`,
        // and `a` can be cleanly merged when `a` is the base.
        //
        // bc_result auto-resoultion tree:
        // foo.txt: c

        assert_commit_tree_matches(&repo, result, &[("foo.txt", b"c"), ("bar.txt", b"a")])?;
        Ok(())
    }

    #[test]
    fn merging_conflicted_commit_with_conflicted_incoming() -> anyhow::Result<()> {
        let (repo, _tmp) = test_repository("rebase-merge-two-conflicted-clean-result");

        let foo_result =
            gitbutler_merge_commits(&repo, "target-foo", "incoming-foo", "master", "feature")?;
        let bar_result =
            gitbutler_merge_commits(&repo, "target-bar", "incoming-bar", "master", "feature")?;
        let result = merge_commits(
            &repo,
            foo_result,
            bar_result,
            "Merge `feature` into `master`",
        )?;

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

        assert_commit_tree_matches(&repo, result, &[("foo.txt", b"c"), ("bar.txt", b"c")])?;
        Ok(())
    }

    #[test]
    fn merging_conflicted_commit_with_conflicted_incoming_and_results_in_conflicted()
    -> anyhow::Result<()> {
        let (repo, _tmp) = test_repository("rebase-merge-two-conflicted-conflict-result");

        let first_result =
            gitbutler_merge_commits(&repo, "target-one", "incoming-one", "master", "feature")?;
        let second_result =
            gitbutler_merge_commits(&repo, "target-two", "incoming-two", "master", "feature")?;
        let result = merge_commits(
            &repo,
            first_result,
            second_result,
            "Merge `feature` into `master`",
        )?;

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

        assert_commit_tree_matches(
            &repo,
            result,
            &[
                (".auto-resolution/foo.txt", b"f"), // Incoming change preferred
                (".conflict-base-0/foo.txt", b"a"), // Base should match A
                (".conflict-side-0/foo.txt", b"f"), // Side 0 should be incoming change
                (".conflict-side-1/foo.txt", b"b"), // Side 1 should be target change
            ],
        )?;
        Ok(())
    }

    fn assert_commit_tree_matches(
        repo: &gix::Repository,
        commit_id: gix::ObjectId,
        files: &[(&str, &[u8])],
    ) -> anyhow::Result<()> {
        for (path, content) in files {
            let revspec = format!("{commit_id}:{path}");
            let object = repo.rev_parse_single(revspec.as_str())?.object()?;
            assert_eq!(
                object.data,
                *content,
                "{}: expect {} == {}",
                path,
                String::from_utf8_lossy(&object.data),
                String::from_utf8_lossy(content)
            );
        }
        Ok(())
    }
}

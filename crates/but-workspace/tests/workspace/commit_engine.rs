mod new_commit {
    use crate::commit_engine::utils::{stable_env, to_change_requests, writable_scenario};
    use but_workspace::commit_engine;
    use but_workspace::commit_engine::{Destination, RefFrame};
    use gix::prelude::ObjectIdExt;
    use serial_test::serial;

    #[test]
    #[serial]
    fn from_unborn_head() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("unborn-untracked");
        let outcome = commit_engine::mutate(
            &repo,
            RefFrame::default(),
            Destination::AncestorForNewCommit(None),
            None,
            to_change_requests(but_core::diff::worktree_changes(&repo)?),
            "the commit message",
        )?;
        insta::assert_debug_snapshot!(&outcome, @r#"
        MutateOutcome {
            rejected_requests: [],
            new_commit: Some(
                Sha1(2eb2e90e37a7f23052db17b67b91eb5c4a7a1e81),
            ),
            rewritten_commits: [],
            ref_edits: [
                RefEdit {
                    change: Update {
                        log: LogChange {
                            mode: AndReference,
                            force_create_reflog: false,
                            message: "the commit message",
                        },
                        expected: Any,
                        new: Object(
                            Sha1(2eb2e90e37a7f23052db17b67b91eb5c4a7a1e81),
                        ),
                    },
                    name: FullName(
                        "refs/heads/main",
                    ),
                    deref: false,
                },
            ],
        }
        "#);

        let new_commit_id = outcome.new_commit.expect("a new commit was created");
        assert_eq!(
            repo.head_id()?,
            new_commit_id,
            "HEAD should have been updated"
        );
        assert_eq!(
            repo.head_ref()?.expect("not detached").name().as_bstr(),
            "refs/heads/main",
            "it kept the head-ref"
        );

        let new_commit = new_commit_id.attach(&repo).object()?.peel_to_commit()?;
        assert_eq!(new_commit.message_raw()?, "the commit message");

        let tree = gitbutler_testsupport::visualize_gix_tree(new_commit.tree_id()?);
        insta::assert_snapshot!(tree, @r#"
        861d6e2
        └── not-yet-tracked:100644:d95f3ad "content\n"
        "#);

        std::fs::write(
            repo.work_dir().expect("non-bare").join("new-untracked"),
            "new-content",
        )?;
        let outcome = commit_engine::mutate(
            &repo,
            RefFrame {
                topmost_ref: Some(repo.head_name()?.expect("not detached")),
                target_ref: None,
            },
            Destination::AncestorForNewCommit(Some(new_commit_id)),
            None,
            to_change_requests(but_core::diff::worktree_changes(&repo)?),
            "the second commit",
        )?;

        insta::assert_debug_snapshot!(&outcome, @r#"
        MutateOutcome {
            rejected_requests: [],
            new_commit: Some(
                Sha1(9fa45065e99a2f0492bca947cf462dfafd905516),
            ),
            rewritten_commits: [],
            ref_edits: [
                RefEdit {
                    change: Update {
                        log: LogChange {
                            mode: AndReference,
                            force_create_reflog: false,
                            message: "the second commit",
                        },
                        expected: MustExistAndMatch(
                            Object(
                                Sha1(2eb2e90e37a7f23052db17b67b91eb5c4a7a1e81),
                            ),
                        ),
                        new: Object(
                            Sha1(9fa45065e99a2f0492bca947cf462dfafd905516),
                        ),
                    },
                    name: FullName(
                        "refs/heads/main",
                    ),
                    deref: false,
                },
            ],
        }
        "#);
        let current_tip = outcome.new_commit.expect("a new commit was created");
        let head_ref = repo.head_ref()?.expect("not detached");
        assert_eq!(head_ref.id(), current_tip, "HEAD should have been updated");

        let tree = gitbutler_testsupport::visualize_gix_tree(
            outcome
                .new_commit
                .expect("no rejected changes")
                .attach(&repo)
                .object()?
                .peel_to_commit()?
                .tree_id()?,
        );
        insta::assert_snapshot!(tree, @r#"
        a004469
        ├── new-untracked:100644:72278a7 "new-content"
        └── not-yet-tracked:100644:d95f3ad "content\n"
        "#);
        Ok(())
    }

    #[test]
    #[serial]
    fn amend_to_base_commit() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("two-commits-with-line-offset");
        let outcome = commit_engine::mutate(
            &repo,
            RefFrame::default(),
            Destination::AncestorForNewCommit(Some(repo.rev_parse_single("first-commit")?.into())),
            None,
            to_change_requests(but_core::diff::worktree_changes(&repo)?),
            "we apply a change with line offsets on top of the first commit, so the patch wouldn't apply without fuzzy matching.",
        )?;

        let tree = gitbutler_testsupport::visualize_gix_tree(
            outcome
                .new_commit
                .expect("no rejected changes")
                .attach(&repo)
                .object()?
                .peel_to_commit()?
                .tree_id()?,
        );
        insta::assert_snapshot!(tree, @r#"
        46ae2ef
        └── file:100644:190423f "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n"
        "#);
        Ok(())
    }

    #[test]
    #[ignore = "TBD worktree filtering"]
    fn unborn_untracked_worktree_filters_are_applied_to_whole_files() -> anyhow::Result<()> {
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("unborn-untracked-crlf");
        let outcome = commit_engine::mutate(
            &repo,
            RefFrame::default(),
            Destination::AncestorForNewCommit(None),
            None,
            to_change_requests(but_core::diff::worktree_changes(&repo)?),
            "the commit message",
        )?;
        insta::assert_debug_snapshot!(&outcome, @r#"
        MutateOutcome {
            rejected_requests: [],
            new_commit: Some(
                Sha1(6c73ab1f2080c6297750565ef73fdc818ccb08b3),
            ),
            rewritten_commits: [],
            ref_edits: [
                RefEdit {
                    change: Update {
                        log: LogChange {
                            mode: AndReference,
                            force_create_reflog: false,
                            message: "the commit message",
                        },
                        expected: Any,
                        new: Object(
                            Sha1(6c73ab1f2080c6297750565ef73fdc818ccb08b3),
                        ),
                    },
                    name: FullName(
                        "refs/heads/main",
                    ),
                    deref: false,
                },
            ],
        }
        "#);

        let new_commit_id = outcome.new_commit.expect("a new commit was created");
        assert_eq!(
            repo.head_id()?,
            new_commit_id,
            "HEAD should have been updated"
        );
        assert_eq!(
            repo.head_ref()?.expect("not detached").name().as_bstr(),
            "refs/heads/main",
            "it kept the head-ref"
        );

        let new_commit = new_commit_id.attach(&repo).object()?.peel_to_commit()?;
        assert_eq!(new_commit.message_raw()?, "the commit message");

        // What's in Git is unix style newlines
        let tree = gitbutler_testsupport::visualize_gix_tree(new_commit.tree_id()?);
        insta::assert_snapshot!(tree, @r#"
        a0de2eb
        └── not-yet-tracked:100644:bd956ea "1\n2\n"
        "#);

        std::fs::write(
            repo.work_dir().expect("non-bare").join("new-untracked"),
            "one\r\ntwo\r\n",
        )?;
        let outcome = commit_engine::mutate(
            &repo,
            RefFrame {
                topmost_ref: Some(repo.head_name()?.expect("not detached")),
                target_ref: None,
            },
            Destination::AncestorForNewCommit(Some(new_commit_id)),
            None,
            to_change_requests(but_core::diff::worktree_changes(&repo)?),
            "the second commit",
        )?;

        insta::assert_debug_snapshot!(&outcome, @r#"
        MutateOutcome {
            rejected_requests: [],
            new_commit: Some(
                Sha1(71a25145d9c00a560a781c4a90c8d1666f43f496),
            ),
            rewritten_commits: [],
            ref_edits: [
                RefEdit {
                    change: Update {
                        log: LogChange {
                            mode: AndReference,
                            force_create_reflog: false,
                            message: "the second commit",
                        },
                        expected: MustExistAndMatch(
                            Object(
                                Sha1(6c73ab1f2080c6297750565ef73fdc818ccb08b3),
                            ),
                        ),
                        new: Object(
                            Sha1(71a25145d9c00a560a781c4a90c8d1666f43f496),
                        ),
                    },
                    name: FullName(
                        "refs/heads/main",
                    ),
                    deref: false,
                },
            ],
        }
        "#);
        let current_tip = outcome.new_commit.expect("a new commit was created");
        let head_ref = repo.head_ref()?.expect("not detached");
        assert_eq!(head_ref.id(), current_tip, "HEAD should have been updated");

        let tree = gitbutler_testsupport::visualize_gix_tree(
            outcome
                .new_commit
                .expect("no rejected changes")
                .attach(&repo)
                .object()?
                .peel_to_commit()?
                .tree_id()?,
        );
        insta::assert_snapshot!(tree, @r#"
        b973af8
        ├── new-untracked:100644:4e349b5 "one\r\ntwo\r\n"
        └── not-yet-tracked:100644:21be815 "1\r\n2\r\n\n"
        "#);

        Ok(())
    }

    #[test]
    #[ignore = "TBD"]
    fn worktree_filters_are_applied_to_whole_hunks() {}

    #[test]
    #[ignore = "TBD"]
    fn figure_out_commit_signature_test() {}

    #[test]
    #[ignore = "TBD"]
    fn validate_no_change_on_noop() {}
}

mod utils {
    use but_workspace::commit_engine::DiffSpec;

    /// Returns an environment that assure commits are reproducible. This needs the `testing` feature enabled in `but-core` as well to work.
    /// Note that this is racy once other tests rely on other values for these environment variables.
    pub fn stable_env() -> gix_testtools::Env<'static> {
        gix_testtools::Env::new()
            .set("GIT_AUTHOR_DATE", "2000-01-01 00:00:00 +0000")
            .set("GIT_AUTHOR_EMAIL", "author@example.com")
            .set("GIT_AUTHOR_NAME", "author")
            .set("GIT_COMMITTER_DATE", "2000-01-02 00:00:00 +0000")
            .set("GIT_COMMITTER_EMAIL", "committer@example.com")
            .set("GIT_COMMITTER_NAME", "committer")
            .set("CHANGE_ID", "committer")
    }

    fn writable_scenario_inner(
        name: &str,
    ) -> anyhow::Result<(gix::Repository, gix_testtools::tempfile::TempDir)> {
        let tmp = gix_testtools::scripted_fixture_writable(format!("scenario/{name}.sh"))
            .map_err(anyhow::Error::from_boxed)?;
        let mut options = gix::open::Options::isolated();
        options.permissions.env = gix::open::permissions::Environment::all();
        let repo = gix::open_opts(tmp.path(), options)?;
        Ok((repo, tmp))
    }

    pub fn writable_scenario(name: &str) -> (gix::Repository, gix_testtools::tempfile::TempDir) {
        writable_scenario_inner(name).expect("fixtures will yield valid repositories")
    }
    /// Always use all the hunks.
    pub fn to_change_requests(changes: but_core::WorktreeChanges) -> Vec<DiffSpec> {
        let out: Vec<_> = changes
            .changes
            .into_iter()
            .map(|change| DiffSpec {
                path: change.path,
                hunk_headers: Vec::new(),
            })
            .collect();
        assert!(
            !out.is_empty(),
            "fixture should contain actual changes to turn into requests"
        );
        out
    }
}

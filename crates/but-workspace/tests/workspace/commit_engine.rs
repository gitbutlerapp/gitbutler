mod new_commit {
    use crate::commit_engine::utils::{stable_env, to_change_requests, writable_scenario};
    use but_workspace::commit_engine;
    use but_workspace::commit_engine::{Destination, RefFrame};
    use gix::prelude::ObjectIdExt;

    #[test]
    fn from_unborn_head() -> anyhow::Result<()> {
        // TODO: use the same changeid everywhere, just to ignore it basically, or remove it. Otherwise it's racy.
        let _env = stable_env();

        let (repo, _tmp) = writable_scenario("unborn-untracked");
        let outcome = commit_engine::mutate(
            &repo,
            RefFrame::default(),
            Destination::HeadAsAncestorForNewCommit,
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

        std::fs::write(
            repo.work_dir().expect("non-bare").join("new-untracked"),
            "new-content",
        )?;

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

        let outcome = commit_engine::mutate(
            &repo,
            RefFrame {
                topmost_ref: Some(repo.head_name()?.expect("not detached")),
                target_ref: None,
            },
            Destination::AncestorForNewCommit(new_commit_id),
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
    #[ignore = "TBD"]
    fn worktree_filters_are_applied_to_whole_files() {}

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
    use but_workspace::commit_engine::ChangeRequest;

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
        let repo = gix::open_opts(tmp.path(), gix::open::Options::isolated())?;
        Ok((repo, tmp))
    }

    pub fn writable_scenario(name: &str) -> (gix::Repository, gix_testtools::tempfile::TempDir) {
        writable_scenario_inner(name).expect("fixtures will yield valid repositories")
    }
    /// Always use all the hunks.
    pub fn to_change_requests(changes: but_core::WorktreeChanges) -> Vec<ChangeRequest> {
        let out: Vec<_> = changes
            .changes
            .into_iter()
            .map(|change| ChangeRequest {
                path: change.path,
                hunks: Vec::new(),
            })
            .collect();
        assert!(
            !out.is_empty(),
            "fixture should contain actual changes to turn into requests"
        );
        out
    }
}

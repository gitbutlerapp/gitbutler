mod changes_in_branch {
    use crate::ref_info::with_workspace_commit::utils::read_only_in_memory_scenario;
    use but_graph::init::Options;
    use but_testsupport::visualize_commit_graph_all;
    use but_workspace::ui;

    #[test]
    fn multiple_inside_and_outside_of_workspace() -> anyhow::Result<()> {
        let (repo, meta) = read_only_in_memory_scenario("remote-advanced-ff")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * fb27086 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        | * 89cc2d3 (origin/A) change in A
        |/  
        * d79bba9 (A) new file in A
        * c166d42 (origin/main, origin/HEAD, main) init-integration
        ");

        let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
        let ws = graph.to_workspace()?;

        insta::assert_debug_snapshot!(ui::diff::changes_in_branch(&repo, &ws, r("refs/heads/A"))?, @r#"
        TreeChanges {
            changes: [
                TreeChange {
                    path: BStringForFrontend(
                        "file-in-A",
                    ),
                    path_bytes: "file-in-A",
                    status: Addition {
                        state: ChangeState {
                            id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                            kind: Blob,
                        },
                        is_untracked: false,
                    },
                },
            ],
            stats: TreeStats {
                lines_added: 0,
                lines_removed: 0,
                files_changed: 1,
            },
        }
        "#);
        insta::assert_debug_snapshot!(ui::diff::changes_in_branch(&repo, &ws, r("refs/remotes/origin/A"))?, @r#"
        TreeChanges {
            changes: [
                TreeChange {
                    path: BStringForFrontend(
                        "file-in-A",
                    ),
                    path_bytes: "file-in-A",
                    status: Addition {
                        state: ChangeState {
                            id: Sha1(0835e4f9714005ed591f68d306eea0d6d2ae8fd7),
                            kind: Blob,
                        },
                        is_untracked: false,
                    },
                },
            ],
            stats: TreeStats {
                lines_added: 1,
                lines_removed: 0,
                files_changed: 1,
            },
        }
        "#);
        // The same as what's in A, but it can find it.
        insta::assert_debug_snapshot!(ui::diff::changes_in_branch(&repo, &ws, r("refs/heads/gitbutler/workspace"))?, @r#"
        TreeChanges {
            changes: [
                TreeChange {
                    path: BStringForFrontend(
                        "file-in-A",
                    ),
                    path_bytes: "file-in-A",
                    status: Addition {
                        state: ChangeState {
                            id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                            kind: Blob,
                        },
                        is_untracked: false,
                    },
                },
            ],
            stats: TreeStats {
                lines_added: 0,
                lines_removed: 0,
                files_changed: 1,
            },
        }
        "#);

        let err =
            ui::diff::changes_in_branch(&repo, &ws, r("refs/heads/does-not-exist")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "The reference 'refs/heads/does-not-exist' did not exist",
            "passing strange ref-names still causes an error - they must exist"
        );
        Ok(())
    }

    fn r(name: &str) -> &gix::refs::FullNameRef {
        name.try_into().expect("statically known valid ref-name")
    }
}

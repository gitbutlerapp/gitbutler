mod dependencies {

    #[test]
    fn hunk_dependencies_json_sample() -> anyhow::Result<()> {
        let (actual, _ctx) = hunk_dependencies_for_workspace(
            "complex-file-manipulation-multiple-hunks-with-changes",
        )?;
        let actual_str = serde_json::to_string_pretty(&actual).unwrap();
        let actual_str =
            simplify_stack_ids_in_string(actual.commit_dependencies.keys(), actual_str);
        insta::assert_snapshot!(actual_str, @r#"
        {
          "diffs": [
            [
              3708607749576748282,
              [
                {
                  "stackId": "stack_1",
                  "commitId": "4bc98513b032b5992b85be8dd551e841bf959a3f"
                }
              ]
            ],
            [
              2434601357784245452,
              [
                {
                  "stackId": "stack_1",
                  "commitId": "dea907e862f2101c2ac493554e86abb1225b278e"
                },
                {
                  "stackId": "stack_1",
                  "commitId": "7558793046d64ea2070bf0856d5b2500371f0da6"
                },
                {
                  "stackId": "stack_1",
                  "commitId": "3bdeccbfca50778abfe67960f0732b0e4e065ab9"
                }
              ]
            ],
            [
              16712260417274738957,
              [
                {
                  "stackId": "stack_1",
                  "commitId": "7558793046d64ea2070bf0856d5b2500371f0da6"
                }
              ]
            ]
          ],
          "errors": []
        }
        "#);
        Ok(())
    }

    #[test]
    fn every_commit_is_independent() -> anyhow::Result<()> {
        let (actual, ctx) = hunk_dependencies_for_workspace("independent-commits")?;
        let stack = ctx.stack_with_branch("top-series");

        // No uncommited changes
        assert_eq!(actual.diffs.len(), 0);
        // One stack
        assert_eq!(actual.commit_dependencies.len(), 1);
        // No interdependencies
        let stack_commit_dependencies = actual.commit_dependencies.get(&stack.id).unwrap();
        assert_eq!(stack_commit_dependencies.len(), 0);
        let stack_inverse_commit_dependencies =
            actual.inverse_commit_dependencies.get(&stack.id).unwrap();
        assert_eq!(stack_inverse_commit_dependencies.len(), 0);
        assert_eq!(actual.commit_dependent_diffs.len(), 0);

        Ok(())
    }

    #[test]
    fn every_commit_is_independent_multi_stack() -> anyhow::Result<()> {
        let (actual, ctx) = hunk_dependencies_for_workspace("independent-commits-multi-stack")?;
        let stack = ctx.stack_with_branch("top-series");

        // No uncommited changes
        assert_eq!(actual.diffs.len(), 0);
        // One stack
        assert_eq!(actual.commit_dependencies.len(), 2);
        // No interdependencies
        let stack_commit_dependencies = actual.commit_dependencies.get(&stack.id).unwrap();
        assert_eq!(stack_commit_dependencies.len(), 0);
        let stack_inverse_commit_dependencies =
            actual.inverse_commit_dependencies.get(&stack.id).unwrap();
        assert_eq!(stack_inverse_commit_dependencies.len(), 0);
        assert_eq!(actual.commit_dependent_diffs.len(), 0);

        Ok(())
    }

    #[test]
    fn every_commit_is_sequentially_dependent() -> anyhow::Result<()> {
        let (actual, ctx) = hunk_dependencies_for_workspace("sequentially-dependent-commits")?;
        let stack = ctx.stack_with_branch("top-series");

        // No uncommited changes
        assert_eq!(actual.diffs.len(), 0);
        // One stack
        assert_eq!(actual.commit_dependencies.len(), 1);
        // Interdependencies
        let stack_commit_dependencies = actual.commit_dependencies.get(&stack.id).unwrap();
        assert_eq!(stack_commit_dependencies.len(), 5);
        assert_commit_map_matches_by_message(
            &ctx.repo,
            stack_commit_dependencies,
            HashMap::from([
                ("overwrite file with b", vec!["add file"]),
                ("overwrite file with c", vec!["overwrite file with b"]),
                ("overwrite file with d", vec!["overwrite file with c"]),
                ("overwrite file with e", vec!["overwrite file with d"]),
                ("overwrite file with f", vec!["overwrite file with e"]),
            ]),
            "commit_dependencies",
        )?;

        let stack_inverse_commit_dependencies =
            actual.inverse_commit_dependencies.get(&stack.id).unwrap();
        assert_eq!(stack_inverse_commit_dependencies.len(), 5);
        assert_commit_map_matches_by_message(
            &ctx.repo,
            stack_inverse_commit_dependencies,
            HashMap::from([
                ("add file", vec!["overwrite file with b"]),
                ("overwrite file with b", vec!["overwrite file with c"]),
                ("overwrite file with c", vec!["overwrite file with d"]),
                ("overwrite file with d", vec!["overwrite file with e"]),
                ("overwrite file with e", vec!["overwrite file with f"]),
            ]),
            "inverse_commit_dependencies",
        )?;

        assert_eq!(actual.commit_dependent_diffs.len(), 0);
        Ok(())
    }

    #[test]
    fn every_commit_is_sequentially_dependent_multi_stack() -> anyhow::Result<()> {
        let (actual, ctx) =
            hunk_dependencies_for_workspace("sequentially-dependent-commits-multi-stack")?;
        let other_stack = ctx.stack_with_branch("other-top-series");

        // No uncommited changes
        assert_eq!(actual.diffs.len(), 0);
        // One stack
        assert_eq!(actual.commit_dependencies.len(), 2);
        // Interdependencies - other_stack
        let stack_commit_dependencies = actual.commit_dependencies.get(&other_stack.id).unwrap();
        assert_eq!(stack_commit_dependencies.len(), 5);
        assert_commit_map_matches_by_message(
            &ctx.repo,
            stack_commit_dependencies,
            HashMap::from([
                ("overwrite file with b", vec!["add file"]),
                ("overwrite file with c", vec!["overwrite file with b"]),
                ("overwrite file with d", vec!["overwrite file with c"]),
                ("overwrite file with e", vec!["overwrite file with d"]),
                ("overwrite file with f", vec!["overwrite file with e"]),
            ]),
            "other_stack - commit_dependencies",
        )?;

        let stack_inverse_commit_dependencies = actual
            .inverse_commit_dependencies
            .get(&other_stack.id)
            .unwrap();
        assert_eq!(stack_inverse_commit_dependencies.len(), 5);
        assert_commit_map_matches_by_message(
            &ctx.repo,
            stack_inverse_commit_dependencies,
            HashMap::from([
                ("add file", vec!["overwrite file with b"]),
                ("overwrite file with b", vec!["overwrite file with c"]),
                ("overwrite file with c", vec!["overwrite file with d"]),
                ("overwrite file with d", vec!["overwrite file with e"]),
                ("overwrite file with e", vec!["overwrite file with f"]),
            ]),
            "other_stack - inverse_commit_dependencies",
        )?;

        // Interdependencies - my_stack
        let my_stack = ctx.stack_with_branch("top-series");
        let stack_commit_dependencies = actual.commit_dependencies.get(&my_stack.id).unwrap();
        assert_eq!(stack_commit_dependencies.len(), 5);
        assert_commit_map_matches_by_message(
            &ctx.repo,
            stack_commit_dependencies,
            HashMap::from([
                ("overwrite file_2 with b", vec!["add file_2"]),
                ("overwrite file_2 with c", vec!["overwrite file_2 with b"]),
                ("overwrite file_2 with d", vec!["overwrite file_2 with c"]),
                ("overwrite file_2 with e", vec!["overwrite file_2 with d"]),
                ("overwrite file_2 with f", vec!["overwrite file_2 with e"]),
            ]),
            "my_stack - commit_dependencies",
        )?;

        let stack_inverse_commit_dependencies = actual
            .inverse_commit_dependencies
            .get(&my_stack.id)
            .unwrap();
        assert_eq!(stack_inverse_commit_dependencies.len(), 5);
        assert_commit_map_matches_by_message(
            &ctx.repo,
            stack_inverse_commit_dependencies,
            HashMap::from([
                ("add file_2", vec!["overwrite file_2 with b"]),
                ("overwrite file_2 with b", vec!["overwrite file_2 with c"]),
                ("overwrite file_2 with c", vec!["overwrite file_2 with d"]),
                ("overwrite file_2 with d", vec!["overwrite file_2 with e"]),
                ("overwrite file_2 with e", vec!["overwrite file_2 with f"]),
            ]),
            "my_stack - inverse_commit_dependencies",
        )?;

        assert_eq!(actual.commit_dependent_diffs.len(), 0);

        Ok(())
    }

    #[test]
    fn delete_and_recreate_file_multi_stack() -> anyhow::Result<()> {
        let (actual, ctx) =
            hunk_dependencies_for_workspace("delete-and-recreate-file-multi-stack")?;
        let other_stack = ctx.stack_with_branch("other-top-series");

        // No uncommited changes
        assert_eq!(actual.diffs.len(), 0);
        // One stack
        assert_eq!(actual.commit_dependencies.len(), 2);
        // Interdependencies - other_stack
        let stack_commit_dependencies = actual.commit_dependencies.get(&other_stack.id).unwrap();

        assert_commit_map_matches_by_message(
            &ctx.repo,
            stack_commit_dependencies,
            HashMap::from([
                ("overwrite file with b", vec!["add file"]),
                ("remove file", vec!["overwrite file with b"]),
                ("recreate file with d", vec!["remove file"]),
                ("remove file again", vec!["recreate file with d"]),
                ("recreate file with f", vec!["remove file again"]),
            ]),
            "other_stack - commit_dependencies",
        )?;

        let stack_inverse_commit_dependencies = actual
            .inverse_commit_dependencies
            .get(&other_stack.id)
            .unwrap();
        assert_eq!(stack_inverse_commit_dependencies.len(), 5);
        assert_commit_map_matches_by_message(
            &ctx.repo,
            stack_inverse_commit_dependencies,
            HashMap::from([
                ("add file", vec!["overwrite file with b"]),
                ("overwrite file with b", vec!["remove file"]),
                ("remove file", vec!["recreate file with d"]),
                ("recreate file with d", vec!["remove file again"]),
                ("remove file again", vec!["recreate file with f"]),
            ]),
            "other_stack - inverse_commit_dependencies",
        )?;

        // Interdependencies - my_stack
        let my_stack = ctx.stack_with_branch("top-series");
        let stack_commit_dependencies = actual.commit_dependencies.get(&my_stack.id).unwrap();
        assert_eq!(stack_commit_dependencies.len(), 5);
        assert_commit_map_matches_by_message(
            &ctx.repo,
            stack_commit_dependencies,
            HashMap::from([
                ("remove file_2", vec!["add file_2"]),
                ("recreate file_2 with c", vec!["remove file_2"]),
                ("remove file_2 again", vec!["recreate file_2 with c"]),
                ("recreate file_2 with e", vec!["remove file_2 again"]),
                (
                    "remove file_2 one last time",
                    vec!["recreate file_2 with e"],
                ),
            ]),
            "my_stack - commit_dependencies",
        )?;

        let stack_inverse_commit_dependencies = actual
            .inverse_commit_dependencies
            .get(&my_stack.id)
            .unwrap();
        assert_eq!(stack_inverse_commit_dependencies.len(), 5);
        assert_commit_map_matches_by_message(
            &ctx.repo,
            stack_inverse_commit_dependencies,
            HashMap::from([
                ("add file_2", vec!["remove file_2"]),
                ("remove file_2", vec!["recreate file_2 with c"]),
                ("recreate file_2 with c", vec!["remove file_2 again"]),
                ("remove file_2 again", vec!["recreate file_2 with e"]),
                (
                    "recreate file_2 with e",
                    vec!["remove file_2 one last time"],
                ),
            ]),
            "my_stack - inverse_commit_dependencies",
        )?;

        assert_eq!(actual.commit_dependent_diffs.len(), 0);

        Ok(())
    }

    #[test]
    fn complex_file_manipulation() -> anyhow::Result<()> {
        let (actual, ctx) = hunk_dependencies_for_workspace("complex-file-manipulation")?;
        let my_stack = ctx.stack_with_branch("top-series");

        let commit_dependencies = actual.commit_dependencies.get(&my_stack.id).unwrap();
        assert_commit_map_matches_by_message(
            &ctx.repo,
            commit_dependencies,
            HashMap::from([
                ("modify line 5", vec!["add file"]),
                (
                    "file: add lines d and e at the beginning | file_2: modify line 1",
                    vec!["add file", "recreate file"],
                ),
                (
                    "remove file",
                    vec!["file: delete lines 4, 5 and 6 | file_2: delete lines g, h and i"],
                ),
                ("recreate file", vec!["remove file"]),
                (
                    "file: delete lines 4, 5 and 6 | file_2: delete lines g, h and i",
                    vec!["modify line 5", "add file"],
                ),
                ("add lines a, b and c at the end", vec!["recreate file"]),
            ]),
            "commit_dependencies",
        )?;
        let inverse_commit_dependencies = actual
            .inverse_commit_dependencies
            .get(&my_stack.id)
            .unwrap();

        assert_commit_map_matches_by_message(
            &ctx.repo,
            inverse_commit_dependencies,
            HashMap::from([
                (
                    "recreate file",
                    vec![
                        "file: add lines d and e at the beginning | file_2: modify line 1",
                        "add lines a, b and c at the end",
                    ],
                ),
                (
                    "modify line 5",
                    vec!["file: delete lines 4, 5 and 6 | file_2: delete lines g, h and i"],
                ),
                (
                    "file: delete lines 4, 5 and 6 | file_2: delete lines g, h and i",
                    vec!["remove file"],
                ),
                (
                    "add file",
                    vec![
                        "file: delete lines 4, 5 and 6 | file_2: delete lines g, h and i",
                        "file: add lines d and e at the beginning | file_2: modify line 1",
                        "modify line 5",
                    ],
                ),
                ("remove file", vec!["recreate file"]),
            ]),
            "inverse_commit_dependencies",
        )?;

        Ok(())
    }

    #[test]
    fn complex_file_manipulation_with_uncommitted_changes() -> anyhow::Result<()> {
        let (actual, _ctx) =
            hunk_dependencies_for_workspace("complex-file-manipulation-with-worktree-changes")?;

        let actual: StableHunkDependencies = actual.into();
        // One hunk has two commits, the other has one.[
        insta::assert_snapshot!(simplify_stack_ids(actual.commit_dependencies.iter().map(|t| t.0), &actual.diffs), @r"
        [
            (
                15941114795339476802,
                [
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(be6a4b2f1372586f4f1f434abea8087634b54148),
                    },
                ],
            ),
            (
                17676568081731369438,
                [
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(60a2022fc47aec1959cdf3e9b4a2c3cf441c918c),
                    },
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(0bfc8418d23102d123a376394f3364db66e0ef16),
                    },
                ],
            ),
        ]
        ");
        Ok(())
    }

    #[test]
    fn complex_file_manipulation_multiple_hunks() -> anyhow::Result<()> {
        let (actual, ctx) =
            hunk_dependencies_for_workspace("complex-file-manipulation-multiple-hunks")?;
        let my_stack = ctx.stack_with_branch("a-branch-1");

        let commit_dependencies = actual.commit_dependencies.get(&my_stack.id).unwrap();
        assert_commit_map_matches_by_message(
            &ctx.repo,
            commit_dependencies,
            HashMap::from([
                ("modify lines 4 and 8", vec!["create file"]),
                (
                    "insert 2 lines after 2, modify line 4 and remove line 6",
                    vec!["modify lines 4 and 8", "create file"],
                ),
                (
                    "insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7",
                    vec![
                        "insert 2 lines after 2, modify line 4 and remove line 6",
                        "create file",
                    ],
                ),
            ]),
            "commit_dependencies",
        )?;

        let inverse_commit_dependencies = actual
            .inverse_commit_dependencies
            .get(&my_stack.id)
            .unwrap();
        assert_commit_map_matches_by_message(
            &ctx.repo,
            inverse_commit_dependencies,
            HashMap::from([
                (
                    "create file",
                    vec![
                        "insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7",
                        "insert 2 lines after 2, modify line 4 and remove line 6",
                        "modify lines 4 and 8",
                    ],
                ),
                (
                    "modify lines 4 and 8",
                    vec!["insert 2 lines after 2, modify line 4 and remove line 6"],
                ),
                (
                    "insert 2 lines after 2, modify line 4 and remove line 6",
                    vec!["insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7"],
                ),
            ]),
            "inverse_commit_dependencies",
        )?;

        Ok(())
    }

    #[test]
    fn complex_file_manipulation_multiple_hunks_with_uncommitted_changes() -> anyhow::Result<()> {
        let (actual, _ctx) = hunk_dependencies_for_workspace(
            "complex-file-manipulation-multiple-hunks-with-changes",
        )?;

        // TODO: this is actually wrong, one of the single-lock hunks should have two.
        let actual: StableHunkDependencies = actual.into();
        insta::assert_snapshot!(simplify_stack_ids(actual.commit_dependencies.iter().map(|t| t.0), &actual.diffs), @r"
        [
            (
                2434601357784245452,
                [
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(dea907e862f2101c2ac493554e86abb1225b278e),
                    },
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(7558793046d64ea2070bf0856d5b2500371f0da6),
                    },
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(3bdeccbfca50778abfe67960f0732b0e4e065ab9),
                    },
                ],
            ),
            (
                3708607749576748282,
                [
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(4bc98513b032b5992b85be8dd551e841bf959a3f),
                    },
                ],
            ),
            (
                16712260417274738957,
                [
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(7558793046d64ea2070bf0856d5b2500371f0da6),
                    },
                ],
            ),
        ]
        ");

        // let hunk_1_locks = dependencies.diffs.get(&file_hunk_1_hash).unwrap();
        // assert_eq!(hunk_1_locks.len(), 2);
        // assert_hunk_lock_matches_by_message(
        //     hunk_1_locks[0],
        //     "create file",
        //     &ctx,
        //     "Hunk depends on the commit that created the file",
        // );
        // assert_hunk_lock_matches_by_message(
        //     hunk_1_locks[1],
        //     "insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7",
        //     &ctx,
        //     "Hunk depends on the commit that deleted lines 3 and 4",
        // );
        //
        // let hunk_2_locks = dependencies.diffs.get(&file_hunk_2_hash).unwrap();
        // assert_eq!(hunk_2_locks.len(), 3);
        // assert_hunk_lock_matches_by_message(
        //     hunk_2_locks[0],
        //     "insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7",
        //     &ctx,
        //     "Hunk depends on the commit that updated the line 7",
        // );
        // assert_hunk_lock_matches_by_message(
        //     hunk_2_locks[1],
        //     "create file",
        //     &ctx,
        //     "Hunk depends on the commit that created the file",
        // );
        // assert_hunk_lock_matches_by_message(
        //     hunk_2_locks[2],
        //     "modify lines 4 and 8",
        //     &ctx,
        //     "Hunk depends on the commit updated line 8",
        // );
        //
        // let hunk_3_locks = dependencies.diffs.get(&file_hunk_3_hash).unwrap();
        // assert_eq!(hunk_3_locks.len(), 1);
        // assert_hunk_lock_matches_by_message(
        //     hunk_3_locks[0],
        //     "insert 1 line at the top and bottom, remove lines 3 and 4 and update line 7",
        //     &ctx,
        //     "hunk 3",
        // );
        //
        Ok(())
    }

    #[test]
    fn dependencies_ignore_merge_commits() -> anyhow::Result<()> {
        let (actual, _ctx) = hunk_dependencies_for_workspace("merge-commit")?;

        let actual: StableHunkDependencies = actual.into();
        // There are no commit interdependencies
        // Hunk 1 should not have any dependencies, because it only intersects with a merge commit
        // update line 8 and delete the line after 7
        insta::assert_snapshot!(simplify_stack_ids(actual.commit_dependencies.iter().map(|t| t.0), &actual), @r"
        StableHunkDependencies {
            diffs: [
                (
                    12435379862484897562,
                    [
                        HunkLock {
                            stack_id: stack_1,
                            commit_id: Sha1(306c01518260b03afa8b92443850bf27b45d6d84),
                        },
                    ],
                ),
            ],
            commit_dependencies: [
                (
                    stack_1,
                    [],
                ),
            ],
            inverse_commit_dependencies: [
                (
                    stack_1,
                    [],
                ),
            ],
            commit_dependent_diffs: [
                (
                    stack_1,
                    [
                        (
                            Sha1(306c01518260b03afa8b92443850bf27b45d6d84),
                            [
                                12435379862484897562,
                            ],
                        ),
                    ],
                ),
            ],
            errors: [],
        }
        ");
        Ok(())
    }

    #[test]
    fn dependencies_handle_complex_branch_checkout() -> anyhow::Result<()> {
        // This test ensures that checking out branches with *complex* histories
        // does not cause the dependency calculation to fail.
        //
        // The *complexity* of the branch is that it contains a merge commit from itself to itself,
        // mimicking checking-out a remote branch that had PRs merged to it.
        let (actual, _ctx) = hunk_dependencies_for_workspace("complex-branch-checkout")?;
        let actual_stable: StableHunkDependencies = actual.clone().into();
        insta::assert_snapshot!(simplify_stack_ids(actual_stable.commit_dependencies.iter().map(|t| t.0), &actual_stable), @r"
        StableHunkDependencies {
            diffs: [],
            commit_dependencies: [
                (
                    stack_1,
                    [
                        (
                            Sha1(025094bc81dd5d9cc8f22ceaa4bfe455b629a4f6),
                            [
                                Sha1(3a3bd95137fb74f4e1b91dd5caf27d40f762654c),
                            ],
                        ),
                        (
                            Sha1(29e20dfbf0130dde207c6e3c704469a869db920e),
                            [
                                Sha1(ec0e5f750f7107aa403b1ca89cf836a78194505a),
                            ],
                        ),
                        (
                            Sha1(ec0e5f750f7107aa403b1ca89cf836a78194505a),
                            [
                                Sha1(557c961ed6a10fcc3f15d370aa07fdd5c49243d5),
                            ],
                        ),
                    ],
                ),
            ],
            inverse_commit_dependencies: [
                (
                    stack_1,
                    [
                        (
                            Sha1(3a3bd95137fb74f4e1b91dd5caf27d40f762654c),
                            [
                                Sha1(025094bc81dd5d9cc8f22ceaa4bfe455b629a4f6),
                            ],
                        ),
                        (
                            Sha1(557c961ed6a10fcc3f15d370aa07fdd5c49243d5),
                            [
                                Sha1(ec0e5f750f7107aa403b1ca89cf836a78194505a),
                            ],
                        ),
                        (
                            Sha1(ec0e5f750f7107aa403b1ca89cf836a78194505a),
                            [
                                Sha1(29e20dfbf0130dde207c6e3c704469a869db920e),
                            ],
                        ),
                    ],
                ),
            ],
            commit_dependent_diffs: [],
            errors: [],
        }
        ");

        // // TODO: The commented out portion fails.
        // let my_stack = ctx.stack_with_branch("a-branch-1");
        //
        // let commit_dependencies = actual.commit_dependencies.get(&my_stack.id).unwrap();
        // assert_commit_map_matches_by_message(
        //     &ctx.repo,
        //     commit_dependencies,
        //     HashMap::from([
        //         ("update a again", vec!["update a"]),
        //         ("update a", vec!["add a"]),
        //         ("Merge branch 'delete-b' into my_stack", vec!["add b"]),
        //     ]),
        //     "Commit interdependencies correctly calculated. They should only pick up the merge commit when calculating dependencies",
        // )?;

        Ok(())
    }

    mod util {
        use gitbutler_stack::StackId;
        use gitbutler_tauri::workspace::{HunkDependencies, HunkHash, HunkLock};
        use itertools::Itertools;
        use std::collections::{HashMap, HashSet};
        use std::path::PathBuf;

        pub fn simplify_stack_ids(
            stack_ids: impl Iterator<Item = impl ToString>,
            to_simplify: &impl std::fmt::Debug,
        ) -> String {
            simplify_stack_ids_in_string(stack_ids, format!("{to_simplify:#?}"))
        }

        pub fn simplify_stack_ids_in_string(
            stack_ids: impl Iterator<Item = impl ToString>,
            mut to_simplify: String,
        ) -> String {
            let mut count = 1;
            for stack_id in stack_ids {
                to_simplify = to_simplify.replace(&stack_id.to_string(), &format!("stack_{count}"));
                count += 1;
            }
            to_simplify
        }

        pub fn hunk_dependencies_for_workspace(
            name: &str,
        ) -> anyhow::Result<(HunkDependencies, TestContext)> {
            let ctx = test_ctx_at(
                "../../../but-hunk-dependency/tests/fixtures/dependencies.sh",
                name,
            )?;
            let deps = hunk_dependencies_for_workspace_by_ctx(&ctx)?;
            Ok((deps, ctx))
        }

        fn hunk_dependencies_for_workspace_by_ctx(
            ctx: &TestContext,
        ) -> anyhow::Result<HunkDependencies> {
            gitbutler_tauri::workspace::hunk_dependencies_for_workspace_changes_by_worktree_dir(
                ctx.repo.work_dir().expect("We don't support bare repos"),
                &ctx.gitbutler_dir,
            )
        }

        #[derive(Debug)]
        #[allow(dead_code)]
        #[allow(clippy::type_complexity)]
        pub struct StableHunkDependencies {
            pub diffs: Vec<(HunkHash, Vec<HunkLock>)>,
            pub commit_dependencies: Vec<(StackId, Vec<(gix::ObjectId, Vec<gix::ObjectId>)>)>,
            pub inverse_commit_dependencies:
                Vec<(StackId, Vec<(gix::ObjectId, Vec<gix::ObjectId>)>)>,
            pub commit_dependent_diffs: Vec<(StackId, Vec<(gix::ObjectId, Vec<HunkHash>)>)>,
            pub errors: Vec<but_hunk_dependency::CalculationError>,
        }

        impl From<HunkDependencies> for StableHunkDependencies {
            fn from(
                HunkDependencies {
                    diffs,
                    commit_dependencies,
                    inverse_commit_dependencies,
                    commit_dependent_diffs,
                    errors,
                }: HunkDependencies,
            ) -> Self {
                #[allow(clippy::type_complexity)]
                fn nested_to_vec<A: Ord + Copy, B: Ord + Copy, C: Ord>(
                    m: HashMap<A, HashMap<B, HashSet<C>>>,
                ) -> Vec<(A, Vec<(B, Vec<C>)>)> {
                    m.into_iter()
                        .map(|(key, nested_value)| {
                            (
                                key,
                                nested_value
                                    .into_iter()
                                    .map(|(key, nested_value)| {
                                        (key, {
                                            let mut v: Vec<_> = nested_value.into_iter().collect();
                                            v.sort();
                                            v
                                        })
                                    })
                                    .sorted_by_key(|t| t.0)
                                    .collect(),
                            )
                        })
                        .sorted_by_key(|t| t.0)
                        .collect()
                }
                StableHunkDependencies {
                    diffs: diffs.into_iter().sorted_by_key(|t| t.0).collect(),
                    commit_dependencies: nested_to_vec(commit_dependencies),
                    inverse_commit_dependencies: nested_to_vec(inverse_commit_dependencies),
                    commit_dependent_diffs: nested_to_vec(commit_dependent_diffs),
                    errors,
                }
            }
        }

        pub struct TestContext {
            pub repo: gix::Repository,
            /// All the stacks in the workspace
            pub stacks_entries: Vec<but_workspace::StackEntry>,
            /// The storage directory for GitButler.
            pub gitbutler_dir: PathBuf,
        }

        fn test_ctx_at(script_name: &str, name: &str) -> anyhow::Result<TestContext> {
            let ctx = gitbutler_testsupport::read_only::fixture(script_name, name)?;
            let stacks = but_workspace::stacks(&ctx.project().gb_dir())?;

            Ok(TestContext {
                repo: gix::open_opts(&ctx.project().path, gix::open::Options::isolated())?,
                gitbutler_dir: ctx.project().gb_dir(),
                stacks_entries: stacks,
            })
        }

        impl TestContext {
            /// Find a stack which contains a branch with the given `short_name`.
            pub fn stack_with_branch(&self, short_name: &str) -> &but_workspace::StackEntry {
                self.stacks_entries
                    .iter()
                    .find(|s| s.name() == Some(short_name.into()))
                    .expect("fixture should have such a stack if it was asked for")
            }
        }

        pub fn assert_commit_map_matches_by_message(
            repo: &gix::Repository,
            actual: &HashMap<gix::ObjectId, HashSet<gix::ObjectId>>,
            expected: HashMap<&str, Vec<&str>>,
            message: &str,
        ) -> anyhow::Result<()> {
            fn extract_commit_messages(
                repo: &gix::Repository,
                actual: &HashMap<gix::ObjectId, HashSet<gix::ObjectId>>,
            ) -> anyhow::Result<HashMap<String, Vec<String>>> {
                let mut messages = HashMap::new();
                for (oid_key, oid_values) in actual {
                    let key_commit = repo.find_commit(*oid_key)?;
                    let key_message = key_commit.message()?.title.to_string();
                    let actual_values: Vec<_> = oid_values
                        .iter()
                        .map(|oid| -> anyhow::Result<_> {
                            let value_commit = repo.find_commit(*oid)?;
                            let value_commit_message = value_commit.message()?.title.to_string();
                            Ok(value_commit_message.to_string())
                        })
                        .collect::<Result<_, _>>()?;

                    messages.insert(key_message, actual_values);
                }
                Ok(messages)
            }

            let actual_messages = extract_commit_messages(repo, actual)?;

            let expected: HashMap<String, Vec<String>> = expected
                .iter()
                .map(|(key, values)| {
                    let values: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                    (key.to_string(), values)
                })
                .collect();

            assert_eq!(
                actual_messages.len(),
                expected.len(),
                "should have same size {}",
                message
            );

            for (key, values) in expected {
                assert!(
                    actual_messages.contains_key(&key),
                    "should contain key '{}' - {}",
                    key,
                    message
                );
                let actual_values = actual_messages.get(&key).unwrap();
                assert_eq!(
                    actual_values.len(),
                    values.len(),
                    "should have same length '{}' - {}",
                    key,
                    message
                );
                for value in values {
                    assert!(
                        actual_values.contains(&value.to_string()),
                        "'{}' should contain '{}' - {}",
                        key,
                        value,
                        message
                    );
                }
            }
            Ok(())
        }
    }
    use std::collections::HashMap;
    use util::{
        assert_commit_map_matches_by_message, hunk_dependencies_for_workspace, simplify_stack_ids,
        simplify_stack_ids_in_string, StableHunkDependencies,
    };
}

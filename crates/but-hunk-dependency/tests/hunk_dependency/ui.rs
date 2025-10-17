#[test]
fn hunk_dependencies_json_sample() -> anyhow::Result<()> {
    let (actual, _ctx) =
        hunk_dependencies_for_workspace("complex-file-manipulation-multiple-hunks-with-changes")?;
    let actual_str = serde_json::to_string_pretty(&actual).unwrap();
    let stack_ids = stack_ids_by_diffs(&actual);
    let actual_str = simplify_stack_ids_in_string(stack_ids.iter(), actual_str);
    insta::assert_snapshot!(actual_str, @r#"
    {
      "diffs": [
        [
          "file",
          {
            "oldStart": 3,
            "oldLines": 1,
            "newStart": 3,
            "newLines": 2,
            "diff": "@@ -3,1 +3,2 @@\n-2\n+aaaaa\n+aaaaa\n"
          },
          [
            {
              "stackId": "stack_1",
              "commitId": "375e35becbf67fe2b246b120bc76bf070e3e41d8"
            }
          ]
        ],
        [
          "file",
          {
            "oldStart": 7,
            "oldLines": 2,
            "newStart": 8,
            "newLines": 1,
            "diff": "@@ -7,2 +8,1 @@\n-update 7\n-update 8\n+aaaaa\n"
          },
          [
            {
              "stackId": "stack_1",
              "commitId": "e954269ca7be71d09da50ec389b13f268a779c27"
            },
            {
              "stackId": "stack_1",
              "commitId": "fba21e9ecacde86f327537add23f96775064a486"
            },
            {
              "stackId": "stack_1",
              "commitId": "250b92ba3b6451781f6632cd34be70db814ec4ac"
            }
          ]
        ],
        [
          "file",
          {
            "oldStart": 10,
            "oldLines": 1,
            "newStart": 10,
            "newLines": 2,
            "diff": "@@ -10,1 +10,2 @@\n-added at the bottom\n+update bottom\n+add another line\n"
          },
          [
            {
              "stackId": "stack_1",
              "commitId": "fba21e9ecacde86f327537add23f96775064a486"
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
fn complex_file_manipulation_with_uncommitted_changes() -> anyhow::Result<()> {
    let (actual, _ctx) =
        hunk_dependencies_for_workspace("complex-file-manipulation-with-worktree-changes")?;
    insta::assert_snapshot!(to_stable_string(actual), @r#"
    StableHunkDependencies {
        diffs: [
            (
                "file",
                DiffHunk("@@ -2,4 +2,3 @@
                -e
                -1
                -2
                -3
                +updated line 3
                +updated line 4
                +updated line 5
                "),
                [
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(b2510a88eab2cf2f7dde23202c7a8e536905f4f7),
                    },
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(3113ca08b2b23d7646cdb9c9b88ffde8d1c7784a),
                    },
                ],
            ),
            (
                "file_2",
                DiffHunk("@@ -4,1 +4,1 @@
                -d
                +updated d
                "),
                [
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(98231f63bb3539acf42356c886c07b140d42b68c),
                    },
                ],
            ),
        ],
        errors: [],
    }
    "#);
    Ok(())
}

#[test]
fn complex_file_manipulation_multiple_hunks_with_uncommitted_changes() -> anyhow::Result<()> {
    let (actual, _ctx) =
        hunk_dependencies_for_workspace("complex-file-manipulation-multiple-hunks-with-changes")?;
    insta::assert_snapshot!(to_stable_string(actual), @r#"
    StableHunkDependencies {
        diffs: [
            (
                "file",
                DiffHunk("@@ -3,1 +3,2 @@
                -2
                +aaaaa
                +aaaaa
                "),
                [
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(375e35becbf67fe2b246b120bc76bf070e3e41d8),
                    },
                ],
            ),
            (
                "file",
                DiffHunk("@@ -7,2 +8,1 @@
                -update 7
                -update 8
                +aaaaa
                "),
                [
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(e954269ca7be71d09da50ec389b13f268a779c27),
                    },
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(fba21e9ecacde86f327537add23f96775064a486),
                    },
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(250b92ba3b6451781f6632cd34be70db814ec4ac),
                    },
                ],
            ),
            (
                "file",
                DiffHunk("@@ -10,1 +10,2 @@
                -added at the bottom
                +update bottom
                +add another line
                "),
                [
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(fba21e9ecacde86f327537add23f96775064a486),
                    },
                ],
            ),
        ],
        errors: [],
    }
    "#);
    Ok(())
}

#[test]
fn dependencies_ignore_merge_commits() -> anyhow::Result<()> {
    let (actual, _ctx) = hunk_dependencies_for_workspace("merge-commit")?;
    insta::assert_snapshot!(to_stable_string(actual), @r#"
    StableHunkDependencies {
        diffs: [
            (
                "file",
                DiffHunk("@@ -8,1 +8,1 @@
                -update line 8
                +update line 8 again
                "),
                [
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(1179d94bda967d9fb70b518b9b7225524c185218),
                    },
                ],
            ),
        ],
        errors: [],
    }
    "#);
    Ok(())
}

mod util {
    use std::{collections::HashSet, path::PathBuf};

    use but_core::unified_diff::DiffHunk;
    use but_hunk_dependency::ui::{
        HunkDependencies, HunkLock, hunk_dependencies_for_workspace_changes_by_worktree_dir,
    };
    use gitbutler_command_context::CommandContext;
    use gitbutler_stack::StackId;
    use itertools::Itertools;

    pub fn to_stable_string(deps: HunkDependencies) -> String {
        let stack_ids = stack_ids_by_diffs(&deps);
        let deps: StableHunkDependencies = deps.into();
        simplify_stack_ids(stack_ids, &deps)
    }

    pub fn simplify_stack_ids(
        stack_ids: impl IntoIterator<Item = impl ToString>,
        to_simplify: &impl std::fmt::Debug,
    ) -> String {
        simplify_stack_ids_in_string(stack_ids.into_iter(), format!("{to_simplify:#?}"))
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
        let script_name = "../../../but-hunk-dependency/tests/fixtures/dependencies.sh";
        let ctx = test_ctx_at(script_name, name)?;
        let command_context = gitbutler_testsupport::read_only::fixture(script_name, name)?;
        let deps = hunk_dependencies_for_workspace_by_ctx(&ctx, &command_context)?;
        Ok((deps, ctx))
    }

    fn hunk_dependencies_for_workspace_by_ctx(
        ctx: &TestContext,
        command_context: &CommandContext,
    ) -> anyhow::Result<HunkDependencies> {
        hunk_dependencies_for_workspace_changes_by_worktree_dir(
            command_context,
            ctx.repo.workdir().expect("We don't support bare repos"),
            &ctx.gitbutler_dir,
            None,
        )
    }

    #[derive(Debug)]
    #[expect(dead_code)]
    pub struct StableHunkDependencies {
        pub diffs: Vec<(String, DiffHunk, Vec<HunkLock>)>,
        pub errors: Vec<but_hunk_dependency::CalculationError>,
    }

    impl From<HunkDependencies> for StableHunkDependencies {
        fn from(HunkDependencies { diffs, errors }: HunkDependencies) -> Self {
            StableHunkDependencies {
                diffs: diffs
                    .into_iter()
                    .sorted_by(|a, b| {
                        Ord::cmp(&a.0, &b.0)
                            .then_with(|| Ord::cmp(&a.1.old_start, &b.1.old_start))
                            .then_with(|| Ord::cmp(&a.1.new_start, &b.1.new_start))
                    })
                    .collect(),
                errors,
            }
        }
    }

    pub struct TestContext {
        pub repo: gix::Repository,
        /// All the stacks in the workspace
        pub stacks_entries: Vec<but_workspace::ui::StackEntry>,
        /// The storage directory for GitButler.
        pub gitbutler_dir: PathBuf,
    }

    fn test_ctx_at(script_name: &str, name: &str) -> anyhow::Result<TestContext> {
        let ctx = gitbutler_testsupport::read_only::fixture(script_name, name)?;
        let stacks = but_workspace::stacks(
            &ctx,
            &ctx.project().gb_dir(),
            &ctx.gix_repo()?,
            Default::default(),
        )?;

        Ok(TestContext {
            repo: gix::open_opts(&ctx.project().path, gix::open::Options::isolated())?,
            gitbutler_dir: ctx.project().gb_dir(),
            stacks_entries: stacks,
        })
    }

    impl TestContext {
        /// Find a stack which contains a branch with the given `short_name`.
        #[expect(unused)]
        pub fn stack_with_branch(&self, short_name: &str) -> &but_workspace::ui::StackEntry {
            self.stacks_entries
                .iter()
                .find(|s| s.name() == Some(short_name.into()))
                .expect("fixture should have such a stack if it was asked for")
        }
    }

    pub fn stack_ids_by_diffs(deps: &HunkDependencies) -> HashSet<StackId> {
        deps.diffs
            .iter()
            .flat_map(|(_, _, locks)| locks.iter().map(|lock| lock.stack_id))
            .collect()
    }
}
use util::{
    hunk_dependencies_for_workspace, simplify_stack_ids_in_string, stack_ids_by_diffs,
    to_stable_string,
};

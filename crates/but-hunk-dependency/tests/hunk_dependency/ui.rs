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
              "commitId": "4bc98513b032b5992b85be8dd551e841bf959a3f"
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
                        commit_id: Sha1(60a2022fc47aec1959cdf3e9b4a2c3cf441c918c),
                    },
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(0bfc8418d23102d123a376394f3364db66e0ef16),
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
                        commit_id: Sha1(be6a4b2f1372586f4f1f434abea8087634b54148),
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
                        commit_id: Sha1(4bc98513b032b5992b85be8dd551e841bf959a3f),
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
                "file",
                DiffHunk("@@ -10,1 +10,2 @@
                -added at the bottom
                +update bottom
                +add another line
                "),
                [
                    HunkLock {
                        stack_id: stack_1,
                        commit_id: Sha1(7558793046d64ea2070bf0856d5b2500371f0da6),
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
                        commit_id: Sha1(306c01518260b03afa8b92443850bf27b45d6d84),
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
    use but_core::unified_diff::DiffHunk;
    use but_hunk_dependency::ui::{
        HunkDependencies, HunkLock, hunk_dependencies_for_workspace_changes_by_worktree_dir,
    };
    use gitbutler_command_context::CommandContext;
    use gitbutler_stack::StackId;
    use itertools::Itertools;
    use std::collections::HashSet;
    use std::path::PathBuf;

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
        )
    }

    #[derive(Debug)]
    #[allow(dead_code)]
    #[allow(clippy::type_complexity)]
    pub struct StableHunkDependencies {
        pub diffs: Vec<(String, DiffHunk, Vec<HunkLock>)>,
        pub errors: Vec<but_hunk_dependency::CalculationError>,
    }

    impl From<HunkDependencies> for StableHunkDependencies {
        fn from(HunkDependencies { diffs, errors }: HunkDependencies) -> Self {
            #[allow(clippy::type_complexity)]
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
        #[allow(unused)]
        pub stacks_entries: Vec<but_workspace::StackEntry>,
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
        #[allow(unused)]
        pub fn stack_with_branch(&self, short_name: &str) -> &but_workspace::StackEntry {
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

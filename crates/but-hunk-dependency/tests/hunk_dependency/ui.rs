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
              15989967088397270668,
              [
                {
                  "stackId": "stack_1",
                  "commitId": "4bc98513b032b5992b85be8dd551e841bf959a3f"
                }
              ]
            ],
            [
              8240534308515296008,
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
              18120254504138851193,
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
    insta::assert_snapshot!(to_stable_string(actual), @r"
        StableHunkDependencies {
            diffs: [
                (
                    5660422847701176503,
                    [
                        HunkLock {
                            stack_id: stack_1,
                            commit_id: Sha1(be6a4b2f1372586f4f1f434abea8087634b54148),
                        },
                    ],
                ),
                (
                    6048968734524995517,
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
            ],
            errors: [],
        }
        ");
    Ok(())
}

#[test]
fn complex_file_manipulation_multiple_hunks_with_uncommitted_changes() -> anyhow::Result<()> {
    let (actual, _ctx) =
        hunk_dependencies_for_workspace("complex-file-manipulation-multiple-hunks-with-changes")?;
    insta::assert_snapshot!(to_stable_string(actual), @r"
        StableHunkDependencies {
            diffs: [
                (
                    8240534308515296008,
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
                    15989967088397270668,
                    [
                        HunkLock {
                            stack_id: stack_1,
                            commit_id: Sha1(4bc98513b032b5992b85be8dd551e841bf959a3f),
                        },
                    ],
                ),
                (
                    18120254504138851193,
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
        ");
    Ok(())
}

#[test]
fn dependencies_ignore_merge_commits() -> anyhow::Result<()> {
    let (actual, _ctx) = hunk_dependencies_for_workspace("merge-commit")?;
    insta::assert_snapshot!(to_stable_string(actual), @r"
        StableHunkDependencies {
            diffs: [
                (
                    2654316029522724523,
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
        ");
    Ok(())
}

mod util {
    use but_hunk_dependency::ui::{
        HunkDependencies, HunkHash, HunkLock,
        hunk_dependencies_for_workspace_changes_by_worktree_dir,
    };
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
        hunk_dependencies_for_workspace_changes_by_worktree_dir(
            ctx.repo.work_dir().expect("We don't support bare repos"),
            &ctx.gitbutler_dir,
        )
    }

    #[derive(Debug)]
    #[allow(dead_code)]
    #[allow(clippy::type_complexity)]
    pub struct StableHunkDependencies {
        pub diffs: Vec<(HunkHash, Vec<HunkLock>)>,
        pub errors: Vec<but_hunk_dependency::CalculationError>,
    }

    impl From<HunkDependencies> for StableHunkDependencies {
        fn from(HunkDependencies { diffs, errors }: HunkDependencies) -> Self {
            #[allow(clippy::type_complexity)]
            StableHunkDependencies {
                diffs: diffs.into_iter().sorted_by_key(|t| t.0).collect(),
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
        let stacks = but_workspace::stacks(&ctx.project().gb_dir())?;

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
            .flat_map(|(_, locks)| locks.iter().map(|lock| lock.stack_id))
            .collect()
    }
}
use util::{
    hunk_dependencies_for_workspace, simplify_stack_ids_in_string, stack_ids_by_diffs,
    to_stable_string,
};

#[test]
fn hunk_dependencies_json_sample() -> anyhow::Result<()> {
    let (actual, _ctx) = hunk_dependencies_for_workspace_separated(
        "complex-file-manipulation-multiple-hunks-with-changes",
    )?;
    let actual_str = serde_json::to_string_pretty(&actual).unwrap();
    let stack_ids = targets_by_diffs(&actual);
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
              "target": {
                "type": "stack_1"
              },
              "commitId": "9dd99b79687c4b67024d0855c08a6ad55bfa6632"
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
              "target": {
                "type": "stack_1"
              },
              "commitId": "9c7ad9af048c7b9ad1e84b0c17641347b84db1d1"
            },
            {
              "target": {
                "type": "stack_1"
              },
              "commitId": "500b70982f4576f707583412a28a6797211d180a"
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
              "target": {
                "type": "stack_1"
              },
              "commitId": "9dd99b79687c4b67024d0855c08a6ad55bfa6632"
            },
            {
              "target": {
                "type": "stack_1"
              },
              "commitId": "9c7ad9af048c7b9ad1e84b0c17641347b84db1d1"
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
    let (actual, _ctx) = hunk_dependencies_for_workspace_separated(
        "complex-file-manipulation-with-worktree-changes",
    )?;
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
                        target: Unidentified,
                        commit_id: Sha1(9375fb38f57a4c4b1b27da267cf2ccc1b54dbfab),
                    },
                    HunkLock {
                        target: Unidentified,
                        commit_id: Sha1(c85aaba55013536a1d44d8972d0b9fe9e484eb6f),
                    },
                    HunkLock {
                        target: Unidentified,
                        commit_id: Sha1(9375fb38f57a4c4b1b27da267cf2ccc1b54dbfab),
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
                        target: Unidentified,
                        commit_id: Sha1(109a5229203def5810b0e9fbaf053039f6d601b4),
                    },
                    HunkLock {
                        target: Unidentified,
                        commit_id: Sha1(109a5229203def5810b0e9fbaf053039f6d601b4),
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
    let (actual, _ctx) = hunk_dependencies_for_workspace_separated(
        "complex-file-manipulation-multiple-hunks-with-changes",
    )?;
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
                        target: Unidentified,
                        commit_id: Sha1(9dd99b79687c4b67024d0855c08a6ad55bfa6632),
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
                        target: Unidentified,
                        commit_id: Sha1(9c7ad9af048c7b9ad1e84b0c17641347b84db1d1),
                    },
                    HunkLock {
                        target: Unidentified,
                        commit_id: Sha1(500b70982f4576f707583412a28a6797211d180a),
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
                        target: Unidentified,
                        commit_id: Sha1(9dd99b79687c4b67024d0855c08a6ad55bfa6632),
                    },
                    HunkLock {
                        target: Unidentified,
                        commit_id: Sha1(9c7ad9af048c7b9ad1e84b0c17641347b84db1d1),
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
    let (actual, _ctx) = hunk_dependencies_for_workspace_separated("merge-commit")?;
    insta::assert_snapshot!(to_stable_string(actual), @r#"
    StableHunkDependencies {
        diffs: [
            (
                "file",
                DiffHunk("@@ -5,1 +5,1 @@
                -update line 5
                +update line 5 again
                "),
                [
                    HunkLock {
                        target: Unidentified,
                        commit_id: Sha1(e418c1bb8d15211edb026fa562fb4c83199d8481),
                    },
                ],
            ),
            (
                "file",
                DiffHunk("@@ -8,1 +8,1 @@
                -update line 8
                +update line 8 again
                "),
                [
                    HunkLock {
                        target: Unidentified,
                        commit_id: Sha1(df241a53d1a4dae37f16def5f6d9405ea236fbf6),
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
    use std::collections::HashSet;

    use but_core::unified_diff::DiffHunk;
    use but_ctx::Context;
    use but_hunk_dependency::ui::{
        HunkDependencies, HunkLock, HunkLockTarget,
        hunk_dependencies_for_workspace_changes_by_worktree_dir,
    };
    use but_testsupport::gix_testtools::tempfile;
    use itertools::Itertools;

    pub fn to_stable_string(deps: HunkDependencies) -> String {
        let stack_ids = targets_by_diffs(&deps);
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

    pub fn hunk_dependencies_for_workspace_separated(
        name: &str,
    ) -> anyhow::Result<(HunkDependencies, TestContext)> {
        let (ctx, command_context) = test_scenario(name)?;
        let deps = hunk_dependencies_for_workspace_by_ctx(&command_context)?;
        Ok((deps, ctx))
    }

    fn hunk_dependencies_for_workspace_by_ctx(
        command_context: &Context,
    ) -> anyhow::Result<HunkDependencies> {
        let (_guard, repo, ws, _) = command_context.workspace_and_db()?;
        hunk_dependencies_for_workspace_changes_by_worktree_dir(&repo, &ws, None)
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
        // TODO: remove this once this has been ported to 'modern' code, i.e. uses `but-graph::projection::Workspace`.
        #[allow(dead_code)]
        pub tmpdir: Option<tempfile::TempDir>,
    }

    fn test_scenario(name: &str) -> anyhow::Result<(TestContext, Context)> {
        // TODO: make this a read-only scenario once we don't rely on vb.toml anymore.
        let (repo, tmpdir) = but_testsupport::writable_scenario(name);
        let ctx = Context::from_repo(repo)?;
        Ok((
            TestContext {
                tmpdir: Some(tmpdir),
            },
            ctx,
        ))
    }

    pub fn targets_by_diffs(deps: &HunkDependencies) -> HashSet<HunkLockTarget> {
        deps.diffs
            .iter()
            .flat_map(|(_, _, locks)| locks.iter().map(|lock| lock.target))
            .collect()
    }
}
use util::{simplify_stack_ids_in_string, targets_by_diffs, to_stable_string};

use crate::ui::util::hunk_dependencies_for_workspace_separated;

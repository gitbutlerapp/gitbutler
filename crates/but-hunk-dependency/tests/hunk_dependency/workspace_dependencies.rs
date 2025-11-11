#[test]
fn every_commit_is_independent() -> anyhow::Result<()> {
    let actual = worktree_ranges_digest_for_workspace_named("independent-commits");
    // Nothing to see here, there is no worktree change.
    insta::assert_debug_snapshot!(actual, @r#"
    Ok(
        WorkspaceDigest {
            ranges_by_path: [
                (
                    "a",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(c7bae2e8c4ed7aba203c752cd6736f70f04fd544),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "b",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(f3235c8c52ba77ee341c02cc9fa3b8bd9969f048),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "c",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(6a97e922f1021217da59dcb91e7b5e1088ad0ac8),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "d",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(9b0dc3c781e575aba25d7dca67df9bace74d72d3),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "e",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(0b15614231606e65246f7ad52685612b495d63ff),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "f",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(0c8146ee1326d65c2e2b895a636a5dc26d55e6e4),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
            ],
            intersections_by_path: [],
            missed_hunks: [],
        },
    )
    "#);
    Ok(())
}

#[test]
fn multiple_stacks_with_multiple_branches_each() -> anyhow::Result<()> {
    let actual = worktree_ranges_digest_for_workspace_named("independent-commits-multi-stack");
    // Nothing to see here, there is no worktree change.
    insta::assert_debug_snapshot!(actual, @r#"
    Ok(
        WorkspaceDigest {
            ranges_by_path: [
                (
                    "a",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(c7bae2e8c4ed7aba203c752cd6736f70f04fd544),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "b",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(f3235c8c52ba77ee341c02cc9fa3b8bd9969f048),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "c",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(6a97e922f1021217da59dcb91e7b5e1088ad0ac8),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "d",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(9b0dc3c781e575aba25d7dca67df9bace74d72d3),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "e",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(0b15614231606e65246f7ad52685612b495d63ff),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "f",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(0c8146ee1326d65c2e2b895a636a5dc26d55e6e4),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "g",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(2ecdbf9d2deb34a2a11caa9d6ceddf567033d8b9),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "h",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(5469306a5386f7ce63eb914c638198b8e8549e75),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "i",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(0d5184fdb9b4735f2717dc040e0995f5cc5f5af1),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "j",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(e2af430b8fe8fa7c208c1c9548fffa70fb7c5923),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "k",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(855154a10af961174b5a0a1544d36641c1a9e85e),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
                (
                    "l",
                    [
                        StableHunkRange {
                            change_type: Addition,
                            commit_id: Sha1(874b74abfca585b57deb72f98ffcded86016221b),
                            start: 1,
                            lines: 1,
                            line_shift: 1,
                        },
                    ],
                ),
            ],
            intersections_by_path: [],
            missed_hunks: [],
        },
    )
    "#);
    Ok(())
}

#[test]
fn every_commit_is_sequentially_dependent() -> anyhow::Result<()> {
    let actual = worktree_ranges_digest_for_workspace_named("sequentially-dependent-commits");
    insta::assert_debug_snapshot!(actual, @r#"
    Ok(
        WorkspaceDigest {
            ranges_by_path: [
                (
                    "file",
                    [
                        StableHunkRange {
                            change_type: Modification,
                            commit_id: Sha1(90bdc8c6752c9045fc667f7aea605d526ad8e574),
                            start: 1,
                            lines: 1,
                            line_shift: 0,
                        },
                    ],
                ),
            ],
            intersections_by_path: [],
            missed_hunks: [],
        },
    )
    "#);
    Ok(())
}

#[test]
fn every_commit_is_sequentially_dependent_multi_stack() -> anyhow::Result<()> {
    let actual =
        worktree_ranges_digest_for_workspace_named("sequentially-dependent-commits-multi-stack");
    insta::assert_debug_snapshot!(actual, @r#"
    Ok(
        WorkspaceDigest {
            ranges_by_path: [
                (
                    "file",
                    [
                        StableHunkRange {
                            change_type: Modification,
                            commit_id: Sha1(90bdc8c6752c9045fc667f7aea605d526ad8e574),
                            start: 1,
                            lines: 1,
                            line_shift: 0,
                        },
                    ],
                ),
                (
                    "file_2",
                    [
                        StableHunkRange {
                            change_type: Modification,
                            commit_id: Sha1(517d4baf97701f602df4b3672618b4199d4b1a71),
                            start: 1,
                            lines: 1,
                            line_shift: 0,
                        },
                    ],
                ),
            ],
            intersections_by_path: [],
            missed_hunks: [],
        },
    )
    "#);
    Ok(())
}

#[test]
fn delete_and_recreate_file_multi_stack() -> anyhow::Result<()> {
    let actual =
        worktree_ranges_digest_for_workspace_named("delete-and-recreate-file-multi-stack")?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorkspaceDigest {
        ranges_by_path: [
            (
                "file",
                [
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(925d483a4608e3588d24d8c28c3b626c6d946a93),
                        start: 1,
                        lines: 1,
                        line_shift: 1,
                    },
                ],
            ),
            (
                "file_2",
                [
                    StableHunkRange {
                        change_type: Deletion,
                        commit_id: Sha1(36e261b96c0a7ef265c66909c1efc1c354038874),
                        start: 1,
                        lines: 0,
                        line_shift: 0,
                    },
                ],
            ),
        ],
        intersections_by_path: [],
        missed_hunks: [],
    }
    "#);
    Ok(())
}

#[test]
fn complex_file_manipulation() -> anyhow::Result<()> {
    let actual = worktree_ranges_digest_for_workspace_named("complex-file-manipulation")?;
    // TODO: why are there additions with 0 lines?
    insta::assert_debug_snapshot!(actual, @r#"
    WorkspaceDigest {
        ranges_by_path: [
            (
                "file",
                [
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(3113ca08b2b23d7646cdb9c9b88ffde8d1c7784a),
                        start: 1,
                        lines: 0,
                        line_shift: 7,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(b2510a88eab2cf2f7dde23202c7a8e536905f4f7),
                        start: 1,
                        lines: 2,
                        line_shift: 2,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(3113ca08b2b23d7646cdb9c9b88ffde8d1c7784a),
                        start: 3,
                        lines: 6,
                        line_shift: 7,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(4f6e1f7479f83a4c669f45e2381d69c02b2c5e1f),
                        start: 9,
                        lines: 3,
                        line_shift: 2,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(3113ca08b2b23d7646cdb9c9b88ffde8d1c7784a),
                        start: 12,
                        lines: 0,
                        line_shift: 7,
                    },
                ],
            ),
            (
                "file_2",
                [
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(b2510a88eab2cf2f7dde23202c7a8e536905f4f7),
                        start: 1,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(98231f63bb3539acf42356c886c07b140d42b68c),
                        start: 2,
                        lines: 5,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(dfb757b02a06e28a9d6822376b2f8829f90d46bd),
                        start: 7,
                        lines: 0,
                        line_shift: -3,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(98231f63bb3539acf42356c886c07b140d42b68c),
                        start: 7,
                        lines: 1,
                        line_shift: 10,
                    },
                ],
            ),
        ],
        intersections_by_path: [],
        missed_hunks: [],
    }
    "#);
    Ok(())
}

#[test]
fn complex_file_manipulation_with_uncommitted_changes() -> anyhow::Result<()> {
    let actual = worktree_ranges_digest_for_workspace_named(
        "complex-file-manipulation-with-worktree-changes",
    )?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorkspaceDigest {
        ranges_by_path: [
            (
                "file",
                [
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(3113ca08b2b23d7646cdb9c9b88ffde8d1c7784a),
                        start: 1,
                        lines: 0,
                        line_shift: 7,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(b2510a88eab2cf2f7dde23202c7a8e536905f4f7),
                        start: 1,
                        lines: 2,
                        line_shift: 2,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(3113ca08b2b23d7646cdb9c9b88ffde8d1c7784a),
                        start: 3,
                        lines: 6,
                        line_shift: 7,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(4f6e1f7479f83a4c669f45e2381d69c02b2c5e1f),
                        start: 9,
                        lines: 3,
                        line_shift: 2,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(3113ca08b2b23d7646cdb9c9b88ffde8d1c7784a),
                        start: 12,
                        lines: 0,
                        line_shift: 7,
                    },
                ],
            ),
            (
                "file_2",
                [
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(b2510a88eab2cf2f7dde23202c7a8e536905f4f7),
                        start: 1,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(98231f63bb3539acf42356c886c07b140d42b68c),
                        start: 2,
                        lines: 5,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(dfb757b02a06e28a9d6822376b2f8829f90d46bd),
                        start: 7,
                        lines: 0,
                        line_shift: -3,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(98231f63bb3539acf42356c886c07b140d42b68c),
                        start: 7,
                        lines: 1,
                        line_shift: 10,
                    },
                ],
            ),
        ],
        intersections_by_path: [
            (
                "file",
                [
                    HunkIntersection {
                        hunk: DiffHunk("@@ -2,4 +2,3 @@
                        -e
                        -1
                        -2
                        -3
                        +updated line 3
                        +updated line 4
                        +updated line 5
                        "),
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(b2510a88eab2cf2f7dde23202c7a8e536905f4f7),
                                start: 1,
                                lines: 2,
                                line_shift: 2,
                            },
                            StableHunkRange {
                                change_type: Addition,
                                commit_id: Sha1(3113ca08b2b23d7646cdb9c9b88ffde8d1c7784a),
                                start: 3,
                                lines: 6,
                                line_shift: 7,
                            },
                        ],
                    },
                ],
            ),
            (
                "file_2",
                [
                    HunkIntersection {
                        hunk: DiffHunk("@@ -4,1 +4,1 @@
                        -d
                        +updated d
                        "),
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Addition,
                                commit_id: Sha1(98231f63bb3539acf42356c886c07b140d42b68c),
                                start: 2,
                                lines: 5,
                                line_shift: 0,
                            },
                        ],
                    },
                ],
            ),
        ],
        missed_hunks: [],
    }
    "#);
    Ok(())
}

#[test]
fn complex_file_manipulation_multiple_hunks() -> anyhow::Result<()> {
    let actual =
        worktree_ranges_digest_for_workspace_named("complex-file-manipulation-multiple-hunks")?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorkspaceDigest {
        ranges_by_path: [
            (
                "file",
                [
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(375e35becbf67fe2b246b120bc76bf070e3e41d8),
                        start: 1,
                        lines: 0,
                        line_shift: 9,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(fba21e9ecacde86f327537add23f96775064a486),
                        start: 1,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(375e35becbf67fe2b246b120bc76bf070e3e41d8),
                        start: 2,
                        lines: 2,
                        line_shift: 9,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(fba21e9ecacde86f327537add23f96775064a486),
                        start: 4,
                        lines: 0,
                        line_shift: -2,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(e954269ca7be71d09da50ec389b13f268a779c27),
                        start: 5,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(375e35becbf67fe2b246b120bc76bf070e3e41d8),
                        start: 6,
                        lines: 1,
                        line_shift: 9,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(e954269ca7be71d09da50ec389b13f268a779c27),
                        start: 7,
                        lines: 0,
                        line_shift: -1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(fba21e9ecacde86f327537add23f96775064a486),
                        start: 7,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(250b92ba3b6451781f6632cd34be70db814ec4ac),
                        start: 8,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(375e35becbf67fe2b246b120bc76bf070e3e41d8),
                        start: 9,
                        lines: 1,
                        line_shift: 9,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(fba21e9ecacde86f327537add23f96775064a486),
                        start: 10,
                        lines: 1,
                        line_shift: 1,
                    },
                ],
            ),
        ],
        intersections_by_path: [],
        missed_hunks: [],
    }
    "#);
    Ok(())
}

#[test]
fn complex_file_manipulation_multiple_hunks_with_uncommitted_changes() -> anyhow::Result<()> {
    let actual = worktree_ranges_digest_for_workspace_named(
        "complex-file-manipulation-multiple-hunks-with-changes",
    )?;

    insta::assert_debug_snapshot!(actual.partial(), @r#"
    WorkspaceWithoutRanges {
        intersections_by_path: [
            (
                "file",
                [
                    HunkIntersection {
                        hunk: DiffHunk("@@ -3,1 +3,2 @@
                        -2
                        +aaaaa
                        +aaaaa
                        "),
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(375e35becbf67fe2b246b120bc76bf070e3e41d8),
                                start: 2,
                                lines: 2,
                                line_shift: 9,
                            },
                        ],
                    },
                    HunkIntersection {
                        hunk: DiffHunk("@@ -7,2 +8,1 @@
                        -update 7
                        -update 8
                        +aaaaa
                        "),
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(e954269ca7be71d09da50ec389b13f268a779c27),
                                start: 7,
                                lines: 0,
                                line_shift: -1,
                            },
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(fba21e9ecacde86f327537add23f96775064a486),
                                start: 7,
                                lines: 1,
                                line_shift: 0,
                            },
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(250b92ba3b6451781f6632cd34be70db814ec4ac),
                                start: 8,
                                lines: 1,
                                line_shift: 0,
                            },
                        ],
                    },
                    HunkIntersection {
                        hunk: DiffHunk("@@ -10,1 +10,2 @@
                        -added at the bottom
                        +update bottom
                        +add another line
                        "),
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(fba21e9ecacde86f327537add23f96775064a486),
                                start: 10,
                                lines: 1,
                                line_shift: 1,
                            },
                        ],
                    },
                ],
            ),
        ],
        missed_hunks: [],
    }
    "#);
    Ok(())
}

#[test]
fn dependencies_ignore_merge_commits() -> anyhow::Result<()> {
    let actual = worktree_ranges_digest_for_workspace_named("merge-commit")?;

    insta::assert_debug_snapshot!(actual.partial(), @r#"
    WorkspaceWithoutRanges {
        intersections_by_path: [
            (
                "file",
                [
                    HunkIntersection {
                        hunk: DiffHunk("@@ -8,1 +8,1 @@
                        -update line 8
                        +update line 8 again
                        "),
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(1179d94bda967d9fb70b518b9b7225524c185218),
                                start: 8,
                                lines: 1,
                                line_shift: -1,
                            },
                        ],
                    },
                ],
            ),
        ],
        missed_hunks: [
            (
                "file",
                DiffHunk("@@ -5,1 +5,1 @@
                -update line 5
                +update line 5 again
                "),
            ),
        ],
    }
    "#);
    Ok(())
}

#[test]
fn dependencies_handle_complex_branch_checkout() -> anyhow::Result<()> {
    // This test ensures that checking out branches with *complex* histories
    // does not cause the dependency calculation to fail.
    //
    // The *complexity* of the branch is that it contains a merge commit from itself to itself,
    // mimicking checking-out a remote branch that had PRs merged to it.
    let actual = worktree_ranges_digest_for_workspace_named("complex-branch-checkout")?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorkspaceDigest {
        ranges_by_path: [
            (
                "a",
                [
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(29e20dfbf0130dde207c6e3c704469a869db920e),
                        start: 1,
                        lines: 1,
                        line_shift: 0,
                    },
                ],
            ),
            (
                "b",
                [
                    StableHunkRange {
                        change_type: Deletion,
                        commit_id: Sha1(025094bc81dd5d9cc8f22ceaa4bfe455b629a4f6),
                        start: 1,
                        lines: 0,
                        line_shift: 0,
                    },
                ],
            ),
            (
                "c",
                [
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(37b7e2e334faf6998dbb8d7f4646c1187b46b354),
                        start: 1,
                        lines: 1,
                        line_shift: 1,
                    },
                ],
            ),
        ],
        intersections_by_path: [],
        missed_hunks: [],
    }
    "#);

    Ok(())
}

mod util {
    use but_oxidize::OidExt;
    use gitbutler_stack::VirtualBranchesHandle;

    use crate::{WorkspaceDigest, intersect_workspace_ranges};

    pub fn worktree_ranges_digest_for_workspace_named(
        name: &str,
    ) -> anyhow::Result<WorkspaceDigest> {
        let test_ctx = test_ctx(name)?;
        worktree_ranges_digest_for_workspace(&test_ctx)
    }
    fn worktree_ranges_digest_for_workspace(ctx: &TestContext) -> anyhow::Result<WorkspaceDigest> {
        let input_stacks = but_hunk_dependency::workspace_stacks_to_input_stacks(
            &ctx.repo,
            &ctx.stacks_entries,
            ctx.common_merge_base,
        )?;
        let ranges = but_hunk_dependency::WorkspaceRanges::try_from_stacks(input_stacks)?;
        let worktree_changes = but_core::diff::worktree_changes(&ctx.repo)?.changes;
        intersect_workspace_ranges(&ctx.repo, ranges, worktree_changes)
    }

    fn test_ctx_at(script_name: &str, name: &str) -> anyhow::Result<TestContext> {
        let ctx = gitbutler_testsupport::read_only::fixture(script_name, name)?;
        let stacks = but_workspace::legacy::stacks(
            &ctx,
            &ctx.project().gb_dir(),
            &ctx.gix_repo()?,
            Default::default(),
        )?;
        let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());

        Ok(TestContext {
            repo: ctx.project().open_isolated()?,
            stacks_entries: stacks,
            common_merge_base: handle.get_default_target()?.sha.to_gix(),
        })
    }

    pub fn test_ctx(name: &str) -> anyhow::Result<TestContext> {
        test_ctx_at("dependencies.sh", name)
    }

    pub struct TestContext {
        pub repo: gix::Repository,
        /// All the stacks in the workspace
        pub stacks_entries: Vec<but_workspace::legacy::ui::StackEntry>,
        /// The tip of the local branch that tracks the upstream one.
        pub common_merge_base: gix::ObjectId,
    }
}

use crate::workspace_dependencies::util::worktree_ranges_digest_for_workspace_named;

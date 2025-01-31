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
                            commit_id: Sha1(51ec59cb5b96509c755b0ec0b656dcb66d4c38b5),
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
                            commit_id: Sha1(5233c17e9ae7ae9902a377cf18f526c87ea10090),
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
                            commit_id: Sha1(847c738a74b7e8dc9a041abb324cc30bb1ec7601),
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
                            commit_id: Sha1(91d1bdabf8ea1f82ccc4650c944e30e37b3fb7d0),
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
                            commit_id: Sha1(69a5d567e227e7925501ce12543ee8d078bed387),
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
                            commit_id: Sha1(b801632fbe7050fa95b68914c392cc7472e53f1c),
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
                            commit_id: Sha1(51ec59cb5b96509c755b0ec0b656dcb66d4c38b5),
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
                            commit_id: Sha1(5233c17e9ae7ae9902a377cf18f526c87ea10090),
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
                            commit_id: Sha1(847c738a74b7e8dc9a041abb324cc30bb1ec7601),
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
                            commit_id: Sha1(91d1bdabf8ea1f82ccc4650c944e30e37b3fb7d0),
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
                            commit_id: Sha1(69a5d567e227e7925501ce12543ee8d078bed387),
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
                            commit_id: Sha1(b801632fbe7050fa95b68914c392cc7472e53f1c),
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
                            commit_id: Sha1(e134730e7c1b2e3ddc8d94ae2a1344866d002e1d),
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
                            commit_id: Sha1(a655019fd0b2df1cb380dd14c80a3290786408cb),
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
                            commit_id: Sha1(a74fc2d6bc6027a28c68b8bac5fdf48fb85fadfa),
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
                            commit_id: Sha1(4a82a7a0f6910e52f96d062606fbf1040c300a13),
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
                            commit_id: Sha1(862d101dbbcdec1f1d263670f1833b7e68569628),
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
                            commit_id: Sha1(6690284f0d4ffb291fa619db4dd3f46a0023f9ff),
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
                            commit_id: Sha1(a1e0611ace0aeaf59f58550c1801575d240e292a),
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
                            commit_id: Sha1(a1e0611ace0aeaf59f58550c1801575d240e292a),
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
                            commit_id: Sha1(ec45a328b989fc64aa072f625731caa00d03a89d),
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
                        commit_id: Sha1(32ffdb24b5272de977351fa9c2e71a7ec1fe40e1),
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
                        commit_id: Sha1(c59d9958755e135db22be8d9e81723f8b7bfa8e4),
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
                        commit_id: Sha1(0bfc8418d23102d123a376394f3364db66e0ef16),
                        start: 1,
                        lines: 0,
                        line_shift: 7,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(60a2022fc47aec1959cdf3e9b4a2c3cf441c918c),
                        start: 1,
                        lines: 2,
                        line_shift: 2,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(0bfc8418d23102d123a376394f3364db66e0ef16),
                        start: 3,
                        lines: 6,
                        line_shift: 7,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(96f1e1fe9da3a1e977a0cf5177eb0f6a3da17980),
                        start: 9,
                        lines: 3,
                        line_shift: 2,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(0bfc8418d23102d123a376394f3364db66e0ef16),
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
                        commit_id: Sha1(60a2022fc47aec1959cdf3e9b4a2c3cf441c918c),
                        start: 1,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(be6a4b2f1372586f4f1f434abea8087634b54148),
                        start: 2,
                        lines: 5,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(d183b4d1138e672911177f3338c9f2e11f973a68),
                        start: 7,
                        lines: 0,
                        line_shift: -3,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(be6a4b2f1372586f4f1f434abea8087634b54148),
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
                        commit_id: Sha1(0bfc8418d23102d123a376394f3364db66e0ef16),
                        start: 1,
                        lines: 0,
                        line_shift: 7,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(60a2022fc47aec1959cdf3e9b4a2c3cf441c918c),
                        start: 1,
                        lines: 2,
                        line_shift: 2,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(0bfc8418d23102d123a376394f3364db66e0ef16),
                        start: 3,
                        lines: 6,
                        line_shift: 7,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(96f1e1fe9da3a1e977a0cf5177eb0f6a3da17980),
                        start: 9,
                        lines: 3,
                        line_shift: 2,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(0bfc8418d23102d123a376394f3364db66e0ef16),
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
                        commit_id: Sha1(60a2022fc47aec1959cdf3e9b4a2c3cf441c918c),
                        start: 1,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(be6a4b2f1372586f4f1f434abea8087634b54148),
                        start: 2,
                        lines: 5,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(d183b4d1138e672911177f3338c9f2e11f973a68),
                        start: 7,
                        lines: 0,
                        line_shift: -3,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(be6a4b2f1372586f4f1f434abea8087634b54148),
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
                        hunk: DiffHunk {
                            old_start: 2,
                            old_lines: 4,
                            new_start: 2,
                            new_lines: 3,
                            diff: "@@ -2,4 +2,3 @@\n-e\n-1\n-2\n-3\n+updated line 3\n+updated line 4\n+updated line 5\n",
                        },
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(60a2022fc47aec1959cdf3e9b4a2c3cf441c918c),
                                start: 1,
                                lines: 2,
                                line_shift: 2,
                            },
                            StableHunkRange {
                                change_type: Addition,
                                commit_id: Sha1(0bfc8418d23102d123a376394f3364db66e0ef16),
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
                        hunk: DiffHunk {
                            old_start: 4,
                            old_lines: 1,
                            new_start: 4,
                            new_lines: 1,
                            diff: "@@ -4,1 +4,1 @@\n-d\n+updated d\n",
                        },
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Addition,
                                commit_id: Sha1(be6a4b2f1372586f4f1f434abea8087634b54148),
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
                        commit_id: Sha1(4bc98513b032b5992b85be8dd551e841bf959a3f),
                        start: 1,
                        lines: 0,
                        line_shift: 9,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(7558793046d64ea2070bf0856d5b2500371f0da6),
                        start: 1,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(4bc98513b032b5992b85be8dd551e841bf959a3f),
                        start: 2,
                        lines: 2,
                        line_shift: 9,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(7558793046d64ea2070bf0856d5b2500371f0da6),
                        start: 4,
                        lines: 0,
                        line_shift: -2,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(dea907e862f2101c2ac493554e86abb1225b278e),
                        start: 5,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(4bc98513b032b5992b85be8dd551e841bf959a3f),
                        start: 6,
                        lines: 1,
                        line_shift: 9,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(dea907e862f2101c2ac493554e86abb1225b278e),
                        start: 7,
                        lines: 0,
                        line_shift: -1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(7558793046d64ea2070bf0856d5b2500371f0da6),
                        start: 7,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(3bdeccbfca50778abfe67960f0732b0e4e065ab9),
                        start: 8,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(4bc98513b032b5992b85be8dd551e841bf959a3f),
                        start: 9,
                        lines: 1,
                        line_shift: 9,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(7558793046d64ea2070bf0856d5b2500371f0da6),
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
                        hunk: DiffHunk {
                            old_start: 3,
                            old_lines: 1,
                            new_start: 3,
                            new_lines: 2,
                            diff: "@@ -3,1 +3,2 @@\n-2\n+aaaaa\n+aaaaa\n",
                        },
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(4bc98513b032b5992b85be8dd551e841bf959a3f),
                                start: 2,
                                lines: 2,
                                line_shift: 9,
                            },
                        ],
                    },
                    HunkIntersection {
                        hunk: DiffHunk {
                            old_start: 7,
                            old_lines: 2,
                            new_start: 8,
                            new_lines: 1,
                            diff: "@@ -7,2 +8,1 @@\n-update 7\n-update 8\n+aaaaa\n",
                        },
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(dea907e862f2101c2ac493554e86abb1225b278e),
                                start: 7,
                                lines: 0,
                                line_shift: -1,
                            },
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(7558793046d64ea2070bf0856d5b2500371f0da6),
                                start: 7,
                                lines: 1,
                                line_shift: 0,
                            },
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(3bdeccbfca50778abfe67960f0732b0e4e065ab9),
                                start: 8,
                                lines: 1,
                                line_shift: 0,
                            },
                        ],
                    },
                    HunkIntersection {
                        hunk: DiffHunk {
                            old_start: 10,
                            old_lines: 1,
                            new_start: 10,
                            new_lines: 2,
                            diff: "@@ -10,1 +10,2 @@\n-added at the bottom\n+update bottom\n+add another line\n",
                        },
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(7558793046d64ea2070bf0856d5b2500371f0da6),
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
                        hunk: DiffHunk {
                            old_start: 8,
                            old_lines: 1,
                            new_start: 8,
                            new_lines: 1,
                            diff: "@@ -8,1 +8,1 @@\n-update line 8\n+update line 8 again\n",
                        },
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(306c01518260b03afa8b92443850bf27b45d6d84),
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
                DiffHunk {
                    old_start: 5,
                    old_lines: 1,
                    new_start: 5,
                    new_lines: 1,
                    diff: "@@ -5,1 +5,1 @@\n-update line 5\n+update line 5 again\n",
                },
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
    use crate::{intersect_workspace_ranges, WorkspaceDigest};
    use gitbutler_oxidize::OidExt;
    use gitbutler_stack::VirtualBranchesHandle;

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
        let stacks = but_workspace::stacks(&ctx.project().gb_dir())?;
        let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());

        Ok(TestContext {
            repo: gix::open_opts(&ctx.project().path, gix::open::Options::isolated())?,
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
        pub stacks_entries: Vec<but_workspace::StackEntry>,
        /// The tip of the local branch that tracks the upstream one.
        pub common_merge_base: gix::ObjectId,
    }
}

use crate::workspace_dependencies::util::worktree_ranges_digest_for_workspace_named;

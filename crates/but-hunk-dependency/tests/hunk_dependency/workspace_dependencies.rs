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
                            commit_id: Sha1(dd032c30828a4b01bda81345ceb868217e10be7e),
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
                            commit_id: Sha1(540b214204e6b4a89a0c46d4970fd751d3c8c4e3),
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
                            commit_id: Sha1(953c773f98972d6c6594fc74c5c39c1f90a9ec89),
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
                            commit_id: Sha1(18447fcdb2b577d16aa2aecedaabadaf9939fa6d),
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
                            commit_id: Sha1(67d6107c5b55c70045625a36dbad0d8d31229e83),
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
                            commit_id: Sha1(a2033dbf8d0cb460a48155fba36197c051b77fb7),
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
                            commit_id: Sha1(058c375fac02853aeba6913c45812fa45b64d073),
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
                            commit_id: Sha1(fe846061bc5c8b622373524a4ce2e30ce096c82f),
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
                            commit_id: Sha1(efe74eb074a9349f83162fc985b60ae4b294b806),
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
                            commit_id: Sha1(71c95acc33dec9ac32facd7b534fcdbea72b443c),
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
                            commit_id: Sha1(918040101954762e07c9c11be044373ec1b8674a),
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
                            commit_id: Sha1(c96f89b592c531f95f57fcb15edf627a606fb316),
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
                            commit_id: Sha1(9431eb7c2e667a5a9e493a6e4720f2eb8a1d69e5),
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
                            commit_id: Sha1(79af6a15f53e4048e1e86514ab44bc4e0507047b),
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
                            commit_id: Sha1(69c653c9427368006da50ef0dfc446aa845d5e79),
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
                            commit_id: Sha1(5cdf5f2b5738c7ba5ca26efff73502cc5aede13e),
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
                            commit_id: Sha1(f65a1007f48e2ab20dc286aa00889ba075cbc780),
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
                            commit_id: Sha1(3cd8b25ac64bd67773cc47d1f1995f02dfd7f1e9),
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
                            commit_id: Sha1(cb15b49f819e5ced052ba3371e7f0cade52cf5e0),
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
                            commit_id: Sha1(25121b40e27784892bffe7b94fd97b7d39f513d4),
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
                            commit_id: Sha1(63d8c3629fd435cc7bfbc80bcb7eb7a53e11c886),
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
                        commit_id: Sha1(7b47ea7b952d9a37f16b547eb406ee1274db6dd2),
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
                        change_type: Deletion,
                        commit_id: Sha1(1e9f6204003a49812f4bf993f567314810b2f14f),
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
    insta::assert_debug_snapshot!(actual, @r#"
    WorkspaceDigest {
        ranges_by_path: [
            (
                "file",
                [
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(f4c3a647443f90d704fcfad276d8462a3bec48d5),
                        start: 1,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(bc72ed73931238daaef3fee09b22c919b279b508),
                        start: 1,
                        lines: 2,
                        line_shift: 2,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(f4c3a647443f90d704fcfad276d8462a3bec48d5),
                        start: 4,
                        lines: 5,
                        line_shift: 5,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(322c7d36c24097e595a9578e46fdf0d1c514331e),
                        start: 9,
                        lines: 3,
                        line_shift: 3,
                    },
                ],
            ),
            (
                "file_2",
                [
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(bc72ed73931238daaef3fee09b22c919b279b508),
                        start: 1,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(8ad1b241738f9842a37160db2caf86df3c1cd7c4),
                        start: 2,
                        lines: 5,
                        line_shift: 5,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(6badf2d56db3944ea13240069b0c3b20a7b4a918),
                        start: 7,
                        lines: 0,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(8ad1b241738f9842a37160db2caf86df3c1cd7c4),
                        start: 7,
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
                        commit_id: Sha1(9a0811d534f44325970b0598712ff921db6369c9),
                        start: 1,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(bd10ed57558c598f6713a07508ce3b0206344109),
                        start: 1,
                        lines: 2,
                        line_shift: 2,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(9a0811d534f44325970b0598712ff921db6369c9),
                        start: 4,
                        lines: 5,
                        line_shift: 5,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(ab38338ac7f1546850b825e18cde50b8a1d3f7ab),
                        start: 9,
                        lines: 3,
                        line_shift: 3,
                    },
                ],
            ),
            (
                "file_2",
                [
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(bd10ed57558c598f6713a07508ce3b0206344109),
                        start: 1,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(26abc4794f2d99206c3fe4ae2bd67186600903c7),
                        start: 2,
                        lines: 5,
                        line_shift: 5,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(d320d3b1e9b847a644714a7b5d32ca08d1dc57c9),
                        start: 7,
                        lines: 0,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(26abc4794f2d99206c3fe4ae2bd67186600903c7),
                        start: 7,
                        lines: 1,
                        line_shift: 1,
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
                                commit_id: Sha1(bd10ed57558c598f6713a07508ce3b0206344109),
                                start: 1,
                                lines: 2,
                                line_shift: 2,
                            },
                            StableHunkRange {
                                change_type: Addition,
                                commit_id: Sha1(9a0811d534f44325970b0598712ff921db6369c9),
                                start: 4,
                                lines: 5,
                                line_shift: 5,
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
                                commit_id: Sha1(26abc4794f2d99206c3fe4ae2bd67186600903c7),
                                start: 2,
                                lines: 5,
                                line_shift: 5,
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
                        commit_id: Sha1(9eca7c76bff250f547c4b715a9a21a61578afece),
                        start: 1,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(abd0892c3cb5c110dc6112aa59990f412cc58b7f),
                        start: 1,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(9eca7c76bff250f547c4b715a9a21a61578afece),
                        start: 3,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(abd0892c3cb5c110dc6112aa59990f412cc58b7f),
                        start: 4,
                        lines: 0,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(689ad5ee589cb460c8bb7bcf785ab48c81e1c9c7),
                        start: 5,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(9eca7c76bff250f547c4b715a9a21a61578afece),
                        start: 6,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(abd0892c3cb5c110dc6112aa59990f412cc58b7f),
                        start: 7,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(3742937a251a2d7d239b6c04344b6e301c767901),
                        start: 8,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(9eca7c76bff250f547c4b715a9a21a61578afece),
                        start: 9,
                        lines: 2,
                        line_shift: 2,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(abd0892c3cb5c110dc6112aa59990f412cc58b7f),
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
                                commit_id: Sha1(2a49c842657487e64be1689a2661e339f8cb9732),
                                start: 3,
                                lines: 1,
                                line_shift: 1,
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
                                commit_id: Sha1(8161dd9c7e96717f0c180636171e5480f67e3896),
                                start: 7,
                                lines: 1,
                                line_shift: 1,
                            },
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(ae58594e5efee2b4241e543c20ffe6849e4eee30),
                                start: 8,
                                lines: 1,
                                line_shift: 1,
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
                                commit_id: Sha1(2a49c842657487e64be1689a2661e339f8cb9732),
                                start: 9,
                                lines: 2,
                                line_shift: 2,
                            },
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(8161dd9c7e96717f0c180636171e5480f67e3896),
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
                                commit_id: Sha1(8a7146c11d0ab9078ee88e52a9d692968dda6556),
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
                        line_shift: 1,
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
        let meta = but_meta::VirtualBranchesTomlMetadata::from_path(
            ctx.legacy_project.gb_dir().join("virtual_branches.toml"),
        )?;
        let stacks =
            but_workspace::legacy::stacks_v3(&*ctx.repo.get()?, &meta, Default::default(), None)?;
        let handle = VirtualBranchesHandle::new(ctx.project_data_dir());

        Ok(TestContext {
            repo: ctx.legacy_project.open_isolated_repo()?,
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

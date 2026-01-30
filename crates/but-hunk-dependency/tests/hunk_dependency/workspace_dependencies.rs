#[test]
fn every_commit_is_independent() -> anyhow::Result<()> {
    let actual = worktree_ranges_digest_for_workspace_separated("independent-commits");
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
                            commit_id: Sha1(01748f8d123aab24bf729a1809bcd82616f742bf),
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
                            commit_id: Sha1(a66fde9ea18488f3ab518bfbe261cd03af8334d2),
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
                            commit_id: Sha1(2d88e9209a4323b70b3ce93be39e073a0a695d4f),
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
                            commit_id: Sha1(ed0f7ae8f8603056a7045c400275800bb13d8dd3),
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
                            commit_id: Sha1(d1494ebe135156ea661cb8701d1b2da60fce665f),
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
                            commit_id: Sha1(bc1fd83f3dbc83becc18820a407e8d36efe3bba0),
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
    let actual = worktree_ranges_digest_for_workspace_separated("independent-commits-multi-stack");
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
                            commit_id: Sha1(01748f8d123aab24bf729a1809bcd82616f742bf),
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
                            commit_id: Sha1(a66fde9ea18488f3ab518bfbe261cd03af8334d2),
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
                            commit_id: Sha1(2d88e9209a4323b70b3ce93be39e073a0a695d4f),
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
                            commit_id: Sha1(ed0f7ae8f8603056a7045c400275800bb13d8dd3),
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
                            commit_id: Sha1(d1494ebe135156ea661cb8701d1b2da60fce665f),
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
                            commit_id: Sha1(bc1fd83f3dbc83becc18820a407e8d36efe3bba0),
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
                            commit_id: Sha1(42113c518333751f8bd156f9e5b985e878c1c645),
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
                            commit_id: Sha1(0551717004df34ad131b301a5fa8f88b9225a505),
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
                            commit_id: Sha1(9e10a08477f94790a594c998930020c0b54907b9),
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
                            commit_id: Sha1(856884376cde4aacfa9292392dff529b7ed857a2),
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
                            commit_id: Sha1(aa39024b201fb1e4d9a008026dfef52ac1c383e9),
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
                            commit_id: Sha1(5edb55a15807e0f9845b8b0f20365cc34650b33a),
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
    let actual = worktree_ranges_digest_for_workspace_separated("sequentially-dependent-commits");
    insta::assert_debug_snapshot!(actual, @r#"
    Ok(
        WorkspaceDigest {
            ranges_by_path: [
                (
                    "file",
                    [
                        StableHunkRange {
                            change_type: Modification,
                            commit_id: Sha1(724dc2ecec68013210a087d75914c3ddde47d4fb),
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
    let actual = worktree_ranges_digest_for_workspace_separated(
        "sequentially-dependent-commits-multi-stack",
    );
    insta::assert_debug_snapshot!(actual, @r#"
    Ok(
        WorkspaceDigest {
            ranges_by_path: [
                (
                    "file",
                    [
                        StableHunkRange {
                            change_type: Modification,
                            commit_id: Sha1(724dc2ecec68013210a087d75914c3ddde47d4fb),
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
                            commit_id: Sha1(81aa1e19a33a9c88e0895bd8a91a52898353e19f),
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
        worktree_ranges_digest_for_workspace_separated("delete-and-recreate-file-multi-stack")?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorkspaceDigest {
        ranges_by_path: [
            (
                "file",
                [
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(82b2fb0253a9017ca51819f40f19ab90f5294616),
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
                        commit_id: Sha1(71e78915f66bcfbdd6d577b194b8abb5307fba6b),
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
    let actual = worktree_ranges_digest_for_workspace_separated("complex-file-manipulation")?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorkspaceDigest {
        ranges_by_path: [
            (
                "file",
                [
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(9375fb38f57a4c4b1b27da267cf2ccc1b54dbfab),
                        start: 1,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(c85aaba55013536a1d44d8972d0b9fe9e484eb6f),
                        start: 1,
                        lines: 2,
                        line_shift: 2,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(9375fb38f57a4c4b1b27da267cf2ccc1b54dbfab),
                        start: 4,
                        lines: 5,
                        line_shift: 5,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(767d7bb203e81a98c87bbcb74f25cfcfc26f2115),
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
                        commit_id: Sha1(c85aaba55013536a1d44d8972d0b9fe9e484eb6f),
                        start: 1,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(109a5229203def5810b0e9fbaf053039f6d601b4),
                        start: 2,
                        lines: 5,
                        line_shift: 5,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(8c7e5c65e6fd926939ccecd709d7ea81286684f7),
                        start: 7,
                        lines: 0,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(109a5229203def5810b0e9fbaf053039f6d601b4),
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
                        commit_id: Sha1(9375fb38f57a4c4b1b27da267cf2ccc1b54dbfab),
                        start: 1,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(c85aaba55013536a1d44d8972d0b9fe9e484eb6f),
                        start: 1,
                        lines: 2,
                        line_shift: 2,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(9375fb38f57a4c4b1b27da267cf2ccc1b54dbfab),
                        start: 4,
                        lines: 5,
                        line_shift: 5,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(767d7bb203e81a98c87bbcb74f25cfcfc26f2115),
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
                        commit_id: Sha1(c85aaba55013536a1d44d8972d0b9fe9e484eb6f),
                        start: 1,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(109a5229203def5810b0e9fbaf053039f6d601b4),
                        start: 2,
                        lines: 5,
                        line_shift: 5,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(8c7e5c65e6fd926939ccecd709d7ea81286684f7),
                        start: 7,
                        lines: 0,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Addition,
                        commit_id: Sha1(109a5229203def5810b0e9fbaf053039f6d601b4),
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
                                change_type: Addition,
                                commit_id: Sha1(9375fb38f57a4c4b1b27da267cf2ccc1b54dbfab),
                                start: 1,
                                lines: 1,
                                line_shift: 0,
                            },
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(c85aaba55013536a1d44d8972d0b9fe9e484eb6f),
                                start: 1,
                                lines: 2,
                                line_shift: 2,
                            },
                            StableHunkRange {
                                change_type: Addition,
                                commit_id: Sha1(9375fb38f57a4c4b1b27da267cf2ccc1b54dbfab),
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
                                commit_id: Sha1(109a5229203def5810b0e9fbaf053039f6d601b4),
                                start: 2,
                                lines: 5,
                                line_shift: 5,
                            },
                            StableHunkRange {
                                change_type: Addition,
                                commit_id: Sha1(109a5229203def5810b0e9fbaf053039f6d601b4),
                                start: 7,
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
fn complex_file_manipulation_multiple_hunks() -> anyhow::Result<()> {
    let actual =
        worktree_ranges_digest_for_workspace_separated("complex-file-manipulation-multiple-hunks")?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorkspaceDigest {
        ranges_by_path: [
            (
                "file",
                [
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(9dd99b79687c4b67024d0855c08a6ad55bfa6632),
                        start: 1,
                        lines: 1,
                        line_shift: 0,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(9c7ad9af048c7b9ad1e84b0c17641347b84db1d1),
                        start: 1,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(9dd99b79687c4b67024d0855c08a6ad55bfa6632),
                        start: 3,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(9c7ad9af048c7b9ad1e84b0c17641347b84db1d1),
                        start: 4,
                        lines: 0,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(b200fb1a42de9c5fe4c1ac51239bc7ff47d287d8),
                        start: 5,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(9dd99b79687c4b67024d0855c08a6ad55bfa6632),
                        start: 6,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(9c7ad9af048c7b9ad1e84b0c17641347b84db1d1),
                        start: 7,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(500b70982f4576f707583412a28a6797211d180a),
                        start: 8,
                        lines: 1,
                        line_shift: 1,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(9dd99b79687c4b67024d0855c08a6ad55bfa6632),
                        start: 9,
                        lines: 2,
                        line_shift: 2,
                    },
                    StableHunkRange {
                        change_type: Modification,
                        commit_id: Sha1(9c7ad9af048c7b9ad1e84b0c17641347b84db1d1),
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
                                commit_id: Sha1(9dd99b79687c4b67024d0855c08a6ad55bfa6632),
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
                                commit_id: Sha1(9c7ad9af048c7b9ad1e84b0c17641347b84db1d1),
                                start: 7,
                                lines: 1,
                                line_shift: 1,
                            },
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(500b70982f4576f707583412a28a6797211d180a),
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
                                commit_id: Sha1(9dd99b79687c4b67024d0855c08a6ad55bfa6632),
                                start: 9,
                                lines: 2,
                                line_shift: 2,
                            },
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(9c7ad9af048c7b9ad1e84b0c17641347b84db1d1),
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
    let actual = worktree_ranges_digest_for_workspace_separated("merge-commit")?;

    insta::assert_debug_snapshot!(actual.partial(), @r#"
    WorkspaceWithoutRanges {
        intersections_by_path: [
            (
                "file",
                [
                    HunkIntersection {
                        hunk: DiffHunk("@@ -5,1 +5,1 @@
                        -update line 5
                        +update line 5 again
                        "),
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(e418c1bb8d15211edb026fa562fb4c83199d8481),
                                start: 5,
                                lines: 1,
                                line_shift: 0,
                            },
                        ],
                    },
                    HunkIntersection {
                        hunk: DiffHunk("@@ -8,1 +8,1 @@
                        -update line 8
                        +update line 8 again
                        "),
                        commit_intersections: [
                            StableHunkRange {
                                change_type: Modification,
                                commit_id: Sha1(df241a53d1a4dae37f16def5f6d9405ea236fbf6),
                                start: 8,
                                lines: 1,
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
fn dependencies_handle_complex_branch_checkout() -> anyhow::Result<()> {
    // This test ensures that checking out branches with *complex* histories
    // does not cause the dependency calculation to fail.
    //
    // The *complexity* of the branch is that it contains a merge commit from itself to itself,
    // mimicking checking-out a remote branch that had PRs merged to it.
    let actual = worktree_ranges_digest_for_workspace_separated("complex-branch-checkout")?;
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
    use but_ctx::Context;
    use but_testsupport::gix_testtools::tempfile;

    use crate::{WorkspaceDigest, intersect_workspace_ranges};

    pub fn worktree_ranges_digest_for_workspace_named(
        name: &str,
    ) -> anyhow::Result<WorkspaceDigest> {
        let test_ctx = test_ctx_separated(name)?;
        worktree_ranges_digest_for_workspace(&test_ctx.ctx)
    }

    pub fn worktree_ranges_digest_for_workspace_separated(
        name: &str,
    ) -> anyhow::Result<WorkspaceDigest> {
        let test_ctx = test_ctx_separated(name)?;
        worktree_ranges_digest_for_workspace(&test_ctx.ctx)
    }

    pub fn worktree_ranges_digest_for_workspace(ctx: &Context) -> anyhow::Result<WorkspaceDigest> {
        let (_guard, repo, ws, _) = ctx.workspace_and_db()?;
        let input_stacks = but_hunk_dependency::new_stacks_to_input_stacks(&repo, &ws)?;
        let ranges = but_hunk_dependency::WorkspaceRanges::try_from_stacks(input_stacks)?;
        let worktree_changes = but_core::diff::worktree_changes(&repo)?.changes;
        intersect_workspace_ranges(&repo, ranges, worktree_changes)
    }

    fn test_scenario(name: &str) -> anyhow::Result<TestContext> {
        // TODO: make this a read-only scenario once we don't rely on vb.toml anymore.
        let (repo, tmpdir) = but_testsupport::writable_scenario(name);
        let ctx = but_ctx::Context::from_repo(repo)?;

        Ok(TestContext {
            ctx,
            tmpdir: Some(tmpdir),
        })
    }

    pub fn test_ctx_separated(name: &str) -> anyhow::Result<TestContext> {
        test_scenario(name)
    }

    pub struct TestContext {
        pub ctx: Context,
        // TODO: make non-optional
        // TODO: remove once we don't need vb.toml anymore which is produced on the fly.
        #[allow(dead_code)]
        tmpdir: Option<tempfile::TempDir>,
    }
}

use crate::workspace_dependencies::util::{
    worktree_ranges_digest_for_workspace_named, worktree_ranges_digest_for_workspace_separated,
};

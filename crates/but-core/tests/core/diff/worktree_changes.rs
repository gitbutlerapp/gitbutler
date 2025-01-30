use anyhow::Result;
use but_core::diff;
use but_core::{UnifiedDiff, WorktreeChanges};

#[test]
#[cfg(unix)]
fn non_files_are_ignored() -> Result<()> {
    let repo = repo_unix("untracked-fifo")?;
    let actual = diff::worktree_changes(&repo)?;
    assert_eq!(
        actual.changes.len(),
        0,
        "FIFOs don't even show up and are thus completely ignored"
    );
    assert_eq!(
        actual.ignored_changes.len(),
        0,
        "But they are not made visible in any way either"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn executable_bit_added_in_worktree() -> Result<()> {
    let repo = repo_unix("add-executable-bit-in-worktree")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "exe",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: BlobExecutable,
                    },
                    flags: Some(
                        ExecutableBitAdded,
                    ),
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r"
    [
        Patch {
            hunks: [],
        },
    ]
    ");
    Ok(())
}

#[test]
#[cfg(unix)]
fn executable_bit_removed_in_worktree() -> Result<()> {
    let repo = repo_unix("remove-executable-bit-in-worktree")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "exe",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: BlobExecutable,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    flags: Some(
                        ExecutableBitRemoved,
                    ),
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r"
    [
        Patch {
            hunks: [],
        },
    ]
    ");
    Ok(())
}

#[test]
#[cfg(unix)]
fn executable_bit_removed_in_index() -> Result<()> {
    let repo = repo_unix("remove-executable-bit-in-index")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "exe",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: BlobExecutable,
                    },
                    state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    flags: Some(
                        ExecutableBitRemoved,
                    ),
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r"
    [
        Patch {
            hunks: [],
        },
    ]
    ");
    Ok(())
}

#[test]
#[cfg(unix)]
fn executable_bit_added_in_index() -> Result<()> {
    let repo = repo_unix("add-executable-bit-in-index")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "exe",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: BlobExecutable,
                    },
                    flags: Some(
                        ExecutableBitAdded,
                    ),
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r"
    [
        Patch {
            hunks: [],
        },
    ]
    ");
    Ok(())
}

#[test]
fn untracked_in_unborn() -> Result<()> {
    let repo = repo("untracked-unborn")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "untracked",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    is_untracked: true,
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r"
    [
        Patch {
            hunks: [],
        },
    ]
    ");
    Ok(())
}

#[test]
fn added_in_unborn() -> Result<()> {
    let repo = repo("added-unborn")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "untracked",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    is_untracked: false,
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r"
    [
        Patch {
            hunks: [],
        },
    ]
    ");
    Ok(())
}

#[test]
fn submodule_added_in_unborn() -> Result<()> {
    let repo = repo("submodule-added-unborn")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: ".gitmodules",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(46f8c8b821d79a888a1ea0b30ec9f5d7e90821b0),
                        kind: Blob,
                    },
                    is_untracked: false,
                },
            },
            TreeChange {
                path: "submodule",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(e95516bd2f49a83a6cdb98cfec40b2717fbc2c1b),
                        kind: Commit,
                    },
                    is_untracked: false,
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    assert_eq!(
        unified_diffs(actual, &repo).unwrap_err().to_string(),
        "Can only diff blobs and links, not Commit"
    );
    Ok(())
}

#[test]
fn submodule_changed_head() -> Result<()> {
    let repo = repo("submodule-changed-head")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "submodule",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(e95516bd2f49a83a6cdb98cfec40b2717fbc2c1b),
                        kind: Commit,
                    },
                    state: ChangeState {
                        id: Sha1(800a5398d76f28db44bc976b561d8885687fd1b6),
                        kind: Commit,
                    },
                    flags: None,
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    assert_eq!(
        unified_diffs(actual, &repo).unwrap_err().to_string(),
        "Can only diff blobs and links, not Commit"
    );
    Ok(())
}

#[test]
fn case_folding_worktree_changes() -> Result<()> {
    let repo = repo("case-folding-worktree-changes")?;
    if !gix::fs::Capabilities::probe(repo.git_dir()).ignore_case {
        return Ok(());
    }
    let actual = diff::worktree_changes(&repo)?;
    // This gives the strange situation that the file seems to have changed because it compares `FILE`
    // to `file` that is actually checked out on disk.
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "FILE",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r#"
    [
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 0,
                    diff: "@@ -1,1 +1,0 @@\n-content\n",
                },
            ],
        },
    ]
    "#);
    Ok(())
}

#[test]
fn case_folding_worktree_and_index_changes() -> Result<()> {
    let repo = repo("case-folding-worktree-and-index-changes")?;
    if !gix::fs::Capabilities::probe(repo.git_dir()).ignore_case {
        return Ok(());
    }
    let actual = diff::worktree_changes(&repo)?;
    // Here we TreeChange `FILE` to be empty, and add that TreeChange to the index. This shows up as expected.
    // This also means that now `FILE` is compared against `file` on disk which happens to be empty too,
    // so no worktree TreeChange shows up.
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "FILE",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r#"
    [
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 0,
                    diff: "@@ -1,1 +1,0 @@\n-content\n",
                },
            ],
        },
    ]
    "#);
    Ok(())
}

#[test]
fn file_to_dir_in_worktree() -> Result<()> {
    let repo = repo("file-to-dir-in-worktree")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "file-then-dir",
                status: Deletion {
                    previous_state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                },
            },
            TreeChange {
                path: "file-then-dir/new-file",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    is_untracked: true,
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r#"
    [
        Patch {
            hunks: [],
        },
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 0,
                    new_start: 1,
                    new_lines: 1,
                    diff: "@@ -1,0 +1,1 @@\n+content\n",
                },
            ],
        },
    ]
    "#);
    Ok(())
}

#[test]
fn file_to_dir_in_index() -> Result<()> {
    let repo = repo("file-to-dir-in-index")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "file-then-dir",
                status: Deletion {
                    previous_state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                },
            },
            TreeChange {
                path: "file-then-dir/new-file",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    is_untracked: false,
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r#"
    [
        Patch {
            hunks: [],
        },
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 0,
                    new_start: 1,
                    new_lines: 1,
                    diff: "@@ -1,0 +1,1 @@\n+content\n",
                },
            ],
        },
    ]
    "#);
    Ok(())
}

#[test]
fn dir_to_file_in_worktree() -> Result<()> {
    let repo = repo("dir-to-file-in-worktree")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "dir-soon-file",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    is_untracked: true,
                },
            },
            TreeChange {
                path: "dir-soon-file/file",
                status: Deletion {
                    previous_state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r#"
    [
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 0,
                    new_start: 1,
                    new_lines: 1,
                    diff: "@@ -1,0 +1,1 @@\n+content\n",
                },
            ],
        },
        Patch {
            hunks: [],
        },
    ]
    "#);
    Ok(())
}

#[test]
fn dir_to_file_in_index() -> Result<()> {
    let repo = repo("dir-to-file-in-index")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "dir-soon-file",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    is_untracked: false,
                },
            },
            TreeChange {
                path: "dir-soon-file/file",
                status: Deletion {
                    previous_state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r#"
    [
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 0,
                    new_start: 1,
                    new_lines: 1,
                    diff: "@@ -1,0 +1,1 @@\n+content\n",
                },
            ],
        },
        Patch {
            hunks: [],
        },
    ]
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn file_to_symlink_in_worktree() -> Result<()> {
    let repo = repo_unix("file-to-symlink-in-worktree")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "file-soon-symlink",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Link,
                    },
                    flags: Some(
                        TypeChangeFileToLink,
                    ),
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r#"
    [
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 1,
                    diff: "@@ -1,1 +1,1 @@\n-content\n+does-not-exist\n",
                },
            ],
        },
    ]
    "#);
    Ok(())
}

#[test]
fn conflict() -> Result<()> {
    let repo = repo("conflicting")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [],
        ignored_changes: [
            IgnoredWorktreeChange {
                path: "conflicting",
                status: Conflict,
            },
        ],
    }
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn file_to_symlink_in_index() -> Result<()> {
    let repo = repo_unix("file-to-symlink-in-index")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "file-soon-symlink",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(cfa0a46515b5e7117875427e7bb0480066d2e380),
                        kind: Link,
                    },
                    flags: Some(
                        TypeChangeFileToLink,
                    ),
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r#"
    [
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 1,
                    diff: "@@ -1,1 +1,1 @@\n-content\n+does-not-exist\n",
                },
            ],
        },
    ]
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn symlink_to_file_in_worktree() -> Result<()> {
    let repo = repo_unix("symlink-to-file-in-worktree")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "symlink-soon-file",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(1de565933b05f74c75ff9a6520af5f9f8a5a2f1d),
                        kind: Link,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    flags: Some(
                        TypeChangeLinkToFile,
                    ),
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r#"
    [
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 1,
                    diff: "@@ -1,1 +1,1 @@\n-target\n+content\n",
                },
            ],
        },
    ]
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn symlink_to_file_in_index() -> Result<()> {
    let repo = repo_unix("symlink-to-file-in-index")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "symlink-soon-file",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(1de565933b05f74c75ff9a6520af5f9f8a5a2f1d),
                        kind: Link,
                    },
                    state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    flags: Some(
                        TypeChangeLinkToFile,
                    ),
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r#"
    [
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 1,
                    diff: "@@ -1,1 +1,1 @@\n-target\n+content\n",
                },
            ],
        },
    ]
    "#);
    Ok(())
}

#[test]
fn added_modified_in_worktree() -> Result<()> {
    let repo = repo("added-modified-in-worktree")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "added",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    is_untracked: false,
                },
            },
            TreeChange {
                path: "intent-to-add",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
            TreeChange {
                path: "modified",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(deba01fc8d98200761c46eb139f11ac244cf6eb5),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r#"
    [
        Patch {
            hunks: [],
        },
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 0,
                    new_start: 1,
                    new_lines: 1,
                    diff: "@@ -1,0 +1,1 @@\n+content\n",
                },
            ],
        },
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 1,
                    diff: "@@ -1,1 +1,1 @@\n-something\n+change\n",
                },
            ],
        },
    ]
    "#);
    Ok(())
}

#[test]
fn modified_in_index() -> Result<()> {
    let repo = repo("modified-in-index")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "modified",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(deba01fc8d98200761c46eb139f11ac244cf6eb5),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0835e4f9714005ed591f68d306eea0d6d2ae8fd7),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r#"
    [
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 1,
                    diff: "@@ -1,1 +1,1 @@\n-something\n+change\n",
                },
            ],
        },
    ]
    "#);
    Ok(())
}

#[test]
fn deleted_in_worktree() -> Result<()> {
    let repo = repo("deleted-in-worktree")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "deleted",
                status: Deletion {
                    previous_state: ChangeState {
                        id: Sha1(deba01fc8d98200761c46eb139f11ac244cf6eb5),
                        kind: Blob,
                    },
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r#"
    [
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 0,
                    diff: "@@ -1,1 +1,0 @@\n-something\n",
                },
            ],
        },
    ]
    "#);
    Ok(())
}

#[test]
fn deleted_in_index() -> Result<()> {
    let repo = repo("deleted-in-index")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "deleted",
                status: Deletion {
                    previous_state: ChangeState {
                        id: Sha1(deba01fc8d98200761c46eb139f11ac244cf6eb5),
                        kind: Blob,
                    },
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r#"
    [
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 0,
                    diff: "@@ -1,1 +1,0 @@\n-something\n",
                },
            ],
        },
    ]
    "#);
    Ok(())
}

#[test]
fn renamed_in_index() -> Result<()> {
    let repo = repo("renamed-in-index")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "new-name",
                status: Rename {
                    previous_path: "to-be-renamed",
                    previous_state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r"
    [
        Patch {
            hunks: [],
        },
    ]
    ");
    Ok(())
}

#[test]
fn renamed_in_worktree() -> Result<()> {
    let repo = repo("renamed-in-worktree")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "new-name",
                status: Rename {
                    previous_path: "to-be-renamed",
                    previous_state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
        ],
        ignored_changes: [],
    }
    "#);
    insta::assert_debug_snapshot!(unified_diffs(actual, &repo)?, @r"
    [
        Patch {
            hunks: [],
        },
    ]
    ");
    Ok(())
}

#[test]
fn modified_in_index_and_workingtree() -> Result<()> {
    let repo = repo("modified-in-index-and-worktree")?;
    let actual = diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "dual-modified",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(8ea0713f9d637081cc0098035465c365c0c32949),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
        ],
        ignored_changes: [
            IgnoredWorktreeChange {
                path: "dual-modified",
                status: TreeIndex,
            },
        ],
    }
    "#);

    let actual = unified_diffs(actual, &repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        Patch {
            hunks: [
                DiffHunk {
                    old_start: 1,
                    old_lines: 2,
                    new_start: 1,
                    new_lines: 3,
                    diff: "@@ -1,2 +1,3 @@\n initial\n change\n+second-change\n",
                },
            ],
        },
    ]
    "#);
    let [UnifiedDiff::Patch { hunks }] = &actual[..] else {
        unreachable!("need hunks")
    };
    // newlines at the end should work.
    insta::assert_snapshot!(hunks[0].diff, @r"
    @@ -1,2 +1,3 @@
     initial
     change
    +second-change
    ");
    Ok(())
}

fn unified_diffs(
    worktree: WorktreeChanges,
    repo: &gix::Repository,
) -> anyhow::Result<Vec<UnifiedDiff>> {
    worktree
        .changes
        .into_iter()
        .map(|c| c.unified_diff(repo))
        .collect()
}

pub fn repo(fixture_name: &str) -> anyhow::Result<gix::Repository> {
    let root = gix_testtools::scripted_fixture_read_only("worktree-changes.sh")
        .map_err(anyhow::Error::from_boxed)?;
    let worktree_root = root.join(fixture_name);
    Ok(gix::open(worktree_root)?)
}

pub fn repo_unix(fixture_name: &str) -> anyhow::Result<gix::Repository> {
    let root = gix_testtools::scripted_fixture_read_only("worktree-changes-unix.sh")
        .map_err(anyhow::Error::from_boxed)?;
    let worktree_root = root.join(fixture_name);
    Ok(gix::open(worktree_root)?)
}

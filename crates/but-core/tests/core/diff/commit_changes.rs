use crate::commit::conflict_repo;
use crate::diff::ui::repo;
use crate::diff::unified_diffs;

#[test]
fn many_changes() -> anyhow::Result<()> {
    let repo = repo("many-in-tree")?;
    let previous_commit_id = repo.rev_parse_single("@~1")?;
    let current_commit_id = repo.rev_parse_single("@")?;
    let changes = but_core::diff::tree_changes(
        &repo,
        Some(previous_commit_id.into()),
        current_commit_id.into(),
    )?;
    insta::assert_debug_snapshot!(changes, @r#"
    (
        [
            TreeChange {
                path: "aa-renamed-new-name",
                status: Rename {
                    previous_path: "aa-renamed-old-name",
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
            TreeChange {
                path: "executable-bit-added",
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
            TreeChange {
                path: "file-to-link",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(7ad106d48bf91c7ef87a38db2397b661a50102f5),
                        kind: Link,
                    },
                    flags: Some(
                        TypeChangeFileToLink,
                    ),
                },
            },
            TreeChange {
                path: "modified",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0835e4f9714005ed591f68d306eea0d6d2ae8fd7),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
            TreeChange {
                path: "removed",
                status: Deletion {
                    previous_state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                },
            },
        ],
        Stats {
            lines_added: 2,
            lines_removed: 0,
            files_changed: 5,
        },
    )
    "#);

    insta::assert_debug_snapshot!(unified_diffs(changes.0, &repo)?, @r#"
    [
        Patch {
            hunks: [],
            is_result_of_binary_to_text_conversion: false,
            lines_added: 0,
            lines_removed: 0,
        },
        Patch {
            hunks: [],
            is_result_of_binary_to_text_conversion: false,
            lines_added: 0,
            lines_removed: 0,
        },
        Patch {
            hunks: [
                DiffHunk("@@ -1,0 +1,1 @@
                +link-target
                "),
            ],
            is_result_of_binary_to_text_conversion: false,
            lines_added: 1,
            lines_removed: 0,
        },
        Patch {
            hunks: [
                DiffHunk("@@ -1,0 +1,1 @@
                +change
                "),
            ],
            is_result_of_binary_to_text_conversion: false,
            lines_added: 1,
            lines_removed: 0,
        },
        Patch {
            hunks: [],
            is_result_of_binary_to_text_conversion: false,
            lines_added: 0,
            lines_removed: 0,
        },
    ]
    "#);

    let changes = but_core::diff::tree_changes(
        &repo,
        Some(current_commit_id.into()),
        previous_commit_id.into(),
    )?;
    insta::assert_debug_snapshot!(changes, @r#"
    (
        [
            TreeChange {
                path: "aa-renamed-old-name",
                status: Rename {
                    previous_path: "aa-renamed-new-name",
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
            TreeChange {
                path: "executable-bit-added",
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
            TreeChange {
                path: "file-to-link",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(7ad106d48bf91c7ef87a38db2397b661a50102f5),
                        kind: Link,
                    },
                    state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    flags: Some(
                        TypeChangeLinkToFile,
                    ),
                },
            },
            TreeChange {
                path: "modified",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(0835e4f9714005ed591f68d306eea0d6d2ae8fd7),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
            TreeChange {
                path: "removed",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    is_untracked: false,
                },
            },
        ],
        Stats {
            lines_added: 0,
            lines_removed: 2,
            files_changed: 5,
        },
    )
    "#);

    insta::assert_debug_snapshot!(unified_diffs(changes.0, &repo)?, @r#"
    [
        Patch {
            hunks: [],
            is_result_of_binary_to_text_conversion: false,
            lines_added: 0,
            lines_removed: 0,
        },
        Patch {
            hunks: [],
            is_result_of_binary_to_text_conversion: false,
            lines_added: 0,
            lines_removed: 0,
        },
        Patch {
            hunks: [
                DiffHunk("@@ -1,1 +1,0 @@
                -link-target
                "),
            ],
            is_result_of_binary_to_text_conversion: false,
            lines_added: 0,
            lines_removed: 1,
        },
        Patch {
            hunks: [
                DiffHunk("@@ -1,1 +1,0 @@
                -change
                "),
            ],
            is_result_of_binary_to_text_conversion: false,
            lines_added: 0,
            lines_removed: 1,
        },
        Patch {
            hunks: [],
            is_result_of_binary_to_text_conversion: false,
            lines_added: 0,
            lines_removed: 0,
        },
    ]
    "#);
    Ok(())
}

#[test]
fn many_changes_without_symlink_support() -> anyhow::Result<()> {
    let mut repo = repo("many-in-tree")?;
    let previous_commit_id = repo.rev_parse_single("@~1")?;
    let current_commit_id = repo.rev_parse_single("@")?;
    let changes = but_core::diff::tree_changes(
        &repo,
        Some(previous_commit_id.into()),
        current_commit_id.into(),
    )?;

    // When symlinks are disabled, no diff is produced, even though `gitoxide` will error.
    repo.config_snapshot_mut()
        .set_value(&gix::config::tree::Core::SYMLINKS, "false")?;
    let change = &changes.0[2];
    insta::assert_debug_snapshot!(change, @r#"
    TreeChange {
        path: "file-to-link",
        status: Modification {
            previous_state: ChangeState {
                id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                kind: Blob,
            },
            state: ChangeState {
                id: Sha1(7ad106d48bf91c7ef87a38db2397b661a50102f5),
                kind: Link,
            },
            flags: Some(
                TypeChangeFileToLink,
            ),
        },
    }
    "#);
    // It can do it as long as the symlink isn't on disk.
    insta::assert_debug_snapshot!(change.unified_diff(&repo, 3)?, @r#"
    Some(
        Patch {
            hunks: [
                DiffHunk("@@ -1,0 +1,1 @@
                +link-target
                "),
            ],
            is_result_of_binary_to_text_conversion: false,
            lines_added: 1,
            lines_removed: 0,
        },
    )
    "#);
    Ok(())
}

#[test]
fn without_previous_tree() -> anyhow::Result<()> {
    let repo = repo("many-in-tree")?;
    let current_tree_id = repo.rev_parse_single("@^1")?;
    let changes = but_core::diff::tree_changes(&repo, None, current_tree_id.into())?;
    insta::assert_debug_snapshot!(changes, @r#"
    (
        [
            TreeChange {
                path: "aa-renamed-old-name",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    is_untracked: false,
                },
            },
            TreeChange {
                path: "dir/nested",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    is_untracked: false,
                },
            },
            TreeChange {
                path: "executable-bit-added",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    is_untracked: false,
                },
            },
            TreeChange {
                path: "file-to-link",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    is_untracked: false,
                },
            },
            TreeChange {
                path: "modified",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    is_untracked: false,
                },
            },
            TreeChange {
                path: "removed",
                status: Addition {
                    state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    is_untracked: false,
                },
            },
        ],
        Stats {
            lines_added: 1,
            lines_removed: 0,
            files_changed: 6,
        },
    )
    "#);
    Ok(())
}

#[test]
fn changes_between_conflicted_and_normal_commit() -> anyhow::Result<()> {
    let repo = conflict_repo("normal-and-artificial")?;
    let changes = but_core::diff::tree_changes(
        &repo,
        Some(repo.rev_parse_single("normal")?.into()),
        repo.rev_parse_single("conflicted")?.into(),
    )?;
    insta::assert_debug_snapshot!(changes, @r#"
    (
        [
            TreeChange {
                path: "file",
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
        Stats {
            lines_added: 0,
            lines_removed: 1,
            files_changed: 1,
        },
    )
    "#);
    Ok(())
}

#[test]
fn changes_between_conflicted_and_conflicted_commit() -> anyhow::Result<()> {
    let repo = conflict_repo("normal-and-artificial")?;
    let changes = but_core::diff::tree_changes(
        &repo,
        Some(repo.rev_parse_single("conflicted")?.into()),
        repo.rev_parse_single("conflicted")?.into(),
    )?;
    insta::assert_debug_snapshot!(changes, @r"
    (
        [],
        Stats {
            lines_added: 0,
            lines_removed: 0,
            files_changed: 0,
        },
    )
    ");
    Ok(())
}

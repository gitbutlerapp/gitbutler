use anyhow::Result;
use but_core::worktree::changes;

#[test]
#[cfg(unix)]
fn non_files_are_ignored() -> Result<()> {
    let repo = repo_unix("untracked-fifo")?;
    let actual = changes(&repo)?;
    assert_eq!(
        actual.len(),
        0,
        "FIFOs don't even show up and are thus completely ignored"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn executable_bit_added_in_worktree() -> Result<()> {
    let repo = repo_unix("add-executable-bit-in-worktree")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "exe",
            status: Modification {
                origin: IndexWorktree,
                previous_state: ChangeState {
                    id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: BlobExecutable,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn executable_bit_removed_in_worktree() -> Result<()> {
    let repo = repo_unix("remove-executable-bit-in-worktree")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "exe",
            status: Modification {
                origin: IndexWorktree,
                previous_state: ChangeState {
                    id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                    kind: BlobExecutable,
                },
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn executable_bit_removed_in_index() -> Result<()> {
    let repo = repo_unix("remove-executable-bit-in-index")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "exe",
            status: Modification {
                origin: TreeIndex,
                previous_state: ChangeState {
                    id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                    kind: BlobExecutable,
                },
                state: ChangeState {
                    id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn executable_bit_added_in_index() -> Result<()> {
    let repo = repo_unix("add-executable-bit-in-index")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "exe",
            status: Modification {
                origin: TreeIndex,
                previous_state: ChangeState {
                    id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                    kind: BlobExecutable,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn untracked_in_unborn() -> Result<()> {
    let repo = repo("untracked-unborn")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "untracked",
            status: Untracked {
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn added_in_unborn() -> Result<()> {
    let repo = repo("added-unborn")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "untracked",
            status: Addition {
                origin: TreeIndex,
                state: ChangeState {
                    id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn submodule_added_in_unborn() -> Result<()> {
    let repo = repo("submodule-added-unborn")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: ".gitmodules",
            status: Addition {
                origin: TreeIndex,
                state: ChangeState {
                    id: Sha1(46f8c8b821d79a888a1ea0b30ec9f5d7e90821b0),
                    kind: Blob,
                },
            },
        },
        TreeChange {
            path: "submodule",
            status: Addition {
                origin: TreeIndex,
                state: ChangeState {
                    id: Sha1(e95516bd2f49a83a6cdb98cfec40b2717fbc2c1b),
                    kind: Commit,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn submodule_changed_head() -> Result<()> {
    let repo = repo("submodule-changed-head")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "submodule",
            status: Modification {
                origin: IndexWorktree,
                previous_state: ChangeState {
                    id: Sha1(e95516bd2f49a83a6cdb98cfec40b2717fbc2c1b),
                    kind: Commit,
                },
                state: ChangeState {
                    id: Sha1(800a5398d76f28db44bc976b561d8885687fd1b6),
                    kind: Commit,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn case_folding_worktree_changes() -> Result<()> {
    let repo = repo("case-folding-worktree-changes")?;
    if !gix::fs::Capabilities::probe(repo.git_dir()).ignore_case {
        return Ok(());
    }
    let actual = changes(&repo)?;
    // This gives the strange situation that the file seems to have changed because it compares `FILE`
    // to `file` that is actually checked out on disk.
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "FILE",
            status: Modification {
                origin: IndexWorktree,
                previous_state: ChangeState {
                    id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: Blob,
                },
            },
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
    let actual = changes(&repo)?;
    // Here we TreeChange `FILE` to be empty, and add that TreeChange to the index. This shows up as expected.
    // This also means that now `FILE` is compared against `file` on disk which happens to be empty too,
    // so no worktree TreeChange shows up.
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "FILE",
            status: Modification {
                origin: TreeIndex,
                previous_state: ChangeState {
                    id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn file_to_dir_in_worktree() -> Result<()> {
    let repo = repo("file-to-dir-in-worktree")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "file-then-dir",
            status: Deletion {
                origin: IndexWorktree,
                previous_state: ChangeState {
                    id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                    kind: Blob,
                },
            },
        },
        TreeChange {
            path: "file-then-dir/new-file",
            status: Untracked {
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn file_to_dir_in_index() -> Result<()> {
    let repo = repo("file-to-dir-in-index")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "file-then-dir",
            status: Deletion {
                origin: TreeIndex,
                previous_state: ChangeState {
                    id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                    kind: Blob,
                },
            },
        },
        TreeChange {
            path: "file-then-dir/new-file",
            status: Addition {
                origin: TreeIndex,
                state: ChangeState {
                    id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn dir_to_file_in_worktree() -> Result<()> {
    let repo = repo("dir-to-file-in-worktree")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "dir-soon-file",
            status: Untracked {
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: Blob,
                },
            },
        },
        TreeChange {
            path: "dir-soon-file/file",
            status: Deletion {
                origin: IndexWorktree,
                previous_state: ChangeState {
                    id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn dir_to_file_in_index() -> Result<()> {
    let repo = repo("dir-to-file-in-index")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "dir-soon-file",
            status: Addition {
                origin: TreeIndex,
                state: ChangeState {
                    id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                    kind: Blob,
                },
            },
        },
        TreeChange {
            path: "dir-soon-file/file",
            status: Deletion {
                origin: TreeIndex,
                previous_state: ChangeState {
                    id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn file_to_symlink_in_worktree() -> Result<()> {
    let repo = repo_unix("file-to-symlink-in-worktree")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "file-soon-symlink",
            status: Modification {
                origin: IndexWorktree,
                previous_state: ChangeState {
                    id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: Link,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn file_to_symlink_in_index() -> Result<()> {
    let repo = repo_unix("file-to-symlink-in-index")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "file-soon-symlink",
            status: Modification {
                origin: TreeIndex,
                previous_state: ChangeState {
                    id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(cfa0a46515b5e7117875427e7bb0480066d2e380),
                    kind: Link,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn symlink_to_file_in_worktree() -> Result<()> {
    let repo = repo_unix("symlink-to-file-in-worktree")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "symlink-soon-file",
            status: Modification {
                origin: IndexWorktree,
                previous_state: ChangeState {
                    id: Sha1(1de565933b05f74c75ff9a6520af5f9f8a5a2f1d),
                    kind: Link,
                },
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn symlink_to_file_in_index() -> Result<()> {
    let repo = repo_unix("symlink-to-file-in-index")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "symlink-soon-file",
            status: Modification {
                origin: TreeIndex,
                previous_state: ChangeState {
                    id: Sha1(1de565933b05f74c75ff9a6520af5f9f8a5a2f1d),
                    kind: Link,
                },
                state: ChangeState {
                    id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn added_modified_in_worktree() -> Result<()> {
    let repo = repo("added-modified-in-worktree")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "added",
            status: Addition {
                origin: TreeIndex,
                state: ChangeState {
                    id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                    kind: Blob,
                },
            },
        },
        TreeChange {
            path: "intent-to-add",
            status: Modification {
                origin: IndexWorktree,
                previous_state: ChangeState {
                    id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: Blob,
                },
            },
        },
        TreeChange {
            path: "modified",
            status: Modification {
                origin: IndexWorktree,
                previous_state: ChangeState {
                    id: Sha1(deba01fc8d98200761c46eb139f11ac244cf6eb5),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn modified_in_index() -> Result<()> {
    let repo = repo("modified-in-index")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "modified",
            status: Modification {
                origin: TreeIndex,
                previous_state: ChangeState {
                    id: Sha1(deba01fc8d98200761c46eb139f11ac244cf6eb5),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(0835e4f9714005ed591f68d306eea0d6d2ae8fd7),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn deleted_in_worktree() -> Result<()> {
    let repo = repo("deleted-in-worktree")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "deleted",
            status: Deletion {
                origin: IndexWorktree,
                previous_state: ChangeState {
                    id: Sha1(deba01fc8d98200761c46eb139f11ac244cf6eb5),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn deleted_in_index() -> Result<()> {
    let repo = repo("deleted-in-index")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "deleted",
            status: Deletion {
                origin: TreeIndex,
                previous_state: ChangeState {
                    id: Sha1(deba01fc8d98200761c46eb139f11ac244cf6eb5),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn renamed_in_index() -> Result<()> {
    let repo = repo("renamed-in-index")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "new-name",
            status: Rename {
                origin: TreeIndex,
                previous_path: "to-be-renamed",
                previous_state: ChangeState {
                    id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn renamed_in_worktree() -> Result<()> {
    let repo = repo("renamed-in-worktree")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "new-name",
            status: Rename {
                origin: IndexWorktree,
                previous_path: "to-be-renamed",
                previous_state: ChangeState {
                    id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

#[test]
fn modified_in_index_and_workingtree() -> Result<()> {
    let repo = repo("modified-in-index-and-worktree")?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        TreeChange {
            path: "dual-modified",
            status: Modification {
                origin: IndexWorktree,
                previous_state: ChangeState {
                    id: Sha1(8ea0713f9d637081cc0098035465c365c0c32949),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: Blob,
                },
            },
        },
        TreeChange {
            path: "dual-modified",
            status: Modification {
                origin: TreeIndex,
                previous_state: ChangeState {
                    id: Sha1(e79c5e8f964493290a409888d5413a737e8e5dd5),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(8ea0713f9d637081cc0098035465c365c0c32949),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}

fn repo(fixture_name: &str) -> anyhow::Result<gix::Repository> {
    let root = gix_testtools::scripted_fixture_read_only("worktree-changes.sh")
        .map_err(anyhow::Error::from_boxed)?;
    let worktree_root = root.join(fixture_name);
    Ok(gix::open(worktree_root)?)
}

fn repo_unix(fixture_name: &str) -> anyhow::Result<gix::Repository> {
    let root = gix_testtools::scripted_fixture_read_only("worktree-changes-unix.sh")
        .map_err(anyhow::Error::from_boxed)?;
    let worktree_root = root.join(fixture_name);
    Ok(gix::open(worktree_root)?)
}

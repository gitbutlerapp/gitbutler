use but_core::{ChangeState, UnifiedPatch, unified_diff};
use gix::object::tree::EntryKind;
use snapbox::prelude::*;

#[test]
fn file_added_in_worktree() -> anyhow::Result<()> {
    let repo = crate::diff::worktree_changes::repo("added-modified-in-worktree")?;
    let actual = extract_patch(UnifiedPatch::compute(
        &repo,
        "modified".into(),
        None,
        ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Blob,
        },
        None,
        3,
    )?);

    snapbox::assert_data_eq!(
        actual.to_debug(),
        snapbox::str![[r#"
[
    DiffHunk("@@ -1,0 +1,1 @@
    +change
    "),
]

"#]]
    );
    Ok(())
}

#[test]
fn binary_text_failure() -> anyhow::Result<()> {
    let repo = crate::diff::worktree_changes::repo("diff-binary-to-text-failure")?;
    let actual = UnifiedPatch::compute(
        &repo,
        "file.binary".into(),
        None,
        ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Blob,
        },
        None,
        3,
    )?;

    assert!(
        actual.is_none(),
        "any kind of filter failure is ignored for resiliency"
    );

    let mut filter = repo.diff_resource_cache(
        UnifiedPatch::CONVERSION_MODE,
        gix::diff::blob::pipeline::WorktreeRoots {
            old_root: repo.workdir().map(ToOwned::to_owned),
            new_root: None,
        },
    )?;
    let actual = UnifiedPatch::compute_with_filter(
        &repo,
        "file.binary".into(),
        None,
        None,
        ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Blob,
        },
        3,
        &mut filter,
    )?;
    assert!(
        actual.is_none(),
        "filter failures on the previous file are also ignored"
    );
    Ok(())
}

#[test]
fn file_disappeared_before_diffing() -> anyhow::Result<()> {
    let repo = crate::diff::worktree_changes::repo("added-modified-in-worktree")?;
    let actual = UnifiedPatch::compute(
        &repo,
        "no-longer-exists".into(),
        None,
        ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Blob,
        },
        None,
        3,
    )?;
    assert!(
        actual.is_none(),
        "a file absent from both sides has no diff"
    );
    Ok(())
}

#[test]
fn binary_text_in_unborn() -> anyhow::Result<()> {
    let repo = crate::diff::worktree_changes::repo("diff-binary-to-text-unborn")?;
    let actual = extract_patch(UnifiedPatch::compute(
        &repo,
        "file.binary".into(),
        None,
        ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Blob,
        },
        None,
        3,
    )?);

    snapbox::assert_data_eq!(
        actual.to_debug(),
        snapbox::str![[r#"
[
    DiffHunk("@@ -1,0 +1,1 @@
    +hi
    "),
]

"#]]
    );
    Ok(())
}

#[test]
fn binary_text_renamed_unborn() -> anyhow::Result<()> {
    let repo = crate::diff::worktree_changes::repo("diff-binary-to-text-renamed-in-worktree")?;
    // In case of renames, it uses the name of the previous file for attribute lookups.
    let actual = extract_patch(UnifiedPatch::compute(
        &repo,
        "after-rename.binary".into(),
        Some("before-rename.binary".into()),
        ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Blob,
        },
        ChangeState {
            id: repo.rev_parse_single(":before-rename.binary")?.into(),
            kind: EntryKind::Blob,
        },
        3,
    )?);

    snapbox::assert_data_eq!(
        actual.to_debug(),
        snapbox::str![[r#"
[
    DiffHunk("@@ -1,1 +1,1 @@
    -hi
    +ho
    "),
]

"#]]
    );
    Ok(())
}

#[test]
fn file_deleted_in_worktree() -> anyhow::Result<()> {
    let repo = crate::diff::worktree_changes::repo("added-modified-in-worktree")?;
    // Pretending there is no current version does the trick.
    let previous_state = ChangeState {
        id: repo.rev_parse_single("@:modified")?.into(),
        kind: EntryKind::Blob,
    };
    let no_current_state = None;
    let actual = extract_patch(UnifiedPatch::compute(
        &repo,
        "modified".into(),
        None,
        no_current_state,
        previous_state,
        3,
    )?);

    snapbox::assert_data_eq!(
        actual.to_debug(),
        snapbox::str![[r#"
[
    DiffHunk("@@ -1,1 +1,0 @@
    -something
    "),
]

"#]]
    );
    Ok(())
}

#[test]
fn big_file_20_in_worktree() -> anyhow::Result<()> {
    let mut repo = crate::diff::worktree_changes::repo("big-file-20-unborn")?;
    repo.config_snapshot_mut()
        .set_value(&gix::config::tree::Core::BIG_FILE_THRESHOLD, "20")?;
    let actual = UnifiedPatch::compute(
        &repo,
        "big".into(),
        None,
        ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Blob,
        },
        None,
        3,
    )?
    .expect("present");
    match actual {
        UnifiedPatch::Binary | UnifiedPatch::Patch { .. } => {
            unreachable!("Should be considered too large")
        }
        UnifiedPatch::TooLarge { size_in_bytes } => {
            assert_eq!(
                size_in_bytes, 21,
                "at this size, it's one too large for the big-file limit"
            )
        }
    }
    Ok(())
}

#[test]
fn binary_file_in_worktree() -> anyhow::Result<()> {
    let repo = crate::diff::worktree_changes::repo("binary-file-unborn")?;
    let actual = UnifiedPatch::compute(
        &repo,
        "with-null-bytes".into(),
        None,
        ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Blob,
        },
        None,
        3,
    )?
    .expect("present");
    match actual {
        UnifiedPatch::TooLarge { .. } | UnifiedPatch::Patch { .. } => {
            unreachable!("Should be considered binary, but was {actual:?}");
        }
        UnifiedPatch::Binary => {
            // There is no more information here, binary files aren't diffed.
        }
    }
    Ok(())
}

#[test]
#[cfg(unix)]
fn symlink_modified_in_worktree() -> anyhow::Result<()> {
    let repo = crate::diff::worktree_changes::repo_unix("symlink-change-in-worktree")?;
    let actual = extract_patch(UnifiedPatch::compute(
        &repo,
        "symlink".into(),
        None,
        ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Link,
        },
        ChangeState {
            id: repo.rev_parse_single("@:symlink")?.into(),
            kind: EntryKind::Link,
        },
        3,
    )?);

    snapbox::assert_data_eq!(
        actual.to_debug(),
        snapbox::str![[r#"
[
    DiffHunk("@@ -1,1 +1,1 @@
    -target-to-be-changed
    +changed-target
    "),
]

"#]]
    );
    Ok(())
}

#[test]
fn submodule_added() -> anyhow::Result<()> {
    let repo = crate::diff::worktree_changes::repo("submodule-added-unborn")?;
    let changes = but_core::diff::worktree_changes(&repo)?.changes;
    snapbox::assert_data_eq!(
        changes.to_debug(),
        snapbox::str![[r#"
[
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
]

"#]]
    );
    assert!(
        changes[1].unified_patch(&repo, 3)?.is_none(),
        "submodules produce no diffs"
    );
    assert!(
        UnifiedPatch::compute(
            &repo,
            "submodule".into(),
            None,
            ChangeState {
                id: repo.rev_parse_single(":.gitmodules")?.detach(),
                kind: EntryKind::Blob,
            },
            ChangeState {
                id: repo.rev_parse_single(":submodule")?.detach(),
                kind: EntryKind::Commit,
            },
            3,
        )?
        .is_none(),
        "a submodule in the previous state also prevents diffing a type change"
    );
    Ok(())
}

fn extract_patch(diff: Option<UnifiedPatch>) -> Vec<unified_diff::DiffHunk> {
    match diff {
        None | Some(UnifiedPatch::Binary | UnifiedPatch::TooLarge { .. }) => {
            unreachable!("should have patches")
        }
        Some(UnifiedPatch::Patch { hunks, .. }) => hunks,
    }
}

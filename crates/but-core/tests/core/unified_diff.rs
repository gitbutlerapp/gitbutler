use but_core::{unified_diff, worktree, UnifiedDiff};
use gix::object::tree::EntryKind;

#[test]
fn file_added_in_worktree() -> anyhow::Result<()> {
    let repo = crate::worktree::repo("added-modified-in-worktree")?;
    let actual = extract_patch(UnifiedDiff::compute(
        &repo,
        "modified".into(),
        None,
        worktree::ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Blob,
        },
        None,
        3,
    )?);

    insta::assert_debug_snapshot!(actual, @r#"
    [
        DiffHunk {
            old_start: 1,
            old_lines: 0,
            new_start: 1,
            new_lines: 1,
            diff: "@@ -1,0 +1,1 @@\n+change\n\n",
        },
    ]
    "#);
    Ok(())
}

#[test]
fn binary_text_in_unborn() -> anyhow::Result<()> {
    let repo = crate::worktree::repo("diff-binary-to-text-unborn")?;
    let actual = extract_patch(UnifiedDiff::compute(
        &repo,
        "file.binary".into(),
        None,
        worktree::ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Blob,
        },
        None,
        3,
    )?);

    insta::assert_debug_snapshot!(actual, @r#"
    [
        DiffHunk {
            old_start: 1,
            old_lines: 0,
            new_start: 1,
            new_lines: 1,
            diff: "@@ -1,0 +1,1 @@\n+hi\n\n",
        },
    ]
    "#);
    Ok(())
}

#[test]
fn binary_text_renamed_unborn() -> anyhow::Result<()> {
    let repo = crate::worktree::repo("diff-binary-to-text-renamed-in-worktree")?;
    // In case of renames, it uses the name of the previous file for attribute lookups.
    let actual = extract_patch(UnifiedDiff::compute(
        &repo,
        "after-rename.binary".into(),
        Some("before-rename.binary".into()),
        worktree::ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Blob,
        },
        worktree::ChangeState {
            id: repo.rev_parse_single(":before-rename.binary")?.into(),
            kind: EntryKind::Blob,
        },
        3,
    )?);

    insta::assert_debug_snapshot!(actual, @r#"
    [
        DiffHunk {
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 1,
            diff: "@@ -1,1 +1,1 @@\n-hi\n\n+ho\n\n",
        },
    ]
    "#);
    Ok(())
}

#[test]
fn file_deleted_in_worktree() -> anyhow::Result<()> {
    let repo = crate::worktree::repo("added-modified-in-worktree")?;
    // Pretending there is no current version does the trick.
    let previous_state = worktree::ChangeState {
        id: repo.rev_parse_single("@:modified")?.into(),
        kind: EntryKind::Blob,
    };
    let no_current_state = None;
    let actual = extract_patch(UnifiedDiff::compute(
        &repo,
        "modified".into(),
        None,
        no_current_state,
        previous_state,
        3,
    )?);

    insta::assert_debug_snapshot!(actual, @r#"
    [
        DiffHunk {
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 0,
            diff: "@@ -1,1 +1,0 @@\n-something\n\n",
        },
    ]
    "#);
    Ok(())
}

#[test]
fn big_file_20_in_worktree() -> anyhow::Result<()> {
    let mut repo = crate::worktree::repo("big-file-20-unborn")?;
    repo.config_snapshot_mut()
        .set_value(&gix::config::tree::Core::BIG_FILE_THRESHOLD, "20")?;
    let actual = UnifiedDiff::compute(
        &repo,
        "big".into(),
        None,
        worktree::ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Blob,
        },
        None,
        3,
    )?;
    match actual {
        UnifiedDiff::Binary | UnifiedDiff::Patch { .. } => {
            unreachable!("Should be considered too large")
        }
        UnifiedDiff::TooLarge { size_in_bytes } => {
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
    let repo = crate::worktree::repo("binary-file-unborn")?;
    let actual = UnifiedDiff::compute(
        &repo,
        "with-null-bytes".into(),
        None,
        worktree::ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Blob,
        },
        None,
        3,
    )?;
    match actual {
        UnifiedDiff::TooLarge { .. } | UnifiedDiff::Patch { .. } => {
            unreachable!("Should be considered binary, but was {actual:?}");
        }
        UnifiedDiff::Binary => {
            // There is no more information here, binary files aren't diffed.
        }
    }
    Ok(())
}

#[test]
#[cfg(unix)]
fn symlink_modified_in_worktree() -> anyhow::Result<()> {
    let repo = crate::worktree::repo_unix("symlink-change-in-worktree")?;
    let actual = extract_patch(UnifiedDiff::compute(
        &repo,
        "symlink".into(),
        None,
        worktree::ChangeState {
            id: repo.object_hash().null(),
            kind: EntryKind::Link,
        },
        worktree::ChangeState {
            id: repo.rev_parse_single("@:symlink")?.into(),
            kind: EntryKind::Link,
        },
        3,
    )?);

    insta::assert_debug_snapshot!(actual, @r#"
    [
        DiffHunk {
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 1,
            diff: "@@ -1,1 +1,1 @@\n-target-to-be-changed\n+changed-target\n",
        },
    ]
    "#);
    Ok(())
}

fn extract_patch(diff: UnifiedDiff) -> Vec<unified_diff::DiffHunk> {
    match diff {
        UnifiedDiff::Binary | UnifiedDiff::TooLarge { .. } => unreachable!("should have patches"),
        UnifiedDiff::Patch { hunks } => hunks,
    }
}

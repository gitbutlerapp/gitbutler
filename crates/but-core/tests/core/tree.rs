use bstr::ByteSlice;
use but_core::{ChangeState, DiffSpec, HunkHeader, UnifiedPatch, apply_hunks};
use but_testsupport::{CommandExt, git, hunk_header, writable_scenario};
use gix::object::tree::EntryKind;

#[test]
fn filemode_disabled_preserves_conflict_stage_modes() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("filemode-disabled");
    let base = repo.head_tree_id()?.detach();
    for (path, expected_kind, reason) in [
        (
            "modify-delete.sh",
            EntryKind::BlobExecutable,
            "ours is absent, so the executable mode from the remaining stages must be preserved",
        ),
        (
            "base-preferred.sh",
            EntryKind::Blob,
            "ours is absent, so the non-executable base must take precedence over executable theirs",
        ),
        (
            "ours-preferred.sh",
            EntryKind::Blob,
            "non-executable ours must take precedence over executable base and theirs",
        ),
        (
            "theirs-only.sh",
            EntryKind::BlobExecutable,
            "only theirs is available, so its executable mode must be preserved",
        ),
        (
            "untracked.sh",
            EntryKind::Blob,
            "no index entry exists, so the mode must default to non-executable despite the worktree executable bit",
        ),
    ] {
        let mut changes = vec![Ok(spec(None, path))];
        let (tree, _) = but_core::tree::apply_worktree_changes(base, &repo, &mut changes, 0)?;
        assert!(
            changes.iter().all(Result::is_ok),
            "the edit must be accepted"
        );
        let tree = tree.object()?.into_tree();
        let entry = tree.lookup_entry_by_path(path)?.expect("script exists");
        assert_eq!(entry.mode().kind(), expected_kind, "{path}: {reason}");
        assert_eq!(
            entry.object()?.into_blob().data,
            b"after\n",
            "the worktree content must be preserved"
        );
    }
    Ok(())
}

#[test]
fn filemode_disabled_preserves_executable() -> anyhow::Result<()> {
    filemode_disabled_preserves_executable_inner(false)
}

#[test]
fn filemode_disabled_preserves_executable_with_hunks() -> anyhow::Result<()> {
    filemode_disabled_preserves_executable_inner(true)
}

fn filemode_disabled_preserves_executable_inner(with_hunks: bool) -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("filemode-disabled");
    let base = repo.head_tree_id()?.detach();
    for (path, expected_kind) in [
        ("script.sh", EntryKind::BlobExecutable),
        ("added.sh", EntryKind::BlobExecutable),
        ("removed.sh", EntryKind::Blob),
    ] {
        let mut changes = vec![Ok(spec(None, path))];
        if with_hunks {
            changes[0]
                .as_mut()
                .expect("valid spec")
                .hunk_headers
                .push(but_core::HunkHeader {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 1,
                });
        }
        let (tree, _) = but_core::tree::apply_worktree_changes(base, &repo, &mut changes, 0)?;
        assert!(
            changes.iter().all(Result::is_ok),
            "the edit must be accepted"
        );
        let tree = tree.object()?.into_tree();
        let entry = tree.lookup_entry_by_path(path)?.expect("script exists");
        assert_eq!(
            entry.mode().kind(),
            expected_kind,
            "core.fileMode=false must preserve the indexed executable bit for {path}"
        );
        assert_eq!(
            entry.object()?.into_blob().data,
            b"after\n",
            "the edited content must be committed"
        );
    }
    Ok(())
}

#[test]
fn filemode_disabled_commits_index_only_mode_change() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("filemode-disabled");
    let base = repo.head_tree_id()?.detach();
    let status = but_core::diff::worktree_changes(&repo)?;
    assert!(
        status
            .changes
            .iter()
            .any(|change| change.path == "mode-only.sh"),
        "the staged mode-only change must be visible"
    );
    let mut changes = vec![Ok(spec(None, "mode-only.sh"))];
    let (tree, _) = but_core::tree::apply_worktree_changes(base, &repo, &mut changes, 0)?;
    assert!(
        changes.iter().all(Result::is_ok),
        "the mode change must be accepted"
    );
    let tree = tree.object()?.into_tree();
    let entry = tree
        .lookup_entry_by_path("mode-only.sh")?
        .expect("script exists");
    assert_eq!(
        entry.mode().kind(),
        EntryKind::BlobExecutable,
        "the index-only executable bit must be committed"
    );
    assert_eq!(
        entry.object()?.into_blob().data,
        b"before\n",
        "a mode-only change must preserve content"
    );
    Ok(())
}

/// Regression test for data loss when a file is renamed and a *new directory* is committed at the
/// file's old path in the same commit.
///
/// Base tree (stand-in for `HEAD`) has a blob `A`. In the worktree, `A` is renamed to `B` and a new
/// directory `A/` (with files) is created at the old path. Committing all of this once dropped the
/// directory: changes are processed in path-sorted order (`A/one`, `A/two`, then the rename
/// `B` <- `A`), and the rename's unconditional source-removal `remove("A")` pruned the
/// freshly-built `A/` subtree.
#[test]
fn rename_with_new_directory_at_old_path_keeps_directory() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("unborn-empty");
    let work_dir = repo.workdir().expect("non-bare repo").to_owned();

    // Base tree: a single blob at path `A`.
    let mut base = repo.empty_tree().edit()?;
    base.upsert("A", EntryKind::Blob, repo.write_blob("a\n")?.detach())?;
    let base_tree = base.write()?.detach();

    // Worktree: `A` renamed to `B`, plus a brand-new `A/` directory at the old path.
    std::fs::write(work_dir.join("B"), "a\n")?;
    std::fs::create_dir(work_dir.join("A"))?;
    std::fs::write(work_dir.join("A").join("one"), "one\n")?;
    std::fs::write(work_dir.join("A").join("two"), "two\n")?;

    // The order `but` actually feeds in, sorted by destination path: the rename comes last.
    let mut changes = vec![
        Ok(spec(None, "A/one")),
        Ok(spec(None, "A/two")),
        Ok(spec(Some("A"), "B")),
    ];

    let (new_tree, _base) =
        but_core::tree::apply_worktree_changes(base_tree, &repo, &mut changes, 0)?;

    assert!(
        changes.iter().all(|c| c.is_ok()),
        "no change should be rejected: {changes:?}"
    );

    let tree = new_tree.object()?.into_tree();
    assert!(
        tree.lookup_entry_by_path("A/one")?.is_some(),
        "A/one must survive the commit"
    );
    assert!(
        tree.lookup_entry_by_path("A/two")?.is_some(),
        "A/two must survive the commit"
    );
    assert!(
        tree.lookup_entry_by_path("B")?.is_some(),
        "the renamed file B must exist"
    );
    let a = tree
        .lookup_entry_by_path("A")?
        .expect("`A` must exist as the new directory");
    assert!(
        a.mode().is_tree(),
        "`A` must be a directory, not a leftover blob"
    );
    Ok(())
}

#[test]
fn apply_hunks_keeps_line_boundary_after_unterminated_last_line() -> anyhow::Result<()> {
    for (old, new, selected_hunk, expected, reason) in [
        (
            "\u{feff}1\r\n2\r\n3\r\n4\r\n5\r\n6",
            "\u{feff}1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n8\r\n9",
            hunk_header("-7,0", "+8,2"),
            "\u{feff}1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n8\r\n9",
            "selected CRLF lines 8 and 9 remain separate from retained line 6",
        ),
        (
            "5\n6",
            "5\n6\n7\r\n8\r\n9",
            hunk_header("-3,0", "+4,2"),
            "5\n6\n8\r\n9",
            "the terminator is the one right after the old image, not the one of skipped line 7",
        ),
        (
            "5\n6",
            "5\n6\n6\r\n8",
            hunk_header("-3,0", "+4,1"),
            "5\n6\n8",
            "an unselected appended copy of line 6 doesn't decide its terminator",
        ),
        (
            "a\nb",
            "A\nb\nc\nd",
            hunk_header("-3,0", "+4,1"),
            "a\nb\nd",
            "an unselected earlier edit doesn't hide the new counterpart of the last line",
        ),
        (
            "a\nb",
            "A\nb\r\nc\nd",
            hunk_header("-3,0", "+4,1"),
            "a\nb\r\nd",
            "after an earlier edit, the terminator still comes from the counterpart of the last line",
        ),
        (
            "a\nq\nb",
            "b\r\nA\nq\nb\nz",
            hunk_header("-4,0", "+5,1"),
            "a\nq\nb\nz",
            "after an earlier edit, an earlier copy of the last line isn't its counterpart",
        ),
        (
            "5\r\n6",
            "5\r\n6\r\n7\r\n8\r\n9",
            hunk_header("-3,0", "+5,1"),
            "5\r\n6\r\n9",
            "a selected final line without terminator stays unterminated",
        ),
        (
            "5\r\n6",
            "5\r\n7",
            hunk_header("-2,1", "+2,0"),
            "5\r\n",
            "deleting the unterminated line keeps the previous one as is",
        ),
        (
            "",
            "\n2",
            hunk_header("-1,0", "+2,1"),
            "2",
            "nothing is prepended to an empty base",
        ),
    ] {
        assert_eq!(
            apply_hunks(old.into(), new.into(), &[selected_hunk])?,
            expected,
            "{reason}"
        );
    }
    Ok(())
}

/// Selecting the last of several lines appended after an unterminated last line must
/// keep that line's boundary, as reported in #16027.
#[test]
fn selected_additions_after_unterminated_last_line_keep_line_boundary() -> anyhow::Result<()> {
    for (autocrlf, committed, worktree, expected) in [
        (
            false,
            "\u{feff}1\r\n2\r\n3\r\n4\r\n5\r\n6",
            "\u{feff}1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n8\r\n9",
            "\u{feff}1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n8\r\n9",
        ),
        (
            true,
            "1\n2\n3\n4\n5\n6",
            "1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n7\r\n8\r\n9",
            "1\n2\n3\n4\n5\n6\n8\n9",
        ),
    ] {
        let (mut repo, _tmp) = writable_scenario("unborn-empty");
        let work_dir = repo.workdir().expect("non-bare repo").to_owned();
        std::fs::write(work_dir.join("file"), committed)?;
        git(&repo).args(["add", "file"]).run();
        git(&repo).args(["commit", "-m", "base"]).run();
        if autocrlf {
            repo.config_snapshot_mut()
                .set_raw_value("core.autocrlf", "true")?;
        } else {
            let state = |id: gix::Id<'_>| ChangeState {
                id: id.detach(),
                kind: EntryKind::Blob,
            };
            let Some(UnifiedPatch::Patch { hunks, .. }) = UnifiedPatch::compute(
                &repo,
                "file".into(),
                None,
                state(repo.write_blob(worktree)?),
                state(repo.rev_parse_single("HEAD:file")?),
                0,
            )?
            else {
                unreachable!("text files have a patch")
            };
            assert_eq!(
                hunks.into_iter().map(HunkHeader::from).collect::<Vec<_>>(),
                [hunk_header("-7,0", "+7,3")],
                "terminating line 6 isn't a change, so only 7, 8 and 9 are added"
            );
        }
        std::fs::write(work_dir.join("file"), worktree)?;

        let mut changes = vec![Ok(DiffSpec {
            hunk_headers: vec![hunk_header("-0,0", "+8,2")],
            ..spec(None, "file")
        })];
        let (tree, _) = but_core::tree::apply_worktree_changes(
            repo.head_tree_id()?.detach(),
            &repo,
            &mut changes,
            0,
        )?;
        assert!(
            changes.iter().all(Result::is_ok),
            "the selection must be accepted: {changes:?}"
        );
        let blob = tree
            .object()?
            .into_tree()
            .lookup_entry_by_path("file")?
            .expect("file exists")
            .object()?
            .into_blob();
        assert_eq!(
            blob.data.as_bstr(),
            expected,
            "lines 8 and 9 follow line 6 on their own lines (autocrlf = {autocrlf})"
        );
    }
    Ok(())
}

fn spec(previous_path: Option<&str>, path: &str) -> DiffSpec {
    DiffSpec {
        previous_path: previous_path.map(Into::into),
        path: path.into(),
        hunk_headers: vec![],
    }
}

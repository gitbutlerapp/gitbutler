use but_core::DiffSpec;
use but_testsupport::writable_scenario;
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

fn spec(previous_path: Option<&str>, path: &str) -> DiffSpec {
    DiffSpec {
        previous_path: previous_path.map(Into::into),
        path: path.into(),
        hunk_headers: vec![],
    }
}

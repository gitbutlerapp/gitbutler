use but_core::DiffSpec;
use but_testsupport::writable_scenario;
use gix::object::tree::EntryKind;

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

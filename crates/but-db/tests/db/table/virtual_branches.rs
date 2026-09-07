use but_db::{VbStack, VbStackHead, VbState, VirtualBranchesSnapshot};

use crate::table::in_memory_db;

#[test]
fn get_snapshot_empty() -> anyhow::Result<()> {
    let db = in_memory_db();
    assert!(db.virtual_branches().get_snapshot()?.is_none());
    Ok(())
}

#[test]
fn replace_and_get_snapshot() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    let snapshot = sample_snapshot();

    let handle = db.virtual_branches_mut()?;
    handle.replace_snapshot(&snapshot)?;

    let actual = db.virtual_branches().get_snapshot()?;
    assert_eq!(actual, Some(snapshot));
    Ok(())
}

#[test]
fn metadata_edits_write_only_changed_rows() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("metadata.sqlite");
    let mut db = but_db::DbHandle::new_at_path(&path)?;
    let mut snapshot = sample_snapshot();
    snapshot.stacks[0].id = but_core::ref_metadata::StackId::generate().to_string();
    snapshot.stacks[0].sort_order = 0;
    snapshot.heads[0].stack_id = snapshot.stacks[0].id.clone();
    let mut other_stack = snapshot.stacks[0].clone();
    other_stack.id = but_core::ref_metadata::StackId::generate().to_string();
    other_stack.sort_order = 1;
    let mut other_head = snapshot.heads[0].clone();
    other_head.stack_id = other_stack.id.clone();
    other_head.name = "unrelated".into();
    snapshot.stacks.push(other_stack);
    snapshot.heads.push(other_head);
    db.meta_mut()?.replace_snapshot(&snapshot)?;

    let observer = rusqlite::Connection::open(path)?;
    observer.execute(
        "INSERT INTO vb_branch_targets (stack_id, remote_name, branch_name, remote_url, sha)
         VALUES (?1, 'origin', 'main', 'legacy-url', ?2)",
        rusqlite::params![snapshot.stacks[1].id, snapshot.heads[1].head_sha],
    )?;
    observer.execute_batch("CREATE TABLE writes (table_name TEXT, action TEXT);")?;
    for table in ["vb_state", "vb_stacks", "vb_stack_heads"] {
        for action in ["INSERT", "UPDATE", "DELETE"] {
            observer.execute_batch(&format!(
                "CREATE TRIGGER log_{table}_{action} AFTER {action} ON {table}
                 BEGIN INSERT INTO writes VALUES ('{table}', '{action}'); END;"
            ))?;
        }
    }
    let writes = || -> rusqlite::Result<Vec<(String, String)>> {
        observer
            .prepare("SELECT table_name, action FROM writes ORDER BY rowid")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect()
    };

    let name: gix::refs::FullName = "refs/heads/series-a".try_into()?;
    let mut branch = db
        .meta()?
        .branch(name.as_ref())
        .cloned()
        .unwrap_or_default();
    branch.review.pull_request = Some(42);
    db.meta_mut()?.set_branch(name.as_ref(), &branch)?;
    assert_eq!(
        writes()?,
        vec![("vb_stack_heads".into(), "UPDATE".into())],
        "editing one review must leave all other rows untouched"
    );
    observer.execute("DELETE FROM writes", [])?;
    db.meta_mut()?.set_branch(name.as_ref(), &branch)?;
    assert!(
        writes()?.is_empty(),
        "saving unchanged metadata writes no rows"
    );

    let renamed: gix::refs::FullName = "refs/heads/renamed".try_into()?;
    db.meta_mut()?.rename(name.as_ref(), renamed.as_ref())?;
    assert_eq!(
        writes()?,
        vec![("vb_stack_heads".into(), "UPDATE".into())],
        "renaming a branch updates its existing head row"
    );
    observer.execute("DELETE FROM writes", [])?;
    db.meta_mut()?.remove(renamed.as_ref())?;
    let mut removed = writes()?;
    removed.sort();
    assert_eq!(
        removed,
        vec![
            ("vb_stack_heads".into(), "DELETE".into()),
            ("vb_stacks".into(), "DELETE".into()),
        ],
        "removing a stack deletes its rows while preserving the unrelated stack"
    );
    let remaining = db.virtual_branches().get_snapshot()?.expect("state exists");
    assert_eq!(
        remaining.stacks,
        snapshot.stacks[1..],
        "unrelated stack data is unchanged"
    );
    assert_eq!(
        remaining.heads,
        snapshot.heads[1..],
        "unrelated review data is unchanged"
    );
    assert_eq!(
        observer.query_row("SELECT COUNT(*) FROM vb_branch_targets", [], |row| row
            .get::<_, i64>(0))?,
        1,
        "ordinary metadata edits preserve obsolete dependent rows of retained stacks"
    );
    Ok(())
}

#[test]
fn replace_snapshot_replaces_existing_data() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let handle = db.virtual_branches_mut()?;
    handle.replace_snapshot(&sample_snapshot())?;

    let mut next = sample_snapshot();
    next.stacks = vec![VbStack {
        id: "stack-b".into(),
        source_refname: None,
        upstream_remote_name: None,
        upstream_branch_name: None,
        sort_order: 0,
        in_workspace: false,
        legacy_name: "b".into(),
        legacy_notes: String::new(),
        legacy_ownership: String::new(),
        legacy_allow_rebasing: true,
        legacy_post_commits: false,
        legacy_tree_sha: "0000000000000000000000000000000000000000".into(),
        legacy_head_sha: "0000000000000000000000000000000000000000".into(),
        legacy_created_timestamp_ms: "0".into(),
        legacy_updated_timestamp_ms: "0".into(),
    }];
    next.heads = vec![VbStackHead {
        stack_id: "stack-b".into(),
        position: 0,
        name: "series-b".into(),
        head_sha: "3333333333333333333333333333333333333333".into(),
        pr_number: None,
        archived: true,
        review_id: None,
    }];
    let handle = db.virtual_branches_mut()?;
    handle.replace_snapshot(&next)?;

    let actual = db.virtual_branches().get_snapshot()?;
    assert_eq!(actual, Some(next));
    Ok(())
}

#[test]
fn snapshot_changes_rollback_with_transaction() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    let snapshot = sample_snapshot();

    {
        let mut trans = db.transaction()?;
        let handle = trans.virtual_branches_mut()?;
        // Savepoint commits, but outer transaction still controls visibility.
        handle.replace_snapshot(&snapshot)?;
        trans.rollback()?;
    }

    assert!(db.virtual_branches().get_snapshot()?.is_none());
    Ok(())
}

#[test]
fn replace_snapshot_rejects_heads_without_existing_stack() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    let mut snapshot = sample_snapshot();
    snapshot.stacks.clear();

    let handle = db.virtual_branches_mut()?;
    let err = handle
        .replace_snapshot(&snapshot)
        .expect_err("head rows must reference an existing stack");
    assert!(
        err.to_string().contains("FOREIGN KEY"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn set_state_upsert_updates_existing_row() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let mut initial = VbState {
        initialized: true,
        last_pushed_base_sha: Some("1111111111111111111111111111111111111111".into()),
        ..VbState::default()
    };
    {
        let handle = db.virtual_branches_mut()?;
        handle.set_state(&initial)?;
    }

    initial.last_pushed_base_sha = Some("2222222222222222222222222222222222222222".into());
    initial.toml_last_seen_mtime_ns = Some(123);
    initial.toml_last_seen_sha256 = Some("abc123".into());
    {
        let handle = db.virtual_branches_mut()?;
        handle.set_state(&initial)?;
    }

    let snapshot = db
        .virtual_branches()
        .get_snapshot()?
        .expect("state row should exist after set_state");
    assert_eq!(snapshot.state, initial);
    assert!(snapshot.stacks.is_empty());
    assert!(snapshot.heads.is_empty());
    Ok(())
}

#[test]
fn toml_sync_metadata_roundtrips_across_replacements() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let mut first = sample_snapshot();
    first.state.toml_last_seen_mtime_ns = Some(100);
    first.state.toml_last_seen_sha256 = Some("hash-first".into());
    {
        let handle = db.virtual_branches_mut()?;
        handle.replace_snapshot(&first)?;
    }

    let mut second = first.clone();
    second.state.toml_last_seen_mtime_ns = Some(101);
    second.state.toml_last_seen_sha256 = Some("hash-second".into());
    {
        let handle = db.virtual_branches_mut()?;
        handle.replace_snapshot(&second)?;
    }

    let snapshot = db
        .virtual_branches()
        .get_snapshot()?
        .expect("snapshot should exist after replacement");
    assert_eq!(snapshot.state.toml_last_seen_mtime_ns, Some(101));
    assert_eq!(
        snapshot.state.toml_last_seen_sha256.as_deref(),
        Some("hash-second")
    );
    Ok(())
}

#[test]
fn get_snapshot_returns_deterministic_ordering() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    let mut snapshot = sample_snapshot();
    snapshot.stacks = vec![
        VbStack {
            id: "stack-b".into(),
            source_refname: None,
            upstream_remote_name: None,
            upstream_branch_name: None,
            sort_order: 0,
            in_workspace: false,
            legacy_name: "b".into(),
            legacy_notes: String::new(),
            legacy_ownership: String::new(),
            legacy_allow_rebasing: true,
            legacy_post_commits: false,
            legacy_tree_sha: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            legacy_head_sha: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            legacy_created_timestamp_ms: "0".into(),
            legacy_updated_timestamp_ms: "0".into(),
        },
        VbStack {
            id: "stack-a".into(),
            source_refname: None,
            upstream_remote_name: None,
            upstream_branch_name: None,
            sort_order: 0,
            in_workspace: true,
            legacy_name: "a".into(),
            legacy_notes: String::new(),
            legacy_ownership: String::new(),
            legacy_allow_rebasing: true,
            legacy_post_commits: false,
            legacy_tree_sha: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            legacy_head_sha: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            legacy_created_timestamp_ms: "0".into(),
            legacy_updated_timestamp_ms: "0".into(),
        },
    ];
    snapshot.heads = vec![
        VbStackHead {
            stack_id: "stack-b".into(),
            position: 1,
            name: "head-b1".into(),
            head_sha: "1111111111111111111111111111111111111111".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        },
        VbStackHead {
            stack_id: "stack-a".into(),
            position: 1,
            name: "head-a1".into(),
            head_sha: "2222222222222222222222222222222222222222".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        },
        VbStackHead {
            stack_id: "stack-a".into(),
            position: 0,
            name: "head-a0".into(),
            head_sha: "3333333333333333333333333333333333333333".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        },
    ];
    let handle = db.virtual_branches_mut()?;
    handle.replace_snapshot(&snapshot)?;

    let actual = db
        .virtual_branches()
        .get_snapshot()?
        .expect("snapshot should exist after replacement");
    assert_eq!(
        actual
            .stacks
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        vec!["stack-a", "stack-b"]
    );
    assert_eq!(
        actual
            .heads
            .iter()
            .map(|h| format!("{}:{}", h.stack_id, h.position))
            .collect::<Vec<_>>(),
        vec!["stack-a:0", "stack-a:1", "stack-b:1"]
    );
    // Round-trip the already read snapshot to ensure ordering remains stable.
    let handle = db.virtual_branches_mut()?;
    handle.replace_snapshot(&actual)?;

    let roundtripped = db
        .virtual_branches()
        .get_snapshot()?
        .expect("snapshot should exist after round-trip replacement");
    assert_eq!(
        roundtripped
            .stacks
            .iter()
            .map(|s| s.id.as_str())
            .collect::<Vec<_>>(),
        vec!["stack-a", "stack-b"]
    );
    assert_eq!(
        roundtripped
            .heads
            .iter()
            .map(|h| format!("{}:{}", h.stack_id, h.position))
            .collect::<Vec<_>>(),
        vec!["stack-a:0", "stack-a:1", "stack-b:1"]
    );
    Ok(())
}

#[test]
fn deleting_stack_cascades_to_heads() -> anyhow::Result<()> {
    let mut conn = rusqlite::Connection::open_in_memory()?;
    but_db::migration::run(&mut conn, but_db::migration::ours())?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    conn.execute(
        "INSERT INTO vb_stacks (id, sort_order, in_workspace) VALUES (?1, ?2, ?3)",
        rusqlite::params!["stack-a", 0_i64, true],
    )?;
    conn.execute(
        "INSERT INTO vb_stack_heads (stack_id, position, name, head_sha) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            "stack-a",
            0_i64,
            "series-a",
            "1111111111111111111111111111111111111111"
        ],
    )?;
    conn.execute(
        "DELETE FROM vb_stacks WHERE id = ?1",
        rusqlite::params!["stack-a"],
    )?;

    let heads: i64 = conn.query_row(
        "SELECT COUNT(*) FROM vb_stack_heads WHERE stack_id = ?1",
        rusqlite::params!["stack-a"],
        |row| row.get(0),
    )?;
    assert_eq!(heads, 0);
    Ok(())
}

fn sample_snapshot() -> VirtualBranchesSnapshot {
    VirtualBranchesSnapshot {
        state: VbState {
            initialized: true,
            last_pushed_base_sha: Some("2222222222222222222222222222222222222222".into()),
            toml_last_seen_mtime_ns: Some(42),
            toml_last_seen_sha256: Some("abc".into()),
        },
        stacks: vec![VbStack {
            id: "stack-a".into(),
            source_refname: Some("refs/heads/feature".into()),
            upstream_remote_name: Some("origin".into()),
            upstream_branch_name: Some("feature".into()),
            sort_order: 1,
            in_workspace: true,
            legacy_name: "a".into(),
            legacy_notes: String::new(),
            legacy_ownership: String::new(),
            legacy_allow_rebasing: true,
            legacy_post_commits: false,
            legacy_tree_sha: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            legacy_head_sha: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            legacy_created_timestamp_ms: "1".into(),
            legacy_updated_timestamp_ms: "2".into(),
        }],
        heads: vec![VbStackHead {
            stack_id: "stack-a".into(),
            position: 0,
            name: "series-a".into(),
            head_sha: "cccccccccccccccccccccccccccccccccccccccc".into(),
            pr_number: Some(7),
            archived: false,
            review_id: Some("rvw_1".into()),
        }],
    }
}

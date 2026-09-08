use but_core::{
    WORKSPACE_REF_NAME,
    ref_metadata::{
        StackId, Workspace, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
    },
};
use but_db::DbHandle;
use gix::refs::FullName;

#[test]
fn workspaces_are_independent_and_roundtrip_complete_values() -> anyhow::Result<()> {
    let mut db = DbHandle::new_at_path(":memory:")?;
    let first: FullName = "refs/heads/gitbutler/workspaces/first".try_into()?;
    let second: FullName = "refs/heads/gitbutler/workspaces/second".try_into()?;
    let mut first_value = workspace(&db)?;
    first_value.ref_info.created_at = Some(gix::date::Time::new(123, 3600));
    first_value.ref_info.updated_at = Some(gix::date::Time::new(456, -1800));
    first_value.stacks[0].workspacecommit_relation = WorkspaceCommitRelation::MergeFrom {
        commit_id: Some(gix::ObjectId::from_hex(
            b"1111111111111111111111111111111111111111",
        )?),
    };
    let mut second_value = first_value.clone();
    second_value.stacks[0].workspacecommit_relation =
        WorkspaceCommitRelation::MergeFrom { commit_id: None };
    second_value.stacks[0].branches[0].archived = true;
    db.meta_mut()?.set_workspace(first.as_ref(), &first_value)?;
    db.meta_mut()?
        .set_workspace(second.as_ref(), &second_value)?;
    let metadata = db.meta()?;
    assert_eq!(
        metadata.workspace(first.as_ref()),
        Some(&first_value),
        "saving another workspace preserves the first workspace's full value"
    );
    assert_eq!(
        metadata.workspace(second.as_ref()),
        Some(&second_value),
        "workspaces may share stack IDs and branch refs without sharing their state"
    );
    assert_eq!(
        metadata.workspaces().count(),
        2,
        "both workspace refs are stored"
    );
    db.meta_mut()?.remove(first.as_ref())?;
    let metadata = db.meta()?;
    assert!(
        metadata.workspace(first.as_ref()).is_none(),
        "removal applies only to the named workspace"
    );
    assert_eq!(
        metadata.workspace(second.as_ref()),
        Some(&second_value),
        "removing another workspace preserves this workspace"
    );
    db.meta_mut()?
        .set_workspace(second.as_ref(), &Workspace::default())?;
    assert_eq!(
        db.meta()?.workspace(second.as_ref()),
        Some(&Workspace::default()),
        "an explicitly saved empty workspace remains present"
    );
    Ok(())
}

#[test]
fn ref_metadata_preserves_full_names_and_non_utf8_bytes() -> anyhow::Result<()> {
    use but_core::ref_metadata::{Branch, RefInfo, Review};
    use gix::bstr::ByteSlice as _;
    let mut db = DbHandle::new_at_path(":memory:")?;
    let name: FullName = b"refs/remotes/origin/feature-\xff".as_bstr().try_into()?;
    let value = Branch {
        ref_info: RefInfo {
            created_at: Some(gix::date::Time::new(17, -3600)),
            updated_at: Some(gix::date::Time::new(19, 1800)),
        },
        review: Review {
            pull_request: Some(42),
            review_id: Some("review".into()),
        },
    };
    db.meta_mut()?.set_branch(name.as_ref(), &value)?;
    assert_eq!(
        db.meta()?.branch(name.as_ref()),
        Some(&value),
        "branch metadata preserves ref namespaces, bytes, timestamps, and reviews"
    );
    assert_eq!(
        db.meta()?.workspaces().count(),
        0,
        "saving branch metadata does not invent a workspace or stack"
    );
    Ok(())
}

#[test]
fn branch_order_preserves_non_utf8_refs_through_metadata_operations() -> anyhow::Result<()> {
    use gix::bstr::ByteSlice as _;
    let mut db = DbHandle::new_at_path(":memory:")?;
    let top: FullName = b"refs/heads/top-\xff".as_bstr().try_into()?;
    let middle: FullName = "refs/heads/middle".try_into()?;
    let bottom: FullName = b"refs/heads/bottom-\xfe".as_bstr().try_into()?;
    let renamed: FullName = b"refs/heads/renamed-\xfd".as_bstr().try_into()?;
    let order = vec![top.clone(), middle.clone(), bottom.clone()];
    db.meta_mut()?.set_branch_stack_order(&order)?;
    let snapshot = db.meta()?;
    snapshot.validate()?;
    assert_eq!(
        snapshot.branch_stack_order(top.as_ref()),
        Some(order.as_slice()),
        "ad-hoc order accepts the same ref bytes as workspace and branch metadata"
    );
    db.meta_mut()?.rename(middle.as_ref(), renamed.as_ref())?;
    db.meta_mut()?.remove(bottom.as_ref())?;
    let remaining = vec![top.clone(), renamed.clone()];
    db.meta_mut()?
        .remove_missing_branch_stack_order_references(&remaining)?;
    assert_eq!(
        db.meta()?.branch_stack_order(top.as_ref()),
        Some(remaining.as_slice()),
        "rename, removal, and pruning preserve byte-valued names and chain order"
    );
    db.meta_mut()?.replace_snapshot(&snapshot)?;
    assert_eq!(
        db.meta()?,
        snapshot,
        "full metadata restoration also preserves non-UTF-8 ad-hoc ordering"
    );
    Ok(())
}

#[test]
fn branch_order_migration_preserves_existing_text_rows_as_blobs() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("branch-order.sqlite");
    let mut conn = rusqlite::Connection::open(&path)?;
    let (metadata_migrations, table_migrations) = but_db::MIGRATIONS
        .split_last()
        .expect("metadata migrations follow table creation");
    let old_migrations = table_migrations
        .iter()
        .flat_map(|group| group.iter())
        .chain(metadata_migrations.iter().take(1))
        .copied()
        .collect::<Vec<_>>();
    but_db::migration::run(&mut conn, old_migrations.iter().copied())?;
    conn.execute_batch(
        "INSERT INTO branch_order (branch_ref_name, parent_ref_name) VALUES
         ('refs/heads/top', 'refs/heads/base'), ('refs/heads/base', NULL);",
    )?;
    let db = DbHandle::new_at_path(&path)?;
    assert!(
        but_db::migration::run(&mut conn, old_migrations).is_err(),
        "old binaries must reject BLOB refs instead of trying to decode them as strings"
    );
    let mut stmt = conn.prepare("SELECT type FROM pragma_table_info('branch_order')")?;
    let column_types = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        column_types,
        ["BLOB", "BLOB"],
        "the forward migration replaces UTF-8 text storage with Git ref bytes"
    );
    let blob_rows: i64 = conn.query_row(
        "SELECT COUNT(*) FROM branch_order WHERE typeof(branch_ref_name) = 'blob'
         AND (parent_ref_name IS NULL OR typeof(parent_ref_name) = 'blob')",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(
        blob_rows, 2,
        "existing text values are converted without dropping rows"
    );
    let expected = vec!["refs/heads/top".try_into()?, "refs/heads/base".try_into()?];
    assert_eq!(
        db.meta()?.branch_stack_order("refs/heads/base".try_into()?),
        Some(expected.as_slice()),
        "migration preserves the existing chain"
    );
    Ok(())
}

fn workspace(db: &DbHandle) -> anyhow::Result<Workspace> {
    let mut workspace = db
        .meta()?
        .workspace(WORKSPACE_REF_NAME.try_into()?)
        .cloned()
        .unwrap_or_default();
    workspace.stacks.push(WorkspaceStack {
        id: StackId::generate(),
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
        branches: vec![WorkspaceStackBranch {
            ref_name: "refs/heads/feature".try_into()?,
            archived: false,
        }],
    });
    Ok(workspace)
}

#[test]
fn standalone_metadata_mutation_reserves_writer_before_reading() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut db = DbHandle::new_in_directory(dir.path())?;
    let mut observer = DbHandle::new_in_directory(dir.path())?;
    let workspace = workspace(&db)?;

    for commit in [false, true] {
        let mutation = db.meta_mut()?;
        assert!(
            observer.immediate_transaction_nonblocking()?.is_none(),
            "another writer must not invalidate the metadata snapshot before its writes"
        );
        assert!(
            observer.meta()?.workspaces().next().is_none(),
            "readers remain able to inspect committed metadata"
        );
        if commit {
            mutation.set_workspace(WORKSPACE_REF_NAME.try_into()?, &workspace)?;
        } else {
            drop(mutation);
        }
        assert!(
            observer.immediate_transaction_nonblocking()?.is_some(),
            "committing or dropping the metadata mutation releases the writer lock"
        );
        assert_eq!(
            dir.path().join("REFRESH").exists(),
            commit,
            "only a committed metadata mutation notifies observers"
        );
    }
    Ok(())
}

#[test]
fn metadata_is_isolated_and_notifies_only_after_commit() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut db = DbHandle::new_in_directory(dir.path())?;
    let observer = DbHandle::new_in_directory(dir.path())?;
    let empty = db.meta()?;
    let workspace = workspace(&db)?;
    let name: FullName = "refs/heads/feature".try_into()?;
    // This value predates the workspace write and can be saved independently of it.
    let mut branch = empty.branch(name.as_ref()).cloned().unwrap_or_default();
    branch.review.pull_request = Some(42);
    let sentinel = dir.path().join("REFRESH");

    for commit in [false, true] {
        let mut tx = db.immediate_transaction()?;
        tx.meta_mut()?
            .set_workspace(WORKSPACE_REF_NAME.try_into()?, &workspace)?;
        tx.meta_mut()?.set_branch(name.as_ref(), &branch)?;
        assert_eq!(
            tx.meta()?
                .branch(name.as_ref())
                .expect("branch metadata was saved")
                .review
                .pull_request,
            Some(42),
            "writes are visible inside the transaction"
        );
        assert!(
            observer.meta()?.branch(name.as_ref()).is_none(),
            "other connections cannot see pending metadata"
        );
        assert!(
            !sentinel.exists(),
            "pending metadata must not notify observers"
        );
        if commit {
            tx.commit()?;
        } else {
            tx.rollback()?;
        }
        assert_eq!(
            sentinel.exists(),
            commit,
            "only committed metadata emits REFRESH"
        );
    }

    assert_eq!(
        observer
            .meta()?
            .branch(name.as_ref())
            .expect("branch metadata was saved")
            .review
            .pull_request,
        Some(42),
        "committed writes become visible through existing connections"
    );
    assert!(
        empty.branch(name.as_ref()).is_none(),
        "metadata snapshots do not change after reads"
    );
    assert!(
        !dir.path().join("virtual_branches.toml").exists(),
        "metadata never creates the former storage file"
    );
    std::fs::remove_file(&sentinel)?;
    {
        let mut tx = db.immediate_transaction()?;
        tx.meta_mut()?.remove(name.as_ref())?;
    }
    assert!(
        !sentinel.exists(),
        "dropping a transaction rolls back without notifying"
    );
    assert!(
        db.meta()?.branch(name.as_ref()).is_some(),
        "drop preserved committed metadata"
    );
    Ok(())
}

#[test]
fn invalid_workspace_preserves_metadata_and_order() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut db = DbHandle::new_in_directory(dir.path())?;
    let mut workspace = workspace(&db)?;
    db.meta_mut()?
        .set_workspace(WORKSPACE_REF_NAME.try_into()?, &workspace)?;
    let before = db.meta()?;
    let order_before = db.branch_order().get_snapshot()?;
    std::fs::remove_file(dir.path().join("REFRESH"))?;
    workspace.stacks[0].branches.insert(
        0,
        WorkspaceStackBranch {
            ref_name: "refs/heads/new-tip".try_into()?,
            archived: false,
        },
    );
    // Validation rejects the whole value before any writes become observable.
    workspace.stacks.push(WorkspaceStack {
        id: StackId::generate(),
        branches: Vec::new(),
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
    });
    assert!(
        db.meta_mut()?
            .set_workspace(WORKSPACE_REF_NAME.try_into()?, &workspace)
            .is_err(),
        "an empty incoming stack is invalid"
    );
    assert_eq!(
        db.meta()?,
        before,
        "failed workspace updates preserve all metadata"
    );
    assert_eq!(
        db.branch_order().get_snapshot()?,
        order_before,
        "rejected workspace values preserve explicit ordering"
    );
    assert!(
        !dir.path().join("REFRESH").exists(),
        "failed writes must not notify observers"
    );
    Ok(())
}

#[test]
fn worktree_adoption_composes_with_a_transaction() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let repo = gix::init(dir.path())?;
    let mut db = DbHandle::new_at_path(":memory:")?;
    for commit in [false, true] {
        let mut tx = db.immediate_transaction()?;
        let entries = but_db::worktrees::worktrees_with_state(&repo, &mut tx.connection_mut())?;
        assert!(
            entries.is_empty(),
            "the fixture contains no linked worktrees"
        );
        assert!(
            tx.worktree_meta().adoption_ran()?,
            "adoption is visible inside the transaction"
        );
        if commit {
            tx.commit()?;
        } else {
            tx.rollback()?;
        }
        assert_eq!(
            db.worktree_meta().adoption_ran()?,
            commit,
            "adoption follows the enclosing transaction's outcome"
        );
    }
    Ok(())
}

#[test]
fn rename_preserves_workspace_and_branch_values_and_rejects_collisions() -> anyhow::Result<()> {
    let mut db = DbHandle::new_at_path(":memory:")?;
    let old: FullName = "refs/heads/feature".try_into()?;
    let new: FullName = "refs/heads/renamed".try_into()?;
    let workspace_name: FullName = "refs/heads/custom-workspace".try_into()?;
    let mut value = workspace(&db)?;
    let branch = but_core::ref_metadata::Branch::default();
    db.meta_mut()?
        .set_workspace(workspace_name.as_ref(), &value)?;
    db.meta_mut()?.set_branch(old.as_ref(), &branch)?;
    db.meta_mut()?
        .set_branch_stack_order(std::slice::from_ref(&new))?;
    let before = db.meta()?;
    assert!(
        db.meta_mut()?.rename(old.as_ref(), new.as_ref()).is_err(),
        "renaming cannot overwrite an ordered ref"
    );
    assert_eq!(
        db.meta()?,
        before,
        "failed renames leave all metadata intact"
    );
    db.meta_mut()?.remove(new.as_ref())?;
    db.meta_mut()?.rename(old.as_ref(), new.as_ref())?;
    value.stacks[0].branches[0].ref_name = new.clone();
    assert_eq!(
        db.meta()?.workspace(workspace_name.as_ref()),
        Some(&value),
        "renaming updates full workspace memberships"
    );
    assert_eq!(
        db.meta()?.branch(new.as_ref()),
        Some(&branch),
        "renaming preserves all branch fields"
    );
    let new_workspace: FullName = "refs/heads/new-workspace".try_into()?;
    db.meta_mut()?
        .rename(workspace_name.as_ref(), new_workspace.as_ref())?;
    assert_eq!(
        db.meta()?.workspace(new_workspace.as_ref()),
        Some(&value),
        "workspace renaming cascades through every stack and branch"
    );
    Ok(())
}

#[test]
fn snapshot_restoration_is_atomic_and_preserves_all_refs() -> anyhow::Result<()> {
    let mut db = DbHandle::new_at_path(":memory:")?;
    let name: FullName = "refs/heads/custom-workspace".try_into()?;
    let value = workspace(&db)?;
    db.meta_mut()?.set_workspace(name.as_ref(), &value)?;
    let snapshot = db.meta()?;
    // Duplicate reference names are invalid even if the payload arrived through deserialization.
    let invalid = but_db::Metadata::from_parts(
        vec![(name.clone(), value.clone()), (name.clone(), value)],
        vec![],
        vec![],
    );
    assert!(
        db.meta_mut()?.replace_snapshot(&invalid).is_err(),
        "invalid snapshots fail before replacement starts"
    );
    assert_eq!(
        db.meta()?,
        snapshot,
        "failed restores preserve the complete prior snapshot"
    );
    db.meta_mut()?
        .replace_snapshot(&but_db::Metadata::default())?;
    assert!(
        db.meta()?.workspaces().next().is_none(),
        "explicit empty snapshots clear metadata"
    );
    db.meta_mut()?.replace_snapshot(&snapshot)?;
    assert_eq!(
        db.meta()?,
        snapshot,
        "snapshots roundtrip every ref and all workspace details"
    );
    Ok(())
}

#[test]
fn per_ref_migration_preserves_values_and_removes_singleton_tables() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("migration.sqlite");
    let mut conn = rusqlite::Connection::open(&path)?;
    // The final migration group replaces the singleton schema. Seed the previous schema directly
    // so the test exercises real upgrades without keeping its obsolete Rust structures alive.
    but_db::migration::run(
        &mut conn,
        but_db::MIGRATIONS[..but_db::MIGRATIONS.len() - 1]
            .iter()
            .flat_map(|group| group.iter())
            .copied(),
    )?;
    conn.execute_batch("INSERT INTO vb_state (id, initialized) VALUES (1, 1);
        INSERT INTO vb_stacks (id, sort_order, in_workspace) VALUES ('00000000-0000-0000-0000-000000000001', 7, 1), ('00000000-0000-0000-0000-000000000002', 3, 0);
        INSERT INTO vb_stack_heads (stack_id, position, name, head_sha, pr_number, archived, review_id) VALUES
        ('00000000-0000-0000-0000-000000000001', 0, 'base', '', 42, 1, 'review'),
        ('00000000-0000-0000-0000-000000000001', 1, 'tip', '', NULL, 0, NULL),
        ('00000000-0000-0000-0000-000000000002', 0, 'outside', '', NULL, 0, NULL);")?;
    let db = DbHandle::new_at_path(&path)?;
    let snapshot = db.meta()?;
    let workspace = snapshot
        .workspace(WORKSPACE_REF_NAME.try_into()?)
        .expect("existing singleton workspace was migrated");
    assert_eq!(
        workspace
            .stacks
            .iter()
            .map(|stack| stack.id)
            .collect::<Vec<_>>(),
        [
            StackId::from_number_for_testing(2),
            StackId::from_number_for_testing(1)
        ],
        "migration preserves display order and stack identity"
    );
    assert_eq!(
        workspace.stacks[0].workspacecommit_relation,
        WorkspaceCommitRelation::Outside,
        "unapplied stacks remain outside"
    );
    assert_eq!(
        workspace.stacks[1]
            .branches
            .iter()
            .map(|branch| branch.ref_name.shorten().to_string())
            .collect::<Vec<_>>(),
        ["tip", "base"],
        "old base-to-tip heads are migrated into tip-to-base order"
    );
    assert!(
        workspace.stacks[1].branches[1].archived,
        "archived membership survives migration"
    );
    assert_eq!(
        workspace.ref_info.created_at,
        Some(gix::date::Time::new(1675176957, 0)),
        "migration preserves the previous managed-workspace marker"
    );
    assert_eq!(
        snapshot
            .branch("refs/heads/base".try_into()?)
            .map(|branch| &branch.review),
        Some(&but_core::ref_metadata::Review {
            pull_request: Some(42),
            review_id: Some("review".into())
        }),
        "branch review data is independent from stack membership"
    );
    let tables: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name LIKE 'vb_%'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(
        tables, 0,
        "the old singleton tables must be removed entirely"
    );
    assert!(
        but_db::migration::run(
            &mut conn,
            but_db::MIGRATIONS[..but_db::MIGRATIONS.len() - 1]
                .iter()
                .flat_map(|group| group.iter())
                .copied()
        )
        .is_err(),
        "older binaries reject the incompatible database"
    );
    Ok(())
}

#[test]
fn migration_discards_workspace_derived_branch_order_and_preserves_ad_hoc_chains()
-> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("branch-order-migration.sqlite");
    let mut conn = rusqlite::Connection::open(&path)?;
    but_db::migration::run(
        &mut conn,
        but_db::MIGRATIONS[..but_db::MIGRATIONS.len() - 1]
            .iter()
            .flat_map(|group| group.iter())
            .copied(),
    )?;
    conn.execute_batch(
        "INSERT INTO vb_state (id, initialized) VALUES (1, 1);
        INSERT INTO vb_stacks (id, sort_order, in_workspace) VALUES
            ('00000000-0000-0000-0000-000000000001', 0, 1),
            ('00000000-0000-0000-0000-000000000002', 1, 0);
        INSERT INTO vb_stack_heads (stack_id, position, name, head_sha) VALUES
            ('00000000-0000-0000-0000-000000000001', 0, 'bottom', ''),
            ('00000000-0000-0000-0000-000000000001', 1, 'middle', ''),
            ('00000000-0000-0000-0000-000000000001', 2, 'top', ''),
            ('00000000-0000-0000-0000-000000000002', 0, 'extended-base', ''),
            ('00000000-0000-0000-0000-000000000002', 1, 'extended-tip', '');
        INSERT INTO branch_order (branch_ref_name, parent_ref_name) VALUES
            ('refs/heads/top', 'refs/heads/middle'),
            ('refs/heads/middle', 'refs/heads/bottom'),
            ('refs/heads/bottom', NULL),
            ('refs/heads/independent-tip', 'refs/heads/independent-base'),
            ('refs/heads/independent-base', NULL),
            ('refs/heads/extra', 'refs/heads/extended-tip'),
            ('refs/heads/extended-tip', 'refs/heads/extended-base'),
            ('refs/heads/extended-base', NULL);",
    )?;

    let mut db = DbHandle::new_at_path(&path)?;
    let workspace_ref = WORKSPACE_REF_NAME.try_into()?;
    let top: FullName = "refs/heads/top".try_into()?;
    let middle: FullName = "refs/heads/middle".try_into()?;
    let bottom: FullName = "refs/heads/bottom".try_into()?;
    let mut workspace = db
        .meta()?
        .workspace(workspace_ref)
        .expect("the legacy workspace was migrated")
        .clone();

    // The old workspace writer duplicated this chain into branch_order. After migration,
    // its order must follow workspace edits instead of acting as an explicit override.
    workspace.stacks[0].branches.reverse();
    db.meta_mut()?.set_workspace(workspace_ref, &workspace)?;
    assert_eq!(
        db.meta()?.branch_stack_order(top.as_ref()),
        Some([bottom.clone(), middle.clone(), top.clone()].as_slice()),
        "migrated workspace order must follow a later reorder"
    );

    let split_branches = workspace.stacks[0].branches.split_off(2);
    workspace.stacks.push(WorkspaceStack {
        id: StackId::from_number_for_testing(3),
        branches: split_branches,
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
    });
    db.meta_mut()?.set_workspace(workspace_ref, &workspace)?;
    let metadata = db.meta()?;
    assert_eq!(
        metadata.branch_stack_order(top.as_ref()),
        Some(std::slice::from_ref(&top)),
        "splitting a workspace stack must not restore its old chain"
    );
    assert_eq!(
        metadata.branch_stack_order(bottom.as_ref()),
        Some([bottom, middle].as_slice()),
        "the remaining workspace stack retains its new order"
    );

    let remaining_orders = metadata.branch_orders().collect::<Vec<_>>();
    assert_eq!(
        remaining_orders.len(),
        2,
        "only the independent and extended ad-hoc chains remain explicit"
    );
    for names in [
        &["independent-tip", "independent-base"][..],
        &["extra", "extended-tip", "extended-base"][..],
    ] {
        let expected = names
            .iter()
            .map(|name| format!("refs/heads/{name}").try_into())
            .collect::<Result<Vec<FullName>, _>>()?;
        assert!(
            remaining_orders.contains(&expected.as_slice()),
            "migration preserves independent ad-hoc orders, including a chain extending a workspace stack"
        );
    }
    Ok(())
}

#[test]
fn workspace_saves_touch_only_changed_rows_and_roll_back_sql_failures() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("metadata.sqlite");
    let mut db = DbHandle::new_at_path(&path)?;
    let mut value = workspace(&db)?;
    let name: FullName = WORKSPACE_REF_NAME.try_into()?;
    value.stacks[0].branches.push(WorkspaceStackBranch {
        ref_name: "refs/heads/base".try_into()?,
        archived: false,
    });
    db.meta_mut()?.set_workspace(name.as_ref(), &value)?;
    let sql = rusqlite::Connection::open(&path)?;
    sql.execute_batch("CREATE TABLE observed_writes (name TEXT NOT NULL);
        CREATE TRIGGER observe_workspace AFTER UPDATE ON workspace_metadata BEGIN INSERT INTO observed_writes VALUES ('workspace'); END;
        CREATE TRIGGER observe_stack AFTER UPDATE ON workspace_stacks BEGIN INSERT INTO observed_writes VALUES ('stack'); END;
        CREATE TRIGGER observe_branch AFTER UPDATE ON workspace_stack_branches BEGIN INSERT INTO observed_writes VALUES ('branch'); END;
        CREATE TRIGGER reject_bad_branch BEFORE INSERT ON workspace_stack_branches WHEN NEW.ref_name = CAST('refs/heads/reject' AS BLOB) BEGIN SELECT RAISE(ABORT, 'rejected branch'); END;")?;
    db.meta_mut()?.set_workspace(name.as_ref(), &value)?;
    let writes = || -> rusqlite::Result<i64> {
        sql.query_row("SELECT count(*) FROM observed_writes", [], |row| row.get(0))
    };
    assert_eq!(
        writes()?,
        0,
        "saving an unchanged Rust value does not update any rows"
    );
    value.stacks[0].branches[1].archived = true;
    db.meta_mut()?.set_workspace(name.as_ref(), &value)?;
    assert_eq!(
        writes()?,
        1,
        "one archived flag changes only its branch membership row"
    );
    let before = db.meta()?;
    value.stacks[0].branches[0].archived = true;
    value.stacks[0].branches[1].ref_name = "refs/heads/reject".try_into()?;
    assert!(
        db.meta_mut()?.set_workspace(name.as_ref(), &value).is_err(),
        "an SQL failure rejects the entire workspace value"
    );
    assert_eq!(
        db.meta()?,
        before,
        "earlier successful statements roll back with the rejected row"
    );
    assert_eq!(
        writes()?,
        1,
        "the write observer rolls back with failed writes"
    );
    Ok(())
}

#[test]
fn ad_hoc_order_is_unambiguous_and_does_not_mutate_workspace_order() -> anyhow::Result<()> {
    let mut db = DbHandle::new_at_path(":memory:")?;
    let first: FullName = "refs/heads/gitbutler/workspaces/first".try_into()?;
    let second: FullName = "refs/heads/gitbutler/workspaces/second".try_into()?;
    let mut value = workspace(&db)?;
    let tip = value.stacks[0].branches[0].ref_name.clone();
    let base: FullName = "refs/heads/base".try_into()?;
    value.stacks[0].branches.push(WorkspaceStackBranch {
        ref_name: base.clone(),
        archived: false,
    });
    db.meta_mut()?.set_workspace(first.as_ref(), &value)?;
    let mut reversed = value.clone();
    reversed.stacks[0].branches.reverse();
    db.meta_mut()?.set_workspace(second.as_ref(), &reversed)?;
    assert!(
        db.meta()?.branch_stack_order(tip.as_ref()).is_none(),
        "ad-hoc checkout must not pick between conflicting workspace orders"
    );
    db.meta_mut()?
        .set_branch_stack_order(&[tip.clone(), base.clone()])?;
    let metadata = db.meta()?;
    assert_eq!(
        metadata.branch_stack_order(tip.as_ref()),
        Some([tip.clone(), base].as_slice()),
        "explicit ad-hoc order resolves the ambiguity"
    );
    assert_eq!(
        metadata.workspace(second.as_ref()),
        Some(&reversed),
        "ad-hoc ordering does not overwrite a workspace's own order"
    );
    db.meta_mut()?.remove(first.as_ref())?;
    assert_eq!(
        db.meta()?.workspace(second.as_ref()),
        Some(&reversed),
        "deleting one workspace preserves the other's order"
    );
    Ok(())
}

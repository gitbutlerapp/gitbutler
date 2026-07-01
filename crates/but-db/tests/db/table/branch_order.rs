use crate::table::in_memory_db;

#[test]
fn adding_branch_above_another_updates_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/B", "refs/heads/C"]))?;

    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]))?;

    assert_eq!(
        db.branch_order().order_for_reference("refs/heads/B")?,
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"])),
        "inserting a branch above B should make it the new tip"
    );
    assert_eq!(
        db.branch_order().order_for_reference("refs/heads/A")?,
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"])),
        "the new top branch should resolve to the full chain"
    );
    Ok(())
}

#[test]
fn adding_branch_below_another_updates_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B"]))?;

    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]))?;

    assert_eq!(
        db.branch_order().order_for_reference("refs/heads/B")?,
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"])),
        "inserting a branch below B should make it B's parent"
    );
    assert_eq!(
        db.branch_order().order_for_reference("refs/heads/C")?,
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"])),
        "the new bottom branch should resolve to the full chain"
    );
    Ok(())
}

#[test]
fn adding_branch_between_two_branches_updates_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/C"]))?;

    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]))?;

    assert_eq!(
        db.branch_order().order_for_reference("refs/heads/B")?,
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"])),
        "inserting a branch between A and C should splice it into the chain"
    );
    Ok(())
}

#[test]
fn replacing_chain_with_shorter_order_drops_disconnected_tail() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?.set_order(&refs([
        "refs/heads/A",
        "refs/heads/B",
        "refs/heads/C",
        "refs/heads/D",
    ]))?;

    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/B", "refs/heads/C"]))?;

    assert_eq!(
        db.branch_order().order_for_reference("refs/heads/B")?,
        Some(refs(["refs/heads/B", "refs/heads/C"])),
        "replacement order should be the complete remaining chain"
    );
    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/A")?
            .is_none(),
        "the old chain head should not keep stale order metadata"
    );
    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/D")?
            .is_none(),
        "the old chain tail should not keep stale singleton metadata"
    );
    Ok(())
}

#[test]
fn removing_reference_splices_chain() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]))?;

    db.branch_order_mut()?.remove_reference("refs/heads/B")?;

    assert_eq!(
        db.branch_order().order_for_reference("refs/heads/A")?,
        Some(refs(["refs/heads/A", "refs/heads/C"])),
        "removing the middle branch should connect A directly to C"
    );
    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/B")?
            .is_none(),
        "the removed branch should no longer have order metadata"
    );
    Ok(())
}

#[test]
fn removing_top_from_two_branch_chain_removes_singleton_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B"]))?;

    db.branch_order_mut()?.remove_reference("refs/heads/A")?;

    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/B")?
            .is_none(),
        "a single remaining branch does not need durable order metadata"
    );
    Ok(())
}

#[test]
fn removing_bottom_from_two_branch_chain_removes_singleton_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B"]))?;

    db.branch_order_mut()?.remove_reference("refs/heads/B")?;

    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/A")?
            .is_none(),
        "a single remaining branch does not need durable order metadata"
    );
    Ok(())
}

#[test]
fn renaming_top_reference_preserves_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]))?;

    db.branch_order_mut()?
        .rename_reference("refs/heads/A", "refs/heads/renamed")?;

    assert_eq!(
        db.branch_order()
            .order_for_reference("refs/heads/renamed")?,
        Some(refs(["refs/heads/renamed", "refs/heads/B", "refs/heads/C"])),
        "renaming the top branch should preserve its child relationship"
    );
    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/A")?
            .is_none(),
        "old top branch metadata should be gone"
    );
    Ok(())
}

#[test]
fn renaming_middle_reference_preserves_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]))?;

    db.branch_order_mut()?
        .rename_reference("refs/heads/B", "refs/heads/renamed")?;

    assert_eq!(
        db.branch_order()
            .order_for_reference("refs/heads/renamed")?,
        Some(refs(["refs/heads/A", "refs/heads/renamed", "refs/heads/C"])),
        "renaming the middle branch should update both incoming and outgoing edges"
    );
    Ok(())
}

#[test]
fn renaming_bottom_reference_preserves_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]))?;

    db.branch_order_mut()?
        .rename_reference("refs/heads/C", "refs/heads/renamed")?;

    assert_eq!(
        db.branch_order()
            .order_for_reference("refs/heads/renamed")?,
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/renamed"])),
        "renaming the bottom branch should preserve its parent relationship"
    );
    Ok(())
}

#[test]
fn renaming_unordered_reference_is_noop() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B"]))?;

    db.branch_order_mut()?
        .rename_reference("refs/heads/unordered", "refs/heads/renamed")?;

    assert_eq!(
        db.branch_order().order_for_reference("refs/heads/A")?,
        Some(refs(["refs/heads/A", "refs/heads/B"])),
        "renaming an unordered ref should not change existing order metadata"
    );
    Ok(())
}

#[test]
fn renaming_to_ordered_reference_fails_without_partial_changes() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]))?;

    assert!(
        db.branch_order_mut()?
            .rename_reference("refs/heads/A", "refs/heads/C")
            .is_err(),
        "renaming onto another ordered ref should fail"
    );
    assert_eq!(
        db.branch_order().order_for_reference("refs/heads/B")?,
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"])),
        "failed rename should leave order metadata unchanged"
    );
    Ok(())
}

#[test]
fn removing_missing_top_reference_splices_chain() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]))?;

    db.branch_order_mut()?
        .remove_missing_references(&refs(["refs/heads/B", "refs/heads/C"]))?;

    assert_eq!(
        db.branch_order().order_for_reference("refs/heads/B")?,
        Some(refs(["refs/heads/B", "refs/heads/C"])),
        "missing top branch should be pruned from the chain"
    );
    Ok(())
}

#[test]
fn removing_missing_middle_reference_splices_chain() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]))?;

    db.branch_order_mut()?
        .remove_missing_references(&refs(["refs/heads/A", "refs/heads/C"]))?;

    assert_eq!(
        db.branch_order().order_for_reference("refs/heads/A")?,
        Some(refs(["refs/heads/A", "refs/heads/C"])),
        "missing middle branch should connect the surviving neighbors"
    );
    Ok(())
}

#[test]
fn removing_missing_bottom_reference_splices_chain() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]))?;

    db.branch_order_mut()?
        .remove_missing_references(&refs(["refs/heads/A", "refs/heads/B"]))?;

    assert_eq!(
        db.branch_order().order_for_reference("refs/heads/A")?,
        Some(refs(["refs/heads/A", "refs/heads/B"])),
        "missing bottom branch should be pruned from the chain"
    );
    Ok(())
}

#[test]
fn removing_missing_references_drops_singleton_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B"]))?;

    db.branch_order_mut()?
        .remove_missing_references(&refs(["refs/heads/A"]))?;

    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/A")?
            .is_none(),
        "a single surviving branch should not keep durable order metadata"
    );
    Ok(())
}

#[test]
fn cycle_in_order_chain_is_ignored() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let db_path = tmp.path().join("but.sqlite");
    let db = but_db::DbHandle::new_at_path(&db_path)?;
    drop(db);

    let conn = rusqlite::Connection::open(&db_path)?;
    conn.execute(
        "INSERT INTO branch_order (branch_ref_name, parent_ref_name) VALUES (?1, ?2)",
        ("refs/heads/A", "refs/heads/B"),
    )?;
    conn.execute(
        "INSERT INTO branch_order (branch_ref_name, parent_ref_name) VALUES (?1, ?2)",
        ("refs/heads/B", "refs/heads/A"),
    )?;
    drop(conn);

    let db = but_db::DbHandle::new_at_path(&db_path)?;
    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/A")?
            .is_none(),
        "cyclic branch-order metadata should be ignored instead of failing reads"
    );
    Ok(())
}

#[test]
fn chain_attached_to_cycle_is_ignored() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let db_path = tmp.path().join("but.sqlite");
    let db = but_db::DbHandle::new_at_path(&db_path)?;
    drop(db);

    let conn = rusqlite::Connection::open(&db_path)?;
    conn.execute(
        "INSERT INTO branch_order (branch_ref_name, parent_ref_name) VALUES (?1, ?2)",
        ("refs/heads/A", "refs/heads/B"),
    )?;
    conn.execute(
        "INSERT INTO branch_order (branch_ref_name, parent_ref_name) VALUES (?1, ?2)",
        ("refs/heads/B", "refs/heads/C"),
    )?;
    conn.execute(
        "INSERT INTO branch_order (branch_ref_name, parent_ref_name) VALUES (?1, ?2)",
        ("refs/heads/C", "refs/heads/A"),
    )?;
    drop(conn);

    let db = but_db::DbHandle::new_at_path(&db_path)?;
    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/B")?
            .is_none(),
        "a branch attached to cyclic branch-order metadata should degrade to unordered"
    );
    Ok(())
}

fn refs<const N: usize>(refs: [&str; N]) -> Vec<String> {
    refs.into_iter().map(ToOwned::to_owned).collect()
}

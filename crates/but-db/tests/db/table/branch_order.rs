use gix::refs::FullName;

use crate::table::in_memory_db;

#[test]
fn cyclic_snapshot_is_rejected_without_changing_rows() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?
        .set_branch_stack_order(&refs(["refs/heads/original", "refs/heads/base"]))?;
    let before = db.branch_order().get_snapshot()?;
    let invalid = but_db::BranchOrderSnapshot {
        entries: vec![
            but_db::BranchOrderEntry {
                branch_ref_name: "refs/heads/A".try_into()?,
                parent_ref_name: Some("refs/heads/B".try_into()?),
            },
            but_db::BranchOrderEntry {
                branch_ref_name: "refs/heads/B".try_into()?,
                parent_ref_name: Some("refs/heads/A".try_into()?),
            },
        ],
    };
    assert!(
        db.meta_mut()?.replace_branch_order(&invalid).is_err(),
        "a cyclic snapshot must be rejected before replacing existing rows"
    );
    assert_eq!(db.branch_order().get_snapshot()?, before);
    Ok(())
}

#[test]
fn invalid_snapshot_relationships_preserve_existing_rows() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?
        .set_branch_stack_order(&refs(["refs/heads/original", "refs/heads/base"]))?;
    let before = db.branch_order().get_snapshot()?;
    for entries in [
        vec![("refs/heads/A", None), ("refs/heads/A", None)],
        vec![
            ("refs/heads/A", Some("refs/heads/B")),
            ("refs/heads/C", Some("refs/heads/B")),
        ],
        vec![("refs/heads/A", Some("refs/heads/A"))],
    ] {
        let invalid = but_db::BranchOrderSnapshot {
            entries: entries
                .into_iter()
                .map(|(branch, parent)| {
                    Ok(but_db::BranchOrderEntry {
                        branch_ref_name: branch.try_into()?,
                        parent_ref_name: parent.map(TryInto::try_into).transpose()?,
                    })
                })
                .collect::<anyhow::Result<_>>()?,
        };
        assert!(
            invalid.validate().is_err(),
            "invalid branch-order relationships must fail public snapshot validation"
        );
        assert!(db.meta_mut()?.replace_branch_order(&invalid).is_err());
        assert_eq!(db.branch_order().get_snapshot()?, before);
    }
    Ok(())
}

#[test]
fn chains_preserve_terminal_parents_without_rows_and_sort_by_tip() -> anyhow::Result<()> {
    let snapshot = but_db::BranchOrderSnapshot {
        entries: [
            ("refs/heads/C", Some("refs/heads/D")),
            ("refs/heads/E", None),
            ("refs/heads/B", None),
            ("refs/heads/A", Some("refs/heads/B")),
        ]
        .into_iter()
        .map(|(branch, parent)| {
            Ok(but_db::BranchOrderEntry {
                branch_ref_name: branch.try_into()?,
                parent_ref_name: parent.map(TryInto::try_into).transpose()?,
            })
        })
        .collect::<anyhow::Result<_>>()?,
    };
    snapshot.validate()?;
    let mut db = in_memory_db();
    db.meta_mut()?.replace_branch_order(&snapshot)?;
    let meta = db.meta()?;
    let expected_chains = [
        refs(["refs/heads/A", "refs/heads/B"]),
        refs(["refs/heads/C", "refs/heads/D"]),
        refs(["refs/heads/E"]),
    ];
    assert_eq!(
        meta.branch_orders().collect::<Vec<_>>(),
        expected_chains
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>(),
        "restored chains preserve terminal parents and singletons and are sorted by tip"
    );
    let mut expected = snapshot.clone();
    expected
        .entries
        .sort_by(|left, right| left.branch_ref_name.cmp(&right.branch_ref_name));
    assert_eq!(
        db.branch_order().get_snapshot()?,
        expected,
        "restoration preserves exact rows without adding a row for the terminal parent"
    );
    assert_eq!(
        meta.branch_stack_order("refs/heads/D".try_into()?),
        Some(refs(["refs/heads/C", "refs/heads/D"]).as_slice()),
        "a terminal parent without a row still resolves to its chain"
    );
    assert_eq!(
        meta.branch_stack_order("refs/heads/E".try_into()?),
        Some(refs(["refs/heads/E"]).as_slice()),
        "an explicitly restored singleton remains queryable"
    );
    Ok(())
}

#[test]
fn snapshot_roundtrip_replaces_all_rows() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?
        .set_branch_stack_order(&refs(["refs/heads/A", "refs/heads/B"]))?;
    let snapshot = db.branch_order().get_snapshot()?;

    db.meta_mut()?
        .set_branch_stack_order(&refs(["refs/heads/C", "refs/heads/D"]))?;
    db.meta_mut()?.replace_branch_order(&snapshot)?;

    assert_eq!(
        db.branch_order().get_snapshot()?,
        snapshot,
        "restoring a snapshot should replace the complete table"
    );
    assert!(
        db.meta()?
            .branch_stack_order("refs/heads/C".try_into()?)
            .is_none(),
        "rows absent from the snapshot should be removed"
    );
    Ok(())
}

#[test]
fn empty_snapshot_clears_all_rows() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    let empty = db.branch_order().get_snapshot()?;
    db.meta_mut()?
        .set_branch_stack_order(&refs(["refs/heads/A", "refs/heads/B"]))?;

    db.meta_mut()?.replace_branch_order(&empty)?;

    assert_eq!(
        db.branch_order().get_snapshot()?,
        empty,
        "an explicitly empty snapshot should clear the table"
    );
    Ok(())
}

#[test]
fn adding_branch_above_another_updates_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?
        .set_branch_stack_order(&refs(["refs/heads/B", "refs/heads/C"]))?;

    db.meta_mut()?.set_branch_stack_order(&refs([
        "refs/heads/A",
        "refs/heads/B",
        "refs/heads/C",
    ]))?;

    assert_eq!(
        db.meta()?.branch_stack_order("refs/heads/B".try_into()?),
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]).as_slice()),
        "inserting a branch above B should make it the new tip"
    );
    assert_eq!(
        db.meta()?.branch_stack_order("refs/heads/A".try_into()?),
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]).as_slice()),
        "the new top branch should resolve to the full chain"
    );
    Ok(())
}

#[test]
fn adding_branch_below_another_updates_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?
        .set_branch_stack_order(&refs(["refs/heads/A", "refs/heads/B"]))?;

    db.meta_mut()?.set_branch_stack_order(&refs([
        "refs/heads/A",
        "refs/heads/B",
        "refs/heads/C",
    ]))?;

    assert_eq!(
        db.meta()?.branch_stack_order("refs/heads/B".try_into()?),
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]).as_slice()),
        "inserting a branch below B should make it B's parent"
    );
    assert_eq!(
        db.meta()?.branch_stack_order("refs/heads/C".try_into()?),
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]).as_slice()),
        "the new bottom branch should resolve to the full chain"
    );
    Ok(())
}

#[test]
fn adding_branch_between_two_branches_updates_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?
        .set_branch_stack_order(&refs(["refs/heads/A", "refs/heads/C"]))?;

    db.meta_mut()?.set_branch_stack_order(&refs([
        "refs/heads/A",
        "refs/heads/B",
        "refs/heads/C",
    ]))?;

    assert_eq!(
        db.meta()?.branch_stack_order("refs/heads/B".try_into()?),
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]).as_slice()),
        "inserting a branch between A and C should splice it into the chain"
    );
    Ok(())
}

#[test]
fn replacing_chain_with_shorter_order_drops_disconnected_tail() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?.set_branch_stack_order(&refs([
        "refs/heads/A",
        "refs/heads/B",
        "refs/heads/C",
        "refs/heads/D",
    ]))?;

    db.meta_mut()?
        .set_branch_stack_order(&refs(["refs/heads/B", "refs/heads/C"]))?;

    assert_eq!(
        db.meta()?.branch_stack_order("refs/heads/B".try_into()?),
        Some(refs(["refs/heads/B", "refs/heads/C"]).as_slice()),
        "replacement order should be the complete remaining chain"
    );
    assert!(
        db.meta()?
            .branch_stack_order("refs/heads/A".try_into()?)
            .is_none(),
        "the old chain head should not keep stale order metadata"
    );
    assert!(
        db.meta()?
            .branch_stack_order("refs/heads/D".try_into()?)
            .is_none(),
        "the old chain tail should not keep stale singleton metadata"
    );
    Ok(())
}

#[test]
fn removing_reference_splices_chain() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?.set_branch_stack_order(&refs([
        "refs/heads/A",
        "refs/heads/B",
        "refs/heads/C",
    ]))?;

    db.meta_mut()?.remove("refs/heads/B".try_into()?)?;

    assert_eq!(
        db.meta()?.branch_stack_order("refs/heads/A".try_into()?),
        Some(refs(["refs/heads/A", "refs/heads/C"]).as_slice()),
        "removing the middle branch should connect A directly to C"
    );
    assert!(
        db.meta()?
            .branch_stack_order("refs/heads/B".try_into()?)
            .is_none(),
        "the removed branch should no longer have order metadata"
    );
    Ok(())
}

#[test]
fn removing_top_from_two_branch_chain_removes_singleton_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?
        .set_branch_stack_order(&refs(["refs/heads/A", "refs/heads/B"]))?;

    db.meta_mut()?.remove("refs/heads/A".try_into()?)?;

    assert!(
        db.meta()?
            .branch_stack_order("refs/heads/B".try_into()?)
            .is_none(),
        "a single remaining branch does not need durable order metadata"
    );
    Ok(())
}

#[test]
fn removing_bottom_from_two_branch_chain_removes_singleton_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?
        .set_branch_stack_order(&refs(["refs/heads/A", "refs/heads/B"]))?;

    db.meta_mut()?.remove("refs/heads/B".try_into()?)?;

    assert!(
        db.meta()?
            .branch_stack_order("refs/heads/A".try_into()?)
            .is_none(),
        "a single remaining branch does not need durable order metadata"
    );
    Ok(())
}

#[test]
fn renaming_top_reference_preserves_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?.set_branch_stack_order(&refs([
        "refs/heads/A",
        "refs/heads/B",
        "refs/heads/C",
    ]))?;

    db.meta_mut()?
        .rename("refs/heads/A".try_into()?, "refs/heads/renamed".try_into()?)?;

    assert_eq!(
        db.meta()?
            .branch_stack_order("refs/heads/renamed".try_into()?),
        Some(refs(["refs/heads/renamed", "refs/heads/B", "refs/heads/C"]).as_slice()),
        "renaming the top branch should preserve its child relationship"
    );
    assert!(
        db.meta()?
            .branch_stack_order("refs/heads/A".try_into()?)
            .is_none(),
        "old top branch metadata should be gone"
    );
    Ok(())
}

#[test]
fn renaming_middle_reference_preserves_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?.set_branch_stack_order(&refs([
        "refs/heads/A",
        "refs/heads/B",
        "refs/heads/C",
    ]))?;

    db.meta_mut()?
        .rename("refs/heads/B".try_into()?, "refs/heads/renamed".try_into()?)?;

    assert_eq!(
        db.meta()?
            .branch_stack_order("refs/heads/renamed".try_into()?),
        Some(refs(["refs/heads/A", "refs/heads/renamed", "refs/heads/C"]).as_slice()),
        "renaming the middle branch should update both incoming and outgoing edges"
    );
    Ok(())
}

#[test]
fn renaming_bottom_reference_preserves_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?.set_branch_stack_order(&refs([
        "refs/heads/A",
        "refs/heads/B",
        "refs/heads/C",
    ]))?;

    db.meta_mut()?
        .rename("refs/heads/C".try_into()?, "refs/heads/renamed".try_into()?)?;

    assert_eq!(
        db.meta()?
            .branch_stack_order("refs/heads/renamed".try_into()?),
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/renamed"]).as_slice()),
        "renaming the bottom branch should preserve its parent relationship"
    );
    Ok(())
}

#[test]
fn renaming_unordered_reference_is_noop() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?
        .set_branch_stack_order(&refs(["refs/heads/A", "refs/heads/B"]))?;

    db.meta_mut()?.rename(
        "refs/heads/unordered".try_into()?,
        "refs/heads/renamed".try_into()?,
    )?;

    assert_eq!(
        db.meta()?.branch_stack_order("refs/heads/A".try_into()?),
        Some(refs(["refs/heads/A", "refs/heads/B"]).as_slice()),
        "renaming an unordered ref should not change existing order metadata"
    );
    Ok(())
}

#[test]
fn removing_missing_top_reference_splices_chain() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?.set_branch_stack_order(&refs([
        "refs/heads/A",
        "refs/heads/B",
        "refs/heads/C",
    ]))?;

    db.meta_mut()?
        .remove_missing_branch_stack_order_references(&refs(["refs/heads/B", "refs/heads/C"]))?;

    assert_eq!(
        db.meta()?.branch_stack_order("refs/heads/B".try_into()?),
        Some(refs(["refs/heads/B", "refs/heads/C"]).as_slice()),
        "missing top branch should be pruned from the chain"
    );
    Ok(())
}

#[test]
fn removing_missing_middle_reference_splices_chain() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?.set_branch_stack_order(&refs([
        "refs/heads/A",
        "refs/heads/B",
        "refs/heads/C",
    ]))?;

    db.meta_mut()?
        .remove_missing_branch_stack_order_references(&refs(["refs/heads/A", "refs/heads/C"]))?;

    assert_eq!(
        db.meta()?.branch_stack_order("refs/heads/A".try_into()?),
        Some(refs(["refs/heads/A", "refs/heads/C"]).as_slice()),
        "missing middle branch should connect the surviving neighbors"
    );
    Ok(())
}

#[test]
fn removing_missing_bottom_reference_splices_chain() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?.set_branch_stack_order(&refs([
        "refs/heads/A",
        "refs/heads/B",
        "refs/heads/C",
    ]))?;

    db.meta_mut()?
        .remove_missing_branch_stack_order_references(&refs(["refs/heads/A", "refs/heads/B"]))?;

    assert_eq!(
        db.meta()?.branch_stack_order("refs/heads/A".try_into()?),
        Some(refs(["refs/heads/A", "refs/heads/B"]).as_slice()),
        "missing bottom branch should be pruned from the chain"
    );
    Ok(())
}

#[test]
fn removing_missing_references_drops_singleton_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.meta_mut()?
        .set_branch_stack_order(&refs(["refs/heads/A", "refs/heads/B"]))?;

    db.meta_mut()?
        .remove_missing_branch_stack_order_references(&refs(["refs/heads/A"]))?;

    assert!(
        db.meta()?
            .branch_stack_order("refs/heads/A".try_into()?)
            .is_none(),
        "a single surviving branch should not keep durable order metadata"
    );
    Ok(())
}

#[test]
fn cycle_in_order_chain_is_rejected() -> anyhow::Result<()> {
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
        db.meta().is_err(),
        "cyclic branch-order metadata must fail reads"
    );
    Ok(())
}

#[test]
fn longer_cycle_in_order_chain_is_rejected() -> anyhow::Result<()> {
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
        db.meta().is_err(),
        "cyclic branch-order metadata must fail reads"
    );
    Ok(())
}

fn refs<const N: usize>(refs: [&str; N]) -> Vec<FullName> {
    refs.into_iter()
        .map(|name| name.try_into().expect("valid test ref name"))
        .collect()
}

use gix::refs::FullName;

use crate::table::in_memory_db;

#[test]
fn cyclic_snapshot_is_rejected_without_changing_rows() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/original", "refs/heads/base"]))?;
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
        db.branch_order_mut()?.replace_snapshot(&invalid).is_err(),
        "a cyclic snapshot must be rejected before replacing existing rows"
    );
    assert_eq!(db.branch_order().get_snapshot()?, before);
    Ok(())
}

#[test]
fn invalid_snapshot_relationships_preserve_existing_rows() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/original", "refs/heads/base"]))?;
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
        assert!(invalid.clone().into_chains().is_err());
        assert!(db.branch_order_mut()?.replace_snapshot(&invalid).is_err());
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
    assert_eq!(
        snapshot.clone().into_chains()?,
        vec![
            refs(["refs/heads/A", "refs/heads/B"]),
            refs(["refs/heads/C", "refs/heads/D"]),
            refs(["refs/heads/E"]),
        ]
    );
    let mut db = in_memory_db();
    db.branch_order_mut()?.replace_snapshot(&snapshot)?;
    assert_eq!(db.branch_order().get_snapshot()?.entries.len(), 4);
    assert_eq!(
        db.branch_order()
            .order_for_reference("refs/heads/D".try_into()?)?,
        Some(refs(["refs/heads/C", "refs/heads/D"])),
        "a terminal parent without a row still resolves to its chain"
    );
    Ok(())
}

#[test]
fn non_utf8_names_survive_order_mutations() -> anyhow::Result<()> {
    use gix::bstr::ByteSlice as _;

    let mut db = in_memory_db();
    let top: FullName = b"refs/heads/top-\xff".as_bstr().try_into()?;
    let base: FullName = b"refs/heads/base-\xfe".as_bstr().try_into()?;
    let middle: FullName = b"refs/heads/middle-\xfc".as_bstr().try_into()?;
    let renamed: FullName = b"refs/heads/renamed-\xfd".as_bstr().try_into()?;
    db.branch_order_mut()?
        .set_order(&[top.clone(), base.clone()])?;
    let before = db.branch_order().get_snapshot()?;
    db.branch_order_mut()?
        .rename_reference(top.as_ref(), renamed.as_ref())?;
    assert_eq!(
        db.branch_order().order_for_reference(base.as_ref())?,
        Some(vec![renamed.clone(), base.clone()])
    );

    db.branch_order_mut()?
        .set_order(&[renamed.clone(), middle.clone(), base.clone()])?;
    db.branch_order_mut()?.remove_reference(middle.as_ref())?;
    assert_eq!(
        db.branch_order().order_for_reference(base.as_ref())?,
        Some(vec![renamed.clone(), base.clone()])
    );
    db.branch_order_mut()?
        .remove_missing_references(std::slice::from_ref(&renamed))?;
    assert!(
        db.branch_order()
            .order_for_reference(renamed.as_ref())?
            .is_none()
    );

    db.branch_order_mut()?.replace_snapshot(&before)?;
    assert_eq!(
        db.branch_order().order_for_reference(top.as_ref())?,
        Some(vec![top, base])
    );
    Ok(())
}

#[test]
fn snapshot_roundtrip_replaces_all_rows() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B"]))?;
    let snapshot = db.branch_order().get_snapshot()?;

    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/C", "refs/heads/D"]))?;
    db.branch_order_mut()?.replace_snapshot(&snapshot)?;

    assert_eq!(
        db.branch_order().get_snapshot()?,
        snapshot,
        "restoring a snapshot should replace the complete table"
    );
    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/C".try_into()?)?
            .is_none(),
        "rows absent from the snapshot should be removed"
    );
    Ok(())
}

#[test]
fn empty_snapshot_clears_all_rows() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    let empty = db.branch_order().get_snapshot()?;
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B"]))?;

    db.branch_order_mut()?.replace_snapshot(&empty)?;

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
    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/B", "refs/heads/C"]))?;

    db.branch_order_mut()?
        .set_order(&refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"]))?;

    assert_eq!(
        db.branch_order()
            .order_for_reference("refs/heads/B".try_into()?)?,
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"])),
        "inserting a branch above B should make it the new tip"
    );
    assert_eq!(
        db.branch_order()
            .order_for_reference("refs/heads/A".try_into()?)?,
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
        db.branch_order()
            .order_for_reference("refs/heads/B".try_into()?)?,
        Some(refs(["refs/heads/A", "refs/heads/B", "refs/heads/C"])),
        "inserting a branch below B should make it B's parent"
    );
    assert_eq!(
        db.branch_order()
            .order_for_reference("refs/heads/C".try_into()?)?,
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
        db.branch_order()
            .order_for_reference("refs/heads/B".try_into()?)?,
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
        db.branch_order()
            .order_for_reference("refs/heads/B".try_into()?)?,
        Some(refs(["refs/heads/B", "refs/heads/C"])),
        "replacement order should be the complete remaining chain"
    );
    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/A".try_into()?)?
            .is_none(),
        "the old chain head should not keep stale order metadata"
    );
    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/D".try_into()?)?
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

    db.branch_order_mut()?
        .remove_reference("refs/heads/B".try_into()?)?;

    assert_eq!(
        db.branch_order()
            .order_for_reference("refs/heads/A".try_into()?)?,
        Some(refs(["refs/heads/A", "refs/heads/C"])),
        "removing the middle branch should connect A directly to C"
    );
    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/B".try_into()?)?
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

    db.branch_order_mut()?
        .remove_reference("refs/heads/A".try_into()?)?;

    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/B".try_into()?)?
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

    db.branch_order_mut()?
        .remove_reference("refs/heads/B".try_into()?)?;

    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/A".try_into()?)?
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
        .rename_reference("refs/heads/A".try_into()?, "refs/heads/renamed".try_into()?)?;

    assert_eq!(
        db.branch_order()
            .order_for_reference("refs/heads/renamed".try_into()?)?,
        Some(refs(["refs/heads/renamed", "refs/heads/B", "refs/heads/C"])),
        "renaming the top branch should preserve its child relationship"
    );
    assert!(
        db.branch_order()
            .order_for_reference("refs/heads/A".try_into()?)?
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
        .rename_reference("refs/heads/B".try_into()?, "refs/heads/renamed".try_into()?)?;

    assert_eq!(
        db.branch_order()
            .order_for_reference("refs/heads/renamed".try_into()?)?,
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
        .rename_reference("refs/heads/C".try_into()?, "refs/heads/renamed".try_into()?)?;

    assert_eq!(
        db.branch_order()
            .order_for_reference("refs/heads/renamed".try_into()?)?,
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

    db.branch_order_mut()?.rename_reference(
        "refs/heads/unordered".try_into()?,
        "refs/heads/renamed".try_into()?,
    )?;

    assert_eq!(
        db.branch_order()
            .order_for_reference("refs/heads/A".try_into()?)?,
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
            .rename_reference("refs/heads/A".try_into()?, "refs/heads/C".try_into()?)
            .is_err(),
        "renaming onto another ordered ref should fail"
    );
    assert_eq!(
        db.branch_order()
            .order_for_reference("refs/heads/B".try_into()?)?,
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
        db.branch_order()
            .order_for_reference("refs/heads/B".try_into()?)?,
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
        db.branch_order()
            .order_for_reference("refs/heads/A".try_into()?)?,
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
        db.branch_order()
            .order_for_reference("refs/heads/A".try_into()?)?,
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
            .order_for_reference("refs/heads/A".try_into()?)?
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
        db.branch_order()
            .order_for_reference("refs/heads/A".try_into()?)
            .is_err(),
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
        db.branch_order()
            .order_for_reference("refs/heads/B".try_into()?)
            .is_err(),
        "cyclic branch-order metadata must fail reads"
    );
    Ok(())
}

fn refs<const N: usize>(refs: [&str; N]) -> Vec<FullName> {
    refs.into_iter()
        .map(|name| name.try_into().expect("valid test ref name"))
        .collect()
}

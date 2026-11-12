use but_db::ObsMarker;

use crate::table::in_memory_db;

fn marker(id: &[u8], prec: &[u8], succs: &[&[u8]]) -> ObsMarker {
    ObsMarker {
        id: id.to_vec(),
        prec: prec.to_vec(),
        succs: succs.iter().map(|succ| succ.to_vec()).collect(),
        meta: vec![
            (b"operation".to_vec(), b"amend".to_vec()),
            (b"user".to_vec(), b"me".to_vec()),
        ],
        date_seconds: 1_000,
        date_offset: -3_600,
        parents: Some(vec![b"p2".to_vec(), b"p1".to_vec()]),
    }
}

fn insert(db: &mut but_db::DbHandle, markers: &[ObsMarker]) -> anyhow::Result<()> {
    let mut tx = db.immediate_transaction()?;
    tx.obs_markers_mut().insert(markers)?;
    tx.commit()?;
    Ok(())
}

#[test]
fn find_nothing() -> anyhow::Result<()> {
    let db = in_memory_db();
    assert_eq!(db.obs_markers().find_by_prec(b"missing")?, vec![]);
    assert_eq!(db.obs_markers().find_by_succ(b"missing")?, vec![]);
    Ok(())
}

#[test]
fn round_trips_through_both_lookups_in_field_order() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    let split = marker(b"id", b"prec", &[b"z", b"a"]);
    insert(&mut db, std::slice::from_ref(&split))?;

    assert_eq!(db.obs_markers().find_by_prec(b"prec")?, vec![split.clone()]);
    assert_eq!(db.obs_markers().find_by_succ(b"z")?, vec![split.clone()]);
    assert_eq!(db.obs_markers().find_by_succ(b"a")?, vec![split]);
    Ok(())
}

#[test]
fn competing_markers_are_all_found() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    let first = marker(b"1", b"prec", &[b"a"]);
    let second = marker(b"2", b"prec", &[b"b"]);
    insert(&mut db, &[first.clone(), second.clone()])?;

    assert_eq!(db.obs_markers().find_by_prec(b"prec")?, vec![first, second]);
    Ok(())
}

#[test]
fn a_repeated_successor_finds_its_marker_once() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    let repeated = marker(b"id", b"prec", &[b"a", b"a"]);
    insert(&mut db, std::slice::from_ref(&repeated))?;

    assert_eq!(db.obs_markers().find_by_succ(b"a")?, vec![repeated]);
    Ok(())
}

#[test]
fn inserting_a_known_id_is_a_no_op() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    let stored = marker(b"id", b"prec", &[b"a"]);
    insert(&mut db, std::slice::from_ref(&stored))?;
    insert(&mut db, &[stored.clone(), stored.clone()])?;

    assert_eq!(db.obs_markers().find_by_prec(b"prec")?, vec![stored]);
    Ok(())
}

#[test]
fn unrecorded_parents_differ_from_a_root_commit() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    let unrecorded = ObsMarker {
        parents: None,
        ..marker(b"unrecorded", b"unrecorded-prec", &[])
    };
    let root = ObsMarker {
        parents: Some(vec![]),
        ..marker(b"root", b"root-prec", &[])
    };
    insert(&mut db, &[unrecorded.clone(), root.clone()])?;

    assert_eq!(
        db.obs_markers().find_by_prec(b"unrecorded-prec")?,
        vec![unrecorded]
    );
    assert_eq!(db.obs_markers().find_by_prec(b"root-prec")?, vec![root]);
    Ok(())
}

#[test]
fn a_dropped_transaction_stores_nothing() -> anyhow::Result<()> {
    let mut db = in_memory_db();
    {
        let mut tx = db.immediate_transaction()?;
        tx.obs_markers_mut()
            .insert(&[marker(b"id", b"prec", &[b"a"])])?;
    }

    assert_eq!(db.obs_markers().find_by_prec(b"prec")?, vec![]);
    Ok(())
}

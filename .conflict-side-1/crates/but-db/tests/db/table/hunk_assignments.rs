use but_db::HunkAssignment;

use crate::table::in_memory_db;

#[test]
fn insert_and_read() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let hunk1 = hunk_assignment("id1", Some("@@ -1,3 +1,4 @@"), "path/to/file.txt", None);
    let hunk2 = hunk_assignment("id2", Some("@@ -10,5 +11,6 @@"), "path/to/file.txt", None);
    let hunk3 = hunk_assignment("id3", Some("@@ -20,2 +21,3 @@"), "path/to/other.txt", None);

    db.hunk_assignments_mut()?
        .set_all(vec![hunk1.clone(), hunk2.clone(), hunk3.clone()])?;

    let assignments = db.hunk_assignments().list_all()?;
    assert_eq!(assignments.len(), 3);
    assert!(assignments.contains(&hunk1));
    assert!(assignments.contains(&hunk2));
    assert!(assignments.contains(&hunk3));

    Ok(())
}

#[test]
fn set_all_replaces_existing() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let hunk1 = hunk_assignment("id1", Some("@@ -1,3 +1,4 @@"), "path/to/file.txt", None);
    let hunk2 = hunk_assignment("id2", Some("@@ -10,5 +11,6 @@"), "path/to/file.txt", None);
    let hunk3 = hunk_assignment("id3", Some("@@ -1,3 +1,4 @@"), "different.txt", None);

    db.hunk_assignments_mut()?
        .set_all(vec![hunk1.clone(), hunk2.clone()])?;

    let assignments = db.hunk_assignments().list_all()?;
    assert_eq!(assignments.len(), 2);

    db.hunk_assignments_mut()?.set_all(vec![hunk3.clone()])?;

    let assignments = db.hunk_assignments().list_all()?;
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0], hunk3);

    Ok(())
}

#[test]
fn set_all_replaces_existing_with_outer_transaction_and_rollback() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let mut trans = db.transaction()?;

    let assignments = trans.hunk_assignments().list_all()?;
    assert!(assignments.is_empty(), "it starts out empty");

    let hunk1 = hunk_assignment("id1", Some("@@ -1,3 +1,4 @@"), "path/to/file.txt", None);
    let hunk2 = hunk_assignment("id2", Some("@@ -10,5 +11,6 @@"), "path/to/file.txt", None);
    let hunk3 = hunk_assignment("id3", Some("@@ -1,3 +1,4 @@"), "different.txt", None);

    trans
        .hunk_assignments_mut()?
        .set_all(vec![hunk1.clone(), hunk2.clone()])?;

    trans.hunk_assignments_mut()?.set_all(vec![hunk3.clone()])?;

    let assignments = trans.hunk_assignments().list_all()?;
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0], hunk3);

    trans.rollback()?;

    let assignments = db.hunk_assignments().list_all()?;
    assert!(
        assignments.is_empty(),
        "while the change was observable in the transaction, after rollback it's gone"
    );

    Ok(())
}

#[test]
fn set_empty_clears_the_table() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let hunk1 = hunk_assignment("id1", Some("@@ -1,3 +1,4 @@"), "path/to/file.txt", None);

    db.hunk_assignments_mut()?.set_all(vec![hunk1])?;

    let assignments = db.hunk_assignments().list_all()?;
    assert_eq!(assignments.len(), 1);

    db.hunk_assignments_mut()?.set_all(vec![])?;

    let assignments = db.hunk_assignments().list_all()?;
    assert!(assignments.is_empty());

    Ok(())
}

#[test]
fn optional_fields() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let hunk_no_header = HunkAssignment {
        id: Some("id1".to_string()),
        hunk_header: None,
        path: "path/to/file.txt".to_string(),
        path_bytes: b"path/to/file.txt".to_vec(),
        stack_id: None,
        branch_ref_bytes: None,
    };

    let hunk_no_branch = HunkAssignment {
        id: Some("id2".to_string()),
        hunk_header: Some("@@ -1,3 +1,4 @@".to_string()),
        path: "path/to/other.txt".to_string(),
        path_bytes: b"path/to/other.txt".to_vec(),
        stack_id: None,
        branch_ref_bytes: None,
    };

    let hunk_no_id = HunkAssignment {
        id: None,
        hunk_header: Some("@@ -10,5 +11,6 @@".to_string()),
        path: "path/to/third.txt".to_string(),
        path_bytes: b"path/to/third.txt".to_vec(),
        stack_id: None,
        branch_ref_bytes: None,
    };

    db.hunk_assignments_mut()?.set_all(vec![
        hunk_no_header.clone(),
        hunk_no_branch.clone(),
        hunk_no_id.clone(),
    ])?;

    let assignments = db.hunk_assignments().list_all()?;
    assert_eq!(assignments.len(), 3);
    assert!(assignments.contains(&hunk_no_header));
    assert!(assignments.contains(&hunk_no_branch));
    assert!(assignments.contains(&hunk_no_id));

    Ok(())
}

#[test]
fn non_utf8_path_bytes() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let hunk_with_non_utf8 = HunkAssignment {
        id: Some("id1".to_string()),
        hunk_header: Some("@@ -1,3 +1,4 @@".to_string()),
        path: "valid/path.txt".to_string(),
        path_bytes: vec![0xFF, 0xFE, 0xFD, 0x00, 0x80],
        stack_id: None,
        branch_ref_bytes: None,
    };

    db.hunk_assignments_mut()?
        .set_all(vec![hunk_with_non_utf8.clone()])?;

    let assignments = db.hunk_assignments().list_all()?;
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0], hunk_with_non_utf8);
    assert_eq!(
        assignments[0].path_bytes,
        vec![0xFF, 0xFE, 0xFD, 0x00, 0x80]
    );

    Ok(())
}

#[test]
fn branch_ref_bytes_roundtrip() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let hunk_with_branch = hunk_assignment(
        "id1",
        Some("@@ -1,3 +1,4 @@"),
        "path/to/file.txt",
        Some("refs/heads/feature"),
    );
    let hunk_without_branch =
        hunk_assignment("id2", Some("@@ -10,5 +11,6 @@"), "path/to/file.txt", None);

    db.hunk_assignments_mut()?
        .set_all(vec![hunk_with_branch.clone(), hunk_without_branch.clone()])?;

    let assignments = db.hunk_assignments().list_all()?;
    assert_eq!(assignments.len(), 2);
    assert!(assignments.contains(&hunk_with_branch));
    assert!(assignments.contains(&hunk_without_branch));

    let with_branch = assignments
        .iter()
        .find(|a| a.id.as_deref() == Some("id1"))
        .unwrap();
    assert_eq!(
        with_branch.branch_ref_bytes.as_deref(),
        Some(b"refs/heads/feature".as_slice())
    );

    let without_branch = assignments
        .iter()
        .find(|a| a.id.as_deref() == Some("id2"))
        .unwrap();
    assert_eq!(without_branch.branch_ref_bytes, None);

    Ok(())
}

#[test]
fn legacy_stack_id_fallback() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let db_path = tmp.path().join("test.db");

    {
        let mut conn = rusqlite::Connection::open(&db_path)?;
        but_db::migration::run(&mut conn, but_db::migration::ours())?;
        conn.execute(
            "INSERT INTO hunk_assignments (id, hunk_header, path, path_bytes, stack_id) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                "legacy-id",
                "@@ -1,3 +1,4 @@",
                "legacy/file.txt",
                b"legacy/file.txt".to_vec(),
                "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
            ],
        )?;
    }

    let db = but_db::DbHandle::new_at_path(&db_path)?;
    let assignments = db.hunk_assignments().list_all()?;
    assert_eq!(assignments.len(), 1);
    assert_eq!(
        assignments[0].stack_id.as_deref(),
        Some("a1b2c3d4-e5f6-7890-abcd-ef1234567890")
    );
    assert_eq!(assignments[0].branch_ref_bytes, None);

    Ok(())
}

fn hunk_assignment(
    id: &str,
    hunk_header: Option<&str>,
    path: &str,
    branch_ref_bytes: Option<&str>,
) -> HunkAssignment {
    HunkAssignment {
        id: Some(id.to_string()),
        hunk_header: hunk_header.map(String::from),
        path: path.to_string(),
        path_bytes: path.as_bytes().to_vec(),
        stack_id: None,
        branch_ref_bytes: branch_ref_bytes.map(|s| s.as_bytes().to_vec()),
    }
}

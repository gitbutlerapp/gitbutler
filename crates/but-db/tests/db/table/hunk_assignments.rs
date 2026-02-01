use but_db::HunkAssignment;

use crate::table::in_memory_db;

#[test]
fn insert_and_read() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let hunk1 = hunk_assignment(
        "id1",
        Some("@@ -1,3 +1,4 @@"),
        "path/to/file.txt",
        Some("stack1"),
    );
    let hunk2 = hunk_assignment(
        "id2",
        Some("@@ -10,5 +11,6 @@"),
        "path/to/file.txt",
        Some("stack1"),
    );
    let hunk3 = hunk_assignment(
        "id3",
        Some("@@ -20,2 +21,3 @@"),
        "path/to/other.txt",
        Some("stack2"),
    );

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

    let hunk1 = hunk_assignment(
        "id1",
        Some("@@ -1,3 +1,4 @@"),
        "path/to/file.txt",
        Some("stack1"),
    );
    let hunk2 = hunk_assignment(
        "id2",
        Some("@@ -10,5 +11,6 @@"),
        "path/to/file.txt",
        Some("stack1"),
    );
    let hunk3 = hunk_assignment(
        "id3",
        Some("@@ -1,3 +1,4 @@"),
        "different.txt",
        Some("stack2"),
    );

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

    let hunk1 = hunk_assignment(
        "id1",
        Some("@@ -1,3 +1,4 @@"),
        "path/to/file.txt",
        Some("stack1"),
    );
    let hunk2 = hunk_assignment(
        "id2",
        Some("@@ -10,5 +11,6 @@"),
        "path/to/file.txt",
        Some("stack1"),
    );
    let hunk3 = hunk_assignment(
        "id3",
        Some("@@ -1,3 +1,4 @@"),
        "different.txt",
        Some("stack2"),
    );

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

    let hunk1 = hunk_assignment(
        "id1",
        Some("@@ -1,3 +1,4 @@"),
        "path/to/file.txt",
        Some("stack1"),
    );

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
        stack_id: Some("stack1".to_string()),
    };

    let hunk_no_stack = HunkAssignment {
        id: Some("id2".to_string()),
        hunk_header: Some("@@ -1,3 +1,4 @@".to_string()),
        path: "path/to/other.txt".to_string(),
        path_bytes: b"path/to/other.txt".to_vec(),
        stack_id: None,
    };

    let hunk_no_id = HunkAssignment {
        id: None,
        hunk_header: Some("@@ -10,5 +11,6 @@".to_string()),
        path: "path/to/third.txt".to_string(),
        path_bytes: b"path/to/third.txt".to_vec(),
        stack_id: Some("stack2".to_string()),
    };

    db.hunk_assignments_mut()?.set_all(vec![
        hunk_no_header.clone(),
        hunk_no_stack.clone(),
        hunk_no_id.clone(),
    ])?;

    let assignments = db.hunk_assignments().list_all()?;
    assert_eq!(assignments.len(), 3);
    assert!(assignments.contains(&hunk_no_header));
    assert!(assignments.contains(&hunk_no_stack));
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
        stack_id: Some("stack1".to_string()),
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

fn hunk_assignment(
    id: &str,
    hunk_header: Option<&str>,
    path: &str,
    stack_id: Option<&str>,
) -> HunkAssignment {
    HunkAssignment {
        id: Some(id.to_string()),
        hunk_header: hunk_header.map(String::from),
        path: path.to_string(),
        path_bytes: path.as_bytes().to_vec(),
        stack_id: stack_id.map(String::from),
    }
}

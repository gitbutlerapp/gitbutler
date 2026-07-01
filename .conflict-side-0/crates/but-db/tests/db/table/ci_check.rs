use but_db::CiCheck;

use crate::table::in_memory_db;

#[test]
fn insert_and_read() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let check1 = ci_check(1, "refs/heads/main");
    let check2 = ci_check(2, "refs/heads/main");
    let check3 = ci_check(3, "refs/heads/feature");

    // Insert checks using rusqlite
    db.ci_checks_mut()?
        .set_for_reference("refs/heads/main", vec![check1.clone(), check2.clone()])?;
    db.ci_checks_mut()?
        .set_for_reference("refs/heads/feature", vec![check3.clone()])?;

    // Read checks using rusqlite
    let main_checks = db.ci_checks().list_for_reference("refs/heads/main")?;
    assert_eq!(main_checks.len(), 2);
    assert_eq!(main_checks[0], check1);
    assert_eq!(main_checks[1], check2);

    let feature_checks = db.ci_checks().list_for_reference("refs/heads/feature")?;
    assert_eq!(feature_checks.len(), 1);
    assert_eq!(feature_checks[0], check3);

    Ok(())
}

#[test]
fn set_replaces_existing_with_outer_transaction() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let mut trans = db.transaction()?;

    let check1 = ci_check(1, "refs/heads/main");
    let check2 = ci_check(2, "refs/heads/main");
    let check3 = ci_check(3, "refs/heads/main");

    // Insert initial checks
    trans
        .ci_checks_mut()?
        .set_for_reference("refs/heads/main", vec![check1.clone(), check2.clone()])?;

    // Replace with different checks
    trans
        .ci_checks_mut()?
        .set_for_reference("refs/heads/main", vec![check3.clone()])?;

    // Should only have the new check
    let checks = trans.ci_checks().list_for_reference("refs/heads/main")?;
    assert_eq!(checks[0], check3);

    trans.commit()?;

    let checks = db.ci_checks().list_for_reference("refs/heads/main")?;
    assert_eq!(checks[0], check3);

    Ok(())
}

#[test]
fn set_replaces_existing() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let check1 = ci_check(1, "refs/heads/main");
    let check2 = ci_check(2, "refs/heads/main");
    let check3 = ci_check(3, "refs/heads/main");

    // Insert initial checks
    db.ci_checks_mut()?
        .set_for_reference("refs/heads/main", vec![check1.clone(), check2.clone()])?;

    // Replace with different checks
    db.ci_checks_mut()?
        .set_for_reference("refs/heads/main", vec![check3.clone()])?;

    // Should only have the new check
    let checks = db.ci_checks().list_for_reference("refs/heads/main")?;
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0], check3);

    Ok(())
}

#[test]
fn empty_list() -> anyhow::Result<()> {
    let db = in_memory_db();

    // Read from non-existent reference
    let checks = db
        .ci_checks()
        .list_for_reference("refs/heads/nonexistent")?;
    assert!(checks.is_empty());

    Ok(())
}

#[test]
fn set_empty() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let check1 = ci_check(1, "refs/heads/main");

    // Insert a check
    db.ci_checks_mut()?
        .set_for_reference("refs/heads/main", vec![check1])?;

    // Clear it with empty vec
    db.ci_checks_mut()?
        .set_for_reference("refs/heads/main", vec![])?;

    // Should be empty now
    let checks = db.ci_checks().list_for_reference("refs/heads/main")?;
    assert!(checks.is_empty());

    Ok(())
}

#[test]
fn list_all_references() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let check1 = ci_check(1, "refs/heads/main");
    let check2 = ci_check(2, "refs/heads/feature");
    let check3 = ci_check(3, "refs/heads/dev");

    // Insert checks for different references
    {
        let mut tx = db.transaction()?;
        tx.ci_checks_mut()?
            .set_for_reference("refs/heads/main", vec![check1])?;
        tx.commit()?;
    }
    {
        let mut tx = db.transaction()?;
        tx.ci_checks_mut()?
            .set_for_reference("refs/heads/feature", vec![check2])?;
        tx.commit()?;
    }
    {
        let mut tx = db.transaction()?;
        tx.ci_checks_mut()?
            .set_for_reference("refs/heads/dev", vec![check3])?;
        tx.commit()?;
    }

    // List all references
    {
        let tx = db.transaction()?;
        // It follows insertion order, and transactions don't change the result.
        let references = tx.ci_checks().list_all_references()?;

        assert_eq!(references.len(), 3);
        assert_eq!(references[0], "refs/heads/dev");
        assert_eq!(references[1], "refs/heads/feature");
        assert_eq!(references[2], "refs/heads/main");
    }

    Ok(())
}

#[test]
fn delete_for_reference() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let check1 = ci_check(1, "refs/heads/main");
    let check2 = ci_check(2, "refs/heads/feature");

    // Insert checks
    {
        let mut tx = db.transaction()?;
        tx.ci_checks_mut()?
            .set_for_reference("refs/heads/main", vec![check1])?;
        tx.commit()?;
    }
    {
        let mut tx = db.transaction()?;
        tx.ci_checks_mut()?
            .set_for_reference("refs/heads/feature", vec![check2.clone()])?;
        tx.commit()?;
    }

    // Delete main reference
    {
        let mut tx = db.transaction()?;
        tx.ci_checks_mut()?
            .delete_for_reference("refs/heads/main")?;
        tx.commit()?;
    }

    // Main should be empty, feature should still exist
    {
        let tx = db.transaction()?;
        let main_checks = tx.ci_checks().list_for_reference("refs/heads/main")?;
        assert!(main_checks.is_empty());

        let feature_checks = tx.ci_checks().list_for_reference("refs/heads/feature")?;
        assert_eq!(feature_checks.len(), 1);
        assert_eq!(feature_checks[0], check2);
    }

    Ok(())
}

fn ci_check(id: i64, ref_name: &str) -> CiCheck {
    CiCheck {
        id,
        name: format!("Test Check {id}"),
        output_summary: "Summary".to_string(),
        output_text: "Output text".to_string(),
        output_title: "Title".to_string(),
        started_at: Some(
            chrono::DateTime::from_timestamp(1000000, 0)
                .unwrap()
                .naive_utc(),
        ),
        status_type: "completed".to_string(),
        status_conclusion: Some("success".to_string()),
        status_completed_at: Some(
            chrono::DateTime::from_timestamp(1000100, 0)
                .unwrap()
                .naive_utc(),
        ),
        head_sha: "abc123".to_string(),
        url: "https://example.com/check".to_string(),
        html_url: "https://example.com/check/html".to_string(),
        details_url: "https://example.com/check/details".to_string(),
        pull_requests: "[]".to_string(),
        reference: ref_name.to_string(),
        last_sync_at: chrono::DateTime::from_timestamp(1000200, 0)
            .unwrap()
            .naive_utc(),
        struct_version: 1,
    }
}

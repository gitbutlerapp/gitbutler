use but_db::GerritMeta;

use crate::table::in_memory_db;

#[test]
fn get_nonexistent() -> anyhow::Result<()> {
    let db = in_memory_db();

    let result = db.gerrit_metadata().get("I1234567890abcdef")?;
    assert!(result.is_none());

    Ok(())
}

#[test]
fn insert_and_get() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let meta = gerrit_meta(
        "I1234567890abcdef",
        "commit123",
        "https://gerrit.example.com/1",
    );

    db.gerrit_metadata_mut().insert(meta.clone())?;

    let retrieved = db.gerrit_metadata().get(&meta.change_id)?;
    assert_eq!(retrieved, Some(meta));

    Ok(())
}

#[test]
fn update_existing() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let meta = gerrit_meta(
        "I1234567890abcdef",
        "commit123",
        "https://gerrit.example.com/1",
    );

    db.gerrit_metadata_mut().insert(meta.clone())?;

    let updated_meta = GerritMeta {
        change_id: meta.change_id.clone(),
        commit_id: "commit456".to_string(),
        review_url: "https://gerrit.example.com/2".to_string(),
        created_at: meta.created_at,
        updated_at: chrono::DateTime::from_timestamp(2000000, 0)
            .unwrap()
            .naive_utc(),
    };

    db.gerrit_metadata_mut().update(updated_meta.clone())?;

    let retrieved = db.gerrit_metadata().get(&meta.change_id)?;
    assert_eq!(retrieved, Some(updated_meta));

    Ok(())
}

#[test]
fn with_transaction() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let meta = gerrit_meta(
        "I1234567890abcdef",
        "commit123",
        "https://gerrit.example.com/1",
    );

    let mut trans = db.transaction()?;
    trans.gerrit_metadata_mut().insert(meta.clone())?;

    let retrieved_in_trans = trans.gerrit_metadata().get(&meta.change_id)?;
    assert_eq!(retrieved_in_trans, Some(meta.clone()));

    trans.commit()?;

    let retrieved_after_commit = db.gerrit_metadata().get(&meta.change_id)?;
    assert_eq!(retrieved_after_commit, Some(meta));

    Ok(())
}

#[test]
fn transaction_rollback() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let meta1 = gerrit_meta(
        "I1234567890abcdef",
        "commit123",
        "https://gerrit.example.com/1",
    );
    db.gerrit_metadata_mut().insert(meta1.clone())?;

    let meta2 = gerrit_meta(
        "I9999999999999999",
        "commit999",
        "https://gerrit.example.com/999",
    );
    let mut trans = db.transaction()?;
    trans.gerrit_metadata_mut().insert(meta2.clone())?;
    trans.rollback()?;

    let retrieved1 = db.gerrit_metadata().get(&meta1.change_id)?;
    let retrieved2 = db.gerrit_metadata().get(&meta2.change_id)?;

    assert_eq!(retrieved1, Some(meta1));
    assert_eq!(retrieved2, None);

    Ok(())
}

fn gerrit_meta(change_id: &str, commit_id: &str, review_url: &str) -> GerritMeta {
    GerritMeta {
        change_id: change_id.to_string(),
        commit_id: commit_id.to_string(),
        review_url: review_url.to_string(),
        created_at: chrono::DateTime::from_timestamp(1000000, 0)
            .unwrap()
            .naive_utc(),
        updated_at: chrono::DateTime::from_timestamp(1000000, 0)
            .unwrap()
            .naive_utc(),
    }
}

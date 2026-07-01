use but_db::ButlerAction;

use crate::table::in_memory_db;

#[test]
fn insert_and_list() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let action1 = butler_action(
        "id1",
        "handler1",
        "external_summary1",
        "snapshot_before1",
        "snapshot_after1",
    );
    let action2 = butler_action(
        "id2",
        "handler2",
        "external_summary2",
        "snapshot_before2",
        "snapshot_after2",
    );

    db.butler_actions_mut().insert(action1.clone())?;
    db.butler_actions_mut().insert(action2.clone())?;

    let (total, actions) = db.butler_actions().list(0, 10)?;

    assert_eq!(total, 2);
    assert_eq!(actions.len(), 2);
    // Most recent first (action2 was inserted second, has later timestamp)
    assert_eq!(actions[0].id, action2.id);
    assert_eq!(actions[1].id, action1.id);

    Ok(())
}

#[test]
fn list_with_pagination() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let mut expected = Vec::new();
    for i in 0..5 {
        let action = butler_action(
            &format!("id{i}"),
            "handler",
            "summary",
            "snapshot_before",
            "snapshot_after",
        );
        expected.insert(0, action.clone());
        db.butler_actions_mut().insert(action)?;
    }

    let (total, actual) = db.butler_actions().list(0, 10)?;
    assert_eq!(total, 5);
    assert_eq!(actual, expected);

    let (total, actual) = db.butler_actions().list(0, 2)?;
    assert_eq!(total, 5);
    assert_eq!(actual.len(), 2);
    assert_eq!(actual, &expected[..2]);

    let (total, actual) = db.butler_actions().list(2, 2)?;
    assert_eq!(total, 5);
    assert_eq!(actual.len(), 2);
    assert_eq!(actual, &expected[2..][..2]);

    let (total, actual) = db.butler_actions().list(4, 2)?;
    assert_eq!(total, 5);
    assert_eq!(actual.len(), 1);
    assert_eq!(actual, &expected[4..][..1]);

    Ok(())
}

#[test]
fn list_empty() -> anyhow::Result<()> {
    let db = in_memory_db();

    let (total, actions) = db.butler_actions().list(0, 10)?;

    assert_eq!(total, 0);
    assert_eq!(actions.len(), 0);

    Ok(())
}

#[test]
fn with_transaction() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let action = butler_action(
        "id1",
        "handler1",
        "external_summary1",
        "snapshot_before1",
        "snapshot_after1",
    );

    let mut trans = db.transaction()?;
    trans.butler_actions_mut().insert(action.clone())?;

    let (total_in_trans, actions_in_trans) = trans.butler_actions().list(0, 10)?;
    assert_eq!(total_in_trans, 1);
    assert_eq!(actions_in_trans.len(), 1);
    assert_eq!(actions_in_trans[0], action);

    trans.commit()?;

    let (total_after_commit, actions_after_commit) = db.butler_actions().list(0, 10)?;
    assert_eq!(total_after_commit, 1);
    assert_eq!(actions_after_commit.len(), 1);
    assert_eq!(actions_after_commit[0], action);

    Ok(())
}

#[test]
fn transaction_rollback() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let action1 = butler_action(
        "id1",
        "handler1",
        "external_summary1",
        "snapshot_before1",
        "snapshot_after1",
    );
    db.butler_actions_mut().insert(action1.clone())?;

    let action2 = butler_action(
        "id2",
        "handler2",
        "external_summary2",
        "snapshot_before2",
        "snapshot_after2",
    );
    let mut trans = db.transaction()?;
    trans.butler_actions_mut().insert(action2)?;
    let (total, actions) = trans.butler_actions().list(0, 10)?;
    assert_eq!(total, 2);
    assert_eq!(actions.len(), 2, "observable result of the insertion");
    trans.rollback()?;

    let (total, actions) = db.butler_actions().list(0, 10)?;

    assert_eq!(total, 1);
    assert_eq!(
        actions.len(),
        1,
        "after the rollback, it's like nothing happened"
    );
    assert_eq!(actions[0], action1);

    Ok(())
}

fn butler_action(
    id: &str,
    handler: &str,
    external_summary: &str,
    snapshot_before: &str,
    snapshot_after: &str,
) -> ButlerAction {
    ButlerAction {
        id: id.to_string(),
        created_at: chrono::DateTime::from_timestamp(1000000, 0)
            .unwrap()
            .naive_utc(),
        external_prompt: None,
        external_summary: external_summary.to_string(),
        handler: handler.to_string(),
        snapshot_before: snapshot_before.to_string(),
        snapshot_after: snapshot_after.to_string(),
        response: None,
        error: None,
        source: None,
    }
}

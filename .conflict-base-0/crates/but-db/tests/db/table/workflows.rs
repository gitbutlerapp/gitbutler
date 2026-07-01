use but_db::Workflow;

use crate::table::in_memory_db;

#[test]
fn insert_and_list() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let workflow1 = create_workflow("wf1", "rebase", "user");
    let workflow2 = create_workflow("wf2", "merge", "auto");

    db.workflows_mut().insert(workflow1.clone())?;
    db.workflows_mut().insert(workflow2.clone())?;

    let (total, workflows) = db.workflows().list(0, 10)?;
    assert_eq!(total, 2);
    assert_eq!(workflows.len(), 2);

    // Should be ordered by created_at DESC, but since we use the same timestamp,
    // we'll just check both are present
    assert!(workflows.contains(&workflow1));
    assert!(workflows.contains(&workflow2));

    Ok(())
}

#[test]
fn list_with_pagination() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    // Insert 5 workflows
    let mut expected = Vec::new();
    for i in 0..5 {
        let workflow = create_workflow(&format!("wf{i}"), "rebase", "user");
        expected.push(workflow.clone());
        db.workflows_mut().insert(workflow)?;
    }

    // Get first 2
    let (total, workflows) = db.workflows().list(0, 2)?;
    assert_eq!(total, 5);
    assert_eq!(workflows, &expected[0..2]);

    // Get next 2
    let (total, workflows) = db.workflows().list(2, 2)?;
    assert_eq!(total, 5);
    assert_eq!(workflows, &expected[2..][..2]);

    // Get last 1
    let (total, workflows) = db.workflows().list(4, 2)?;
    assert_eq!(total, 5);
    assert_eq!(workflows, &expected[4..][..1]);

    Ok(())
}

#[test]
fn list_empty() -> anyhow::Result<()> {
    let db = in_memory_db();

    let (total, workflows) = db.workflows().list(0, 10)?;
    assert_eq!(total, 0);
    assert_eq!(workflows.len(), 0);

    Ok(())
}

#[test]
fn with_transaction() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let workflow = create_workflow("wf1", "rebase", "user");

    let mut trans = db.transaction()?;
    trans.workflows_mut().insert(workflow.clone())?;

    let (total, workflows) = trans.workflows().list(0, 10)?;
    assert_eq!(total, 1);
    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0], workflow);

    trans.commit()?;

    let (total, workflows) = db.workflows().list(0, 10)?;
    assert_eq!(total, 1);
    assert_eq!(workflows[0], workflow);

    Ok(())
}

#[test]
fn transaction_rollback() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let workflow1 = create_workflow("wf1", "rebase", "user");
    db.workflows_mut().insert(workflow1.clone())?;

    let workflow2 = create_workflow("wf2", "merge", "auto");
    let mut trans = db.transaction()?;
    trans.workflows_mut().insert(workflow2)?;
    trans.rollback()?;

    let (total, workflows) = db.workflows().list(0, 10)?;
    assert_eq!(total, 1);
    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0], workflow1);

    Ok(())
}

fn create_workflow(id: &str, kind: &str, triggered_by: &str) -> Workflow {
    Workflow {
        id: id.to_string(),
        created_at: chrono::DateTime::from_timestamp(1000000, 0)
            .unwrap()
            .naive_utc(),
        kind: kind.to_string(),
        triggered_by: triggered_by.to_string(),
        status: "completed".to_string(),
        input_commits: "abc123".to_string(),
        output_commits: "def456".to_string(),
        summary: None,
    }
}

use but_db::WorkspaceRule;

use crate::table::in_memory_db;

#[test]
fn get_nonexistent() -> anyhow::Result<()> {
    let db = in_memory_db();

    let result = db.workspace_rules().get("nonexistent-id")?;
    assert!(result.is_none());

    Ok(())
}

#[test]
fn insert_and_get() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let rule = workspace_rule("rule-1", true, "on_push", "branch:main", "run_tests");

    db.workspace_rules_mut().insert(rule.clone())?;

    let retrieved = db.workspace_rules().get(&rule.id)?;
    assert_eq!(retrieved, Some(rule));

    Ok(())
}

#[test]
fn list_empty() -> anyhow::Result<()> {
    let db = in_memory_db();

    let rules = db.workspace_rules().list()?;
    assert!(rules.is_empty());

    Ok(())
}

#[test]
fn list_multiple() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let rule1 = workspace_rule("rule-1", true, "on_push", "branch:main", "run_tests");
    let rule2 = workspace_rule("rule-2", false, "on_pr", "label:urgent", "notify");
    let rule3 = workspace_rule("rule-3", true, "on_commit", "author:alice", "auto_merge");

    db.workspace_rules_mut().insert(rule1.clone())?;
    db.workspace_rules_mut().insert(rule2.clone())?;
    db.workspace_rules_mut().insert(rule3.clone())?;

    let rules = db.workspace_rules().list()?;
    assert_eq!(rules.len(), 3);
    assert!(rules.contains(&rule1));
    assert!(rules.contains(&rule2));
    assert!(rules.contains(&rule3));

    Ok(())
}

#[test]
fn update_existing() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let rule = workspace_rule("rule-1", true, "on_push", "branch:main", "run_tests");

    db.workspace_rules_mut().insert(rule.clone())?;

    let updated_rule = WorkspaceRule {
        id: rule.id.clone(),
        created_at: rule.created_at,
        enabled: false,
        trigger: "on_pr".to_string(),
        filters: "label:bug".to_string(),
        action: "notify_team".to_string(),
    };

    db.workspace_rules_mut()
        .update(&rule.id, updated_rule.clone())?;

    let retrieved = db.workspace_rules().get(&rule.id)?;
    assert_eq!(retrieved, Some(updated_rule));

    Ok(())
}

#[test]
fn delete_existing() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let rule1 = workspace_rule("rule-1", true, "on_push", "branch:main", "run_tests");
    let rule2 = workspace_rule("rule-2", false, "on_pr", "label:urgent", "notify");

    db.workspace_rules_mut().insert(rule1.clone())?;
    db.workspace_rules_mut().insert(rule2.clone())?;

    db.workspace_rules_mut().delete(&rule1.id)?;

    let retrieved1 = db.workspace_rules().get(&rule1.id)?;
    let retrieved2 = db.workspace_rules().get(&rule2.id)?;

    assert_eq!(retrieved1, None);
    assert_eq!(retrieved2, Some(rule2));

    let rules = db.workspace_rules().list()?;
    assert_eq!(rules.len(), 1);

    Ok(())
}

#[test]
fn delete_nonexistent() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    // Deleting non-existent rule should not error
    db.workspace_rules_mut().delete("nonexistent-id")?;

    Ok(())
}

#[test]
fn with_transaction() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let rule = workspace_rule("rule-1", true, "on_push", "branch:main", "run_tests");

    let mut trans = db.transaction()?;
    trans.workspace_rules_mut().insert(rule.clone())?;

    let retrieved_in_trans = trans.workspace_rules().get(&rule.id)?;
    assert_eq!(retrieved_in_trans, Some(rule.clone()));

    trans.commit()?;

    let retrieved_after_commit = db.workspace_rules().get(&rule.id)?;
    assert_eq!(retrieved_after_commit, Some(rule));

    Ok(())
}

#[test]
fn transaction_rollback() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let rule1 = workspace_rule("rule-1", true, "on_push", "branch:main", "run_tests");
    db.workspace_rules_mut().insert(rule1.clone())?;

    let rule2 = workspace_rule("rule-2", false, "on_pr", "label:urgent", "notify");
    let mut trans = db.transaction()?;
    trans.workspace_rules_mut().insert(rule2.clone())?;
    trans.rollback()?;

    let retrieved1 = db.workspace_rules().get(&rule1.id)?;
    let retrieved2 = db.workspace_rules().get(&rule2.id)?;

    assert_eq!(retrieved1, Some(rule1));
    assert_eq!(retrieved2, None);

    Ok(())
}

fn workspace_rule(
    id: &str,
    enabled: bool,
    trigger: &str,
    filters: &str,
    action: &str,
) -> WorkspaceRule {
    WorkspaceRule {
        id: id.to_string(),
        created_at: chrono::DateTime::from_timestamp(1000000, 0)
            .unwrap()
            .naive_utc(),
        enabled,
        trigger: trigger.to_string(),
        filters: filters.to_string(),
        action: action.to_string(),
    }
}

use but_db::{ClaudeMessage, ClaudePermissionRequest, ClaudeSession};

use crate::table::in_memory_db;

// ====== Claude Sessions Tests ======

#[test]
fn session_get_nonexistent() -> anyhow::Result<()> {
    let db = in_memory_db();

    let result = db.claude().get_session("nonexistent")?;
    assert!(result.is_none());

    Ok(())
}

#[test]
fn session_insert_and_get() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session = claude_session("sess-1", "current-1");

    db.claude_mut().insert_session(session.clone())?;

    let retrieved = db.claude().get_session(&session.id)?;
    assert_eq!(retrieved, Some(session));

    Ok(())
}

#[test]
fn session_get_by_current_id() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session = claude_session("sess-1", "current-1");

    db.claude_mut().insert_session(session.clone())?;

    let retrieved = db.claude().get_session_by_current_id(&session.current_id)?;
    assert_eq!(retrieved, Some(session));

    Ok(())
}

#[test]
fn session_update_current_id() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session = claude_session("sess-1", "current-1");
    db.claude_mut().insert_session(session.clone())?;

    db.claude_mut()
        .update_session_current_id(&session.id, "new-current")?;

    let retrieved = db.claude().get_session(&session.id)?;
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.current_id, "new-current");
    assert_ne!(retrieved.updated_at, session.updated_at);

    Ok(())
}

#[test]
fn session_update_session_ids() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session = claude_session("sess-1", "current-1");
    db.claude_mut().insert_session(session.clone())?;

    let new_session_ids = r#"["id1", "id2", "id3"]"#;
    db.claude_mut()
        .update_session_ids(&session.id, new_session_ids)?;

    let retrieved = db.claude().get_session(&session.id)?;
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().session_ids, new_session_ids);

    Ok(())
}

#[test]
fn session_update_in_gui() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session = claude_session("sess-1", "current-1");
    db.claude_mut().insert_session(session.clone())?;

    db.claude_mut().update_session_in_gui(&session.id, true)?;

    let retrieved = db.claude().get_session(&session.id)?;
    assert!(retrieved.is_some());
    assert!(retrieved.unwrap().in_gui);

    Ok(())
}

#[test]
fn session_update_permissions() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session = claude_session("sess-1", "current-1");
    db.claude_mut().insert_session(session.clone())?;

    let approved = r#"["read", "write"]"#;
    let denied = r#"["delete"]"#;
    db.claude_mut()
        .update_session_permissions(&session.id, approved, denied)?;

    let retrieved = db.claude().get_session(&session.id)?;
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.approved_permissions, approved);
    assert_eq!(retrieved.denied_permissions, denied);

    Ok(())
}

#[test]
fn session_delete() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session = claude_session("sess-1", "current-1");
    db.claude_mut().insert_session(session.clone())?;

    let retrieved = db.claude().get_session(&session.id)?;
    assert!(retrieved.is_some());

    db.claude_mut().delete_session(&session.id)?;

    let retrieved = db.claude().get_session(&session.id)?;
    assert!(retrieved.is_none());

    Ok(())
}

#[test]
fn session_with_transaction() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session = claude_session("sess-1", "current-1");

    let mut trans = db.transaction()?;
    trans.claude_mut().insert_session(session.clone())?;

    let retrieved_in_trans = trans.claude().get_session(&session.id)?;
    assert_eq!(retrieved_in_trans, Some(session.clone()));

    trans.commit()?;

    let retrieved_after_commit = db.claude().get_session(&session.id)?;
    assert_eq!(retrieved_after_commit, Some(session));

    Ok(())
}

#[test]
fn session_transaction_rollback() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session1 = claude_session("sess-1", "current-1");
    db.claude_mut().insert_session(session1.clone())?;

    let session2 = claude_session("sess-2", "current-2");
    let mut trans = db.transaction()?;
    trans.claude_mut().insert_session(session2.clone())?;
    trans.rollback()?;

    let retrieved1 = db.claude().get_session(&session1.id)?;
    let retrieved2 = db.claude().get_session(&session2.id)?;

    assert_eq!(retrieved1, Some(session1));
    assert_eq!(retrieved2, None);

    Ok(())
}

// ====== Claude Messages Tests ======

#[test]
fn message_insert_and_list() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session = claude_session("sess-1", "current-1");
    db.claude_mut().insert_session(session.clone())?;

    let message1 = claude_message("msg-1", &session.id, "user", "Hello");
    let message2 = claude_message("msg-2", &session.id, "assistant", "Hi there");

    db.claude_mut().insert_message(message1.clone())?;
    db.claude_mut().insert_message(message2.clone())?;

    let messages = db.claude().list_messages_by_session(&session.id)?;
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0], message1);
    assert_eq!(messages[1], message2);

    Ok(())
}

#[test]
fn message_list_by_session_empty() -> anyhow::Result<()> {
    let db = in_memory_db();

    let messages = db.claude().list_messages_by_session("nonexistent")?;
    assert!(messages.is_empty());

    Ok(())
}

#[test]
fn message_get_message_of_type() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session = claude_session("sess-1", "current-1");
    db.claude_mut().insert_session(session.clone())?;

    let message1 = claude_message("msg-1", &session.id, "user", "First");
    let message2 = claude_message("msg-2", &session.id, "user", "Second");
    let message3 = claude_message("msg-3", &session.id, "assistant", "Response");

    db.claude_mut().insert_message(message1.clone())?;
    db.claude_mut().insert_message(message2.clone())?;
    db.claude_mut().insert_message(message3.clone())?;

    let most_recent_user = db.claude().get_message_of_type("user".to_string(), None)?;
    assert_eq!(most_recent_user, Some(message2));

    let second_most_recent_user = db
        .claude()
        .get_message_of_type("user".to_string(), Some(1))?;
    assert_eq!(second_most_recent_user, Some(message1));

    Ok(())
}

#[test]
fn message_delete_by_session() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session1 = claude_session("sess-1", "current-1");
    let session2 = claude_session("sess-2", "current-2");
    db.claude_mut().insert_session(session1.clone())?;
    db.claude_mut().insert_session(session2.clone())?;

    let message1 = claude_message("msg-1", &session1.id, "user", "Hello");
    let message2 = claude_message("msg-2", &session1.id, "assistant", "Hi");
    let message3 = claude_message("msg-3", &session2.id, "user", "Other");

    db.claude_mut().insert_message(message1)?;
    db.claude_mut().insert_message(message2)?;
    db.claude_mut().insert_message(message3.clone())?;

    db.claude_mut().delete_messages_by_session(&session1.id)?;

    let messages1 = db.claude().list_messages_by_session(&session1.id)?;
    let messages2 = db.claude().list_messages_by_session(&session2.id)?;

    assert!(messages1.is_empty());
    assert_eq!(messages2.len(), 1);
    assert_eq!(messages2[0], message3);

    Ok(())
}

#[test]
fn message_with_transaction() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session = claude_session("sess-1", "current-1");
    db.claude_mut().insert_session(session.clone())?;

    let message = claude_message("msg-1", &session.id, "user", "Hello");

    let mut trans = db.transaction()?;
    trans.claude_mut().insert_message(message.clone())?;

    let messages_in_trans = trans.claude().list_messages_by_session(&session.id)?;
    assert_eq!(messages_in_trans.len(), 1);

    trans.commit()?;

    let messages_after_commit = db.claude().list_messages_by_session(&session.id)?;
    assert_eq!(messages_after_commit.len(), 1);
    assert_eq!(messages_after_commit[0], message);

    Ok(())
}

// ====== Claude Permission Requests Tests ======

#[test]
fn permission_get_nonexistent() -> anyhow::Result<()> {
    let db = in_memory_db();

    let result = db.claude().get_permission_request("nonexistent")?;
    assert!(result.is_none());

    Ok(())
}

#[test]
fn permission_insert_and_get() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let request = permission_request("req-1", "read_file", r#"{"path": "test.txt"}"#);

    db.claude_mut().insert_permission_request(request.clone())?;

    let retrieved = db.claude().get_permission_request(&request.id)?;
    assert_eq!(retrieved, Some(request));

    Ok(())
}

#[test]
fn permission_list() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let request1 = permission_request("req-1", "read_file", r#"{"path": "test.txt"}"#);
    let request2 = permission_request("req-2", "write_file", r#"{"path": "out.txt"}"#);

    db.claude_mut()
        .insert_permission_request(request1.clone())?;
    db.claude_mut()
        .insert_permission_request(request2.clone())?;

    let requests = db.claude().list_permission_requests()?;
    assert_eq!(requests.len(), 2);
    assert!(requests.contains(&request1));
    assert!(requests.contains(&request2));

    Ok(())
}

#[test]
fn permission_set_decision() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let request = permission_request("req-1", "read_file", r#"{"path": "test.txt"}"#);
    db.claude_mut().insert_permission_request(request.clone())?;

    db.claude_mut()
        .set_permission_request_decision(&request.id, Some("allowSession".to_string()))?;

    let retrieved = db.claude().get_permission_request(&request.id)?;
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.decision, Some("allowSession".to_string()));
    assert_ne!(retrieved.updated_at, request.updated_at);

    Ok(())
}

#[test]
fn permission_set_decision_and_wildcard() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let request = permission_request("req-1", "read_file", r#"{"path": "test.txt"}"#);
    db.claude_mut().insert_permission_request(request.clone())?;

    db.claude_mut()
        .set_permission_request_decision_and_wildcard(
            &request.id,
            Some("allowSession".to_string()),
            true,
        )?;

    let retrieved = db.claude().get_permission_request(&request.id)?;
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.decision, Some("allowSession".to_string()));
    assert!(retrieved.use_wildcard);

    Ok(())
}

#[test]
fn permission_delete() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let request = permission_request("req-1", "read_file", r#"{"path": "test.txt"}"#);
    db.claude_mut().insert_permission_request(request.clone())?;

    let retrieved = db.claude().get_permission_request(&request.id)?;
    assert!(retrieved.is_some());

    db.claude_mut().delete_permission_request(&request.id)?;

    let retrieved = db.claude().get_permission_request(&request.id)?;
    assert!(retrieved.is_none());

    Ok(())
}

#[test]
fn permission_with_transaction() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let request = permission_request("req-1", "read_file", r#"{"path": "test.txt"}"#);

    let mut trans = db.transaction()?;
    trans
        .claude_mut()
        .insert_permission_request(request.clone())?;

    let retrieved_in_trans = trans.claude().get_permission_request(&request.id)?;
    assert_eq!(retrieved_in_trans, Some(request.clone()));

    trans.commit()?;

    let retrieved_after_commit = db.claude().get_permission_request(&request.id)?;
    assert_eq!(retrieved_after_commit, Some(request));

    Ok(())
}

// ====== Delete Session and Messages Test ======

#[test]
fn delete_session_and_messages() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session = claude_session("sess-1", "current-1");
    db.claude_mut().insert_session(session.clone())?;

    let message1 = claude_message("msg-1", &session.id, "user", "Hello");
    let message2 = claude_message("msg-2", &session.id, "assistant", "Hi");

    db.claude_mut().insert_message(message1)?;
    db.claude_mut().insert_message(message2)?;

    let messages_before = db.claude().list_messages_by_session(&session.id)?;
    assert_eq!(messages_before.len(), 2);

    db.claude_mut().delete_session_and_messages(&session.id)?;

    let session_after = db.claude().get_session(&session.id)?;
    let messages_after = db.claude().list_messages_by_session(&session.id)?;

    assert!(session_after.is_none());
    assert!(messages_after.is_empty());

    Ok(())
}

#[test]
fn delete_session_and_messages_in_transaction_with_rollback() -> anyhow::Result<()> {
    let mut db = in_memory_db();

    let session = claude_session("sess-1", "current-1");
    db.claude_mut().insert_session(session.clone())?;

    let message = claude_message("msg-1", &session.id, "user", "Hello");
    db.claude_mut().insert_message(message)?;

    let mut trans = db.transaction()?;
    trans
        .claude_mut()
        .delete_session_and_messages(&session.id)?;
    trans.rollback()?;

    let session_after = db.claude().get_session(&session.id)?;
    let messages_after = db.claude().list_messages_by_session(&session.id)?;

    assert!(session_after.is_some());
    assert!(!messages_after.is_empty());

    Ok(())
}

// ====== Helper Functions ======

fn claude_session(id: &str, current_id: &str) -> ClaudeSession {
    ClaudeSession {
        id: id.to_string(),
        current_id: current_id.to_string(),
        session_ids: "[]".to_string(),
        created_at: chrono::DateTime::from_timestamp(1000000, 0)
            .unwrap()
            .naive_utc(),
        updated_at: chrono::DateTime::from_timestamp(1000000, 0)
            .unwrap()
            .naive_utc(),
        in_gui: false,
        approved_permissions: "[]".to_string(),
        denied_permissions: "[]".to_string(),
    }
}

fn claude_message(id: &str, session_id: &str, content_type: &str, content: &str) -> ClaudeMessage {
    ClaudeMessage {
        id: id.to_string(),
        session_id: session_id.to_string(),
        created_at: chrono::DateTime::from_timestamp(1000000, 0)
            .unwrap()
            .naive_utc(),
        content_type: content_type.to_string(),
        content: content.to_string(),
    }
}

fn permission_request(id: &str, tool_name: &str, input: &str) -> ClaudePermissionRequest {
    ClaudePermissionRequest {
        id: id.to_string(),
        created_at: chrono::DateTime::from_timestamp(1000000, 0)
            .unwrap()
            .naive_utc(),
        updated_at: chrono::DateTime::from_timestamp(1000000, 0)
            .unwrap()
            .naive_utc(),
        tool_name: tool_name.to_string(),
        input: input.to_string(),
        decision: None,
        use_wildcard: false,
    }
}

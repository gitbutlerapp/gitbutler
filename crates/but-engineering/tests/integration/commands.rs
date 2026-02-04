//! Integration tests for commands.

use std::thread;
use std::time::Duration;
use std::{io::Write as _, process::Stdio};

use but_engineering::command;
use but_engineering::command::conflict::{CheckDecision, CheckReasonCode, RequiredAction};
use but_engineering::db::DbHandle;
use tempfile::TempDir;

fn create_test_db() -> (TempDir, DbHandle) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = DbHandle::new_at_path(&db_path).unwrap();
    (dir, db)
}

fn create_test_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let output = std::process::Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    dir
}

fn run_cli(repo: &std::path::Path, args: &[&str], stdin_json: Option<&str>) -> std::process::Output {
    run_cli_with_env(repo, args, stdin_json, &[])
}

fn run_cli_with_env(
    repo: &std::path::Path,
    args: &[&str],
    stdin_json: Option<&str>,
    env_vars: &[(&str, &str)],
) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_but-engineering");
    if let Some(input) = stdin_json {
        let mut cmd = std::process::Command::new(bin);
        cmd.current_dir(repo)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in env_vars {
            cmd.env(key, value);
        }
        let mut child = cmd.spawn().unwrap();
        child.stdin.as_mut().unwrap().write_all(input.as_bytes()).unwrap();
        child.wait_with_output().unwrap()
    } else {
        let mut cmd = std::process::Command::new(bin);
        cmd.current_dir(repo)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in env_vars {
            cmd.env(key, value);
        }
        cmd.output().unwrap()
    }
}

#[test]
fn test_post_message() {
    let (_dir, db) = create_test_db();

    let message = command::post::execute(&db, "Hello, world!".to_string(), "agent-1".to_string()).unwrap();

    assert_eq!(message.content, "Hello, world!");
    assert_eq!(message.agent_id, "agent-1");
    assert!(!message.id.is_empty());
}

#[test]
fn test_post_creates_agent() {
    let (_dir, db) = create_test_db();

    command::post::execute(&db, "Hello".to_string(), "agent-1".to_string()).unwrap();

    let agent = db.get_agent("agent-1").unwrap().unwrap();
    assert_eq!(agent.id, "agent-1");
}

#[test]
fn test_status_set_and_clear() {
    let (_dir, db) = create_test_db();

    // Set status
    let agent = command::status::execute(
        &db,
        "agent-1".to_string(),
        Some("Working on feature".to_string()),
        false,
    )
    .unwrap();

    assert_eq!(agent.status, Some("Working on feature".to_string()));

    // Clear status
    let agent = command::status::execute(&db, "agent-1".to_string(), None, true).unwrap();

    assert_eq!(agent.status, None);
}

#[test]
fn test_list_agents() {
    let (_dir, db) = create_test_db();

    // Create some agents
    command::post::execute(&db, "msg1".to_string(), "agent-1".to_string()).unwrap();
    command::post::execute(&db, "msg2".to_string(), "agent-2".to_string()).unwrap();
    command::post::execute(&db, "msg3".to_string(), "agent-3".to_string()).unwrap();

    // List all agents
    let agents = command::agents::execute(&db, None).unwrap();
    assert_eq!(agents.len(), 3);
}

#[test]
fn test_list_agents_active_within() {
    let (_dir, db) = create_test_db();

    // Create an agent
    command::post::execute(&db, "msg1".to_string(), "agent-1".to_string()).unwrap();

    // List agents active within 5 minutes
    let agents = command::agents::execute(&db, Some("5m".to_string())).unwrap();
    assert_eq!(agents.len(), 1);

    // List agents active within 1 hour
    let agents = command::agents::execute(&db, Some("1h".to_string())).unwrap();
    assert_eq!(agents.len(), 1);
}

#[test]
fn test_read_messages_basic() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = DbHandle::new_at_path(&db_path).unwrap();

    // Post a message
    command::post::execute(&db, "Hello!".to_string(), "agent-1".to_string()).unwrap();

    // Read messages (should get the message we just posted since it's within the hour window)
    let messages = command::read::execute(&db_path, "agent-2".to_string(), None, true, false, None).unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Hello!");
}

#[test]
fn test_read_updates_last_read() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = DbHandle::new_at_path(&db_path).unwrap();

    // Post a message
    command::post::execute(&db, "Hello!".to_string(), "agent-1".to_string()).unwrap();

    // Read messages as agent-2
    command::read::execute(&db_path, "agent-2".to_string(), None, true, false, None).unwrap();

    // Check that agent-2 now has a last_read set
    let db = DbHandle::new_at_path(&db_path).unwrap();
    let agent = db.get_agent("agent-2").unwrap().unwrap();
    assert!(agent.last_read.is_some());
}

#[test]
fn test_multi_agent_communication() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = DbHandle::new_at_path(&db_path).unwrap();

    // Agent 1 posts
    command::post::execute(&db, "Hello from Agent 1".to_string(), "agent-1".to_string()).unwrap();

    // Agent 2 reads
    let messages = command::read::execute(&db_path, "agent-2".to_string(), None, true, false, None).unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Hello from Agent 1");
    assert_eq!(messages[0].agent_id, "agent-1");

    // Agent 2 posts
    let db = DbHandle::new_at_path(&db_path).unwrap();
    command::post::execute(&db, "Hello from Agent 2".to_string(), "agent-2".to_string()).unwrap();

    // Agent 1 reads (should only get Agent 2's message if using unread)
    let messages = command::read::execute(&db_path, "agent-1".to_string(), None, true, false, None).unwrap();

    // Agent 1 is reading for the first time, so should get both messages
    assert_eq!(messages.len(), 2);
}

#[test]
fn test_validation_empty_agent_id() {
    let (_dir, db) = create_test_db();

    let result = command::post::execute(&db, "Hello".to_string(), "".to_string());

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

#[test]
fn test_validation_empty_content() {
    let (_dir, db) = create_test_db();

    let result = command::post::execute(&db, "".to_string(), "agent-1".to_string());

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

#[test]
fn test_concurrent_writes() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");

    // Initialize the database
    let _ = DbHandle::new_at_path(&db_path).unwrap();

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let path = db_path.clone();
            thread::spawn(move || {
                let db = DbHandle::new_at_path(&path).unwrap();
                for j in 0..10 {
                    command::post::execute(&db, format!("Message {j} from thread {i}"), format!("agent-{i}")).unwrap();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all messages were written
    let db = DbHandle::new_at_path(&db_path).unwrap();
    let agents = command::agents::execute(&db, None).unwrap();
    assert_eq!(agents.len(), 10);
}

#[test]
fn test_wait_timeout() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let _ = DbHandle::new_at_path(&db_path).unwrap();

    // Read with wait and very short timeout - should return empty
    let start = std::time::Instant::now();
    let messages = command::read::execute(
        &db_path,
        "agent-1".to_string(),
        None,
        true,
        true,
        Some("100ms".to_string()),
    )
    .unwrap();

    let elapsed = start.elapsed();
    assert!(messages.is_empty());
    assert!(elapsed >= Duration::from_millis(100));
    assert!(elapsed < Duration::from_millis(500)); // Should not take too long
}

#[test]
fn test_wait_returns_on_new_message() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let _ = DbHandle::new_at_path(&db_path).unwrap();

    // First, read to set the baseline
    let _ = command::read::execute(&db_path, "agent-reader".to_string(), None, true, false, None).unwrap();

    let reader_path = db_path.clone();
    let writer_path = db_path.clone();

    // Spawn a thread that will write a message after a short delay
    let writer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        let db = DbHandle::new_at_path(&writer_path).unwrap();
        command::post::execute(&db, "New message!".to_string(), "agent-writer".to_string()).unwrap();
    });

    // Start waiting for messages
    let start = std::time::Instant::now();
    let messages = command::read::execute(
        &reader_path,
        "agent-reader".to_string(),
        None,
        true,
        true,
        Some("5s".to_string()),
    )
    .unwrap();

    let elapsed = start.elapsed();

    writer.join().unwrap();

    // Should have received the message
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "New message!");

    // Should have returned relatively quickly (not waited the full timeout)
    assert!(elapsed < Duration::from_secs(2));
}

#[test]
fn test_bad_since_timestamp_defaults_to_epoch() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = DbHandle::new_at_path(&db_path).unwrap();

    // Post a message
    command::post::execute(&db, "Hello!".to_string(), "agent-1".to_string()).unwrap();

    // Read with an invalid timestamp - should be lenient and return all messages (from epoch)
    let messages = command::read::execute(
        &db_path,
        "agent-2".to_string(),
        Some("not-a-valid-timestamp".to_string()),
        false, // not using unread, using explicit --since
        false,
        None,
    )
    .unwrap();

    // Should get the message because bad timestamp defaults to epoch
    assert_eq!(messages.len(), 1);
}

#[test]
fn test_unread_returns_only_new_messages() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let db = DbHandle::new_at_path(&db_path).unwrap();

    // Agent 1 posts first message
    command::post::execute(&db, "First message".to_string(), "agent-1".to_string()).unwrap();

    // Agent 2 reads (gets first message, sets last_read)
    let messages = command::read::execute(&db_path, "agent-2".to_string(), None, true, false, None).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "First message");

    // Agent 1 posts second message
    let db = DbHandle::new_at_path(&db_path).unwrap();
    command::post::execute(&db, "Second message".to_string(), "agent-1".to_string()).unwrap();

    // Agent 2 reads again with --unread - should only get second message
    let messages = command::read::execute(&db_path, "agent-2".to_string(), None, true, false, None).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Second message");
}

#[test]
fn test_mentions_in_content() {
    let (_dir, db) = create_test_db();

    // Post a message with a mention
    let message =
        command::post::execute(&db, "@agent-2 can you review this?".to_string(), "agent-1".to_string()).unwrap();

    // Verify the mention is in the content (agents filter for their own mentions)
    assert!(message.content.contains("@agent-2"));
}

#[test]
fn test_validation_agent_id_too_long() {
    let (_dir, db) = create_test_db();
    let long_id = "a".repeat(257); // Exceeds MAX_AGENT_ID_LEN (256)

    let result = command::post::execute(&db, "Hello".to_string(), long_id);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("maximum length"));
}

#[test]
fn test_validation_content_too_long() {
    let (_dir, db) = create_test_db();
    let long_content = "x".repeat(16385); // Exceeds MAX_CONTENT_LEN (16384)

    let result = command::post::execute(&db, long_content, "agent-1".to_string());

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("maximum length"));
}

#[test]
fn test_status_validation_empty_agent_id() {
    let (_dir, db) = create_test_db();

    let result = command::status::execute(&db, "".to_string(), Some("Working".to_string()), false);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

#[test]
fn test_status_conflicting_options() {
    let (_dir, db) = create_test_db();

    // Providing both --clear and a status message should fail
    let result = command::status::execute(
        &db,
        "agent-1".to_string(),
        Some("Working".to_string()),
        true, // clear=true
    );

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cannot use both"));
}

#[test]
fn test_read_invalid_timeout_format() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let _ = DbHandle::new_at_path(&db_path).unwrap();

    let result = command::read::execute(
        &db_path,
        "agent-1".to_string(),
        None,
        true,
        true,
        Some("invalid_timeout".to_string()),
    );

    assert!(result.is_err());
    // Should fail with duration parse error
    let err = result.unwrap_err().to_string();
    assert!(err.contains("invalid number") || err.contains("unknown duration"));
}

#[test]
fn test_list_agents_invalid_duration() {
    let (_dir, db) = create_test_db();

    let result = command::agents::execute(&db, Some("not-a-duration".to_string()));

    assert!(result.is_err());
}

#[test]
fn test_status_persists_after_posting() {
    let (_dir, db) = create_test_db();

    // Set status
    command::status::execute(&db, "agent-1".to_string(), Some("Busy".to_string()), false).unwrap();

    // Post a message (should not clear status)
    command::post::execute(&db, "Hello".to_string(), "agent-1".to_string()).unwrap();

    // Verify status is preserved
    let agent = db.get_agent("agent-1").unwrap().unwrap();
    assert_eq!(agent.status, Some("Busy".to_string()));
}

// --- Claims tests ---

#[test]
fn test_claim_files() {
    let (_dir, db) = create_test_db();

    let claims = command::claim::execute(
        &db,
        vec!["src/foo.rs".to_string(), "src/bar.rs".to_string()],
        "agent-1".to_string(),
    )
    .unwrap();

    assert_eq!(claims.len(), 2);
    assert_eq!(claims[0].file_path, "src/foo.rs");
    assert_eq!(claims[0].agent_id, "agent-1");
    assert_eq!(claims[1].file_path, "src/bar.rs");
}

#[test]
fn test_claim_creates_agent() {
    let (_dir, db) = create_test_db();

    command::claim::execute(&db, vec!["src/foo.rs".to_string()], "agent-1".to_string()).unwrap();

    let agent = db.get_agent("agent-1").unwrap().unwrap();
    assert_eq!(agent.id, "agent-1");
}

#[test]
fn test_claim_empty_paths() {
    let (_dir, db) = create_test_db();

    let result = command::claim::execute(&db, vec![], "agent-1".to_string());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("at least one"));
}

#[test]
fn test_release_specific_files() {
    let (_dir, db) = create_test_db();

    // Claim two files
    command::claim::execute(
        &db,
        vec!["src/foo.rs".to_string(), "src/bar.rs".to_string()],
        "agent-1".to_string(),
    )
    .unwrap();

    // Release one
    let result = command::release::execute(&db, vec!["src/foo.rs".to_string()], "agent-1".to_string(), false).unwrap();
    assert_eq!(result.released, 1);

    // Only bar.rs should remain
    let claims = command::claims::execute(&db, None).unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].file_path, "src/bar.rs");
}

#[test]
fn test_release_all() {
    let (_dir, db) = create_test_db();

    // Claim files
    command::claim::execute(
        &db,
        vec!["src/foo.rs".to_string(), "src/bar.rs".to_string()],
        "agent-1".to_string(),
    )
    .unwrap();

    // Release all
    let result = command::release::execute(&db, vec![], "agent-1".to_string(), true).unwrap();
    assert_eq!(result.released, 2);

    // No claims should remain
    let claims = command::claims::execute(&db, None).unwrap();
    assert!(claims.is_empty());
}

#[test]
fn test_release_no_paths_no_all() {
    let (_dir, db) = create_test_db();

    let result = command::release::execute(&db, vec![], "agent-1".to_string(), false);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("--all"));
}

#[test]
fn test_list_claims() {
    let (_dir, db) = create_test_db();

    command::claim::execute(&db, vec!["src/foo.rs".to_string()], "agent-1".to_string()).unwrap();
    command::claim::execute(&db, vec!["src/bar.rs".to_string()], "agent-2".to_string()).unwrap();

    let claims = command::claims::execute(&db, None).unwrap();
    assert_eq!(claims.len(), 2);
}

#[test]
fn test_list_claims_active_within() {
    let (_dir, db) = create_test_db();

    command::claim::execute(&db, vec!["src/foo.rs".to_string()], "agent-1".to_string()).unwrap();

    // Should find the claim (agent was just active)
    let claims = command::claims::execute(&db, Some("5m".to_string())).unwrap();
    assert_eq!(claims.len(), 1);
}

#[test]
fn test_claim_reclaim_refreshes_timestamp() {
    let (_dir, db) = create_test_db();

    // Claim a file
    let claims1 = command::claim::execute(&db, vec!["src/foo.rs".to_string()], "agent-1".to_string()).unwrap();

    // Small delay
    thread::sleep(Duration::from_millis(10));

    // Re-claim the same file
    let claims2 = command::claim::execute(&db, vec!["src/foo.rs".to_string()], "agent-1".to_string()).unwrap();

    // Timestamp should be refreshed
    assert!(claims2[0].claimed_at >= claims1[0].claimed_at);

    // Should still be just one claim in the DB
    let all = command::claims::execute(&db, None).unwrap();
    assert_eq!(all.len(), 1);
}

#[test]
fn test_multiple_agents_claim_same_file() {
    let (_dir, db) = create_test_db();

    // Two agents claim the same file
    command::claim::execute(&db, vec!["src/foo.rs".to_string()], "agent-1".to_string()).unwrap();
    command::claim::execute(&db, vec!["src/foo.rs".to_string()], "agent-2".to_string()).unwrap();

    // Both claims should exist (PK is file_path + agent_id)
    let claims = db.get_claims_for_file("src/foo.rs").unwrap();
    assert_eq!(claims.len(), 2);
}

#[test]
fn test_expire_stale_claims() {
    let (_dir, db) = create_test_db();

    // Create an agent and claim a file
    command::claim::execute(&db, vec!["src/foo.rs".to_string()], "agent-1".to_string()).unwrap();

    // Manually backdate the agent's last_active to 2 hours ago
    let two_hours_ago = chrono::Utc::now() - chrono::Duration::hours(2);
    db.conn()
        .execute(
            "UPDATE agents SET last_active = ?1 WHERE id = ?2",
            rusqlite::params![two_hours_ago, "agent-1"],
        )
        .unwrap();

    // Expire claims from agents inactive for >1 hour
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(1);
    let expired = db.expire_stale_claims(cutoff).unwrap();
    assert_eq!(expired, 1);

    // Claim should be gone
    let claims = command::claims::execute(&db, None).unwrap();
    assert!(claims.is_empty());
}

#[test]
fn test_expire_claims_older_than() {
    let (_dir, db) = create_test_db();

    command::claim::execute(&db, vec!["src/foo.rs".to_string()], "agent-1".to_string()).unwrap();

    let old = chrono::Utc::now() - chrono::Duration::minutes(20);
    db.conn()
        .execute(
            "UPDATE claims SET claimed_at = ?1 WHERE file_path = ?2 AND agent_id = ?3",
            rusqlite::params![old, "src/foo.rs", "agent-1"],
        )
        .unwrap();

    let cutoff = chrono::Utc::now() - chrono::Duration::minutes(5);
    let expired = db.expire_claims_older_than(cutoff).unwrap();
    assert_eq!(expired, 1);

    let claims = command::claims::execute(&db, None).unwrap();
    assert!(claims.is_empty());
}

// --- Plan tests ---

#[test]
fn test_plan_set_and_clear() {
    let (_dir, db) = create_test_db();

    // Set plan
    let agent = command::plan::execute(
        &db,
        "agent-1".to_string(),
        Some("Implement auth — editing login.rs".to_string()),
        false,
    )
    .unwrap();

    assert_eq!(agent.plan, Some("Implement auth — editing login.rs".to_string()));
    assert!(agent.plan_updated_at.is_some());

    // Clear plan
    let agent = command::plan::execute(&db, "agent-1".to_string(), None, true).unwrap();
    assert_eq!(agent.plan, None);
    assert_eq!(agent.plan_updated_at, None);
}

#[test]
fn test_plan_creates_agent() {
    let (_dir, db) = create_test_db();

    command::plan::execute(&db, "agent-1".to_string(), Some("My plan".to_string()), false).unwrap();

    let agent = db.get_agent("agent-1").unwrap().unwrap();
    assert_eq!(agent.id, "agent-1");
    assert_eq!(agent.plan, Some("My plan".to_string()));
}

#[test]
fn test_plan_conflicting_options() {
    let (_dir, db) = create_test_db();

    let result = command::plan::execute(
        &db,
        "agent-1".to_string(),
        Some("My plan".to_string()),
        true, // clear=true
    );

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cannot use both"));
}

#[test]
fn test_plan_too_long() {
    let (_dir, db) = create_test_db();
    let long_plan = "x".repeat(4097);

    let result = command::plan::execute(&db, "agent-1".to_string(), Some(long_plan), false);

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("maximum length"));
}

#[test]
fn test_plan_get_without_setting() {
    let (_dir, db) = create_test_db();

    // Create agent first
    command::post::execute(&db, "hello".to_string(), "agent-1".to_string()).unwrap();

    // Get plan (should return agent with no plan)
    let agent = command::plan::execute(&db, "agent-1".to_string(), None, false).unwrap();
    assert_eq!(agent.plan, None);
}

// --- Discover tests ---

#[test]
fn test_discover_posts_discovery() {
    let (_dir, db) = create_test_db();

    let msg = command::discover::execute(
        &db,
        "The auth module panics if JWT secret is empty".to_string(),
        "agent-1".to_string(),
    )
    .unwrap();

    assert_eq!(msg.kind, MessageKind::Discovery);
    assert!(msg.content.contains("auth module panics"));
}

#[test]
fn test_discover_queryable() {
    let (_dir, db) = create_test_db();

    // Post a regular message and a discovery
    command::post::execute(&db, "Working on auth".to_string(), "agent-1".to_string()).unwrap();
    command::discover::execute(&db, "JWT needs RS256 not HS256".to_string(), "agent-1".to_string()).unwrap();

    // Query discoveries only
    let since = chrono::Utc::now() - chrono::Duration::minutes(5);
    let discoveries = db.query_recent_discoveries(since, 10).unwrap();

    assert_eq!(discoveries.len(), 1);
    assert_eq!(discoveries[0].kind, MessageKind::Discovery);
    assert!(discoveries[0].content.contains("RS256"));
}

#[test]
fn test_discover_appears_in_all_messages() {
    let (_dir, db) = create_test_db();

    command::discover::execute(&db, "Important finding".to_string(), "agent-1".to_string()).unwrap();

    // Regular message query should also include discoveries
    let since = chrono::Utc::now() - chrono::Duration::minutes(5);
    let all = db.query_recent_messages(since, 10).unwrap();

    assert_eq!(all.len(), 1);
    assert_eq!(all[0].kind, MessageKind::Discovery);
}

// --- Done tests ---

#[test]
fn test_done_posts_completion_and_cleans_up() {
    let (_dir, db) = create_test_db();

    command::claim::execute(&db, vec!["src/foo.rs".to_string()], "agent-1".to_string()).unwrap();
    command::plan::execute(
        &db,
        "agent-1".to_string(),
        Some("Finish src/foo.rs changes".to_string()),
        false,
    )
    .unwrap();

    let result =
        command::done::execute(&db, "Updated src/foo.rs and tests".to_string(), "agent-1".to_string()).unwrap();

    assert_eq!(result.agent_id, "agent-1");
    assert_eq!(result.released, 1);
    assert!(result.plan_cleared);
    assert_eq!(result.message.agent_id, "agent-1");
    assert_eq!(result.message.kind, MessageKind::Message);
    assert!(result.message.content.starts_with("DONE: "));
    assert!(result.message.content.contains("Updated src/foo.rs"));

    let claims = command::claims::execute(&db, None).unwrap();
    assert!(claims.is_empty());
    let agent = db.get_agent("agent-1").unwrap().unwrap();
    assert_eq!(agent.plan, None);
}

// --- Stale plan cleanup tests ---

#[test]
fn test_clear_stale_plans() {
    let (_dir, db) = create_test_db();

    // Create agent with a plan
    command::plan::execute(
        &db,
        "agent-1".to_string(),
        Some("Working on feature X".to_string()),
        false,
    )
    .unwrap();

    // Backdate the agent's last_active to 2 hours ago
    let two_hours_ago = chrono::Utc::now() - chrono::Duration::hours(2);
    db.conn()
        .execute(
            "UPDATE agents SET last_active = ?1 WHERE id = ?2",
            rusqlite::params![two_hours_ago, "agent-1"],
        )
        .unwrap();

    // Clear stale plans (agents inactive >1 hour)
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(1);
    let cleared = db.clear_stale_plans(cutoff).unwrap();
    assert_eq!(cleared, 1);

    // Plan should be gone
    let agent = db.get_agent("agent-1").unwrap().unwrap();
    assert_eq!(agent.plan, None);
    assert_eq!(agent.plan_updated_at, None);
}

#[test]
fn test_clear_stale_plans_preserves_active() {
    let (_dir, db) = create_test_db();

    // Create agent with a plan (just now, so it's active)
    command::plan::execute(
        &db,
        "agent-1".to_string(),
        Some("Working on feature X".to_string()),
        false,
    )
    .unwrap();

    // Clear plans from agents inactive >1 hour — should not affect agent-1
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(1);
    let cleared = db.clear_stale_plans(cutoff).unwrap();
    assert_eq!(cleared, 0);

    // Plan should still be there
    let agent = db.get_agent("agent-1").unwrap().unwrap();
    assert_eq!(agent.plan, Some("Working on feature X".to_string()));
}

// --- Block message tests ---

#[test]
fn test_block_message_posted_as_kind_block() {
    let (_dir, db) = create_test_db();

    // Post a block message via execute_with_kind
    let msg = command::post::execute_with_kind(
        &db,
        "BLOCKED on types.rs — @agent-1 please release it".to_string(),
        "agent-2".to_string(),
        MessageKind::Block,
    )
    .unwrap();

    assert_eq!(msg.kind, MessageKind::Block);
    assert!(msg.content.contains("BLOCKED"));
    assert!(msg.content.contains("@agent-1"));
}

#[test]
fn test_query_recent_blocks() {
    let (_dir, db) = create_test_db();

    // Post a regular message, a discovery, and a block
    command::post::execute(&db, "Working on auth".to_string(), "agent-1".to_string()).unwrap();
    command::discover::execute(&db, "JWT needs RS256".to_string(), "agent-1".to_string()).unwrap();
    command::post::execute_with_kind(
        &db,
        "BLOCKED on types.rs — @agent-1 please release it".to_string(),
        "agent-2".to_string(),
        MessageKind::Block,
    )
    .unwrap();

    // Query blocks only
    let since = chrono::Utc::now() - chrono::Duration::minutes(5);
    let blocks = db.query_recent_blocks(since, 10).unwrap();

    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].kind, MessageKind::Block);
    assert_eq!(blocks[0].agent_id, "agent-2");
}

#[test]
fn test_blocks_appear_in_all_messages() {
    let (_dir, db) = create_test_db();

    command::post::execute_with_kind(
        &db,
        "BLOCKED on types.rs".to_string(),
        "agent-2".to_string(),
        MessageKind::Block,
    )
    .unwrap();

    // Regular message query should also include blocks
    let since = chrono::Utc::now() - chrono::Duration::minutes(5);
    let all = db.query_recent_messages(since, 10).unwrap();

    assert_eq!(all.len(), 1);
    assert_eq!(all[0].kind, MessageKind::Block);
}

// --- Check command tests ---

#[test]
fn test_check_deny_returns_ok_with_blockers() {
    let (_dir, db) = create_test_db();

    command::claim::execute(&db, vec!["src/foo.rs".to_string()], "agent-1".to_string()).unwrap();

    let result =
        command::check::execute(&db, "src/foo.rs".to_string(), Some("agent-2".to_string()), false, None).unwrap();

    assert_eq!(result.decision, CheckDecision::Deny);
    assert_eq!(result.blocking_agents, vec!["agent-1".to_string()]);
    assert!(result.reason.as_deref().unwrap().contains("claimed by agent-1"));
    assert_eq!(result.reason_code, CheckReasonCode::ClaimedByOther);
    assert_eq!(
        result.required_actions,
        vec![
            RequiredAction::PostCoordinationMessage,
            RequiredAction::WaitForRelease,
            RequiredAction::RetryCheck
        ]
    );
    assert_eq!(result.action_plan.len(), 3);
    assert_eq!(result.action_plan[0].action, RequiredAction::PostCoordinationMessage);
    assert_eq!(result.action_plan[1].action, RequiredAction::WaitForRelease);
    assert_eq!(result.action_plan[2].action, RequiredAction::RetryCheck);
    assert!(
        result.action_plan[1]
            .commands
            .iter()
            .any(|cmd| cmd.contains("--wait --timeout 5s"))
    );
    assert!(
        result.action_plan[0].commands[0].contains("@agent-1")
            && result.action_plan[0].commands[0].contains("src/foo.rs")
    );
    assert_eq!(result.coordination_mode, command::check::CoordinationMode::Blocked);
    assert_eq!(result.announce_required, true);
    assert_eq!(result.retry_after_seconds, Some(5));
    assert_eq!(result.lock_owner.as_deref(), Some("agent-1"));
    assert_eq!(result.self_agent_id.as_deref(), Some("agent-2"));
    assert_eq!(result.identity_source, hook_common::IdentitySource::Arg);
}

#[test]
fn test_check_allow_with_warning() {
    let (_dir, db) = create_test_db();

    command::post::execute(
        &db,
        "I am editing src/foo.rs right now".to_string(),
        "agent-1".to_string(),
    )
    .unwrap();

    let result =
        command::check::execute(&db, "src/foo.rs".to_string(), Some("agent-2".to_string()), false, None).unwrap();

    assert_eq!(result.decision, CheckDecision::Allow);
    assert!(result.reason.is_none());
    assert!(result.blocking_agents.is_empty());
    assert!(!result.warnings.is_empty());
    assert_eq!(result.reason_code, CheckReasonCode::MessageMention);
    assert_eq!(
        result.required_actions,
        vec![
            RequiredAction::ReadChannel,
            RequiredAction::PostCoordinationMessage,
            RequiredAction::ProceedWithEdit
        ]
    );
    assert_eq!(result.action_plan.len(), 3);
    assert_eq!(result.action_plan[0].action, RequiredAction::ReadChannel);
    assert_eq!(result.action_plan[1].action, RequiredAction::PostCoordinationMessage);
    assert!(result.action_plan[1].required);
    assert_eq!(result.action_plan[2].action, RequiredAction::ProceedWithEdit);
}

#[test]
fn test_check_is_read_only_no_auto_claim_side_effect() {
    let (_dir, db) = create_test_db();

    let _ = command::check::execute(&db, "src/foo.rs".to_string(), Some("agent-2".to_string()), false, None).unwrap();

    let claims = command::claims::execute(&db, None).unwrap();
    assert!(claims.is_empty(), "check should not auto-claim files");
}

#[test]
fn test_check_response_serialization_fields_present() {
    let (_dir, db) = create_test_db();

    let result =
        command::check::execute(&db, "src/foo.rs".to_string(), Some("agent-2".to_string()), false, None).unwrap();
    let json = serde_json::to_value(&result).unwrap();

    assert!(json.get("file_path").is_some());
    assert!(json.get("decision").is_some());
    assert!(json.get("reason").is_some());
    assert!(json.get("blocking_agents").is_some());
    assert!(json.get("warnings").is_some());
    assert!(json.get("reason_code").is_some());
    assert!(json.get("required_actions").is_some());
    assert!(json.get("coordination_hints").is_some());
    assert!(json.get("action_plan").is_some());
    assert!(json.get("coordination_mode").is_some());
    assert!(json.get("announce_required").is_some());
    assert!(json.get("retry_after_seconds").is_some());
    assert!(json.get("lock_owner").is_some());
    assert!(json.get("self_agent_id").is_some());
    assert!(json.get("identity_source").is_some());
    assert!(json["reason"].is_null());
    assert_eq!(json["reason_code"], "no_conflict");
    assert_eq!(
        json["required_actions"],
        serde_json::json!(["read_channel", "post_coordination_message", "proceed_with_edit"])
    );
    assert_eq!(json["coordination_hints"]["dependency_source"], "none");
    assert_eq!(json["action_plan"][0]["action"], "post_coordination_message");
    assert_eq!(json["action_plan"][0]["required"], true);
    assert_eq!(json["action_plan"][1]["action"], "read_channel");
    assert_eq!(json["action_plan"][1]["required"], true);
    assert_eq!(json["action_plan"][2]["action"], "proceed_with_edit");
    assert_eq!(json["action_plan"][2]["required"], true);
    assert_eq!(json["coordination_mode"], "clear");
    assert_eq!(json["announce_required"], true);
    assert!(json["retry_after_seconds"].is_null());
    assert!(json["lock_owner"].is_null());
}

#[test]
fn test_check_exclusive_self_claim_fast_path() {
    let (_dir, db) = create_test_db();
    command::claim::execute(&db, vec!["README.md".to_string()], "agent-1".to_string()).unwrap();

    let result =
        command::check::execute(&db, "README.md".to_string(), Some("agent-1".to_string()), false, None).unwrap();

    assert_eq!(result.decision, CheckDecision::Allow);
    assert_eq!(result.reason_code, CheckReasonCode::NoConflict);
    assert_eq!(result.required_actions, vec![RequiredAction::ProceedWithEdit]);
    assert_eq!(
        result.coordination_mode,
        command::check::CoordinationMode::ExclusiveOwner
    );
    assert!(!result.announce_required);
    assert_eq!(result.retry_after_seconds, None);
    assert_eq!(result.lock_owner.as_deref(), Some("agent-1"));
    assert_eq!(result.action_plan.len(), 1);
    assert_eq!(result.action_plan[0].action, RequiredAction::ProceedWithEdit);
}

#[test]
fn test_check_advisory_from_self_message_only_is_suppressed() {
    let (_dir, db) = create_test_db();

    command::post::execute(
        &db,
        "I am editing src/foo.rs right now".to_string(),
        "agent-1".to_string(),
    )
    .unwrap();

    let result =
        command::check::execute(&db, "src/foo.rs".to_string(), Some("agent-1".to_string()), false, None).unwrap();

    assert_eq!(result.decision, CheckDecision::Allow);
    assert!(result.warnings.is_empty());
    assert_eq!(result.reason_code, CheckReasonCode::NoConflict);
    assert_eq!(result.coordination_mode, command::check::CoordinationMode::Clear);
}

#[test]
fn test_check_reason_code_identity_missing_without_agent_context() {
    let (_dir, db) = create_test_db();

    let result = command::check::execute(&db, "src/foo.rs".to_string(), None, false, None).unwrap();
    assert_eq!(result.decision, CheckDecision::Allow);
    assert_eq!(result.reason_code, CheckReasonCode::IdentityMissing);
    assert_eq!(
        result.required_actions,
        vec![
            RequiredAction::ReadChannel,
            RequiredAction::PostCoordinationMessage,
            RequiredAction::ProceedWithEdit
        ]
    );
    assert_eq!(result.action_plan.len(), 3);
    assert_eq!(result.action_plan[0].action, RequiredAction::ReadChannel);
    assert_eq!(result.action_plan[1].action, RequiredAction::PostCoordinationMessage);
    assert_eq!(result.action_plan[2].action, RequiredAction::RetryCheck);
    assert!(result.action_plan[0].commands[0].contains("--agent-id <id>"));
    assert!(result.action_plan[1].commands[0].contains("--agent-id <id>"));
    assert!(result.action_plan[2].commands[0].contains("--agent-id <id>"));
    assert!(result.self_agent_id.is_none());
    assert_eq!(result.identity_source, hook_common::IdentitySource::None);
}

#[test]
fn test_check_cli_exit_zero_on_deny() {
    let repo = create_test_repo();

    let claim = run_cli(repo.path(), &["claim", "src/foo.rs", "--agent-id", "agent-1"], None);
    assert!(
        claim.status.success(),
        "claim failed: {}",
        String::from_utf8_lossy(&claim.stderr)
    );

    let check = run_cli(repo.path(), &["check", "src/foo.rs", "--agent-id", "agent-2"], None);
    assert!(
        check.status.success(),
        "check should exit 0 on deny decision: {}",
        String::from_utf8_lossy(&check.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(value["decision"], "deny");
    assert_eq!(value["reason_code"], "claimed_by_other");
    assert_eq!(value["blocking_agents"], serde_json::json!(["agent-1"]));
    assert_eq!(
        value["required_actions"],
        serde_json::json!(["post_coordination_message", "wait_for_release", "retry_check"])
    );
    assert_eq!(value["coordination_mode"], "blocked");
    assert_eq!(value["announce_required"], true);
    assert_eq!(value["retry_after_seconds"], 5);
    assert_eq!(value["lock_owner"], "agent-1");
    assert!(value.get("action_plan").is_some());
    assert_eq!(value["action_plan"][0]["action"], "post_coordination_message");
    assert_eq!(value["action_plan"][1]["action"], "wait_for_release");
    assert!(
        value["action_plan"][1]["commands"][0]
            .as_str()
            .unwrap_or("")
            .contains("--wait --timeout 5s")
    );
    let first_command = value["action_plan"][0]["commands"][0].as_str().unwrap_or("");
    assert!(first_command.contains("@agent-1"));
    assert!(first_command.contains("src/foo.rs"));
}

#[test]
fn test_done_cli_announces_and_cleans_up() {
    let repo = create_test_repo();

    let claim = run_cli(repo.path(), &["claim", "README.md", "--agent-id", "agent-1"], None);
    assert!(claim.status.success());
    let plan = run_cli(
        repo.path(),
        &["plan", "--agent-id", "agent-1", "Update README section"],
        None,
    );
    assert!(plan.status.success());

    let done = run_cli(
        repo.path(),
        &["done", "Finished README update", "--agent-id", "agent-1"],
        None,
    );
    assert!(
        done.status.success(),
        "done failed: {}",
        String::from_utf8_lossy(&done.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&done.stdout).unwrap();
    assert_eq!(value["agent_id"], "agent-1");
    assert_eq!(value["plan_cleared"], true);
    assert_eq!(value["released"], 1);
    assert_eq!(value["message"]["agent_id"], "agent-1");
    assert!(value["message"]["content"].as_str().unwrap_or("").starts_with("DONE: "));

    let claims = run_cli(repo.path(), &["claims"], None);
    assert!(claims.status.success());
    let claims_json: serde_json::Value = serde_json::from_slice(&claims.stdout).unwrap();
    assert!(claims_json.as_array().unwrap().is_empty());

    let agents = run_cli(repo.path(), &["agents"], None);
    assert!(agents.status.success());
    let agents_json: serde_json::Value = serde_json::from_slice(&agents.stdout).unwrap();
    let self_agent = agents_json
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "agent-1")
        .expect("agent-1 should exist");
    assert!(self_agent["plan"].is_null());
}

#[test]
fn test_check_include_stack_fields_present() {
    let repo = create_test_repo();
    let check = run_cli(
        repo.path(),
        &["check", "src/foo.rs", "--agent-id", "agent-2", "--include-stack"],
        None,
    );
    assert!(check.status.success());
    let value: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert!(value.get("coordination_hints").is_some());
    assert!(value["coordination_hints"].get("stack_dependency_detected").is_some());
    assert!(value["coordination_hints"].get("dependency_source").is_some());
    assert!(value["coordination_hints"].get("intent_branch").is_some());
    assert!(value["coordination_hints"].get("depends_on_branches").is_some());
    assert!(value["coordination_hints"].get("dependent_agents").is_some());
    assert!(value["coordination_hints"].get("suggested_but_commands").is_some());
    assert!(value["coordination_hints"].get("stack_context_error").is_some());
    assert!(value.get("action_plan").is_some());
}

#[test]
fn test_check_stack_dependency_reason_code_from_override_status_json() {
    let repo = create_test_repo();

    // Peer owns base-branch file and references base branch in channel.
    let claim = run_cli(repo.path(), &["claim", "src/auth.rs", "--agent-id", "peer-a"], None);
    assert!(claim.status.success());
    let post = run_cli(
        repo.path(),
        &["post", "I am changing auth-base internals", "--agent-id", "peer-a"],
        None,
    );
    assert!(post.status.success());

    let status_json = r#"{
      "stacks": [
        {
          "branches": [
            {
              "name": "profile-ui",
              "commits": [{ "changes": [{ "filePath": "src/profile.rs" }] }]
            },
            {
              "name": "auth-base",
              "commits": [{ "changes": [{ "filePath": "src/auth.rs" }] }]
            }
          ]
        }
      ]
    }"#;

    let check = run_cli_with_env(
        repo.path(),
        &[
            "check",
            "src/profile.rs",
            "--agent-id",
            "agent-2",
            "--include-stack",
            "--intent-branch",
            "profile-ui",
        ],
        None,
        &[("BUT_ENGINEERING_TEST_STATUS_JSON", status_json)],
    );

    assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stderr));
    let value: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(value["decision"], "allow");
    assert_eq!(value["reason_code"], "stack_dependency");
    assert_eq!(value["coordination_hints"]["stack_dependency_detected"], true);
    assert_eq!(value["coordination_hints"]["dependency_source"], "but_status");
    assert_eq!(
        value["coordination_hints"]["depends_on_branches"],
        serde_json::json!(["auth-base"])
    );
    assert_eq!(
        value["coordination_hints"]["dependent_agents"],
        serde_json::json!(["peer-a"])
    );
    let action_plan = value["action_plan"].as_array().unwrap_or(&Vec::new()).to_vec();
    assert!(!action_plan.is_empty());
    let commands = action_plan
        .iter()
        .flat_map(|step| step["commands"].as_array().cloned().unwrap_or_default())
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    assert!(commands.iter().any(|cmd| cmd == "but status --json"));
    assert!(
        commands
            .iter()
            .any(|cmd| cmd.contains("but branch new <child> -a auth-base"))
    );
}

// --- eval pre-tool-use output contract tests ---

#[test]
fn test_eval_pre_tool_use_deny_output_contract() {
    let repo = create_test_repo();

    let claim = run_cli(repo.path(), &["claim", "src/foo.rs", "--agent-id", "agent-1"], None);
    assert!(claim.status.success());

    // Make agent-2 the most-recently-active fallback identity.
    let status = run_cli(repo.path(), &["status", "--agent-id", "agent-2", "working"], None);
    assert!(status.status.success());

    let input = r#"{"tool_input":{"file_path":"src/foo.rs"}}"#;
    let output = run_cli(repo.path(), &["eval", "pre-tool-use"], Some(input));
    assert!(
        output.status.success(),
        "eval failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let hook = &value["hookSpecificOutput"];
    assert_eq!(hook["hookEventName"], "PreToolUse");
    assert_eq!(hook["permissionDecision"], "deny");
    assert!(hook.get("permissionDecisionReason").is_some());
    assert!(hook.get("additionalContext").is_none());
}

#[test]
fn test_eval_pre_tool_use_deny_releases_losing_agents_local_claim() {
    let repo = create_test_repo();

    let peer_claim = run_cli(repo.path(), &["claim", "src/foo.rs", "--agent-id", "agent-1"], None);
    assert!(
        peer_claim.status.success(),
        "peer claim failed: {}",
        String::from_utf8_lossy(&peer_claim.stderr)
    );

    let self_claim = run_cli(repo.path(), &["claim", "src/foo.rs", "--agent-id", "agent-2"], None);
    assert!(
        self_claim.status.success(),
        "self claim failed: {}",
        String::from_utf8_lossy(&self_claim.stderr)
    );

    let claims_before = run_cli(repo.path(), &["claims"], None);
    assert!(
        claims_before.status.success(),
        "claims before failed: {}",
        String::from_utf8_lossy(&claims_before.stderr)
    );
    let claims_before_json: serde_json::Value = serde_json::from_slice(&claims_before.stdout).unwrap();
    let foo_claims_before = claims_before_json
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["file_path"] == "src/foo.rs")
        .count();
    assert_eq!(foo_claims_before, 2, "expected two shared claims before deny");

    let input = r#"{"tool_input":{"file_path":"src/foo.rs"}}"#;
    let eval_output = run_cli_with_env(
        repo.path(),
        &["eval", "pre-tool-use"],
        Some(input),
        &[("BUT_ENGINEERING_AGENT_ID", "agent-2")],
    );
    assert!(
        eval_output.status.success(),
        "eval failed: {}",
        String::from_utf8_lossy(&eval_output.stderr)
    );
    let eval_json: serde_json::Value = serde_json::from_slice(&eval_output.stdout).unwrap();
    assert_eq!(
        eval_json["hookSpecificOutput"]["permissionDecision"], "deny",
        "expected losing shared-claim agent to be denied"
    );

    let claims_after = run_cli(repo.path(), &["claims"], None);
    assert!(
        claims_after.status.success(),
        "claims after failed: {}",
        String::from_utf8_lossy(&claims_after.stderr)
    );
    let claims_after_json: serde_json::Value = serde_json::from_slice(&claims_after.stdout).unwrap();
    let foo_claims_after = claims_after_json
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["file_path"] == "src/foo.rs")
        .collect::<Vec<_>>();

    assert_eq!(foo_claims_after.len(), 1, "expected only one claim after deny");
    assert_eq!(
        foo_claims_after[0]["agent_id"], "agent-1",
        "expected losing agent claim to be released"
    );
}

#[test]
fn test_eval_pre_tool_use_advisory_output_contract() {
    let repo = create_test_repo();

    let post = run_cli(
        repo.path(),
        &["post", "Working in src/foo.rs now", "--agent-id", "agent-1"],
        None,
    );
    assert!(post.status.success());

    // Make agent-2 the most-recently-active fallback identity.
    let status = run_cli(repo.path(), &["status", "--agent-id", "agent-2", "reviewing"], None);
    assert!(status.status.success());

    let input = r#"{"tool_input":{"file_path":"src/foo.rs"}}"#;
    let output = run_cli(repo.path(), &["eval", "pre-tool-use"], Some(input));
    assert!(
        output.status.success(),
        "eval failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let hook = &value["hookSpecificOutput"];
    assert_eq!(hook["hookEventName"], "PreToolUse");
    assert!(hook.get("permissionDecision").is_none());
    assert!(hook.get("permissionDecisionReason").is_none());
    assert!(hook["additionalContext"].as_str().unwrap().contains("src/foo.rs"));
}

#[test]
fn test_eval_pre_tool_use_advisory_posts_coordination_message() {
    let repo = create_test_repo();

    let post = run_cli(
        repo.path(),
        &["post", "Working in src/foo.rs now", "--agent-id", "agent-1"],
        None,
    );
    assert!(post.status.success());

    let input = r#"{"tool_input":{"file_path":"src/foo.rs"}}"#;
    let output = run_cli_with_env(
        repo.path(),
        &["eval", "pre-tool-use"],
        Some(input),
        &[("BUT_ENGINEERING_AGENT_ID", "agent-2")],
    );
    assert!(
        output.status.success(),
        "eval failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = run_cli(repo.path(), &["read", "--agent-id", "agent-2"], None);
    assert!(
        read.status.success(),
        "read failed: {}",
        String::from_utf8_lossy(&read.stderr)
    );

    let messages: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    let coord_posts = messages
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| {
            m["agent_id"] == "agent-2"
                && m["content"]
                    .as_str()
                    .map(|c| c.contains("[coordination-check]") && c.contains("src/foo.rs"))
                    .unwrap_or(false)
        })
        .count();

    assert!(
        coord_posts >= 1,
        "expected an advisory coordination post from agent-2, got: {}",
        String::from_utf8_lossy(&read.stdout)
    );
}

#[test]
fn test_eval_pre_tool_use_advisory_post_dedupes_within_window() {
    let repo = create_test_repo();

    let post = run_cli(
        repo.path(),
        &["post", "Working in src/foo.rs now", "--agent-id", "agent-1"],
        None,
    );
    assert!(post.status.success());

    let input = r#"{"tool_input":{"file_path":"src/foo.rs"}}"#;
    let output1 = run_cli_with_env(
        repo.path(),
        &["eval", "pre-tool-use"],
        Some(input),
        &[("BUT_ENGINEERING_AGENT_ID", "agent-2")],
    );
    assert!(
        output1.status.success(),
        "first eval failed: {}",
        String::from_utf8_lossy(&output1.stderr)
    );
    let output2 = run_cli_with_env(
        repo.path(),
        &["eval", "pre-tool-use"],
        Some(input),
        &[("BUT_ENGINEERING_AGENT_ID", "agent-2")],
    );
    assert!(
        output2.status.success(),
        "second eval failed: {}",
        String::from_utf8_lossy(&output2.stderr)
    );

    let read = run_cli(repo.path(), &["read", "--agent-id", "agent-2"], None);
    assert!(
        read.status.success(),
        "read failed: {}",
        String::from_utf8_lossy(&read.stderr)
    );

    let messages: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    let coord_posts = messages
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| {
            m["agent_id"] == "agent-2"
                && m["content"]
                    .as_str()
                    .map(|c| c.contains("[coordination-check]") && c.contains("src/foo.rs"))
                    .unwrap_or(false)
        })
        .count();

    assert_eq!(
        coord_posts,
        1,
        "expected exactly one deduped advisory coordination post, got: {}",
        String::from_utf8_lossy(&read.stdout)
    );
}

#[test]
fn test_eval_pre_tool_use_does_not_use_heuristic_identity_for_side_effect_posts() {
    let repo = create_test_repo();

    let post = run_cli(
        repo.path(),
        &["post", "Working in src/foo.rs now", "--agent-id", "agent-1"],
        None,
    );
    assert!(post.status.success());

    // Make agent-2 the most-recently-active agent to exercise the heuristic path.
    let status = run_cli(repo.path(), &["status", "--agent-id", "agent-2", "reviewing"], None);
    assert!(status.status.success());

    let input = r#"{"tool_input":{"file_path":"src/foo.rs"}}"#;
    let output = run_cli(repo.path(), &["eval", "pre-tool-use"], Some(input));
    assert!(
        output.status.success(),
        "eval failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let read = run_cli(repo.path(), &["read", "--agent-id", "agent-2"], None);
    assert!(
        read.status.success(),
        "read failed: {}",
        String::from_utf8_lossy(&read.stderr)
    );

    let messages: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    let heuristic_posts = messages
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| {
            m["agent_id"] == "agent-2"
                && m["content"]
                    .as_str()
                    .map(|c| c.contains("[coordination-check]") && c.contains("src/foo.rs"))
                    .unwrap_or(false)
        })
        .count();

    assert_eq!(
        heuristic_posts,
        0,
        "heuristic identity should not be used for side-effect posts: {}",
        String::from_utf8_lossy(&read.stdout)
    );
}

#[test]
fn test_eval_pre_tool_use_accepts_tool_input_path_fallback() {
    let repo = create_test_repo();

    let claim = run_cli(repo.path(), &["claim", "src/foo.rs", "--agent-id", "agent-1"], None);
    assert!(claim.status.success());

    // Make agent-2 the most-recently-active fallback identity.
    let status = run_cli(repo.path(), &["status", "--agent-id", "agent-2", "working"], None);
    assert!(status.status.success());

    // MultiEdit-style payloads may use `path` instead of `file_path`.
    let input = r#"{"tool_input":{"path":"src/foo.rs"}}"#;
    let output = run_cli(repo.path(), &["eval", "pre-tool-use"], Some(input));
    assert!(
        output.status.success(),
        "eval failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let hook = &value["hookSpecificOutput"];
    assert_eq!(hook["hookEventName"], "PreToolUse");
    assert_eq!(hook["permissionDecision"], "deny");
}

// =============================================================================
// hook_common utility tests
// =============================================================================

use but_engineering::command::hook_common;
use but_engineering::types::{Agent, Claim, Message, MessageKind};

fn make_agent(id: &str) -> Agent {
    Agent {
        id: id.to_string(),
        status: None,
        last_active: chrono::Utc::now(),
        last_read: None,
        plan: None,
        plan_updated_at: None,
    }
}

fn make_message(agent_id: &str, content: &str, mins_ago: i64) -> Message {
    Message {
        id: format!("msg-{agent_id}-{mins_ago}"),
        agent_id: agent_id.to_string(),
        content: content.to_string(),
        timestamp: chrono::Utc::now() - chrono::Duration::minutes(mins_ago),
        kind: MessageKind::Message,
    }
}

fn make_claim(agent_id: &str, file_path: &str) -> Claim {
    Claim {
        file_path: file_path.to_string(),
        agent_id: agent_id.to_string(),
        claimed_at: chrono::Utc::now(),
    }
}

#[test]
fn test_truncate_ascii() {
    assert_eq!(hook_common::truncate("hello", 10), "hello");
    assert_eq!(hook_common::truncate("hello world", 5), "hello...");
    assert_eq!(hook_common::truncate("", 5), "");
}

#[test]
fn test_truncate_unicode() {
    // Each emoji is multiple bytes but 1 character.
    let emoji_str = "\u{1f600}\u{1f601}\u{1f602}\u{1f603}\u{1f604}"; // 5 emoji chars
    assert_eq!(hook_common::truncate(emoji_str, 5), emoji_str);
    let truncated = hook_common::truncate(emoji_str, 3);
    assert!(truncated.ends_with("..."));
    // Should contain exactly 3 emoji + "..."
    assert_eq!(truncated.chars().count(), 3 + 3); // 3 emoji + 3 dots
}

#[test]
fn test_truncate_replaces_newlines() {
    assert_eq!(hook_common::truncate("line1\nline2", 100), "line1 line2");
}

#[test]
fn test_select_cta_mentions_highest_priority() {
    let result = hook_common::select_cta(true, false, Some("custom"));
    assert_eq!(result, hook_common::CTA_MENTIONS);
}

#[test]
fn test_select_cta_extra_cta_overrides_default() {
    let result = hook_common::select_cta(false, false, Some("custom CTA"));
    assert_eq!(result, "custom CTA");
}

#[test]
fn test_select_cta_no_messages() {
    let result = hook_common::select_cta(false, true, None);
    assert_eq!(result, hook_common::CTA_NO_MESSAGES);
}

#[test]
fn test_select_cta_has_messages() {
    let result = hook_common::select_cta(false, false, None);
    assert_eq!(result, hook_common::CTA_HAS_MESSAGES);
}

#[test]
fn test_build_summary_empty() {
    let (summary, mentions) = hook_common::build_summary(&[], &[], chrono::Utc::now());
    assert!(summary.is_empty());
    assert!(!mentions);
}

#[test]
fn test_build_summary_with_agents_and_messages() {
    let now = chrono::Utc::now();
    let agents = vec![make_agent("agent-1"), make_agent("agent-2")];
    let messages = vec![make_message("agent-1", "Working on auth module", 2)];

    let (summary, mentions) = hook_common::build_summary(&agents, &messages, now);

    assert!(summary.contains("2 agent(s) active"));
    assert!(summary.contains("1 new msg(s)"));
    assert!(summary.contains("Working on auth module"));
    assert!(!mentions);
}

#[test]
fn test_build_summary_detects_mentions() {
    let now = chrono::Utc::now();
    let agents = vec![make_agent("agent-1"), make_agent("agent-2")];
    let messages = vec![make_message("agent-1", "@agent-2 can you review?", 1)];

    let (summary, mentions) = hook_common::build_summary(&agents, &messages, now);

    assert!(mentions);
    assert!(summary.contains("@mentions detected"));
    // Mentions get the >>> prefix.
    assert!(summary.contains(">>>"));
}

#[test]
fn test_format_claims_summary_empty() {
    assert_eq!(hook_common::format_claims_summary(&[], "claims"), "");
}

#[test]
fn test_format_claims_summary_grouped_by_agent() {
    let claims = vec![
        make_claim("agent-1", "src/auth.rs"),
        make_claim("agent-1", "src/db.rs"),
        make_claim("agent-2", "src/api.rs"),
    ];

    let result = hook_common::format_claims_summary(&claims, "claims");
    assert!(result.contains("claims:"));
    assert!(result.contains("agent-1"));
    assert!(result.contains("auth.rs"));
    assert!(result.contains("db.rs"));
    assert!(result.contains("agent-2"));
    assert!(result.contains("api.rs"));
}

#[test]
fn test_short_file_name() {
    assert_eq!(hook_common::short_file_name("src/auth/login.rs"), "login.rs");
    assert_eq!(hook_common::short_file_name("login.rs"), "login.rs");
    assert_eq!(hook_common::short_file_name(""), "");
}

#[test]
fn test_format_minutes_ago() {
    assert_eq!(hook_common::format_minutes_ago(0), "now");
    assert_eq!(hook_common::format_minutes_ago(-1), "now");
    assert_eq!(hook_common::format_minutes_ago(5), "5m ago");
}

#[test]
fn test_format_minutes_short() {
    assert_eq!(hook_common::format_minutes_short(0), "now");
    assert_eq!(hook_common::format_minutes_short(-1), "now");
    assert_eq!(hook_common::format_minutes_short(5), "5m");
}

#[test]
fn test_messages_mentioning_path() {
    let messages = vec![
        make_message("agent-1", "Working on src/auth.rs changes", 1),
        make_message("agent-2", "Looking at the database layer", 2),
    ];

    let matches = hook_common::messages_mentioning_path(&messages, "src/auth.rs");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].agent_id, "agent-1");

    let no_matches = hook_common::messages_mentioning_path(&messages, "src/api.rs");
    assert!(no_matches.is_empty());
}

// --- Mention detection edge cases (via build_summary) ---

#[test]
fn test_build_summary_mention_with_underscore() {
    let now = chrono::Utc::now();
    let agents = vec![make_agent("agent-1")];
    let messages = vec![make_message("agent-1", "@my_agent check this", 1)];

    let (_, mentions) = hook_common::build_summary(&agents, &messages, now);
    assert!(mentions, "@_ should be detected as mention");
}

#[test]
fn test_build_summary_mention_with_hyphen() {
    let now = chrono::Utc::now();
    let agents = vec![make_agent("agent-1")];
    let messages = vec![make_message("agent-1", "@fix-auth-k1 review please", 1)];

    let (_, mentions) = hook_common::build_summary(&agents, &messages, now);
    assert!(mentions, "@- should be detected as mention");
}

#[test]
fn test_build_summary_email_detected_as_mention() {
    // Known behavior: email addresses trigger mention detection.
    // Documenting this as a test so any fix doesn't regress unknowingly.
    let now = chrono::Utc::now();
    let agents = vec![make_agent("agent-1")];
    let messages = vec![make_message("agent-1", "contact user@example.com for help", 1)];

    let (_, mentions) = hook_common::build_summary(&agents, &messages, now);
    assert!(
        mentions,
        "email addresses currently trigger mention detection (known false positive)"
    );
}

#[test]
fn test_build_summary_trailing_at_not_mention() {
    let now = chrono::Utc::now();
    let agents = vec![make_agent("agent-1")];
    let messages = vec![make_message("agent-1", "looking @ the code", 1)];

    let (_, mentions) = hook_common::build_summary(&agents, &messages, now);
    // "@ " — space after @ is not alphanumeric/underscore/hyphen.
    assert!(!mentions, "'@ ' should not be detected as mention");
}

#[test]
fn test_build_summary_at_end_of_string_not_mention() {
    let now = chrono::Utc::now();
    let agents = vec![make_agent("agent-1")];
    let messages = vec![make_message("agent-1", "hello@", 1)];

    let (_, mentions) = hook_common::build_summary(&agents, &messages, now);
    // "hello@" — @ at end with no following character.
    assert!(!mentions, "trailing @ should not be detected as mention");
}

#[test]
fn test_build_summary_with_agent_status() {
    let now = chrono::Utc::now();
    let agents = vec![Agent {
        id: "agent-1".to_string(),
        status: Some("reviewing auth module".to_string()),
        last_active: chrono::Utc::now(),
        last_read: None,
        plan: None,
        plan_updated_at: None,
    }];
    let messages = vec![make_message("agent-1", "working on it", 1)];

    let (summary, _) = hook_common::build_summary(&agents, &messages, now);
    assert!(
        summary.contains("reviewing auth module"),
        "agent status should appear in summary"
    );
    assert!(
        summary.contains("agent-1: reviewing auth module"),
        "status should be prefixed with agent id"
    );
}

#[test]
fn test_truncate_exact_boundary() {
    // String with exactly max characters should not be truncated.
    assert_eq!(hook_common::truncate("abcde", 5), "abcde");
    // One over should truncate.
    assert_eq!(hook_common::truncate("abcdef", 5), "abcde...");
}

// --- Path normalization tests ---

#[test]
fn test_normalize_path_absolute_under_repo() {
    let dir = tempfile::tempdir().unwrap();
    let repo_root = dir.path();

    // Create a file so canonicalize works.
    let file = repo_root.join("src").join("auth.rs");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "").unwrap();

    let abs_path = file.to_str().unwrap();
    let result = hook_common::normalize_path(abs_path, repo_root);
    assert_eq!(result, "src/auth.rs");
}

#[test]
fn test_normalize_path_already_relative() {
    let dir = tempfile::tempdir().unwrap();
    let repo_root = dir.path();

    let result = hook_common::normalize_path("src/auth.rs", repo_root);
    // For relative paths that don't exist on disk, they may or may not resolve.
    // The function should at least return something reasonable without panicking.
    assert!(!result.is_empty());
}

#[test]
fn test_normalize_path_strips_trailing_slash() {
    let dir = tempfile::tempdir().unwrap();
    let repo_root = dir.path();

    let result = hook_common::normalize_path("src/auth/", repo_root);
    assert!(!result.ends_with('/'));
}

// =============================================================================
// Hook output format tests
// =============================================================================

#[test]
fn test_build_hook_json_structure() {
    let json = hook_common::build_hook_json("UserPromptSubmit", "2 agents active");
    let obj = json.as_object().unwrap();

    // Must have exactly one top-level key.
    assert_eq!(obj.len(), 1, "should have exactly hookSpecificOutput key");
    assert!(obj.contains_key("hookSpecificOutput"));

    let hook_output = obj["hookSpecificOutput"].as_object().unwrap();
    assert_eq!(hook_output["hookEventName"], "UserPromptSubmit");
    assert_eq!(hook_output["additionalContext"], "2 agents active");
}

#[test]
fn test_build_hook_json_different_events() {
    for event in &["UserPromptSubmit", "PreToolUse"] {
        let json = hook_common::build_hook_json(event, "test context");
        assert_eq!(json["hookSpecificOutput"]["hookEventName"], *event);
    }
}

#[test]
fn test_build_deny_json_structure() {
    let json = hook_common::build_deny_json("File src/auth.rs is claimed by agent-1");
    let obj = json.as_object().unwrap();

    assert_eq!(obj.len(), 1);
    assert!(obj.contains_key("hookSpecificOutput"));

    let hook_output = obj["hookSpecificOutput"].as_object().unwrap();
    assert_eq!(hook_output["hookEventName"], "PreToolUse");
    assert_eq!(hook_output["permissionDecision"], "deny");
    assert_eq!(
        hook_output["permissionDecisionReason"],
        "File src/auth.rs is claimed by agent-1"
    );
}

#[test]
fn test_build_deny_json_roundtrip() {
    let json = hook_common::build_deny_json("test reason");
    // Verify it serializes to a valid JSON string (what gets printed to stdout).
    let serialized = serde_json::to_string(&json).unwrap();
    // Re-parse to verify round-trip.
    let reparsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(reparsed["hookSpecificOutput"]["permissionDecision"], "deny");
}

#[test]
fn test_build_hook_json_roundtrip() {
    let json = hook_common::build_hook_json("PreToolUse", "warning about file conflict");
    let serialized = serde_json::to_string(&json).unwrap();
    let reparsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
    assert_eq!(
        reparsed["hookSpecificOutput"]["additionalContext"],
        "warning about file conflict"
    );
}

#[test]
fn test_deny_json_does_not_contain_additional_context() {
    // The deny format uses permissionDecision + permissionDecisionReason,
    // NOT additionalContext. Verify the structure is correct.
    let json = hook_common::build_deny_json("reason");
    let hook_output = &json["hookSpecificOutput"];
    assert!(hook_output.get("additionalContext").is_none());
    assert!(hook_output.get("permissionDecision").is_some());
    assert!(hook_output.get("permissionDecisionReason").is_some());
}

#[test]
fn test_hook_json_does_not_contain_permission_decision() {
    // The advisory format uses additionalContext, NOT permissionDecision.
    let json = hook_common::build_hook_json("PreToolUse", "advisory warning");
    let hook_output = &json["hookSpecificOutput"];
    assert!(hook_output.get("additionalContext").is_some());
    assert!(hook_output.get("permissionDecision").is_none());
}

// --- Session tests ---

#[test]
fn test_session_register_and_lookup() {
    let (_dir, db) = create_test_db();
    let now = chrono::Utc::now();

    db.register_session(12345, "agent-1", now).unwrap();

    let agent_id = db.get_session_agent(12345).unwrap();
    assert_eq!(agent_id, Some("agent-1".to_string()));
}

#[test]
fn test_session_lookup_missing() {
    let (_dir, db) = create_test_db();

    let agent_id = db.get_session_agent(99999).unwrap();
    assert_eq!(agent_id, None);
}

#[test]
fn test_session_upsert_updates_agent_id() {
    let (_dir, db) = create_test_db();
    let now = chrono::Utc::now();

    db.register_session(12345, "agent-1", now).unwrap();
    db.register_session(12345, "agent-2", now).unwrap();

    let agent_id = db.get_session_agent(12345).unwrap();
    assert_eq!(agent_id, Some("agent-2".to_string()));
}

#[test]
fn test_session_expire_stale() {
    let (_dir, db) = create_test_db();
    let now = chrono::Utc::now();
    let two_hours_ago = now - chrono::Duration::hours(2);

    db.register_session(12345, "agent-1", two_hours_ago).unwrap();
    db.register_session(67890, "agent-2", now).unwrap();

    let cutoff = now - chrono::Duration::hours(1);
    let expired = db.expire_stale_sessions(cutoff).unwrap();
    assert_eq!(expired, 1);

    // agent-1 session should be gone, agent-2 still present.
    assert_eq!(db.get_session_agent(12345).unwrap(), None);
    assert_eq!(db.get_session_agent(67890).unwrap(), Some("agent-2".to_string()));
}

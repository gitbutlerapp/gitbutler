use std::sync::{Arc, Barrier};
use std::time::Duration;

use rusqlite::{Connection, params};
use serde_json::json;

#[allow(dead_code)]
#[path = "../src/services/acquire.rs"]
mod acquire_impl;
#[allow(dead_code)]
#[path = "../src/cli.rs"]
mod cli;
#[allow(dead_code, unused_imports)]
#[path = "../src/db/mod.rs"]
mod db;
#[allow(dead_code)]
#[path = "../src/payloads.rs"]
mod payloads;
#[allow(dead_code)]
#[path = "../src/repo.rs"]
mod repo;
#[allow(dead_code)]
#[path = "../src/text.rs"]
mod text;

use acquire_impl::{AcquireResponse, acquire_batch};
use tempfile::TempDir;

#[test]
fn dry_run_ignores_free_text_blocking_words() -> anyhow::Result<()> {
    let conn = Connection::open_in_memory()?;
    db::init_db(&conn)?;
    db::insert_history_message(
        &conn,
        db::now_unix_ms()?,
        "peer",
        "message",
        &json!({ "text": "please avoid src/app.txt while I refactor" }).to_string(),
    )?;

    let mut conn = conn;
    let result = acquire_batch(
        &mut conn,
        "me",
        &[String::from("src/app.txt")],
        Duration::from_secs(60),
        false,
        true,
    )?;
    assert_eq!(result.decisions[0].decision, "allow");
    assert_eq!(result.decisions[0].reason_code, "no_conflict");
    Ok(())
}

#[test]
fn dry_run_respects_typed_blocks() -> anyhow::Result<()> {
    let mut conn = Connection::open_in_memory()?;
    db::init_db(&conn)?;
    conn.execute(
        "INSERT INTO blocks(agent_id, mode, reason, created_at_ms, expires_at_ms, resolved_at_ms, resolved_by_agent_id)
         VALUES ('peer', 'hard', 'shared refactor', 10, NULL, NULL, NULL)",
        [],
    )?;
    let block_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO block_paths(block_id, path) VALUES (?1, 'src/app.txt')",
        params![block_id],
    )?;

    let result = acquire_batch(
        &mut conn,
        "me",
        &[String::from("src/app.txt")],
        Duration::from_secs(60),
        false,
        true,
    )?;
    assert_eq!(result.decisions[0].decision, "deny");
    assert_eq!(result.decisions[0].reason_code, "hard_block");
    Ok(())
}

#[test]
fn acquire_claims_clear_paths_and_skips_blocked_paths() -> anyhow::Result<()> {
    let mut conn = Connection::open_in_memory()?;
    db::init_db(&conn)?;
    conn.execute(
        "INSERT INTO blocks(agent_id, mode, reason, created_at_ms, expires_at_ms, resolved_at_ms, resolved_by_agent_id)
         VALUES ('peer', 'hard', 'shared refactor', 10, NULL, NULL, NULL)",
        [],
    )?;
    let block_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO block_paths(block_id, path) VALUES (?1, 'src/blocked.txt')",
        params![block_id],
    )?;

    acquire_batch(
        &mut conn,
        "me",
        &[
            String::from("src/clear.txt"),
            String::from("src/blocked.txt"),
        ],
        Duration::from_secs(900),
        false,
        false,
    )?;

    let claims = db::load_active_claims_for_agent(&conn, "me")?;
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].path, "src/clear.txt");
    Ok(())
}

#[test]
fn acquire_serializes_concurrent_writers_into_blocked_result() -> anyhow::Result<()> {
    let tempdir = TempDir::new()?;
    let db_path = tempdir.path().join("coord.db");

    let conn = Connection::open(&db_path)?;
    db::init_db(&conn)?;
    drop(conn);

    let barrier = Arc::new(Barrier::new(2));
    let db_path_a = db_path.clone();
    let barrier_a = barrier.clone();
    let thread_a = std::thread::spawn(move || -> anyhow::Result<AcquireResponse> {
        let mut conn = Connection::open(db_path_a)?;
        db::init_db(&conn)?;
        barrier_a.wait();
        acquire_batch(
            &mut conn,
            "agent-a",
            &[String::from("src/app.txt")],
            Duration::from_secs(900),
            false,
            false,
        )
    });

    let barrier_b = barrier.clone();
    let thread_b = std::thread::spawn(move || -> anyhow::Result<AcquireResponse> {
        let mut conn = Connection::open(db_path)?;
        db::init_db(&conn)?;
        barrier_b.wait();
        acquire_batch(
            &mut conn,
            "agent-b",
            &[String::from("src/app.txt")],
            Duration::from_secs(900),
            false,
            false,
        )
    });

    let response_a = thread_a.join().expect("thread a panicked")?;
    let response_b = thread_b.join().expect("thread b panicked")?;
    let decisions = [
        response_a.decisions[0].decision,
        response_b.decisions[0].decision,
    ];

    assert_eq!(
        decisions
            .iter()
            .filter(|decision| **decision == "acquired")
            .count(),
        1
    );
    assert_eq!(
        decisions
            .iter()
            .filter(|decision| **decision == "blocked")
            .count(),
        1
    );

    let conn = Connection::open(tempdir.path().join("coord.db"))?;
    db::init_db(&conn)?;
    let claims = db::load_active_claims(&conn, Some("src/app.txt"))?;
    assert_eq!(claims.len(), 1);
    Ok(())
}

#[test]
fn dry_run_matches_mutating_decisions_without_writing_claims() -> anyhow::Result<()> {
    let mut conn = Connection::open_in_memory()?;
    db::init_db(&conn)?;

    let dry_run = acquire_batch(
        &mut conn,
        "me",
        &[String::from("src/app.txt")],
        Duration::from_secs(900),
        false,
        true,
    )?;
    assert!(dry_run.dry_run);
    assert_eq!(dry_run.decisions[0].decision, "allow");
    assert!(db::load_active_claims_for_agent(&conn, "me")?.is_empty());

    let acquired = acquire_batch(
        &mut conn,
        "me",
        &[String::from("src/app.txt")],
        Duration::from_secs(900),
        false,
        false,
    )?;
    assert!(!acquired.dry_run);
    assert_eq!(acquired.decisions[0].decision, "acquired");
    assert_eq!(db::load_active_claims_for_agent(&conn, "me")?.len(), 1);
    Ok(())
}

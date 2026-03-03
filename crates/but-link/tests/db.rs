use rusqlite::{Connection, params};
use serde_json::{Value, json};

#[allow(dead_code, unused_imports)]
#[path = "../src/db/mod.rs"]
mod db;
#[allow(dead_code)]
#[path = "../src/payloads.rs"]
mod payloads;
#[allow(dead_code)]
#[path = "../src/text.rs"]
mod text;

/// Structured surface row used by dependency-hint tests.
struct SurfaceInsert<'a> {
    conn: &'a Connection,
    created_at_ms: i64,
    agent_id: &'a str,
    kind: &'a str,
    scope: &'a str,
    tags: &'a [&'a str],
    surface: &'a [&'a str],
    paths: &'a [&'a str],
}

/// Insert a message row used by cursor tests.
fn insert_message(
    conn: &Connection,
    created_at_ms: i64,
    agent_id: &str,
    kind: &str,
    body: Value,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO messages(created_at_ms, agent_id, kind, body_json) VALUES (?1, ?2, ?3, ?4)",
        params![created_at_ms, agent_id, kind, body.to_string()],
    )?;
    Ok(())
}

/// Insert a typed surface declaration used by dependency tests.
fn insert_surface(input: SurfaceInsert<'_>) -> anyhow::Result<()> {
    input.conn.execute(
        "INSERT INTO surface_declarations(created_at_ms, agent_id, kind, scope)
         VALUES (?1, ?2, ?3, ?4)",
        params![input.created_at_ms, input.agent_id, input.kind, input.scope],
    )?;
    let declaration_id = input.conn.last_insert_rowid();
    for (ord, tag) in input.tags.iter().enumerate() {
        input.conn.execute(
            "INSERT INTO surface_tags(declaration_id, ord, tag) VALUES (?1, ?2, ?3)",
            params![declaration_id, ord as i64, tag],
        )?;
    }
    for (ord, token) in input.surface.iter().enumerate() {
        input.conn.execute(
            "INSERT INTO surface_tokens(declaration_id, ord, token) VALUES (?1, ?2, ?3)",
            params![declaration_id, ord as i64, token],
        )?;
    }
    for (ord, path) in input.paths.iter().enumerate() {
        input.conn.execute(
            "INSERT INTO surface_paths(declaration_id, ord, path) VALUES (?1, ?2, ?3)",
            params![declaration_id, ord as i64, path],
        )?;
    }
    Ok(())
}

#[test]
fn init_db_migrates_agent_state_columns() -> anyhow::Result<()> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(
        "CREATE TABLE agent_state (
            agent_id TEXT PRIMARY KEY,
            status TEXT,
            plan TEXT,
            updated_at_ms INTEGER NOT NULL
        );",
    )?;
    conn.execute(
        "INSERT INTO agent_state(agent_id, status, plan, updated_at_ms) VALUES ('a', NULL, NULL, 42)",
        [],
    )?;

    db::init_db(&conn)?;

    let columns: Vec<String> = conn
        .prepare("PRAGMA table_info(agent_state)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    assert!(!columns.iter().any(|column| column == "updated_at_ms"));
    let snapshot = db::load_agent_snapshots(&conn)?.remove(0);
    assert_eq!(snapshot.last_seen_at_ms, 42);
    assert_eq!(snapshot.last_progress_at_ms, 42);
    Ok(())
}

#[test]
fn unread_inbox_updates_track_directed_messages() -> anyhow::Result<()> {
    let conn = Connection::open_in_memory()?;
    db::init_db(&conn)?;
    insert_message(
        &conn,
        1,
        "peer",
        "message",
        json!({ "text": "@me: ack: saw it" }),
    )?;
    insert_message(&conn, 2, "peer", "message", json!({ "text": "unrelated" }))?;

    let (updates, prev_cursor, new_cursor) = db::unread_inbox_updates(&conn, "me", 1_000)?;
    assert_eq!(prev_cursor, 0);
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].agent_id, "peer");
    assert_eq!(new_cursor, 2);
    Ok(())
}

#[test]
fn dependency_hints_filter_by_requested_paths() -> anyhow::Result<()> {
    let conn = Connection::open_in_memory()?;
    db::init_db(&conn)?;
    insert_surface(SurfaceInsert {
        conn: &conn,
        created_at_ms: 1,
        agent_id: "me",
        kind: "intent",
        scope: "crate::auth",
        tags: &["api"],
        surface: &["AuthToken"],
        paths: &["src/auth.rs"],
    })?;
    insert_surface(SurfaceInsert {
        conn: &conn,
        created_at_ms: 2,
        agent_id: "peer",
        kind: "declaration",
        scope: "crate::auth",
        tags: &["api"],
        surface: &["AuthToken"],
        paths: &["src/auth.rs"],
    })?;
    insert_surface(SurfaceInsert {
        conn: &conn,
        created_at_ms: 3,
        agent_id: "peer-b",
        kind: "declaration",
        scope: "crate::auth",
        tags: &["api"],
        surface: &["AuthToken"],
        paths: &["src/other.rs"],
    })?;

    let hints = db::dependency_hints_for_paths(&conn, "me", &[String::from("src/auth.rs")])?;
    assert_eq!(hints.len(), 1);
    assert_eq!(hints[0].provider_agent_id, "peer");
    Ok(())
}

#[test]
fn migration_removes_legacy_typed_message_rows() -> anyhow::Result<()> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(
        "CREATE TABLE messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            created_at_ms INTEGER NOT NULL,
            agent_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            body_json TEXT NOT NULL
        );",
    )?;
    insert_message(
        &conn,
        1,
        "peer",
        "discovery",
        json!({ "title": "legacy", "evidence": [{"detail":"x"}], "suggested_action": {"cmd":"echo"} }),
    )?;
    insert_message(&conn, 2, "peer", "message", json!({ "text": "keep me" }))?;

    db::init_db(&conn)?;

    let typed_count: i64 = conn.query_row(
        "SELECT COUNT(1) FROM messages WHERE kind IN ('discovery','intent','declaration')",
        [],
        |row| row.get(0),
    )?;
    let free_text_count: i64 = conn.query_row(
        "SELECT COUNT(1) FROM messages WHERE kind = 'message'",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(typed_count, 0);
    assert_eq!(free_text_count, 1);
    Ok(())
}

#[test]
fn ack_exists_since_matches_scoped_and_generic_acks() -> anyhow::Result<()> {
    let conn = Connection::open_in_memory()?;
    db::init_db(&conn)?;
    conn.execute(
        "INSERT INTO acknowledgements(agent_id, target_agent_id, note, created_at_ms)
         VALUES ('me', 'peer', NULL, 10)",
        [],
    )?;
    let generic_ack_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO acknowledgements(agent_id, target_agent_id, note, created_at_ms)
         VALUES ('me', 'peer', NULL, 11)",
        [],
    )?;
    let scoped_ack_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO ack_paths(ack_id, path) VALUES (?1, 'src/app.txt')",
        params![scoped_ack_id],
    )?;

    assert!(db::ack_exists_since(
        &conn,
        "me",
        "peer",
        "src/other.txt",
        0
    )?);
    conn.execute(
        "DELETE FROM acknowledgements WHERE id = ?1",
        params![generic_ack_id],
    )?;
    assert!(db::ack_exists_since(&conn, "me", "peer", "src/app.txt", 0)?);
    assert!(!db::ack_exists_since(
        &conn,
        "me",
        "peer",
        "src/other.txt",
        0
    )?);
    Ok(())
}

#[test]
fn load_open_blocks_matches_between_connection_and_transaction() -> anyhow::Result<()> {
    let mut conn = Connection::open_in_memory()?;
    db::init_db(&conn)?;
    conn.execute(
        "INSERT INTO blocks(agent_id, mode, reason, created_at_ms, expires_at_ms, resolved_at_ms, resolved_by_agent_id)
         VALUES ('peer', 'hard', 'shared refactor', 10, NULL, NULL, NULL)",
        [],
    )?;
    let block_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO block_paths(block_id, path) VALUES (?1, 'src/app.rs')",
        params![block_id],
    )?;

    let expected = db::load_open_blocks(&conn, Some("me"), Some("src/app.rs"))?;
    let tx = conn.transaction()?;
    let actual =
        db::load_open_blocks_with_handle(&tx, Some("me"), Some("src/app.rs"), db::now_unix_ms()?)?;

    assert_eq!(actual.len(), expected.len());
    assert_eq!(actual[0].id, expected[0].id);
    assert_eq!(actual[0].paths, expected[0].paths);
    Ok(())
}

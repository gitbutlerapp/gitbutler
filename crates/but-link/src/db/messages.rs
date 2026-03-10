//! Free-text message history queries and inbox updates.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use super::{HistoryMessageRecord, UnreadUpdate};
use crate::text::{extract_message_text, parse_body};

/// Persist a transcript message.
pub(crate) fn insert_history_message(
    conn: &Connection,
    created_at_ms: i64,
    agent_id: &str,
    kind: &str,
    body_json: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO messages(created_at_ms, agent_id, kind, body_json) VALUES (?1, ?2, ?3, ?4)",
        params![created_at_ms, agent_id, kind, body_json],
    )?;
    Ok(())
}

/// Load unread inbox updates for an agent and advance the cursor.
pub(crate) fn unread_inbox_updates(
    conn: &Connection,
    agent_id: &str,
    now_ms: i64,
) -> anyhow::Result<(Vec<UnreadUpdate>, i64, i64)> {
    let topic = "inbox";
    let prev_cursor: i64 = conn
        .query_row(
            "SELECT last_seen_msg_id FROM agent_cursors WHERE agent_id = ?1 AND topic = ?2",
            params![agent_id, topic],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0);

    let mention = format!("@{agent_id}");
    let mut stmt = conn.prepare(
        "SELECT id, created_at_ms, agent_id, kind, body_json
         FROM messages
         WHERE id > ?1 AND agent_id <> ?2
         ORDER BY id ASC
         LIMIT 500",
    )?;
    let rows = stmt.query_map(params![prev_cursor, agent_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    let mut updates = Vec::new();
    let mut max_id = prev_cursor;
    let mut last_returned_id = prev_cursor;
    let mut reached_limit = false;
    for row in rows {
        let (id, created_at_ms, from_agent, kind, body_json) = row?;
        max_id = max_id.max(id);
        let body: Value =
            serde_json::from_str(&body_json).unwrap_or(Value::String(body_json.clone()));
        let text = extract_message_text(&body, &body_json);
        let is_directed = text.contains(&mention)
            || body
                .get("target_agent_id")
                .and_then(Value::as_str)
                .is_some_and(|target| target == agent_id);
        if !is_directed {
            continue;
        }
        if updates.len() >= 20 {
            reached_limit = true;
            continue;
        }
        last_returned_id = id;
        updates.push(UnreadUpdate {
            id,
            created_at_ms,
            agent_id: from_agent,
            kind,
            body,
        });
    }

    let new_cursor = if reached_limit {
        last_returned_id
    } else {
        max_id
    };
    if new_cursor != prev_cursor {
        conn.execute(
            "INSERT INTO agent_cursors(agent_id, topic, last_seen_msg_id, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(agent_id, topic)
             DO UPDATE SET last_seen_msg_id = excluded.last_seen_msg_id, updated_at_ms = excluded.updated_at_ms",
            params![agent_id, topic, new_cursor, now_ms],
        )?;
    }
    Ok((updates, prev_cursor, new_cursor))
}

/// Load transcript messages since a timestamp.
pub(crate) fn load_messages_since(
    conn: &Connection,
    kind: Option<&str>,
    since_ms: i64,
) -> anyhow::Result<Vec<Value>> {
    let records = load_message_records_since_ms(conn, kind, since_ms, None)?;
    let mut messages = Vec::new();
    for record in records {
        let (body_v, content) = parse_body(&record.body_json);
        messages.push(json!({
            "created_at_ms": record.created_at_ms,
            "agent_id": record.agent_id,
            "kind": record.kind,
            "body": body_v,
            "content": content,
        }));
    }
    Ok(messages)
}

/// Load full history rows since a timestamp for the TUI initial window.
pub(crate) fn load_message_records_since_ms(
    conn: &Connection,
    kind: Option<&str>,
    since_ms: i64,
    limit: Option<i64>,
) -> anyhow::Result<Vec<HistoryMessageRecord>> {
    let kind = kind.filter(|kind| !kind.is_empty() && *kind != "all");
    match (kind, limit) {
        (Some(kind_filter), Some(limit)) => {
            let mut stmt = conn.prepare(
                "SELECT id, created_at_ms, agent_id, kind, body_json FROM messages
                 WHERE created_at_ms >= ?1 AND kind = ?2
                 ORDER BY id ASC
                 LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![since_ms, kind_filter, limit], |row| {
                Ok(HistoryMessageRecord {
                    created_at_ms: row.get(1)?,
                    agent_id: row.get(2)?,
                    kind: row.get(3)?,
                    body_json: row.get(4)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        }
        (Some(kind_filter), None) => {
            let mut stmt = conn.prepare(
                "SELECT id, created_at_ms, agent_id, kind, body_json FROM messages
                 WHERE created_at_ms >= ?1 AND kind = ?2
                 ORDER BY id ASC",
            )?;
            let rows = stmt.query_map(params![since_ms, kind_filter], |row| {
                Ok(HistoryMessageRecord {
                    created_at_ms: row.get(1)?,
                    agent_id: row.get(2)?,
                    kind: row.get(3)?,
                    body_json: row.get(4)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        }
        (None, Some(limit)) => {
            let mut stmt = conn.prepare(
                "SELECT id, created_at_ms, agent_id, kind, body_json FROM messages
                 WHERE created_at_ms >= ?1
                 ORDER BY id ASC
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![since_ms, limit], |row| {
                Ok(HistoryMessageRecord {
                    created_at_ms: row.get(1)?,
                    agent_id: row.get(2)?,
                    kind: row.get(3)?,
                    body_json: row.get(4)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        }
        (None, None) => {
            let mut stmt = conn.prepare(
                "SELECT id, created_at_ms, agent_id, kind, body_json FROM messages
                 WHERE created_at_ms >= ?1
                 ORDER BY id ASC",
            )?;
            let rows = stmt.query_map(params![since_ms], |row| {
                Ok(HistoryMessageRecord {
                    created_at_ms: row.get(1)?,
                    agent_id: row.get(2)?,
                    kind: row.get(3)?,
                    body_json: row.get(4)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        }
    }
}

//! Typed discovery storage and read-side queries.

use rusqlite::{Connection, Transaction, params};
use serde_json::{Value, json};

use super::{DiscoveryRecord, PrepareSql};
use crate::payloads::DiscoveryPayload;
use crate::text::{contains_path_token, relevant_needles_for_path};

/// Insert a structured discovery and its evidence rows.
pub(crate) fn insert_discovery(
    tx: &Transaction<'_>,
    created_at_ms: i64,
    agent_id: &str,
    payload: &DiscoveryPayload,
) -> anyhow::Result<i64> {
    tx.execute(
        "INSERT INTO discoveries(created_at_ms, agent_id, title, signal, suggested_cmd)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            created_at_ms,
            agent_id,
            payload.title,
            payload.signal,
            payload.suggested_action.cmd
        ],
    )?;
    let discovery_id = tx.last_insert_rowid();
    for (ord, evidence) in payload.evidence.iter().enumerate() {
        tx.execute(
            "INSERT INTO discovery_evidence(discovery_id, ord, detail)
             VALUES (?1, ?2, ?3)",
            params![discovery_id, ord as i64, evidence.detail],
        )?;
    }
    Ok(discovery_id)
}

/// Load all discoveries from typed storage.
pub(crate) fn load_discoveries(
    conn: &Connection,
    high_signal_only: bool,
) -> anyhow::Result<Vec<DiscoveryRecord>> {
    load_discoveries_filtered(conn, None, high_signal_only)
}

/// Load discoveries created after the provided timestamp.
pub(crate) fn load_discoveries_since(
    conn: &Connection,
    since_ms: i64,
) -> anyhow::Result<Vec<Value>> {
    Ok(load_discoveries_filtered(conn, Some(since_ms), false)?
        .into_iter()
        .map(discovery_to_value)
        .collect())
}

/// Load discovery rows relevant to the requested paths.
pub(crate) fn recent_discoveries_for_paths(
    conn: &impl PrepareSql,
    paths: &[String],
    limit: usize,
) -> anyhow::Result<Vec<Value>> {
    if paths.is_empty() || limit == 0 {
        return Ok(Vec::new());
    }

    let needles: Vec<String> = paths
        .iter()
        .flat_map(|path| relevant_needles_for_path(path))
        .collect();
    let mut stmt = conn.prepare_query(
        "SELECT d.id, d.created_at_ms, d.agent_id, d.title, d.signal, d.suggested_cmd
         FROM discoveries d
         ORDER BY d.id DESC
         LIMIT 100",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(DiscoveryRecord {
            id: row.get(0)?,
            created_at_ms: row.get(1)?,
            agent_id: row.get(2)?,
            title: row.get(3)?,
            signal: row.get(4)?,
            suggested_cmd: row.get(5)?,
            evidence: Vec::new(),
        })
    })?;

    let mut discoveries = Vec::new();
    for row in rows {
        let mut record = row?;
        record.evidence = load_discovery_evidence_generic(conn, record.id)?;
        let searchable = discovery_searchable_text(&record);
        if !needles
            .iter()
            .any(|needle| contains_path_token(&searchable, needle))
        {
            continue;
        }
        discoveries.push(discovery_to_value(record));
        if discoveries.len() >= limit {
            break;
        }
    }
    Ok(discoveries)
}

/// Convert a discovery record into the JSON shape used by read views.
pub(crate) fn discovery_to_value(record: DiscoveryRecord) -> Value {
    let mut obj = json!({
        "id": record.id,
        "created_at_ms": record.created_at_ms,
        "agent_id": record.agent_id,
        "title": record.title,
        "evidence": record
            .evidence
            .into_iter()
            .map(|detail| json!({ "detail": detail }))
            .collect::<Vec<_>>(),
        "kind": "discovery",
    });
    if let Value::Object(map) = &mut obj {
        if let Some(signal) = record.signal {
            map.insert("signal".to_owned(), Value::String(signal));
        }
        if let Some(cmd) = record.suggested_cmd {
            map.insert("suggested_action".to_owned(), json!({ "cmd": cmd }));
        }
    }
    obj
}

/// Load evidence rows for one discovery using a connection.
fn load_discovery_evidence(conn: &Connection, discovery_id: i64) -> anyhow::Result<Vec<String>> {
    load_discovery_evidence_generic(conn, discovery_id)
}

/// Load discoveries with optional server-side filtering.
fn load_discoveries_filtered(
    conn: &Connection,
    since_ms: Option<i64>,
    high_signal_only: bool,
) -> anyhow::Result<Vec<DiscoveryRecord>> {
    let mut discoveries = Vec::new();
    if let Some(since_ms) = since_ms {
        let mut stmt = conn.prepare(
            "SELECT id, created_at_ms, agent_id, title, signal, suggested_cmd
             FROM discoveries
             WHERE created_at_ms >= ?1
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![since_ms], map_discovery_row)?;
        for row in rows {
            let mut record = row?;
            if high_signal_only && !has_high_signal(record.signal.as_deref()) {
                continue;
            }
            record.evidence = load_discovery_evidence(conn, record.id)?;
            discoveries.push(record);
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, created_at_ms, agent_id, title, signal, suggested_cmd
             FROM discoveries
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], map_discovery_row)?;
        for row in rows {
            let mut record = row?;
            if high_signal_only && !has_high_signal(record.signal.as_deref()) {
                continue;
            }
            record.evidence = load_discovery_evidence(conn, record.id)?;
            discoveries.push(record);
        }
    }
    Ok(discoveries)
}

/// Map one discovery row into the shared typed record.
fn map_discovery_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DiscoveryRecord> {
    Ok(DiscoveryRecord {
        id: row.get(0)?,
        created_at_ms: row.get(1)?,
        agent_id: row.get(2)?,
        title: row.get(3)?,
        signal: row.get(4)?,
        suggested_cmd: row.get(5)?,
        evidence: Vec::new(),
    })
}

/// Return whether a discovery signal is the high-signal level.
fn has_high_signal(signal: Option<&str>) -> bool {
    signal.is_some_and(|signal| signal.eq_ignore_ascii_case("high"))
}

/// Load evidence rows for one discovery using a generic SQL handle.
fn load_discovery_evidence_generic(
    conn: &impl PrepareSql,
    discovery_id: i64,
) -> anyhow::Result<Vec<String>> {
    let mut stmt = conn.prepare_query(
        "SELECT detail
         FROM discovery_evidence
         WHERE discovery_id = ?1
         ORDER BY ord ASC",
    )?;
    let rows = stmt.query_map(params![discovery_id], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Build searchable discovery text for path-scoped advisory lookup.
fn discovery_searchable_text(record: &DiscoveryRecord) -> String {
    let mut parts = vec![record.title.clone()];
    parts.extend(record.evidence.iter().cloned());
    if let Some(cmd) = &record.suggested_cmd {
        parts.push(cmd.clone());
    }
    parts.join("\n")
}

//! Database initialization, queries, and coordination-state helpers.
//!
//! All functions that take a `rusqlite::Connection` live here.

use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Serialize)]
pub(crate) struct StaleAgent {
    pub kind: &'static str,
    pub agent_id: String,
    pub updated_at_ms: i64,
    pub stale_for_ms: i64,
    pub threshold_ms: i64,
    pub is_stale: bool,
    pub suggested_cmd: String,
}

#[derive(Serialize)]
pub(crate) struct DependencyNextStep {
    pub kind: &'static str,
    pub suggested_cmd: String,
}

#[derive(Serialize)]
pub(crate) struct DependencyHint {
    pub kind: &'static str,
    pub provider_agent_id: String,
    pub scope: String,
    pub tags: Vec<String>,
    pub overlap_tokens: Vec<String>,
    pub why: String,
    pub next_step: DependencyNextStep,
}

#[derive(Serialize)]
pub(crate) struct UnreadUpdate {
    pub id: i64,
    pub created_at_ms: i64,
    pub agent_id: String,
    pub kind: String,
    pub body: Value,
}

use crate::text::{
    DiscoveryBlocker, contains_path_token, discovery_block_window_ms, extract_message_text,
    is_discovery_block_text, parse_body, relevant_needles_for_path, strip_common_list_prefix,
    strip_leading_markdown_emphasis, strip_leading_wrappers,
};

pub(crate) fn init_db(conn: &Connection) -> anyhow::Result<()> {
    // Enable WAL mode and set a busy timeout so concurrent agent processes
    // don't immediately fail with "database is locked".
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;\
         PRAGMA busy_timeout = 5000;",
    )?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS claims (\
            path TEXT NOT NULL,\
            agent_id TEXT NOT NULL,\
            expires_at_ms INTEGER NOT NULL\
         );\
         CREATE INDEX IF NOT EXISTS idx_claims_path_expires \
            ON claims(path, expires_at_ms);\
         CREATE TABLE IF NOT EXISTS agent_state (\
            agent_id TEXT PRIMARY KEY,\
            status TEXT,\
            plan TEXT,\
            updated_at_ms INTEGER NOT NULL\
         );\
         CREATE TABLE IF NOT EXISTS messages (\
            id INTEGER PRIMARY KEY AUTOINCREMENT,\
            created_at_ms INTEGER NOT NULL,\
            agent_id TEXT NOT NULL,\
            kind TEXT NOT NULL,\
            body_json TEXT NOT NULL\
         );\
         CREATE TABLE IF NOT EXISTS agent_cursors (\
            agent_id TEXT NOT NULL,\
            topic TEXT NOT NULL,\
            last_seen_msg_id INTEGER NOT NULL,\
            updated_at_ms INTEGER NOT NULL,\
            PRIMARY KEY(agent_id, topic)\
         );\
        ",
    )
    .context("init_db")?;

    // Prune expired claims and old messages to prevent unbounded DB growth.
    prune(conn)?;
    Ok(())
}

/// Maximum number of messages to retain in the database.
const MAX_MESSAGES: i64 = 10_000;

/// Delete expired claims and cap messages to the most recent MAX_MESSAGES.
fn prune(conn: &Connection) -> anyhow::Result<()> {
    let now_ms = now_unix_ms()?;
    conn.execute(
        "DELETE FROM claims WHERE expires_at_ms <= ?1",
        params![now_ms],
    )?;

    let count: i64 = conn.query_row("SELECT COUNT(1) FROM messages", [], |row| row.get(0))?;
    if count > MAX_MESSAGES {
        conn.execute(
            "DELETE FROM messages WHERE id NOT IN (\
                SELECT id FROM messages ORDER BY id DESC LIMIT ?1\
             )",
            params![MAX_MESSAGES],
        )?;
    }
    Ok(())
}

pub(crate) fn ensure_agent_row(
    conn: &Connection,
    agent_id: &str,
    now_ms: i64,
) -> anyhow::Result<()> {
    conn.execute(
        "INSERT INTO agent_state(agent_id, status, plan, updated_at_ms) VALUES (?1, NULL, NULL, ?2) \
         ON CONFLICT(agent_id) DO UPDATE SET updated_at_ms = excluded.updated_at_ms",
        params![agent_id, now_ms],
    )
    .context("ensure_agent_row")?;
    Ok(())
}

pub(crate) fn touch_agent(conn: &Connection, agent_id: &str) -> anyhow::Result<()> {
    let now_ms = now_unix_ms()?;
    ensure_agent_row(conn, agent_id, now_ms)
}

pub(crate) fn now_unix_ms() -> anyhow::Result<i64> {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock")?;
    dur.as_millis().try_into().context("timestamp overflow")
}

fn value_string(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|field| field.as_str())
        .map(std::borrow::ToOwned::to_owned)
}

fn value_string_array(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|field| field.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|item| item.as_str().map(std::borrow::ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Find the most recent message from `from_agent_id` that mentions `path`.
pub(crate) fn last_relevant_update_from_agent(
    conn: &Connection,
    from_agent_id: &str,
    path: &str,
) -> anyhow::Result<Option<(i64, i64, String)>> {
    let needles = relevant_needles_for_path(path);
    let mut stmt = conn.prepare(
        "SELECT id, created_at_ms, body_json FROM messages \
         WHERE agent_id = ?1 AND kind IN ('message','discovery') \
         ORDER BY id DESC \
         LIMIT 500",
    )?;
    let rows = stmt.query_map(params![from_agent_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;

    for r in rows {
        let (id, created_at_ms, body_json) = r?;
        let (_, txt) = parse_body(&body_json);
        if needles.iter().any(|n| contains_path_token(&txt, n)) {
            return Ok(Some((id, created_at_ms, txt)));
        }
    }
    Ok(None)
}

/// Check whether `requester_agent_id` has posted an ack for `target_agent_id` since `since_created_at_ms`.
pub(crate) fn requester_has_acked_since(
    conn: &Connection,
    requester_agent_id: &str,
    target_agent_id: &str,
    since_created_at_ms: i64,
    path_needles: &[String],
) -> anyhow::Result<bool> {
    let mut ack_needles_lower: Vec<String> = Vec::with_capacity(8);
    for base in [
        format!("@{target_agent_id}: ack"),
        format!("@{target_agent_id} ack"),
        format!("@{target_agent_id}: acknowledged"),
        format!("@{target_agent_id} acknowledged"),
        format!("@{target_agent_id}: thanks"),
        format!("@{target_agent_id} thanks"),
        format!("@{target_agent_id}: got it"),
        format!("@{target_agent_id} got it"),
    ] {
        let base_lower = base.to_ascii_lowercase();
        ack_needles_lower.push(base_lower.clone());
        ack_needles_lower.push(format!("{base_lower} "));
        ack_needles_lower.push(format!("{base_lower}\t"));
        for p in [':', '.', '!', '?', ','] {
            ack_needles_lower.push(format!("{base_lower}{p}"));
        }
    }
    let mut stmt = conn.prepare(
        "SELECT body_json FROM messages \
         WHERE agent_id = ?1 AND kind = 'message' AND created_at_ms >= ?2 \
         ORDER BY id DESC \
         LIMIT 500",
    )?;
    let rows = stmt.query_map(params![requester_agent_id, since_created_at_ms], |row| {
        row.get::<_, String>(0)
    })?;

    for r in rows {
        let body_json = r?;
        let (_, txt) = parse_body(&body_json);
        if !path_needles.is_empty() && !path_needles.iter().any(|n| contains_path_token(&txt, n)) {
            continue;
        }
        let mut scanned_bytes: usize = 0;
        let mut in_fenced_code_block = false;
        for line in txt.lines().take(16) {
            scanned_bytes = scanned_bytes.saturating_add(line.len());
            if scanned_bytes > 1024 {
                break;
            }
            let l = line.trim_start();
            if l.starts_with("```") {
                in_fenced_code_block = !in_fenced_code_block;
                continue;
            }
            if in_fenced_code_block || l.is_empty() {
                continue;
            }
            for candidate in [
                l,
                strip_common_list_prefix(l),
                strip_leading_markdown_emphasis(l),
                strip_leading_markdown_emphasis(strip_common_list_prefix(l)),
            ] {
                for c in [candidate, strip_leading_wrappers(candidate)] {
                    let c_lower = c.to_ascii_lowercase();
                    if ack_needles_lower.iter().any(|n| c_lower.starts_with(n)) {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}

/// Find discovery-style blocker messages for a path.
pub(crate) fn discovery_blockers_for_path(
    conn: &Connection,
    agent_id: &str,
    path: &str,
    now_ms: i64,
) -> anyhow::Result<Vec<DiscoveryBlocker>> {
    let window_ms = discovery_block_window_ms();
    if window_ms <= 0 {
        return Ok(Vec::new());
    }
    let since_ms = now_ms.saturating_sub(window_ms);
    let needles = relevant_needles_for_path(path);
    if needles.is_empty() {
        return Ok(Vec::new());
    }

    let mut stmt = conn.prepare(
        "SELECT id, created_at_ms, agent_id, kind, body_json FROM messages \
         WHERE agent_id <> ?1 AND created_at_ms >= ?2 AND kind IN ('message','discovery') \
         ORDER BY id DESC \
         LIMIT 200",
    )?;
    let rows = stmt.query_map(params![agent_id, since_ms], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    let mut blockers: Vec<DiscoveryBlocker> = Vec::new();
    for r in rows {
        let (_id, created_at_ms, from_agent, kind, body_json) = r?;
        let (body_v, txt) = parse_body(&body_json);
        if !needles.iter().any(|n| contains_path_token(&txt, n)) {
            continue;
        }
        if !is_discovery_block_text(&txt) {
            continue;
        }
        blockers.push(DiscoveryBlocker {
            agent_id: from_agent,
            created_at_ms,
            kind,
            body: body_v,
            text: txt,
        });
        if blockers.len() >= 10 {
            break;
        }
    }
    Ok(blockers)
}

/// Check whether the requester has already pinged a blocker about a specific path.
pub(crate) fn requester_already_pinged_blocker(
    conn: &Connection,
    requester_agent_id: &str,
    blocker_agent_id: &str,
    path: &str,
) -> anyhow::Result<bool> {
    let prefix = format!("@{blocker_agent_id}:");
    let mut stmt = conn.prepare(
        "SELECT body_json FROM messages \
         WHERE agent_id = ?1 AND kind = 'message' \
         ORDER BY id DESC \
         LIMIT 200",
    )?;
    let rows = stmt.query_map(params![requester_agent_id], |row| row.get::<_, String>(0))?;
    for r in rows {
        let body_json = r?;
        let v: Value = serde_json::from_str(&body_json).unwrap_or(Value::String(body_json));
        let text = v
            .get("text")
            .and_then(|t| t.as_str())
            .or_else(|| v.as_str())
            .unwrap_or("");
        if !text.contains(&prefix) || !crate::text::contains_path_token(text, path) {
            continue;
        }
        if text.contains("Are you working on it?") {
            return Ok(true);
        }
        let lower = text.to_ascii_lowercase();
        if lower.contains("blocked") || lower.contains("skipping") || lower.contains("skip ") {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Identify blocking agents whose state is stale.
pub(crate) fn stale_agents_for_blockers(
    conn: &Connection,
    requester_agent_id: &str,
    blockers: &[String],
    path: &str,
    now_ms: i64,
) -> anyhow::Result<Vec<StaleAgent>> {
    let thresh_ms = coord_stale_threshold_ms();
    if thresh_ms <= 0 || blockers.is_empty() {
        return Ok(Vec::new());
    }

    let mut out: Vec<StaleAgent> = Vec::new();
    for a in blockers {
        let updated_at_ms: Option<i64> = conn
            .query_row(
                "SELECT updated_at_ms FROM agent_state WHERE agent_id = ?1",
                params![a],
                |row| row.get(0),
            )
            .optional()?;
        let Some(updated_at_ms) = updated_at_ms else {
            continue;
        };

        let stale_for_ms = now_ms.saturating_sub(updated_at_ms);
        if stale_for_ms < thresh_ms {
            continue;
        }

        out.push(StaleAgent {
            kind: "stale_agent",
            agent_id: a.clone(),
            updated_at_ms,
            stale_for_ms,
            threshold_ms: thresh_ms,
            is_stale: true,
            suggested_cmd: format!(
                "but link --agent-id {requester_agent_id} post \"@{a}: can you update your status and plan for {path}? (stale)\""
            ),
        });
    }

    Ok(out)
}

/// Get unread relevant updates for a check operation, advancing the cursor.
pub(crate) fn unread_relevant_updates_for_check(
    conn: &Connection,
    agent_id: &str,
    path: &str,
    now_ms: i64,
) -> anyhow::Result<(Vec<UnreadUpdate>, i64, i64)> {
    let topic = format!("check_path:{path}");

    let prev_cursor: i64 = conn
        .query_row(
            "SELECT last_seen_msg_id FROM agent_cursors WHERE agent_id = ?1 AND topic = ?2",
            params![agent_id, topic],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0);

    let needles = relevant_needles_for_path(path);

    let mut stmt = conn.prepare(
        "SELECT id, created_at_ms, agent_id, kind, body_json FROM messages \
         WHERE id > ?1 AND agent_id <> ?2 AND kind IN ('message','discovery') \
         ORDER BY id ASC \
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

    let mut updates: Vec<UnreadUpdate> = Vec::new();
    let mut max_id = prev_cursor;
    let mut last_returned_relevant_id = prev_cursor;
    let mut reached_return_limit = false;
    for r in rows {
        let (id, created_at_ms, from_agent, kind, body_json) = r?;
        max_id = max_id.max(id);

        let body_v: Value =
            serde_json::from_str(&body_json).unwrap_or(Value::String(body_json.clone()));
        let txt = extract_message_text(&body_v, &body_json);
        if !needles.iter().any(|n| contains_path_token(&txt, n)) {
            continue;
        }
        if updates.len() >= 20 {
            reached_return_limit = true;
            continue;
        }

        last_returned_relevant_id = id;
        updates.push(UnreadUpdate {
            id,
            created_at_ms,
            agent_id: from_agent,
            kind,
            body: body_v,
        });
    }

    let new_cursor = if reached_return_limit {
        last_returned_relevant_id
    } else {
        max_id
    };
    if new_cursor != prev_cursor {
        conn.execute(
            "INSERT INTO agent_cursors(agent_id, topic, last_seen_msg_id, updated_at_ms) VALUES (?1, ?2, ?3, ?4) \
             ON CONFLICT(agent_id, topic) DO UPDATE SET last_seen_msg_id = excluded.last_seen_msg_id, updated_at_ms = excluded.updated_at_ms",
            params![agent_id, topic, new_cursor, now_ms],
        )?;
    }

    Ok((updates, prev_cursor, new_cursor))
}

/// Compute dependency hints based on intent/declaration surface overlap.
pub(crate) fn dependency_hints_for_check(
    conn: &Connection,
    agent_id: &str,
) -> anyhow::Result<Vec<DependencyHint>> {
    let intent_json: Option<String> = conn
        .query_row(
            "SELECT body_json FROM messages WHERE kind = 'intent' AND agent_id = ?1 ORDER BY id DESC LIMIT 1",
            params![agent_id],
            |row| row.get(0),
        )
        .optional()?;

    let Some(intent_json) = intent_json else {
        return Ok(Vec::new());
    };
    let intent_v: Value = serde_json::from_str(&intent_json)?;
    let intent_scope = value_string(&intent_v, "scope");
    let intent_surface: Vec<String> = value_string_array(&intent_v, "surface");

    if intent_surface.is_empty() {
        return Ok(Vec::new());
    }
    let intent_surface_set: HashSet<&str> = intent_surface.iter().map(String::as_str).collect();

    let mut stmt = conn.prepare(
        "SELECT agent_id, body_json FROM messages WHERE kind = 'declaration' AND agent_id <> ?1 ORDER BY id DESC",
    )?;
    let rows = stmt.query_map(params![agent_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut hints: Vec<DependencyHint> = Vec::new();
    let mut seen_provider_scope: HashSet<(String, String)> = HashSet::new();
    for r in rows {
        let (provider_agent_id, decl_json) = r?;
        let decl_v: Value = serde_json::from_str(&decl_json)?;
        let decl_obj = decl_v
            .as_object()
            .context("declaration must be an object")?;

        let scope = decl_obj
            .get("scope")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_owned();

        if let Some(ref intent_scope) = intent_scope
            && scope != *intent_scope
        {
            continue;
        }

        let tags: Vec<String> = value_string_array(&decl_v, "tags");

        if !tags.iter().any(|t| tag_has_api_segment(t)) {
            continue;
        }

        let decl_surface: Vec<String> = value_string_array(&decl_v, "surface");
        if decl_surface.is_empty() {
            continue;
        }

        let overlap: Vec<String> = decl_surface
            .iter()
            .filter(|tok| intent_surface_set.contains(tok.as_str()))
            .cloned()
            .collect();
        if overlap.is_empty() {
            continue;
        }

        if !seen_provider_scope.insert((provider_agent_id.clone(), scope.clone())) {
            continue;
        }

        let why = format!(
            "intent.surface intersects declaration.surface on token(s): {} (declaration tagged api). Coordinate before consuming/changing it.",
            overlap.join(", ")
        );
        let suggested_cmd = format!(
            "but link --agent-id {agent_id} post \"@{provider_agent_id}: I'm about to consume {scope} (overlap: {tok}). Are you changing the contract? Any migration notes?\"",
            tok = overlap
                .first()
                .cloned()
                .unwrap_or_else(|| "unknown".to_owned())
        );

        hints.push(DependencyHint {
            kind: "dependency_hint",
            provider_agent_id,
            scope,
            tags,
            overlap_tokens: overlap,
            why,
            next_step: DependencyNextStep {
                kind: "ask",
                suggested_cmd,
            },
        });
    }

    Ok(hints)
}

/// Load messages since a timestamp, optionally filtered by kind.
pub(crate) fn load_messages_since(
    conn: &Connection,
    kind: Option<&str>,
    since_ms: i64,
) -> anyhow::Result<Vec<Value>> {
    let mut messages: Vec<Value> = Vec::new();
    let mut push_message = |created_at_ms: i64, agent: String, kind: String, body_json: String| {
        let (body_v, content) = parse_body(&body_json);
        messages.push(json!({
            "created_at_ms": created_at_ms,
            "agent_id": agent,
            "kind": kind,
            "body": body_v,
            "content": content,
        }));
    };
    let kind = kind.filter(|k| !k.is_empty());

    if let Some(kind_filter) = kind
        && kind_filter != "all"
    {
        let mut stmt = conn.prepare(
            "SELECT created_at_ms, agent_id, kind, body_json FROM messages \
             WHERE created_at_ms >= ?1 AND kind = ?2 \
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![since_ms, kind_filter], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for r in rows {
            let (created_at_ms, agent, kind, body_json) = r?;
            push_message(created_at_ms, agent, kind, body_json);
        }
        return Ok(messages);
    }

    let mut stmt = conn.prepare(
        "SELECT created_at_ms, agent_id, kind, body_json FROM messages \
         WHERE created_at_ms >= ?1 \
         ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(params![since_ms], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    for r in rows {
        let (created_at_ms, agent, kind, body_json) = r?;
        push_message(created_at_ms, agent, kind, body_json);
    }
    Ok(messages)
}

/// Load discovery messages with optional high-signal filtering.
pub(crate) fn load_discoveries_and_next_steps(
    conn: &Connection,
    kind: &str,
    all: bool,
) -> anyhow::Result<(Vec<Value>, Vec<Value>)> {
    let mut discoveries: Vec<Value> = Vec::new();
    let mut next_steps: Vec<Value> = Vec::new();

    if kind == "discovery" || kind == "all" {
        let mut stmt = conn.prepare(
            "SELECT agent_id, body_json FROM messages WHERE kind = 'discovery' ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        for r in rows {
            let (agent, body_json) = r?;
            let (mut obj, _) = parse_body(&body_json);

            if let Value::Object(m) = &mut obj {
                m.insert("agent_id".to_owned(), Value::String(agent));
            }

            if !all && !is_high_signal_discovery(&obj) {
                continue;
            }

            if let Some(cmd) = obj
                .get("suggested_action")
                .and_then(|sa| sa.get("cmd"))
                .and_then(|c| c.as_str())
            {
                next_steps.push(json!({"kind":"run","cmd":cmd}));
            }

            discoveries.push(obj);
        }
    }

    Ok((discoveries, next_steps))
}

pub(crate) fn validate_discovery_payload(v: &Value) -> anyhow::Result<()> {
    let obj = v.as_object().context("discovery must be an object")?;

    anyhow::ensure!(
        obj.get("title")
            .and_then(|t| t.as_str())
            .is_some_and(|t| !t.trim().is_empty()),
        "title required"
    );
    anyhow::ensure!(
        obj.get("evidence")
            .and_then(|e| e.as_array())
            .is_some_and(|a| !a.is_empty()),
        "evidence required"
    );
    anyhow::ensure!(
        obj.get("suggested_action")
            .and_then(|sa| sa.get("cmd"))
            .and_then(|c| c.as_str())
            .is_some_and(|c| !c.trim().is_empty()),
        "suggested_action.cmd required"
    );
    Ok(())
}

pub(crate) fn validate_surface_payload(v: &Value) -> anyhow::Result<()> {
    let obj = v.as_object().context("payload must be an object")?;

    anyhow::ensure!(
        obj.get("scope")
            .and_then(|s| s.as_str())
            .is_some_and(|s| !s.trim().is_empty()),
        "scope required"
    );
    anyhow::ensure!(
        obj.get("tags")
            .and_then(|t| t.as_array())
            .is_some_and(|a| !a.is_empty() && a.iter().all(|v| v.as_str().is_some())),
        "tags required (non-empty string array)"
    );
    anyhow::ensure!(
        obj.get("surface")
            .and_then(|t| t.as_array())
            .is_some_and(|a| !a.is_empty() && a.iter().all(|v| v.as_str().is_some())),
        "surface required (non-empty string array)"
    );
    Ok(())
}

fn is_high_signal_discovery(v: &Value) -> bool {
    v.get("signal")
        .and_then(|s| s.as_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("high"))
}

fn tag_has_api_segment(tag: &str) -> bool {
    tag.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|seg| seg.eq_ignore_ascii_case("api"))
}

fn coord_stale_threshold_ms() -> i64 {
    let default_s: i64 = 15 * 60;
    let s = std::env::var("COORD_STALE_SECONDS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| *v >= 0)
        .unwrap_or(default_s);
    s.saturating_mul(1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Insert a message row used by unread-update cursor tests.
    fn insert_message(
        conn: &Connection,
        created_at_ms: i64,
        agent_id: &str,
        kind: &str,
        text: &str,
    ) -> anyhow::Result<()> {
        conn.execute(
            "INSERT INTO messages(created_at_ms, agent_id, kind, body_json) VALUES (?1, ?2, ?3, ?4)",
            params![created_at_ms, agent_id, kind, json!({ "text": text }).to_string()],
        )?;
        Ok(())
    }

    #[test]
    fn unread_cursor_stops_at_last_returned_relevant_update_when_capped() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory()?;
        init_db(&conn)?;

        for idx in 0..25 {
            insert_message(
                &conn,
                idx,
                "peer",
                "message",
                &format!("touching src/app.txt update {idx}"),
            )?;
        }

        let (first_batch, first_prev_cursor, first_new_cursor) =
            unread_relevant_updates_for_check(&conn, "me", "src/app.txt", 1_000)?;
        assert_eq!(first_prev_cursor, 0);
        assert_eq!(first_batch.len(), 20);
        assert_eq!(first_new_cursor, first_batch[19].id);

        let (second_batch, second_prev_cursor, second_new_cursor) =
            unread_relevant_updates_for_check(&conn, "me", "src/app.txt", 2_000)?;
        assert_eq!(second_prev_cursor, first_new_cursor);
        assert_eq!(second_batch.len(), 5);
        assert_eq!(second_new_cursor, 25);

        Ok(())
    }

    #[test]
    fn unread_cursor_advances_past_irrelevant_updates_when_not_capped() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory()?;
        init_db(&conn)?;

        insert_message(&conn, 1, "peer", "message", "touching src/app.txt update 1")?;
        insert_message(&conn, 2, "peer", "message", "unrelated note")?;
        insert_message(&conn, 3, "peer", "message", "still unrelated")?;

        let (updates, prev_cursor, new_cursor) =
            unread_relevant_updates_for_check(&conn, "me", "src/app.txt", 1_000)?;
        assert_eq!(prev_cursor, 0);
        assert_eq!(updates.len(), 1);
        assert_eq!(new_cursor, 3);

        let (next_updates, next_prev_cursor, next_new_cursor) =
            unread_relevant_updates_for_check(&conn, "me", "src/app.txt", 2_000)?;
        assert_eq!(next_prev_cursor, 3);
        assert!(next_updates.is_empty());
        assert_eq!(next_new_cursor, 3);

        Ok(())
    }
}

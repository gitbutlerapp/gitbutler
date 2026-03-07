//! Command dispatch and mutation handlers for `but link`.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use rusqlite::{Connection, params};
use serde::Serialize;
use serde_json::{Value, json};

use crate::claiming;
use crate::cli::{BlockMode, CheckFormat, Cmd, ReadView, cmd_name};
use crate::db;
use crate::payloads::{
    AckHistory, BlockHistory, DiscoveryEvidence, DiscoveryPayload, DiscoverySuggestedAction,
    ResolveHistory, SurfacePayload,
};
use crate::read;
use crate::repo;

/// Emit raw JSON text to stdout.
pub(crate) fn print_json(s: &str) {
    println!("{s}");
}

/// Serialize a value to stdout as JSON.
fn emit<T: Serialize>(value: &T) -> anyhow::Result<()> {
    print_json(&serde_json::to_string(value)?);
    Ok(())
}

/// Emit a standard success payload.
fn emit_ok() -> anyhow::Result<()> {
    emit(&json!({ "ok": true }))
}

/// Structured payload for `done`.
#[derive(Serialize)]
struct DoneResponse<'a> {
    /// Standard success marker.
    ok: bool,
    /// Number of released claims.
    released_claims: i64,
    /// Cleared agent-state fields.
    cleared: Vec<&'a str>,
}

/// Maximum command log size before truncation (1 MB).
const MAX_LOG_SIZE: u64 = 1_024 * 1_024;

/// Append a lightweight command log entry for local debugging.
fn append_command_log(path: &Path, agent_id: &str, cmd: &str) {
    if let Ok(meta) = std::fs::metadata(path)
        && meta.len() > MAX_LOG_SIZE
    {
        let _ = std::fs::write(path, b"");
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let entry = json!({
            "ts": db::now_unix_ms().unwrap_or(0),
            "agent_id": agent_id,
            "cmd": cmd,
        });
        let _ = writeln!(file, "{entry}");
    }
}

/// Entry point from the top-level `but` binary.
pub(crate) fn run(platform: crate::cli::Platform, current_dir: &Path) -> anyhow::Result<()> {
    let (agent_id, cmd) = platform.into_runtime()?;

    if matches!(cmd, Cmd::Tui) {
        return crate::tui::run(current_dir);
    }

    let git_dir = repo::discover_git_dir(current_dir)?;
    let repo_root = repo::discover_repo_root(current_dir)?;
    let cwd = current_dir
        .canonicalize()
        .unwrap_or_else(|_| current_dir.to_path_buf());
    let cmd = repo::normalize_command_paths(cmd, &cwd, &repo_root)?;
    let data_dir = git_dir.join("gitbutler");
    std::fs::create_dir_all(&data_dir)?;

    let db_path = data_dir.join("but-link.db");
    let log_path = data_dir.join("but-link.commands.log");

    let mut conn = Connection::open(db_path)?;
    db::init_db(&conn)?;
    db::touch_agent_seen(&conn, &agent_id)?;
    append_command_log(&log_path, &agent_id, cmd_name(&cmd));

    match cmd {
        Cmd::Claim { paths, ttl } => {
            claiming::claim_batch(&mut conn, &agent_id, &paths, ttl)?;
            emit_ok()
        }
        Cmd::Acquire {
            paths,
            ttl,
            strict,
            format,
        } => handle_acquire(&mut conn, &agent_id, &paths, ttl, strict, format),
        Cmd::Release { paths } => {
            claiming::release_batch(&mut conn, &agent_id, &paths)?;
            emit_ok()
        }
        Cmd::Claims { path_prefix } => emit(&json!({
            "ok": true,
            "claims": db::load_active_claims(&conn, path_prefix.as_deref())?,
        })),
        Cmd::Check {
            paths,
            strict,
            format,
        } => handle_check(&conn, &agent_id, &paths, strict, format),
        Cmd::Post { message } => handle_post(&conn, &agent_id, &message),
        Cmd::PostTyped { kind, json } => handle_post_typed(&conn, &agent_id, &kind, &json),
        Cmd::Read { view, since } => handle_read(&conn, &agent_id, view, since),
        Cmd::Brief { kind, all } => {
            let kind = kind.unwrap_or_else(|| "discovery".to_owned());
            emit(&read::brief_view(&conn, &kind, all)?)
        }
        Cmd::Digest { kind, all } => {
            let kind = kind.unwrap_or_else(|| "discovery".to_owned());
            emit(&read::digest_view(&conn, &kind, all)?)
        }
        Cmd::Status { value } => handle_status(&conn, &agent_id, value.as_deref()),
        Cmd::Plan { value } => handle_plan(&conn, &agent_id, value.as_deref()),
        Cmd::Agents => emit(&json!({
            "ok": true,
            "agents": db::load_agent_snapshots(&conn)?,
        })),
        Cmd::Tui => unreachable!("tui handled in read-only fast path"),
        Cmd::Done { summary } => handle_done(&conn, &agent_id, &summary),
        Cmd::Discovery {
            title,
            evidence,
            action,
            signal,
        } => handle_discovery(
            &conn,
            &agent_id,
            &title,
            &evidence,
            &action,
            signal.as_deref(),
        ),
        Cmd::Intent {
            scope,
            tags,
            surface,
            paths,
        } => handle_surface(&conn, &agent_id, "intent", &scope, &tags, &surface, &paths),
        Cmd::Declare {
            scope,
            tags,
            surface,
            paths,
        } => handle_surface(
            &conn,
            &agent_id,
            "declaration",
            &scope,
            &tags,
            &surface,
            &paths,
        ),
        Cmd::Block {
            paths,
            reason,
            mode,
            ttl,
        } => handle_block(&mut conn, &agent_id, &paths, &reason, mode, ttl),
        Cmd::Resolve { block_id } => handle_resolve(&conn, &agent_id, block_id),
        Cmd::Ack {
            target_agent_id,
            paths,
            note,
        } => handle_ack(
            &mut conn,
            &agent_id,
            &target_agent_id,
            &paths,
            note.as_deref(),
        ),
        Cmd::EvalUserPromptSubmit => handle_eval_user_prompt_submit(&conn),
    }
}

/// Handle `check`.
fn handle_check(
    conn: &Connection,
    agent_id: &str,
    paths: &[String],
    strict: bool,
    format: CheckFormat,
) -> anyhow::Result<()> {
    let results = claiming::check_results(conn, agent_id, paths, strict)?;
    if let Some(lines) = claiming::compact_lines_for_check(&results, format) {
        for line in lines {
            println!("{line}");
        }
        return Ok(());
    }
    if results.len() == 1 {
        emit(&results[0])
    } else {
        emit(&results)
    }
}

/// Handle `acquire`.
fn handle_acquire(
    conn: &mut Connection,
    agent_id: &str,
    paths: &[String],
    ttl: std::time::Duration,
    strict: bool,
    format: CheckFormat,
) -> anyhow::Result<()> {
    let response = claiming::acquire_batch(conn, agent_id, paths, ttl, strict)?;
    if let Some(lines) = claiming::compact_lines_for_acquire(&response, format) {
        for line in lines {
            println!("{line}");
        }
        return Ok(());
    }
    emit(&response)
}

/// Insert a free-text message.
fn handle_post(conn: &Connection, agent_id: &str, message: &str) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let body_json = json!({ "text": message }).to_string();
    db::insert_history_message(conn, now_ms, agent_id, "message", &body_json)?;
    db::touch_agent_progress_at(conn, agent_id, now_ms)?;
    emit_ok()
}

/// Insert typed discovery/intent/declaration messages.
fn handle_post_typed(
    conn: &Connection,
    agent_id: &str,
    kind: &str,
    body_json: &str,
) -> anyhow::Result<()> {
    let value: Value = serde_json::from_str(body_json)?;
    match kind {
        "discovery" => db::validate_discovery_payload(&value)?,
        "declaration" | "intent" => db::validate_surface_payload(&value)?,
        _ => anyhow::bail!("unsupported message kind: {kind}"),
    }
    let now_ms = db::now_unix_ms()?;
    db::insert_history_message(conn, now_ms, agent_id, kind, body_json)?;
    db::touch_agent_progress_at(conn, agent_id, now_ms)?;
    emit_ok()
}

/// Insert a discovery helper payload.
fn handle_discovery(
    conn: &Connection,
    agent_id: &str,
    title: &str,
    evidence: &[String],
    action: &str,
    signal: Option<&str>,
) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let payload = DiscoveryPayload {
        title: title.to_owned(),
        evidence: evidence
            .iter()
            .map(|detail| DiscoveryEvidence {
                detail: detail.clone(),
            })
            .collect(),
        suggested_action: DiscoverySuggestedAction {
            cmd: action.to_owned(),
        },
        signal: Some(signal.unwrap_or("high").to_owned()),
    };
    payload.validate()?;
    db::insert_history_message(
        conn,
        now_ms,
        agent_id,
        "discovery",
        &serde_json::to_string(&payload)?,
    )?;
    db::touch_agent_progress_at(conn, agent_id, now_ms)?;
    emit_ok()
}

/// Insert an intent or declaration payload.
fn handle_surface(
    conn: &Connection,
    agent_id: &str,
    kind: &str,
    scope: &str,
    tags: &[String],
    surface: &[String],
    paths: &[String],
) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let payload = SurfacePayload {
        scope: scope.to_owned(),
        tags: tags.to_vec(),
        surface: surface.to_vec(),
        paths: paths.to_vec(),
    };
    payload.validate()?;
    db::insert_history_message(
        conn,
        now_ms,
        agent_id,
        kind,
        &serde_json::to_string(&payload)?,
    )?;
    db::touch_agent_progress_at(conn, agent_id, now_ms)?;
    emit_ok()
}

/// Create a typed authoritative block.
fn handle_block(
    conn: &mut Connection,
    agent_id: &str,
    paths: &[String],
    reason: &str,
    mode: BlockMode,
    ttl: Option<std::time::Duration>,
) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let normalized = repo::normalized_unique_paths(paths);
    let expires_at_ms = ttl
        .map(|duration| i64::try_from(duration.as_millis()))
        .transpose()?
        .map(|millis| now_ms.saturating_add(millis));
    let mode_str = match mode {
        BlockMode::Advisory => "advisory",
        BlockMode::Hard => "hard",
    };

    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO blocks(agent_id, mode, reason, created_at_ms, expires_at_ms, resolved_at_ms, resolved_by_agent_id)
         VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL)",
        params![agent_id, mode_str, reason, now_ms, expires_at_ms],
    )?;
    let block_id = tx.last_insert_rowid();
    for path in &normalized {
        tx.execute(
            "INSERT INTO block_paths(block_id, path) VALUES (?1, ?2)",
            params![block_id, path],
        )?;
    }
    let body = BlockHistory {
        text: format!("block {block_id}: {reason}"),
        block_id,
        mode: mode_str.to_owned(),
        reason: reason.to_owned(),
        paths: normalized,
    };
    db::insert_history_message_tx(
        &tx,
        now_ms,
        agent_id,
        "block",
        &serde_json::to_string(&body)?,
    )?;
    db::touch_agent_progress_tx(&tx, agent_id, now_ms)?;
    tx.commit()?;
    emit(&json!({ "ok": true, "block_id": block_id }))
}

/// Resolve a typed authoritative block.
fn handle_resolve(conn: &Connection, agent_id: &str, block_id: i64) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let target_agent_id = db::load_block_owner(conn, block_id)?
        .ok_or_else(|| anyhow::anyhow!("block not found or already resolved: {block_id}"))?;
    let updated = conn.execute(
        "UPDATE blocks
         SET resolved_at_ms = ?2, resolved_by_agent_id = ?3
         WHERE id = ?1 AND resolved_at_ms IS NULL",
        params![block_id, now_ms, agent_id],
    )?;
    anyhow::ensure!(
        updated > 0,
        "block not found or already resolved: {block_id}"
    );

    let body = ResolveHistory {
        text: format!("resolved block {block_id}"),
        block_id,
        target_agent_id,
        resolved_by_agent_id: agent_id.to_owned(),
    };
    db::insert_history_message(
        conn,
        now_ms,
        agent_id,
        "resolve",
        &serde_json::to_string(&body)?,
    )?;
    db::touch_agent_progress_at(conn, agent_id, now_ms)?;
    emit(&json!({ "ok": true, "resolved_block_id": block_id }))
}

/// Record an authoritative acknowledgement.
fn handle_ack(
    conn: &mut Connection,
    agent_id: &str,
    target_agent_id: &str,
    paths: &[String],
    note: Option<&str>,
) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let normalized = repo::normalized_unique_paths(paths);
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO acknowledgements(agent_id, target_agent_id, note, created_at_ms)
         VALUES (?1, ?2, ?3, ?4)",
        params![agent_id, target_agent_id, note, now_ms],
    )?;
    let ack_id = tx.last_insert_rowid();
    for path in &normalized {
        tx.execute(
            "INSERT INTO ack_paths(ack_id, path) VALUES (?1, ?2)",
            params![ack_id, path],
        )?;
    }
    let body = AckHistory {
        text: note
            .map(|note| format!("@{target_agent_id}: ack: {note}"))
            .unwrap_or_else(|| format!("@{target_agent_id}: ack")),
        ack_id,
        target_agent_id: target_agent_id.to_owned(),
        paths: normalized,
        note: note.map(str::to_owned),
    };
    db::insert_history_message_tx(&tx, now_ms, agent_id, "ack", &serde_json::to_string(&body)?)?;
    db::touch_agent_progress_tx(&tx, agent_id, now_ms)?;
    tx.commit()?;
    emit(&json!({ "ok": true, "ack_id": ack_id }))
}

/// Handle `read`.
fn handle_read(
    conn: &Connection,
    agent_id: &str,
    view: ReadView,
    since: Option<i64>,
) -> anyhow::Result<()> {
    if let Some(since_ms) = since {
        let kind = match view {
            ReadView::Discoveries => Some("discovery"),
            _ => None,
        };
        return emit(&db::load_messages_since(conn, kind, since_ms)?);
    }

    match view {
        ReadView::Inbox => emit(&read::inbox_view(conn, agent_id)?),
        ReadView::Full => emit(&read::full_view(conn)?),
        ReadView::Discoveries => emit(&read::discoveries_view(conn)?),
        ReadView::Messages => emit(&read::messages_view(conn)?),
        ReadView::Claims => emit(&read::claims_view(conn)?),
        ReadView::Agents => emit(&read::agents_view(conn)?),
    }
}

/// Update the agent status.
fn handle_status(conn: &Connection, agent_id: &str, value: Option<&str>) -> anyhow::Result<()> {
    update_agent_state_field(conn, agent_id, AgentStateField::Status, value)
}

/// Update the agent plan.
fn handle_plan(conn: &Connection, agent_id: &str, value: Option<&str>) -> anyhow::Result<()> {
    update_agent_state_field(conn, agent_id, AgentStateField::Plan, value)
}

/// Agent-state field selector.
#[derive(Clone, Copy)]
enum AgentStateField {
    /// Agent status field.
    Status,
    /// Agent plan field.
    Plan,
}

/// Update one agent-state field.
fn update_agent_state_field(
    conn: &Connection,
    agent_id: &str,
    field: AgentStateField,
    value: Option<&str>,
) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    db::ensure_agent_row(conn, agent_id, now_ms)?;
    let sql = match field {
        AgentStateField::Status => {
            "UPDATE agent_state
             SET status = ?2, updated_at_ms = ?3, last_seen_at_ms = ?3, last_progress_at_ms = ?3
             WHERE agent_id = ?1"
        }
        AgentStateField::Plan => {
            "UPDATE agent_state
             SET plan = ?2, updated_at_ms = ?3, last_seen_at_ms = ?3, last_progress_at_ms = ?3
             WHERE agent_id = ?1"
        }
    };
    conn.execute(sql, params![agent_id, value, now_ms])?;
    emit_ok()
}

/// Finish work, release claims, and clear status/plan.
fn handle_done(conn: &Connection, agent_id: &str, summary: &str) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let released: i64 = conn.query_row(
        "SELECT COUNT(1) FROM claims WHERE agent_id = ?1",
        params![agent_id],
        |row| row.get(0),
    )?;
    conn.execute("DELETE FROM claims WHERE agent_id = ?1", params![agent_id])?;
    db::ensure_agent_row(conn, agent_id, now_ms)?;
    conn.execute(
        "UPDATE agent_state
         SET status = NULL,
             plan = NULL,
             updated_at_ms = ?2,
             last_seen_at_ms = ?2,
             last_progress_at_ms = ?2
         WHERE agent_id = ?1",
        params![agent_id, now_ms],
    )?;
    let body_json = json!({ "text": format!("DONE: {summary}") }).to_string();
    db::insert_history_message(conn, now_ms, agent_id, "message", &body_json)?;
    emit(&DoneResponse {
        ok: true,
        released_claims: released,
        cleared: vec!["status", "plan"],
    })
}

/// Hidden evaluation prompt output.
fn handle_eval_user_prompt_submit(conn: &Connection) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let active_claims: i64 = conn.query_row(
        "SELECT COUNT(1) FROM claims WHERE expires_at_ms > ?1",
        params![now_ms],
        |row| row.get(0),
    )?;
    println!("Read your inbox, acquire files, then proceed.");
    println!("claims: {active_claims}");
    Ok(())
}

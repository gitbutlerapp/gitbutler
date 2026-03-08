//! Command dispatch and mutation handlers for `but link`.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use rusqlite::Connection;
use serde::Serialize;
use serde_json::json;

use crate::cli::{CheckFormat, Cmd, ReadView, cmd_name};
use crate::db;
use crate::repo;
use crate::services;

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
        Cmd::Acquire {
            paths,
            ttl,
            strict,
            dry_run,
            format,
        } => handle_acquire(&mut conn, &agent_id, &paths, ttl, strict, dry_run, format),
        Cmd::Post { message } => {
            services::messages::post(&conn, &agent_id, &message)?;
            emit_ok()
        }
        Cmd::Read {
            view,
            format,
            since,
        } => handle_read(&conn, &agent_id, view, format, since),
        Cmd::Status { value } => {
            services::agents::set_status(&conn, &agent_id, value.as_deref())?;
            emit_ok()
        }
        Cmd::Plan { value } => {
            services::agents::set_plan(&conn, &agent_id, value.as_deref())?;
            emit_ok()
        }
        Cmd::Tui => unreachable!("tui handled in read-only fast path"),
        Cmd::Done { summary } => emit(&services::agents::done(&conn, &agent_id, &summary)?),
        Cmd::Discovery {
            title,
            evidence,
            action,
            signal,
        } => {
            services::messages::discovery(
                &mut conn,
                &agent_id,
                &title,
                &evidence,
                &action,
                signal.as_deref(),
            )?;
            emit_ok()
        }
        Cmd::Intent {
            scope,
            tags,
            surface,
            paths,
        } => {
            services::messages::surface(
                &mut conn, &agent_id, "intent", &scope, &tags, &surface, &paths,
            )?;
            emit_ok()
        }
        Cmd::Declare {
            scope,
            tags,
            surface,
            paths,
        } => {
            services::messages::surface(
                &mut conn,
                &agent_id,
                "declaration",
                &scope,
                &tags,
                &surface,
                &paths,
            )?;
            emit_ok()
        }
        Cmd::Block {
            paths,
            reason,
            mode,
            ttl,
        } => emit(&services::blocks::block(
            &mut conn, &agent_id, &paths, &reason, mode, ttl,
        )?),
        Cmd::Resolve { block_id } => emit(&services::blocks::resolve(&conn, &agent_id, block_id)?),
        Cmd::Ack {
            target_agent_id,
            paths,
            note,
        } => emit(&services::blocks::ack(
            &mut conn,
            &agent_id,
            &target_agent_id,
            &paths,
            note.as_deref(),
        )?),
    }
}

/// Handle `acquire`.
fn handle_acquire(
    conn: &mut Connection,
    agent_id: &str,
    paths: &[String],
    ttl: std::time::Duration,
    strict: bool,
    dry_run: bool,
    format: CheckFormat,
) -> anyhow::Result<()> {
    let response = services::acquire::acquire_batch(conn, agent_id, paths, ttl, strict, dry_run)?;
    if let Some(lines) = services::acquire::compact_lines_for_acquire(&response, format) {
        for line in lines {
            println!("{line}");
        }
        return Ok(());
    }
    emit(&response)
}

/// Handle `read`.
fn handle_read(
    conn: &Connection,
    agent_id: &str,
    view: ReadView,
    format: crate::cli::DiscoveryFormat,
    since: Option<i64>,
) -> anyhow::Result<()> {
    services::read::validate_read_args(view, format, since.is_some())?;

    if let Some(since_ms) = since {
        return emit(&services::read::read_since_json(conn, view, since_ms)?);
    }

    emit(&services::read::read_view_json(
        conn, agent_id, view, format,
    )?)
}

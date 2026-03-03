//! Command dispatch and handlers for all `but link` subcommands.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::{CheckFormat, Cmd, cmd_name, normalize_claim_path};
use crate::db;
use crate::text;

pub(crate) fn print_json(s: &str) {
    println!("{s}");
}

fn emit<T: Serialize>(v: &T) -> anyhow::Result<()> {
    print_json(&serde_json::to_string(v)?);
    Ok(())
}

fn emit_ok() -> anyhow::Result<()> {
    emit(&json!({ "ok": true }))
}

fn build_read_message(created_at_ms: i64, agent_id: String, kind: &str, body_json: &str) -> Value {
    let (mut obj, content) = text::parse_body(body_json);
    if let Value::Object(m) = &mut obj {
        m.insert("agent_id".to_owned(), Value::String(agent_id));
        m.insert("created_at_ms".to_owned(), Value::from(created_at_ms));
        m.insert("kind".to_owned(), Value::String(kind.to_owned()));
        m.insert("content".to_owned(), Value::String(content));
    }
    obj
}

fn suggested_run_step(obj: &Value) -> Option<Value> {
    obj.get("suggested_action")
        .and_then(|sa| sa.get("cmd"))
        .and_then(|cmd| cmd.as_str())
        .map(|cmd| json!({ "kind": "run", "cmd": cmd }))
}

/// Load messages for `but link read`, optionally filtered by message kind.
fn load_read_messages(conn: &Connection, kind: Option<&str>) -> anyhow::Result<Vec<Value>> {
    let mut messages = Vec::new();
    let kind = kind.filter(|kind| *kind != "all");

    if let Some(kind) = kind {
        let mut stmt = conn.prepare(
            "SELECT created_at_ms, agent_id, body_json FROM messages WHERE kind = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![kind], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        for row in rows {
            let (created_at_ms, agent_id, body_json) = row?;
            messages.push(build_read_message(
                created_at_ms,
                agent_id,
                kind,
                &body_json,
            ));
        }
        return Ok(messages);
    }

    let mut stmt = conn
        .prepare("SELECT created_at_ms, agent_id, kind, body_json FROM messages ORDER BY id ASC")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;

    for row in rows {
        let (created_at_ms, agent_id, kind, body_json) = row?;
        messages.push(build_read_message(
            created_at_ms,
            agent_id,
            kind.as_str(),
            &body_json,
        ));
    }

    Ok(messages)
}

/// Load currently active claims for the read snapshot.
fn load_read_claims(conn: &Connection) -> anyhow::Result<Vec<Value>> {
    let now_ms = db::now_unix_ms()?;
    let mut stmt = conn.prepare(
        "SELECT path, agent_id, expires_at_ms FROM claims \
         WHERE expires_at_ms > ?1 \
         ORDER BY expires_at_ms ASC, path ASC",
    )?;
    let rows = stmt.query_map(params![now_ms], |row| {
        Ok(json!({
            "path": row.get::<_, String>(0)?,
            "agent_id": row.get::<_, String>(1)?,
            "expires_at_ms": row.get::<_, i64>(2)?,
        }))
    })?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Load agent state rows for the read snapshot.
fn load_read_agents(conn: &Connection) -> anyhow::Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT agent_id, status, plan, updated_at_ms FROM agent_state ORDER BY agent_id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(json!({
            "agent_id": row.get::<_, String>(0)?,
            "status": row.get::<_, Option<String>>(1)?,
            "plan": row.get::<_, Option<String>>(2)?,
            "updated_at_ms": row.get::<_, i64>(3)?,
        }))
    })?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn map_claim_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(String, String, i64)> {
    Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, i64>(2)?,
    ))
}

#[derive(Serialize)]
struct ClaimEntry {
    path: String,
    agent_id: String,
    expires_at_ms: i64,
}

#[derive(Serialize)]
struct ClaimsResponse {
    ok: bool,
    claims: Vec<ClaimEntry>,
}

#[derive(Serialize)]
struct AgentState {
    agent_id: String,
    status: Option<String>,
    plan: Option<String>,
    updated_at_ms: i64,
}

#[derive(Serialize)]
struct AgentsResponse {
    ok: bool,
    agents: Vec<AgentState>,
}

#[derive(Serialize)]
struct DoneResponse<'a> {
    ok: bool,
    released_claims: i64,
    cleared: Vec<&'a str>,
}

#[derive(Serialize)]
struct BlockingClaim {
    agent_id: String,
    path: String,
    expires_at_ms: i64,
}

#[derive(Serialize)]
struct CommandStep {
    cmd: String,
}

#[derive(Serialize)]
struct ActionPlanHints {
    requires_coordination: bool,
    has_read_step: bool,
    has_post_step: bool,
    has_ack_step: bool,
    has_retry_check_step: bool,
    has_claim_step: bool,
}

#[derive(Serialize)]
struct CheckResponse<'a> {
    path: String,
    decision: &'a str,
    reason_code: &'a str,
    self_claim: Value,
    blocking_agents: Vec<String>,
    blocking_claims: Vec<BlockingClaim>,
    blocking_claim_paths_by_agent: BTreeMap<String, Vec<String>>,
    blocking_agents_state: Vec<AgentState>,
    action_plan: Vec<String>,
    next_steps: Vec<CommandStep>,
    action_plan_hints: ActionPlanHints,
    action_plan_by_agent: BTreeMap<String, Vec<String>>,
    discovery_blockers: Vec<text::DiscoveryBlocker>,
    dependency_hints: Vec<db::DependencyHint>,
    stale_agents: Vec<db::StaleAgent>,
    unread_relevant_updates_label: &'static str,
    unread_relevant_updates_prev_cursor: i64,
    unread_relevant_updates_cursor: i64,
    unread_relevant_updates: Vec<db::UnreadUpdate>,
}

/// Maximum command log size before truncation (1 MB).
const MAX_LOG_SIZE: u64 = 1_024 * 1_024;

fn append_command_log(path: &Path, agent_id: &str, cmd: &str) {
    // Truncate if the log exceeds MAX_LOG_SIZE to prevent unbounded growth.
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

fn normalized_unique_paths(paths: &[String]) -> Vec<String> {
    let mut seen = HashSet::with_capacity(paths.len());
    let mut out = Vec::with_capacity(paths.len());
    for raw in paths {
        let path = normalize_claim_path(raw)
            .expect("command paths must be normalized before reaching handlers");
        if seen.insert(path.clone()) {
            out.push(path);
        }
    }
    out
}

fn normalize_absolute_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    let mut saw_root = false;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => {
                normalized.push(component.as_os_str());
                saw_root = true;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized
                    .components()
                    .next_back()
                    .is_some_and(|c| matches!(c, Component::Normal(_)))
                {
                    normalized.pop();
                } else if !saw_root {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }

    normalized
}

fn canonicalize_existing_prefix(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut suffix = Vec::new();

    while !existing.exists() {
        let Some(name) = existing.file_name() else {
            break;
        };
        suffix.push(name.to_owned());
        let Some(parent) = existing.parent() else {
            break;
        };
        existing = parent;
    }

    let mut canonical = existing
        .canonicalize()
        .unwrap_or_else(|_| existing.to_path_buf());
    for component in suffix.iter().rev() {
        canonical.push(component);
    }
    canonical
}

fn resolve_repo_relative_path(
    raw: &str,
    current_dir: &Path,
    repo_root: &Path,
) -> anyhow::Result<String> {
    let candidate = if Path::new(raw).is_absolute() {
        canonicalize_existing_prefix(Path::new(raw))
    } else {
        current_dir.join(raw)
    };
    let normalized = normalize_absolute_path(&candidate);
    let relative = normalized
        .strip_prefix(repo_root)
        .map_err(|_| anyhow::anyhow!("path must stay within repository: {raw}"))?;
    let relative = relative.to_string_lossy().replace('\\', "/");
    let normalized = normalize_claim_path(&relative)?;
    anyhow::ensure!(
        !normalized.is_empty(),
        "path must not resolve to the repository root"
    );
    Ok(normalized)
}

fn resolve_repo_relative_paths(
    paths: Vec<String>,
    current_dir: &Path,
    repo_root: &Path,
) -> anyhow::Result<Vec<String>> {
    paths
        .into_iter()
        .map(|path| resolve_repo_relative_path(&path, current_dir, repo_root))
        .collect()
}

fn normalize_command_paths(cmd: Cmd, current_dir: &Path, repo_root: &Path) -> anyhow::Result<Cmd> {
    Ok(match cmd {
        Cmd::Claim { paths, ttl } => Cmd::Claim {
            paths: resolve_repo_relative_paths(paths, current_dir, repo_root)?,
            ttl,
        },
        Cmd::Release { paths } => Cmd::Release {
            paths: resolve_repo_relative_paths(paths, current_dir, repo_root)?,
        },
        Cmd::Claims { path_prefix } => Cmd::Claims {
            path_prefix: path_prefix
                .map(|path| resolve_repo_relative_path(&path, current_dir, repo_root))
                .transpose()?,
        },
        Cmd::Check {
            paths,
            strict,
            format,
        } => Cmd::Check {
            paths: resolve_repo_relative_paths(paths, current_dir, repo_root)?,
            strict,
            format,
        },
        other => other,
    })
}

/// Discover the `.git` directory by walking up from `start` to find the repository root.
/// Handles both normal repos (`.git` is a directory) and worktrees (`.git` is a file
/// containing `gitdir: <path>`).
pub(crate) fn discover_git_dir(start: &Path) -> anyhow::Result<PathBuf> {
    let mut current = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    loop {
        let dot_git = current.join(".git");
        if dot_git.is_dir() {
            return Ok(dot_git);
        }
        if dot_git.is_file() {
            // Worktree: .git is a file like "gitdir: /path/to/main/.git/worktrees/name"
            let content = std::fs::read_to_string(&dot_git)?;
            let gitdir = content
                .strip_prefix("gitdir:")
                .map(str::trim)
                .ok_or_else(|| {
                    anyhow::anyhow!("unexpected .git file format at {}", dot_git.display())
                })?;
            let gitdir_path = if Path::new(gitdir).is_absolute() {
                PathBuf::from(gitdir)
            } else {
                current.join(gitdir)
            };
            // In a worktree, gitdir points to e.g. `.git/worktrees/name`.
            // We want the main `.git` dir so all agents share the same DB.
            // Walk up from the gitdir to find the root `.git` directory.
            let resolved = gitdir_path.canonicalize().unwrap_or(gitdir_path);
            // If it's inside a `.git/worktrees/` directory, go up to `.git`.
            if let Some(parent) = resolved.parent()
                && parent.file_name().is_some_and(|n| n == "worktrees")
                && let Some(git_dir) = parent.parent()
                && git_dir.is_dir()
            {
                return Ok(git_dir.to_path_buf());
            }
            return Ok(resolved);
        }
        if !current.pop() {
            anyhow::bail!(
                "not a git repository (or any of the parent directories): {}",
                start.display()
            );
        }
    }
}

pub(crate) fn discover_repo_root(start: &Path) -> anyhow::Result<PathBuf> {
    let mut current = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    loop {
        let dot_git = current.join(".git");
        if dot_git.is_dir() || dot_git.is_file() {
            return Ok(current);
        }
        if !current.pop() {
            anyhow::bail!(
                "not a git repository (or any of the parent directories): {}",
                start.display()
            );
        }
    }
}

pub(crate) fn run(platform: crate::cli::Platform, current_dir: &Path) -> anyhow::Result<()> {
    let (agent_id, cmd) = platform.into_runtime()?;

    if matches!(cmd, Cmd::Tui) {
        return crate::tui::run(current_dir);
    }

    let git_dir = discover_git_dir(current_dir)?;
    let repo_root = discover_repo_root(current_dir)?;
    let cwd = current_dir
        .canonicalize()
        .unwrap_or_else(|_| current_dir.to_path_buf());
    let cmd = normalize_command_paths(cmd, &cwd, &repo_root)?;
    let data_dir = git_dir.join("gitbutler");
    std::fs::create_dir_all(&data_dir)?;

    let db_path = data_dir.join("but-link.db");
    let log_path = data_dir.join("but-link.commands.log");

    let mut conn = Connection::open(db_path)?;
    db::init_db(&conn)?;
    db::touch_agent(&conn, &agent_id)?;
    append_command_log(&log_path, &agent_id, cmd_name(&cmd));

    match cmd {
        Cmd::Claim { paths, ttl } => handle_claim_batch(&mut conn, &agent_id, &paths, ttl),
        Cmd::Release { paths } => handle_release_batch(&mut conn, &agent_id, &paths),
        Cmd::Claims { path_prefix } => handle_claims(&conn, path_prefix.as_deref()),
        Cmd::Check {
            paths,
            strict,
            format,
        } => handle_check_batch(&conn, &agent_id, &paths, strict, format),
        Cmd::Post { message } => handle_post(&conn, &agent_id, &message),
        Cmd::PostTyped { kind, json } => handle_post_typed(&conn, &agent_id, &kind, &json),
        Cmd::Read { kind, since } => handle_read(&conn, kind, since),
        Cmd::Brief { kind, all } => handle_brief(&conn, kind, all),
        Cmd::Digest { kind, all } => handle_digest(&conn, kind, all),
        Cmd::Status { value } => handle_status(&conn, &agent_id, value.as_deref()),
        Cmd::Plan { value } => handle_plan(&conn, &agent_id, value.as_deref()),
        Cmd::Agents => handle_agents(&conn),
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
        } => handle_surface(&conn, &agent_id, "intent", &scope, &tags, &surface),
        Cmd::Declare {
            scope,
            tags,
            surface,
        } => handle_surface(&conn, &agent_id, "declaration", &scope, &tags, &surface),
        Cmd::EvalUserPromptSubmit => handle_eval_user_prompt_submit(&conn),
    }
}

fn handle_claim_batch(
    conn: &mut Connection,
    agent_id: &str,
    paths: &[String],
    ttl: std::time::Duration,
) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let ttl_ms: i64 = ttl.as_millis().try_into()?;
    let expires_at_ms = now_ms.saturating_add(ttl_ms);
    let normalized = normalized_unique_paths(paths);
    let tx = conn.transaction()?;
    {
        let mut delete_stmt = tx.prepare("DELETE FROM claims WHERE path = ?1 AND agent_id = ?2")?;
        let mut insert_stmt =
            tx.prepare("INSERT INTO claims(path, agent_id, expires_at_ms) VALUES (?1, ?2, ?3)")?;
        for path in &normalized {
            // Treat repeated claims by the same agent as "renewal" rather than creating duplicates.
            delete_stmt.execute(params![path, agent_id])?;
            insert_stmt.execute(params![path, agent_id, expires_at_ms])?;
        }
    }
    // Post a structured log message so the TUI messages panel shows the claim.
    let body_json = json!({ "text": format!("claimed: {}", normalized.join(", ")) }).to_string();
    tx.execute(
        "INSERT INTO messages(created_at_ms, agent_id, kind, body_json) VALUES (?1, ?2, 'claim', ?3)",
        params![now_ms, agent_id, body_json],
    )?;
    tx.commit()?;
    emit_ok()?;
    Ok(())
}

fn handle_release_batch(
    conn: &mut Connection,
    agent_id: &str,
    paths: &[String],
) -> anyhow::Result<()> {
    let normalized = normalized_unique_paths(paths);
    let tx = conn.transaction()?;
    {
        let mut delete_stmt = tx.prepare("DELETE FROM claims WHERE path = ?1 AND agent_id = ?2")?;
        for path in &normalized {
            delete_stmt.execute(params![path, agent_id])?;
        }
    }
    // Post a structured log message so the TUI messages panel shows the release.
    let now_ms = db::now_unix_ms()?;
    let body_json = json!({ "text": format!("released: {}", normalized.join(", ")) }).to_string();
    tx.execute(
        "INSERT INTO messages(created_at_ms, agent_id, kind, body_json) VALUES (?1, ?2, 'release', ?3)",
        params![now_ms, agent_id, body_json],
    )?;
    tx.commit()?;
    emit_ok()?;
    Ok(())
}

fn handle_claims(conn: &Connection, path_prefix: Option<&str>) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let mut stmt = if path_prefix.is_some() {
        conn.prepare(
            "SELECT path, agent_id, expires_at_ms FROM claims \
             WHERE expires_at_ms > ?1 \
               AND (path = ?2 \
                    OR substr(?2, 1, length(path) + 1) = path || '/' \
                    OR substr(path, 1, length(?2) + 1) = ?2 || '/') \
             ORDER BY expires_at_ms ASC, path ASC",
        )?
    } else {
        conn.prepare(
            "SELECT path, agent_id, expires_at_ms FROM claims \
             WHERE expires_at_ms > ?1 \
             ORDER BY expires_at_ms ASC, path ASC",
        )?
    };

    let rows = if let Some(prefix) = path_prefix {
        stmt.query_map(params![now_ms, prefix], map_claim_row)?
    } else {
        stmt.query_map(params![now_ms], map_claim_row)?
    };

    let claims: Vec<ClaimEntry> = rows
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .map(|(path, agent_id, expires_at_ms)| ClaimEntry {
            path,
            agent_id,
            expires_at_ms,
        })
        .collect();

    emit(&ClaimsResponse { ok: true, claims })?;
    Ok(())
}

fn handle_check_batch(
    conn: &Connection,
    agent_id: &str,
    paths: &[String],
    strict: bool,
    format: CheckFormat,
) -> anyhow::Result<()> {
    match format {
        CheckFormat::Full if paths.len() == 1 => handle_check(conn, agent_id, &paths[0], strict),
        CheckFormat::Full => {
            let results: Vec<CheckResponse<'static>> = paths
                .iter()
                .map(|path| check_result(conn, agent_id, path, strict))
                .collect::<anyhow::Result<_>>()?;
            emit(&results)?;
            Ok(())
        }
        CheckFormat::Compact => {
            for path in paths {
                let result = check_result(conn, agent_id, path, strict)?;
                let blockers = if result.blocking_agents.is_empty() {
                    String::new()
                } else {
                    format!(" {}", result.blocking_agents.join(","))
                };
                println!(
                    "{} {} {}{}",
                    result.decision, result.path, result.reason_code, blockers
                );
            }
            Ok(())
        }
    }
}

fn handle_check(conn: &Connection, agent_id: &str, path: &str, strict: bool) -> anyhow::Result<()> {
    let result = check_result(conn, agent_id, path, strict)?;
    emit(&result)?;
    Ok(())
}

fn check_result(
    conn: &Connection,
    agent_id: &str,
    path: &str,
    strict: bool,
) -> anyhow::Result<CheckResponse<'static>> {
    let path = normalize_claim_path(path)?;
    let path_needles = text::relevant_needles_for_path(&path);
    let now_ms = db::now_unix_ms()?;

    // --- Self-claim status ---
    let self_claim_active: Option<(String, i64)> = conn
        .query_row(
            "SELECT path, expires_at_ms FROM claims \
             WHERE agent_id = ?1 AND expires_at_ms > ?2 \
               AND (path = ?3 \
                    OR substr(?3, 1, length(path) + 1) = path || '/' \
                    OR substr(path, 1, length(?3) + 1) = ?3 || '/') \
             ORDER BY LENGTH(path) DESC, expires_at_ms DESC, path ASC \
             LIMIT 1",
            params![agent_id, now_ms, path],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    let self_claim_stale: Option<(String, i64)> = if self_claim_active.is_none() {
        conn.query_row(
            "SELECT path, expires_at_ms FROM claims \
             WHERE agent_id = ?1 AND expires_at_ms <= ?2 \
               AND (path = ?3 \
                    OR substr(?3, 1, length(path) + 1) = path || '/' \
                    OR substr(path, 1, length(?3) + 1) = ?3 || '/') \
             ORDER BY expires_at_ms DESC, LENGTH(path) DESC, path ASC \
             LIMIT 1",
            params![agent_id, now_ms, path],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
    } else {
        None
    };
    let self_claim = if let Some((p, exp)) = &self_claim_active {
        json!({ "status": "active", "path": p, "expires_at_ms": exp })
    } else if let Some((p, exp)) = &self_claim_stale {
        json!({
            "status": "stale", "path": p, "expires_at_ms": exp,
            "stale_for_ms": now_ms.saturating_sub(*exp),
        })
    } else {
        json!({ "status": "none" })
    };

    // --- Blocking claims from other agents ---
    let mut stmt = conn.prepare(
        "SELECT agent_id, path, expires_at_ms FROM claims \
         WHERE agent_id <> ?2 AND expires_at_ms > ?3 \
           AND (path = ?1 \
                OR substr(?1, 1, length(path) + 1) = path || '/' \
                OR substr(path, 1, length(?1) + 1) = ?1 || '/') \
         ORDER BY expires_at_ms DESC \
         LIMIT 20",
    )?;
    let blocking_claims_raw: Vec<(String, String, i64)> = stmt
        .query_map(params![path, agent_id, now_ms], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut seen = HashSet::<String>::new();
    let mut blocking_agents = Vec::<String>::new();
    let mut best_overlap_claim_by_agent =
        std::collections::BTreeMap::<String, (String, i64)>::new();
    for (blocker, claim_path, expires_at_ms) in blocking_claims_raw {
        if seen.insert(blocker.clone()) {
            blocking_agents.push(blocker.clone());
        }
        match best_overlap_claim_by_agent.get_mut(&blocker) {
            None => {
                best_overlap_claim_by_agent.insert(blocker, (claim_path, expires_at_ms));
            }
            Some((best_path, best_exp)) => {
                let better = claim_path.len() > best_path.len()
                    || (claim_path.len() == best_path.len()
                        && (expires_at_ms > *best_exp
                            || (expires_at_ms == *best_exp && claim_path < *best_path)));
                if better {
                    *best_path = claim_path;
                    *best_exp = expires_at_ms;
                }
            }
        }
    }
    let has_claim_blockers = !blocking_agents.is_empty();

    // --- Discovery blockers ---
    let discovery_blockers_all = db::discovery_blockers_for_path(conn, agent_id, &path, now_ms)?;
    let mut discovery_blocker_agents: HashSet<String> = HashSet::new();
    for b in &discovery_blockers_all {
        if discovery_blocker_agents.insert(b.agent_id.clone()) {
            if seen.insert(b.agent_id.clone()) {
                blocking_agents.push(b.agent_id.clone());
            }
            best_overlap_claim_by_agent
                .entry(b.agent_id.clone())
                .or_insert_with(|| (path.clone(), b.created_at_ms));
        }
    }

    // Cap blocking agents list.
    if blocking_agents.len() > 5 {
        blocking_agents.truncate(5);
    }
    let kept_blockers: HashSet<String> = blocking_agents.iter().cloned().collect();
    best_overlap_claim_by_agent.retain(|k, _| kept_blockers.contains(k));
    let discovery_blockers: Vec<text::DiscoveryBlocker> = discovery_blockers_all
        .into_iter()
        .filter(|b| kept_blockers.contains(&b.agent_id))
        .collect();
    discovery_blocker_agents.retain(|a| kept_blockers.contains(a));

    // --- Blocking claims detail ---
    let mut blocking_claims: Vec<BlockingClaim> = Vec::new();
    let mut blocking_claim_paths_by_agent = BTreeMap::<String, Vec<String>>::new();
    if !blocking_agents.is_empty() {
        let mut stmt_blocking = conn.prepare(
            "SELECT path, expires_at_ms FROM claims \
             WHERE agent_id = ?1 AND expires_at_ms > ?2 \
               AND (path = ?3 \
                    OR substr(?3, 1, length(path) + 1) = path || '/' \
                    OR substr(path, 1, length(?3) + 1) = ?3 || '/') \
             ORDER BY LENGTH(path) DESC, expires_at_ms DESC, path ASC \
             LIMIT 20",
        )?;
        for blocker in &blocking_agents {
            let mut paths_for_blocker: Vec<String> = Vec::new();
            let rows = stmt_blocking.query_map(params![blocker, now_ms, path], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            for r in rows {
                let (p, expires_at_ms) = r?;
                paths_for_blocker.push(p.clone());
                blocking_claims.push(BlockingClaim {
                    agent_id: blocker.clone(),
                    path: p,
                    expires_at_ms,
                });
            }
            if !paths_for_blocker.is_empty() {
                blocking_claim_paths_by_agent.insert(blocker.to_owned(), paths_for_blocker);
            }
        }
    }

    // --- Decision ---
    let (decision, reason_code) = if has_claim_blockers {
        if strict {
            ("deny", "claimed_by_other")
        } else {
            ("warn", "claimed_by_other")
        }
    } else if !discovery_blocker_agents.is_empty() {
        if strict {
            ("deny", "discovery_block")
        } else {
            ("warn", "discovery_block")
        }
    } else {
        ("allow", "no_conflict")
    };

    // --- Unread relevant updates ---
    let (unread_relevant_updates, unread_prev_cursor, unread_cursor) =
        db::unread_relevant_updates_for_check(conn, agent_id, &path, now_ms)?;

    // --- Ack/closure analysis ---
    let ack_to_me_prefix = format!("@{agent_id}: ack:");
    let closure_to_me_prefixes_lower = [
        format!("@{agent_id}: resolve:").to_ascii_lowercase(),
        format!("@{agent_id}: resolved:").to_ascii_lowercase(),
        format!("@{agent_id}: released:").to_ascii_lowercase(),
    ];

    let blocking_set: HashSet<&str> = blocking_agents.iter().map(String::as_str).collect();
    let blockers_with_unread_update: HashSet<String> = unread_relevant_updates
        .iter()
        .map(|u| u.agent_id.clone())
        .filter(|a| blocking_set.contains(a.as_str()))
        .collect();

    let mut blockers_with_any_relevant_update: HashSet<String> = HashSet::new();
    let mut pending_ack_blockers: HashSet<String> = HashSet::new();
    for blocker in &blocking_agents {
        if let Some((_id, created_at_ms, txt)) =
            db::last_relevant_update_from_agent(conn, blocker, &path)?
        {
            blockers_with_any_relevant_update.insert(blocker.to_owned());
            if text::is_explicit_closure_to_me(
                &txt,
                &ack_to_me_prefix,
                [
                    closure_to_me_prefixes_lower[0].as_str(),
                    closure_to_me_prefixes_lower[1].as_str(),
                    closure_to_me_prefixes_lower[2].as_str(),
                ],
            ) {
                continue;
            }
            if !db::requester_has_acked_since(
                conn,
                agent_id,
                blocker,
                created_at_ms,
                &path_needles,
            )? {
                pending_ack_blockers.insert(blocker.to_owned());
            }
        }
    }

    // --- Action plan ---
    let mut pinged_blockers: HashSet<String> = HashSet::new();
    let mut action_plan: Vec<String> = if blocking_agents.is_empty() {
        Vec::new()
    } else {
        let mut plan = vec![format!("but link --agent-id {agent_id} read")];
        for blocker in &blocking_agents {
            let is_discovery_blocker = discovery_blocker_agents.contains(blocker);
            if !is_discovery_blocker
                && (blockers_with_any_relevant_update.contains(blocker)
                    || blockers_with_unread_update.contains(blocker))
            {
                continue;
            }
            if db::requester_already_pinged_blocker(conn, agent_id, blocker, &path)? {
                continue;
            }
            let overlap_claim_path = best_overlap_claim_by_agent
                .get(blocker)
                .map_or(path.as_str(), |(p, _)| p.as_str());
            let message = if is_discovery_blocker {
                format!(
                    "but link --agent-id {agent_id} post \"@{blocker}: ack: saw your update re {path}. Skipping {path} for now; please reply with ETA or mark it safe when refactor wraps.\""
                )
            } else {
                format!(
                    "but link --agent-id {agent_id} post \"@{blocker}: blocked on {path} (overlaps your claim {overlap_claim_path}). Are you working on it? Skipping {path} for now; please reply with ETA or release if you're not working on it.\""
                )
            };
            plan.push(message);
            pinged_blockers.insert(blocker.to_owned());
            if is_discovery_blocker {
                pending_ack_blockers.remove(blocker);
            }
        }
        plan.push(format!(
            "but link --agent-id {agent_id} check --path {path}{}",
            if strict { " --strict" } else { "" }
        ));
        plan
    };

    // Suggest renewal if self-claim is stale.
    if blocking_agents.is_empty()
        && let Some((claim_path, _exp)) = &self_claim_stale
    {
        action_plan.push(format!(
            "but link --agent-id {agent_id} claim --path {claim_path} --ttl 15m"
        ));
        action_plan.push(format!(
            "but link --agent-id {agent_id} check --path {path}{}",
            if strict { " --strict" } else { "" }
        ));
    }

    // --- Per-agent action plans ---
    let mut action_plan_by_agent = BTreeMap::<String, Vec<String>>::new();
    let mut stmt_agents = conn.prepare("SELECT agent_id FROM agent_state ORDER BY agent_id ASC")?;
    let agents: Vec<String> = stmt_agents
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;

    for a in agents {
        if a == agent_id {
            continue;
        }

        if let Some((claim_path, _)) = best_overlap_claim_by_agent.get(&a) {
            let plan = if discovery_blocker_agents.contains(&a) {
                vec![
                    format!("but link --agent-id {a} read"),
                    format!(
                        "but link --agent-id {a} post \"@{agent_id}: still wrapping work on {path}. Will shout when it's safe.\""
                    ),
                ]
            } else {
                vec![
                    format!("but link --agent-id {a} read"),
                    format!(
                        "but link --agent-id {a} post \"@{agent_id}: I'm holding a claim on {claim_path} (overlaps {path}). ETA update soon.\""
                    ),
                    format!("but link --agent-id {a} release --path {claim_path}"),
                ]
            };
            action_plan_by_agent.insert(a.clone(), plan);
            continue;
        }

        let active_claim_path: Option<String> = conn
            .query_row(
                "SELECT path FROM claims WHERE agent_id = ?1 AND expires_at_ms > ?2 ORDER BY expires_at_ms DESC LIMIT 1",
                params![a, now_ms],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(p) = active_claim_path {
            action_plan_by_agent.insert(
                a.clone(),
                vec![format!(
                    "but link --agent-id {a} post \"@{agent_id}: FYI I am working on {p}; not touching {path}.\""
                )],
            );
        }
    }

    // --- Ack suggestions for unread updates ---
    if !unread_relevant_updates.is_empty() || !pending_ack_blockers.is_empty() {
        let pinged_set: HashSet<&str> = pinged_blockers.iter().map(String::as_str).collect();
        let mut ack_agents: BTreeSet<String> = BTreeSet::new();
        for u in &unread_relevant_updates {
            let a = u.agent_id.as_str();
            if a == agent_id || pinged_set.contains(a) {
                continue;
            }
            let body_text = match &u.body {
                Value::Object(m) => m.get("text").and_then(|v| v.as_str()),
                Value::String(s) => Some(s.as_str()),
                _ => None,
            };
            if body_text.is_some_and(|t| {
                text::is_explicit_closure_to_me(
                    t,
                    &ack_to_me_prefix,
                    [
                        closure_to_me_prefixes_lower[0].as_str(),
                        closure_to_me_prefixes_lower[1].as_str(),
                        closure_to_me_prefixes_lower[2].as_str(),
                    ],
                )
            }) {
                continue;
            }
            ack_agents.insert(a.to_owned());
        }

        for a in &pending_ack_blockers {
            if a != agent_id && !pinged_set.contains(a.as_str()) {
                ack_agents.insert(a.to_owned());
            }
        }

        for a in ack_agents {
            let cmd = format!(
                "but link --agent-id {agent_id} post \"@{a}: ack: saw your update re {path}.\""
            );
            if action_plan
                .last()
                .is_some_and(|s| s.contains(" check --path "))
            {
                let idx = action_plan.len().saturating_sub(1);
                action_plan.insert(idx, cmd);
            } else {
                action_plan.push(cmd);
            }
        }
    }

    action_plan_by_agent.insert(agent_id.to_owned(), action_plan.clone());

    // --- Supplemental data ---
    let dependency_hints = db::dependency_hints_for_check(conn, agent_id)?;
    let stale_agents =
        db::stale_agents_for_blockers(conn, agent_id, &blocking_agents, &path, now_ms)?;

    let mut blocking_agents_state: Vec<AgentState> = Vec::new();
    if !blocking_agents.is_empty() {
        let mut stmt = conn
            .prepare("SELECT status, plan, updated_at_ms FROM agent_state WHERE agent_id = ?1")?;
        for blocker in &blocking_agents {
            let row: Option<(Option<String>, Option<String>, i64)> = stmt
                .query_row(params![blocker], |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })
                .optional()?;
            let (status, plan, updated_at_ms) = row.unwrap_or((None, None, 0));
            blocking_agents_state.push(AgentState {
                agent_id: blocker.clone(),
                status,
                plan,
                updated_at_ms,
            });
        }
    }

    let next_steps: Vec<CommandStep> = action_plan
        .iter()
        .map(|cmd| CommandStep { cmd: cmd.clone() })
        .collect();
    let action_plan_hints = ActionPlanHints {
        requires_coordination: decision != "allow",
        has_read_step: action_plan.iter().any(|cmd| cmd.contains(" read")),
        has_post_step: action_plan.iter().any(|cmd| cmd.contains(" post ")),
        has_ack_step: action_plan.iter().any(|cmd| cmd.contains(": ack:")),
        has_retry_check_step: action_plan.iter().any(|cmd| cmd.contains(" check --path ")),
        has_claim_step: action_plan.iter().any(|cmd| cmd.contains(" claim --path ")),
    };

    Ok(CheckResponse {
        path,
        decision,
        reason_code,
        self_claim,
        blocking_agents,
        blocking_claims,
        blocking_claim_paths_by_agent,
        blocking_agents_state,
        action_plan,
        next_steps,
        action_plan_hints,
        action_plan_by_agent,
        discovery_blockers,
        dependency_hints,
        stale_agents,
        unread_relevant_updates_label: "unread relevant updates since last seen",
        unread_relevant_updates_prev_cursor: unread_prev_cursor,
        unread_relevant_updates_cursor: unread_cursor,
        unread_relevant_updates,
    })
}

fn handle_post(conn: &Connection, agent_id: &str, message: &str) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let body_json = json!({ "text": message }).to_string();
    conn.execute(
        "INSERT INTO messages(created_at_ms, agent_id, kind, body_json) VALUES (?1, ?2, 'message', ?3)",
        params![now_ms, agent_id, body_json],
    )?;
    emit_ok()?;
    Ok(())
}

fn handle_post_typed(
    conn: &Connection,
    agent_id: &str,
    kind: &str,
    body_json: &str,
) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let v: Value = serde_json::from_str(body_json)?;
    match kind {
        "discovery" => db::validate_discovery_payload(&v)?,
        "declaration" | "intent" => db::validate_surface_payload(&v)?,
        _ => anyhow::bail!("unsupported message kind: {kind}"),
    }
    conn.execute(
        "INSERT INTO messages(created_at_ms, agent_id, kind, body_json) VALUES (?1, ?2, ?3, ?4)",
        params![now_ms, agent_id, kind, body_json],
    )?;
    emit_ok()?;
    Ok(())
}

fn handle_discovery(
    conn: &Connection,
    agent_id: &str,
    title: &str,
    evidence: &[String],
    action: &str,
    signal: Option<&str>,
) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let evidence_array: Vec<Value> = evidence.iter().map(|e| json!({ "detail": e })).collect();
    let signal = signal.unwrap_or("high");
    let payload = json!({
        "title": title,
        "evidence": evidence_array,
        "suggested_action": { "cmd": action },
        "signal": signal,
    });
    db::validate_discovery_payload(&payload)?;
    let body_json = payload.to_string();
    conn.execute(
        "INSERT INTO messages(created_at_ms, agent_id, kind, body_json) VALUES (?1, ?2, 'discovery', ?3)",
        params![now_ms, agent_id, body_json],
    )?;
    emit_ok()?;
    Ok(())
}

fn handle_surface(
    conn: &Connection,
    agent_id: &str,
    kind: &str,
    scope: &str,
    tags: &[String],
    surface: &[String],
) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let payload = json!({
        "scope": scope,
        "tags": tags,
        "surface": surface,
    });
    db::validate_surface_payload(&payload)?;
    let body_json = payload.to_string();
    conn.execute(
        "INSERT INTO messages(created_at_ms, agent_id, kind, body_json) VALUES (?1, ?2, ?3, ?4)",
        params![now_ms, agent_id, kind, body_json],
    )?;
    emit_ok()?;
    Ok(())
}

fn handle_read(conn: &Connection, kind: Option<String>, since: Option<i64>) -> anyhow::Result<()> {
    if let Some(since_ms) = since {
        let messages = db::load_messages_since(conn, kind.as_deref(), since_ms)?;
        emit(&messages)?;
        return Ok(());
    }

    let kind = kind.unwrap_or_else(|| "all".to_owned());
    if kind == "all" {
        let messages = load_read_messages(conn, None)?;
        let discoveries: Vec<Value> = messages
            .iter()
            .filter(|message| message["kind"] == "discovery")
            .cloned()
            .collect();
        let next_steps: Vec<Value> = discoveries.iter().filter_map(suggested_run_step).collect();
        let claims = load_read_claims(conn)?;
        let agents = load_read_agents(conn)?;

        emit(&json!({
            "ok": true,
            "kind": kind,
            "messages": messages,
            "discoveries": discoveries,
            "next_steps": next_steps,
            "claims": claims,
            "agents": agents,
        }))?;
    } else if kind == "discovery" {
        let mut discoveries: Vec<Value> = Vec::new();
        let mut next_steps: Vec<Value> = Vec::new();

        let mut stmt = conn.prepare(
            "SELECT created_at_ms, agent_id, body_json FROM messages WHERE kind = 'discovery' ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        for r in rows {
            let (created_at_ms, agent, body_json) = r?;
            let obj = build_read_message(created_at_ms, agent, "discovery", &body_json);
            if let Some(step) = suggested_run_step(&obj) {
                next_steps.push(step);
            }
            discoveries.push(obj);
        }

        emit(&json!({
            "ok": true,
            "kind": kind,
            "discoveries": discoveries,
            "next_steps": next_steps,
        }))?;
    } else if matches!(
        kind.as_str(),
        "claim" | "release" | "declaration" | "intent" | "message" | "block"
    ) {
        let messages = load_read_messages(conn, Some(kind.as_str()))?;

        emit(&json!({ "ok": true, "kind": kind, "messages": messages }))?;
    } else {
        emit(&json!({ "ok": true, "kind": kind, "messages": [] }))?;
    }
    Ok(())
}

fn handle_brief(conn: &Connection, kind: Option<String>, all: bool) -> anyhow::Result<()> {
    let kind = kind.unwrap_or_else(|| "discovery".to_owned());
    let (discoveries, next_steps) = db::load_discoveries_and_next_steps(conn, &kind, all)?;

    emit(&json!({
        "ok": true,
        "mode": "brief",
        "kind": kind,
        "discoveries": discoveries,
        "next_steps": next_steps,
    }))?;
    Ok(())
}

fn handle_digest(conn: &Connection, kind: Option<String>, all: bool) -> anyhow::Result<()> {
    let kind = kind.unwrap_or_else(|| "discovery".to_owned());
    let (discoveries, next_steps) = db::load_discoveries_and_next_steps(conn, &kind, all)?;

    let discoveries: Vec<Value> = discoveries
        .into_iter()
        .map(|d| match d {
            Value::Object(mut m) => {
                let title = m.remove("title");
                let agent_id = m.remove("agent_id");
                json!({ "title": title, "agent_id": agent_id })
            }
            other => other,
        })
        .collect();

    emit(&json!({
        "ok": true,
        "mode": "digest",
        "kind": kind,
        "discoveries": discoveries,
        "next_steps": next_steps,
    }))?;
    Ok(())
}

fn handle_status(conn: &Connection, agent_id: &str, value: Option<&str>) -> anyhow::Result<()> {
    update_agent_state_field(conn, agent_id, AgentStateField::Status, value)
}

fn handle_plan(conn: &Connection, agent_id: &str, value: Option<&str>) -> anyhow::Result<()> {
    update_agent_state_field(conn, agent_id, AgentStateField::Plan, value)
}

#[derive(Clone, Copy)]
enum AgentStateField {
    Status,
    Plan,
}

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
            "UPDATE agent_state SET status = ?2, updated_at_ms = ?3 WHERE agent_id = ?1"
        }
        AgentStateField::Plan => {
            "UPDATE agent_state SET plan = ?2, updated_at_ms = ?3 WHERE agent_id = ?1"
        }
    };
    conn.execute(sql, params![agent_id, value, now_ms])?;
    emit_ok()?;
    Ok(())
}

fn handle_agents(conn: &Connection) -> anyhow::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT agent_id, status, plan, updated_at_ms FROM agent_state ORDER BY agent_id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;

    let agents: Vec<AgentState> = rows
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .map(|(agent_id, status, plan, updated_at_ms)| AgentState {
            agent_id,
            status,
            plan,
            updated_at_ms,
        })
        .collect();

    emit(&AgentsResponse { ok: true, agents })?;
    Ok(())
}

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
        "UPDATE agent_state SET status = NULL, plan = NULL, updated_at_ms = ?2 WHERE agent_id = ?1",
        params![agent_id, now_ms],
    )?;

    let body_json = json!({ "text": format!("DONE: {summary}") }).to_string();
    conn.execute(
        "INSERT INTO messages(created_at_ms, agent_id, kind, body_json) VALUES (?1, ?2, 'message', ?3)",
        params![now_ms, agent_id, body_json],
    )?;

    emit(&DoneResponse {
        ok: true,
        released_claims: released,
        cleared: vec!["status", "plan"],
    })?;
    Ok(())
}

fn handle_eval_user_prompt_submit(conn: &Connection) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let active_claims: i64 = conn.query_row(
        "SELECT COUNT(1) FROM claims WHERE expires_at_ms > ?1",
        params![now_ms],
        |row| row.get(0),
    )?;

    println!("Announce what you'll do (files), read the channel, then proceed.");
    println!("claims: {active_claims}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_test_dir(name: &str) -> anyhow::Result<PathBuf> {
        let unique = format!(
            "but-link-{}-{}-{}",
            name,
            std::process::id(),
            db::now_unix_ms()?
        );
        let path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }

    #[test]
    fn discovery_subcommand_defaults_signal_to_high() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory()?;
        db::init_db(&conn)?;

        handle_discovery(
            &conn,
            "agent-a",
            "coordination default",
            &["observed regression".to_owned()],
            "but link read --agent-id agent-a",
            None,
        )?;

        let (discoveries, next_steps) =
            db::load_discoveries_and_next_steps(&conn, "discovery", false)?;

        assert_eq!(discoveries.len(), 1);
        assert_eq!(discoveries[0]["title"], "coordination default");
        assert_eq!(discoveries[0]["signal"], "high");
        assert_eq!(next_steps.len(), 1);
        assert_eq!(next_steps[0]["cmd"], "but link read --agent-id agent-a");

        Ok(())
    }

    #[test]
    fn resolve_repo_relative_path_maps_subdir_inputs_to_repo_relative() -> anyhow::Result<()> {
        let tempdir = temp_test_dir("resolve-subdir")?;
        let repo_root = tempdir.join("repo");
        let nested = repo_root.join("src").join("nested");
        std::fs::create_dir_all(&nested)?;
        std::fs::create_dir(repo_root.join(".git"))?;

        let resolved = resolve_repo_relative_path("lib.rs", &nested, &repo_root)?;

        assert_eq!(resolved, "src/nested/lib.rs");
        Ok(())
    }

    #[test]
    fn resolve_repo_relative_path_rejects_outside_repo() -> anyhow::Result<()> {
        let tempdir = temp_test_dir("resolve-outside")?;
        let repo_root = tempdir.join("repo");
        let nested = repo_root.join("src");
        std::fs::create_dir_all(&nested)?;
        std::fs::create_dir(repo_root.join(".git"))?;

        let err = resolve_repo_relative_path("../../elsewhere.rs", &nested, &repo_root)
            .expect_err("outside-repo paths must be rejected");

        assert!(err.to_string().contains("path must stay within repository"));
        Ok(())
    }

    #[test]
    fn check_result_handles_literal_glob_metacharacters_in_paths() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory()?;
        db::init_db(&conn)?;
        let now_ms = db::now_unix_ms()?;
        conn.execute(
            "INSERT INTO claims(path, agent_id, expires_at_ms) VALUES (?1, ?2, ?3)",
            params!["src/[core]", "agent-b", now_ms + 60_000],
        )?;

        let result = check_result(&conn, "agent-a", "src/[core]/mod.rs", false)?;

        assert_eq!(result.decision, "warn");
        assert_eq!(result.reason_code, "claimed_by_other");
        assert_eq!(result.blocking_agents, vec!["agent-b"]);
        Ok(())
    }

    #[test]
    fn handle_claim_batch_is_atomic_with_message_insert() -> anyhow::Result<()> {
        let mut conn = Connection::open_in_memory()?;
        db::init_db(&conn)?;
        conn.execute(
            "CREATE TRIGGER fail_claim_message BEFORE INSERT ON messages BEGIN SELECT RAISE(FAIL, 'boom'); END;",
            [],
        )?;

        let err = handle_claim_batch(
            &mut conn,
            "agent-a",
            &[String::from("src/lib.rs"), String::from("src/main.rs")],
            std::time::Duration::from_secs(60),
        )
        .expect_err("message trigger should fail");

        assert!(err.to_string().contains("boom"));
        let claim_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM claims", [], |row| row.get(0))?;
        assert_eq!(claim_count, 0);
        Ok(())
    }

    #[test]
    fn handle_release_batch_is_atomic_with_message_insert() -> anyhow::Result<()> {
        let mut conn = Connection::open_in_memory()?;
        db::init_db(&conn)?;
        let now_ms = db::now_unix_ms()?;
        conn.execute(
            "INSERT INTO claims(path, agent_id, expires_at_ms) VALUES (?1, ?2, ?3)",
            params!["src/lib.rs", "agent-a", now_ms + 60_000],
        )?;
        conn.execute(
            "CREATE TRIGGER fail_release_message BEFORE INSERT ON messages BEGIN SELECT RAISE(FAIL, 'boom'); END;",
            [],
        )?;

        let err = handle_release_batch(&mut conn, "agent-a", &[String::from("src/lib.rs")])
            .expect_err("message trigger should fail");

        assert!(err.to_string().contains("boom"));
        let claim_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM claims", [], |row| row.get(0))?;
        assert_eq!(claim_count, 1);
        Ok(())
    }
}

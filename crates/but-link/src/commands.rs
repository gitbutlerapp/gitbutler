//! Command dispatch and handlers for `but link`.

use std::collections::{BTreeMap, HashSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::{BlockMode, CheckFormat, Cmd, ReadView, cmd_name, normalize_claim_path};
use crate::db;

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

/// Check response describing path coordination state.
#[derive(Serialize)]
struct CheckResponse {
    /// Checked path.
    path: String,
    /// `allow`, `warn`, or `deny`.
    decision: &'static str,
    /// Stable reason code for machine consumers.
    reason_code: &'static str,
    /// Current claim state for the requester.
    self_claim: Value,
    /// Conflicting claims from other agents.
    blocking_claims: Vec<db::ActiveClaim>,
    /// Relevant hard typed blocks.
    typed_blocks: Vec<db::TypedBlock>,
    /// Relevant advisories (advisory blocks and path discoveries).
    advisories: Vec<Value>,
    /// Relevant dependency hints.
    dependency_hints: Vec<db::DependencyHint>,
    /// Stale claim holders relevant to this path.
    stale_agents: Vec<db::StaleAgent>,
}

/// Per-path acquisition outcome.
#[derive(Clone, Debug, Serialize)]
struct AcquireDecision {
    /// Path evaluated for acquisition.
    path: String,
    /// `acquired` or `blocked`.
    decision: &'static str,
    /// Stable reason code for machine consumers.
    reason_code: &'static str,
    /// Claim blockers from other agents.
    blocking_claims: Vec<db::ActiveClaim>,
    /// Relevant hard typed blocks.
    typed_blocks: Vec<db::TypedBlock>,
    /// Relevant advisories (advisory blocks and path discoveries).
    advisories: Vec<Value>,
    /// Final claim row when acquired.
    claim: Option<db::ActiveClaim>,
}

/// Aggregated acquire response payload.
#[derive(Debug, Serialize)]
struct AcquireResponse {
    /// Standard success marker.
    ok: bool,
    /// Paths successfully acquired.
    acquired_paths: Vec<String>,
    /// Paths that remained blocked.
    blocked_paths: Vec<String>,
    /// Per-path final outcomes.
    decisions: Vec<AcquireDecision>,
    /// Requester claims after acquisition.
    active_claims: Vec<db::ActiveClaim>,
    /// Relevant typed blocks across the requested paths.
    typed_blocks: Vec<db::TypedBlock>,
    /// Relevant dependency hints.
    dependency_hints: Vec<db::DependencyHint>,
    /// Relevant stale claim holders.
    stale_agents: Vec<db::StaleAgent>,
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

/// Claim detail selected for a requester.
#[derive(Clone)]
struct SelfClaimState {
    status: &'static str,
    path: String,
    expires_at_ms: i64,
}

/// Joined block row used when grouping block query results in this module.
type BlockRow = (
    i64,
    String,
    String,
    String,
    i64,
    Option<i64>,
    Option<i64>,
    Option<String>,
    String,
);

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

/// Deduplicate and normalize already-resolved command paths.
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

/// Normalize an absolute path by collapsing `.` and `..`.
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
                    .is_some_and(|component| matches!(component, Component::Normal(_)))
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

/// Canonicalize the longest existing prefix of a path while preserving the suffix.
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

/// Resolve an input path to a normalized repo-relative path.
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

/// Resolve multiple input paths to normalized repo-relative paths.
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

/// Normalize runtime paths for commands that accept repo paths.
fn normalize_command_paths(cmd: Cmd, current_dir: &Path, repo_root: &Path) -> anyhow::Result<Cmd> {
    Ok(match cmd {
        Cmd::Claim { paths, ttl } => Cmd::Claim {
            paths: resolve_repo_relative_paths(paths, current_dir, repo_root)?,
            ttl,
        },
        Cmd::Acquire {
            paths,
            ttl,
            strict,
            format,
        } => Cmd::Acquire {
            paths: resolve_repo_relative_paths(paths, current_dir, repo_root)?,
            ttl,
            strict,
            format,
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
        Cmd::Intent {
            scope,
            tags,
            surface,
            paths,
        } => Cmd::Intent {
            scope,
            tags,
            surface,
            paths: resolve_repo_relative_paths(paths, current_dir, repo_root)?,
        },
        Cmd::Declare {
            scope,
            tags,
            surface,
            paths,
        } => Cmd::Declare {
            scope,
            tags,
            surface,
            paths: resolve_repo_relative_paths(paths, current_dir, repo_root)?,
        },
        Cmd::Block {
            paths,
            reason,
            mode,
            ttl,
        } => Cmd::Block {
            paths: resolve_repo_relative_paths(paths, current_dir, repo_root)?,
            reason,
            mode,
            ttl,
        },
        Cmd::Ack {
            target_agent_id,
            paths,
            note,
        } => Cmd::Ack {
            target_agent_id,
            paths: resolve_repo_relative_paths(paths, current_dir, repo_root)?,
            note,
        },
        other => other,
    })
}

/// Discover the shared `.git` directory for the repository or worktree.
pub(crate) fn discover_git_dir(start: &Path) -> anyhow::Result<PathBuf> {
    let mut current = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    loop {
        let dot_git = current.join(".git");
        if dot_git.is_dir() {
            return Ok(dot_git);
        }
        if dot_git.is_file() {
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
            let resolved = gitdir_path.canonicalize().unwrap_or(gitdir_path);
            if let Some(parent) = resolved.parent()
                && parent.file_name().is_some_and(|name| name == "worktrees")
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

/// Discover the repository root.
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

/// Entry point from the top-level `but` binary.
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
    db::touch_agent_seen(&conn, &agent_id)?;
    append_command_log(&log_path, &agent_id, cmd_name(&cmd));

    match cmd {
        Cmd::Claim { paths, ttl } => handle_claim_batch(&mut conn, &agent_id, &paths, ttl),
        Cmd::Acquire {
            paths,
            ttl,
            strict,
            format,
        } => handle_acquire_batch(&mut conn, &agent_id, &paths, ttl, strict, format),
        Cmd::Release { paths } => handle_release_batch(&mut conn, &agent_id, &paths),
        Cmd::Claims { path_prefix } => handle_claims(&conn, path_prefix.as_deref()),
        Cmd::Check {
            paths,
            strict,
            format,
        } => handle_check_batch(&conn, &agent_id, &paths, strict, format),
        Cmd::Post { message } => handle_post(&conn, &agent_id, &message),
        Cmd::PostTyped { kind, json } => handle_post_typed(&conn, &agent_id, &kind, &json),
        Cmd::Read { view, since } => handle_read(&conn, &agent_id, view, since),
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

/// Persist a free-text history message.
fn insert_history_message(
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

/// Persist a claim message alongside a batch mutation.
fn insert_history_message_tx(
    tx: &Transaction<'_>,
    created_at_ms: i64,
    agent_id: &str,
    kind: &str,
    body_json: &str,
) -> anyhow::Result<()> {
    tx.execute(
        "INSERT INTO messages(created_at_ms, agent_id, kind, body_json) VALUES (?1, ?2, ?3, ?4)",
        params![created_at_ms, agent_id, kind, body_json],
    )?;
    Ok(())
}

/// Update agent progress inside an existing transaction.
fn touch_agent_progress_tx(
    tx: &Transaction<'_>,
    agent_id: &str,
    now_ms: i64,
) -> anyhow::Result<()> {
    tx.execute(
        "INSERT INTO agent_state(
            agent_id,
            status,
            plan,
            updated_at_ms,
            last_seen_at_ms,
            last_progress_at_ms
         ) VALUES (?1, NULL, NULL, 0, 0, 0)
         ON CONFLICT(agent_id) DO NOTHING",
        params![agent_id],
    )?;
    tx.execute(
        "UPDATE agent_state
         SET updated_at_ms = ?2,
             last_seen_at_ms = ?2,
             last_progress_at_ms = ?2
         WHERE agent_id = ?1",
        params![agent_id, now_ms],
    )?;
    Ok(())
}

/// Claim the provided paths without conflict checks.
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
            delete_stmt.execute(params![path, agent_id])?;
            insert_stmt.execute(params![path, agent_id, expires_at_ms])?;
        }
    }
    let body_json = json!({
        "text": format!("claimed: {}", normalized.join(", ")),
        "paths": normalized,
    })
    .to_string();
    insert_history_message_tx(&tx, now_ms, agent_id, "claim", &body_json)?;
    touch_agent_progress_tx(&tx, agent_id, now_ms)?;
    tx.commit()?;
    emit_ok()
}

/// Release the provided paths.
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
    let now_ms = db::now_unix_ms()?;
    let body_json = json!({
        "text": format!("released: {}", normalized.join(", ")),
        "paths": normalized,
    })
    .to_string();
    insert_history_message_tx(&tx, now_ms, agent_id, "release", &body_json)?;
    touch_agent_progress_tx(&tx, agent_id, now_ms)?;
    tx.commit()?;
    emit_ok()
}

/// Return current claims as JSON.
fn handle_claims(conn: &Connection, path_prefix: Option<&str>) -> anyhow::Result<()> {
    emit(&json!({
        "ok": true,
        "claims": db::load_active_claims(conn, path_prefix)?,
    }))
}

/// Handle `check` for single or multiple paths.
fn handle_check_batch(
    conn: &Connection,
    agent_id: &str,
    paths: &[String],
    strict: bool,
    format: CheckFormat,
) -> anyhow::Result<()> {
    match format {
        CheckFormat::Full if paths.len() == 1 => {
            emit(&check_result(conn, agent_id, &paths[0], strict)?)
        }
        CheckFormat::Full => {
            let results: Vec<CheckResponse> = paths
                .iter()
                .map(|path| check_result(conn, agent_id, path, strict))
                .collect::<anyhow::Result<_>>()?;
            emit(&results)
        }
        CheckFormat::Compact => {
            for path in paths {
                let result = check_result(conn, agent_id, path, strict)?;
                let blockers = blocker_summary(&result.blocking_claims, &result.typed_blocks);
                println!(
                    "{} {} {}{}",
                    result.decision, result.path, result.reason_code, blockers
                );
            }
            Ok(())
        }
    }
}

/// Build the read-only check result for a single path.
fn check_result(
    conn: &Connection,
    agent_id: &str,
    path: &str,
    strict: bool,
) -> anyhow::Result<CheckResponse> {
    let path = normalize_claim_path(path)?;
    let now_ms = db::now_unix_ms()?;
    let self_claim = self_claim_state(conn, agent_id, &path, now_ms)?;
    let blocking_claims = claim_conflicts(conn, agent_id, &path, now_ms)?;
    let relevant_blocks = db::load_open_blocks(conn, Some(agent_id), Some(&path))?;
    let typed_blocks: Vec<db::TypedBlock> = relevant_blocks
        .iter()
        .filter(|block| block.mode == "hard")
        .cloned()
        .collect();
    let mut advisories: Vec<Value> = relevant_blocks
        .iter()
        .filter(|block| block.mode == "advisory")
        .map(block_to_advisory_value)
        .collect();
    advisories.extend(db::recent_discoveries_for_paths(
        conn,
        std::slice::from_ref(&path),
        5,
    )?);
    let dependency_hints =
        db::dependency_hints_for_paths(conn, agent_id, std::slice::from_ref(&path))?;
    let stale_agents = db::stale_agents_for_paths(conn, std::slice::from_ref(&path), now_ms)?;

    let (decision, reason_code) = if !blocking_claims.is_empty() {
        if strict {
            ("deny", "claimed_by_other")
        } else {
            ("warn", "claimed_by_other")
        }
    } else if !typed_blocks.is_empty() {
        ("deny", "hard_block")
    } else if !relevant_blocks.is_empty() {
        if strict {
            ("deny", "advisory_block")
        } else {
            ("warn", "advisory_block")
        }
    } else {
        ("allow", "no_conflict")
    };

    Ok(CheckResponse {
        path,
        decision,
        reason_code,
        self_claim: self_claim_json(self_claim),
        blocking_claims,
        typed_blocks,
        advisories,
        dependency_hints,
        stale_agents,
    })
}

/// Acquire the provided paths transactionally with partial success across the batch.
fn handle_acquire_batch(
    conn: &mut Connection,
    agent_id: &str,
    paths: &[String],
    ttl: std::time::Duration,
    strict: bool,
    format: CheckFormat,
) -> anyhow::Result<()> {
    let response = acquire_batch(conn, agent_id, paths, ttl, strict)?;
    match format {
        CheckFormat::Full => emit(&response),
        CheckFormat::Compact => {
            for decision in &response.decisions {
                let blockers = blocker_summary(&decision.blocking_claims, &decision.typed_blocks);
                println!(
                    "{} {} {}{}",
                    decision.decision, decision.path, decision.reason_code, blockers
                );
            }
            Ok(())
        }
    }
}

/// Acquire the provided paths transactionally with partial success across the batch.
fn acquire_batch(
    conn: &mut Connection,
    agent_id: &str,
    paths: &[String],
    ttl: std::time::Duration,
    strict: bool,
) -> anyhow::Result<AcquireResponse> {
    let now_ms = db::now_unix_ms()?;
    let ttl_ms: i64 = ttl.as_millis().try_into()?;
    let expires_at_ms = now_ms.saturating_add(ttl_ms);
    let normalized = normalized_unique_paths(paths);
    // Serialize acquisition writers so the conflict check sees the latest committed claims.
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut decisions = Vec::new();
    let mut acquired_paths = Vec::new();

    for path in &normalized {
        let blocking_claims = claim_conflicts_tx(&tx, agent_id, path, now_ms)?;
        let relevant_blocks = open_blocks_tx(&tx, Some(agent_id), Some(path), now_ms)?;
        let hard_blocks: Vec<db::TypedBlock> = relevant_blocks
            .iter()
            .filter(|block| block.mode == "hard")
            .cloned()
            .collect();
        let mut advisories: Vec<Value> = relevant_blocks
            .iter()
            .filter(|block| block.mode == "advisory")
            .map(block_to_advisory_value)
            .collect();
        advisories.extend(db::recent_discoveries_for_paths(
            &tx,
            std::slice::from_ref(path),
            5,
        )?);

        let (decision, reason_code) = if !blocking_claims.is_empty() {
            ("blocked", "claimed_by_other")
        } else if !hard_blocks.is_empty() {
            ("blocked", "hard_block")
        } else if strict && relevant_blocks.iter().any(|block| block.mode == "advisory") {
            ("blocked", "advisory_block")
        } else if relevant_blocks.iter().any(|block| block.mode == "advisory") {
            ("acquired", "advisory_block")
        } else {
            ("acquired", "no_conflict")
        };

        let claim = if decision == "acquired" {
            tx.execute(
                "DELETE FROM claims WHERE path = ?1 AND agent_id = ?2",
                params![path, agent_id],
            )?;
            tx.execute(
                "INSERT INTO claims(path, agent_id, expires_at_ms) VALUES (?1, ?2, ?3)",
                params![path, agent_id, expires_at_ms],
            )?;
            acquired_paths.push(path.clone());
            Some(db::ActiveClaim {
                path: path.clone(),
                agent_id: agent_id.to_owned(),
                expires_at_ms,
            })
        } else {
            None
        };

        decisions.push(AcquireDecision {
            path: path.clone(),
            decision,
            reason_code,
            blocking_claims,
            typed_blocks: hard_blocks,
            advisories,
            claim,
        });
    }

    if !acquired_paths.is_empty()
        || decisions
            .iter()
            .any(|decision| decision.decision == "blocked")
    {
        let body_json = json!({
            "text": if acquired_paths.is_empty() {
                format!("acquire blocked: {}", normalized.join(", "))
            } else {
                format!("acquired: {}", acquired_paths.join(", "))
            },
            "paths": normalized,
            "acquired_paths": acquired_paths,
        })
        .to_string();
        insert_history_message_tx(&tx, now_ms, agent_id, "acquire", &body_json)?;
    }

    touch_agent_progress_tx(&tx, agent_id, now_ms)?;
    tx.commit()?;

    let active_claims = db::load_active_claims_for_agent(conn, agent_id)?;
    let typed_blocks = collect_unique_blocks(
        decisions
            .iter()
            .flat_map(|decision| decision.typed_blocks.clone())
            .collect(),
    );
    let dependency_hints = db::dependency_hints_for_paths(conn, agent_id, &normalized)?;
    let stale_agents = db::stale_agents_for_paths(conn, &normalized, now_ms)?;
    let blocked_paths: Vec<String> = decisions
        .iter()
        .filter(|decision| decision.decision == "blocked")
        .map(|decision| decision.path.clone())
        .collect();
    Ok(AcquireResponse {
        ok: true,
        acquired_paths: active_claims
            .iter()
            .map(|claim| claim.path.clone())
            .filter(|path| normalized.contains(path))
            .collect(),
        blocked_paths,
        decisions,
        active_claims,
        typed_blocks,
        dependency_hints,
        stale_agents,
    })
}

/// Build a compact blocker summary string.
fn blocker_summary(blocking_claims: &[db::ActiveClaim], typed_blocks: &[db::TypedBlock]) -> String {
    let mut blockers = Vec::new();
    for claim in blocking_claims {
        blockers.push(claim.agent_id.clone());
    }
    for block in typed_blocks {
        blockers.push(block.agent_id.clone());
    }
    blockers.sort();
    blockers.dedup();
    if blockers.is_empty() {
        String::new()
    } else {
        format!(" {}", blockers.join(","))
    }
}

/// Convert a typed block into an advisory JSON object.
fn block_to_advisory_value(block: &db::TypedBlock) -> Value {
    json!({
        "kind": "block",
        "id": block.id,
        "agent_id": block.agent_id,
        "mode": block.mode,
        "reason": block.reason,
        "paths": block.paths,
        "created_at_ms": block.created_at_ms,
        "expires_at_ms": block.expires_at_ms,
    })
}

/// Deduplicate typed blocks by id.
fn collect_unique_blocks(blocks: Vec<db::TypedBlock>) -> Vec<db::TypedBlock> {
    let mut grouped = BTreeMap::new();
    for block in blocks {
        grouped.entry(block.id).or_insert(block);
    }
    grouped.into_values().collect()
}

/// Determine the requester's current claim state for a path.
fn self_claim_state(
    conn: &Connection,
    agent_id: &str,
    path: &str,
    now_ms: i64,
) -> anyhow::Result<Option<SelfClaimState>> {
    let active = conn
        .query_row(
            "SELECT path, expires_at_ms FROM claims
             WHERE agent_id = ?1 AND expires_at_ms > ?2
               AND (path = ?3
                    OR substr(?3, 1, length(path) + 1) = path || '/'
                    OR substr(path, 1, length(?3) + 1) = ?3 || '/')
             ORDER BY LENGTH(path) DESC, expires_at_ms DESC, path ASC
             LIMIT 1",
            params![agent_id, now_ms, path],
            |row| {
                Ok(SelfClaimState {
                    status: "active",
                    path: row.get(0)?,
                    expires_at_ms: row.get(1)?,
                })
            },
        )
        .optional()?;
    if active.is_some() {
        return Ok(active);
    }

    conn.query_row(
        "SELECT path, expires_at_ms FROM claims
         WHERE agent_id = ?1 AND expires_at_ms <= ?2
           AND (path = ?3
                OR substr(?3, 1, length(path) + 1) = path || '/'
                OR substr(path, 1, length(?3) + 1) = ?3 || '/')
         ORDER BY expires_at_ms DESC, LENGTH(path) DESC, path ASC
         LIMIT 1",
        params![agent_id, now_ms, path],
        |row| {
            Ok(SelfClaimState {
                status: "stale",
                path: row.get(0)?,
                expires_at_ms: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

/// Serialize requester claim state to JSON.
fn self_claim_json(claim: Option<SelfClaimState>) -> Value {
    if let Some(claim) = claim {
        json!({
            "status": claim.status,
            "path": claim.path,
            "expires_at_ms": claim.expires_at_ms,
        })
    } else {
        json!({ "status": "none" })
    }
}

/// Query claim conflicts for a single path.
fn claim_conflicts(
    conn: &Connection,
    agent_id: &str,
    path: &str,
    now_ms: i64,
) -> anyhow::Result<Vec<db::ActiveClaim>> {
    let mut stmt = conn.prepare(
        "SELECT path, agent_id, expires_at_ms FROM claims
         WHERE agent_id <> ?2 AND expires_at_ms > ?3
           AND (path = ?1
                OR substr(?1, 1, length(path) + 1) = path || '/'
                OR substr(path, 1, length(?1) + 1) = ?1 || '/')
         ORDER BY LENGTH(path) DESC, expires_at_ms DESC, path ASC",
    )?;
    let rows = stmt.query_map(params![path, agent_id, now_ms], |row| {
        Ok(db::ActiveClaim {
            path: row.get(0)?,
            agent_id: row.get(1)?,
            expires_at_ms: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Query claim conflicts for a single path inside a transaction.
fn claim_conflicts_tx(
    tx: &Transaction<'_>,
    agent_id: &str,
    path: &str,
    now_ms: i64,
) -> anyhow::Result<Vec<db::ActiveClaim>> {
    let mut stmt = tx.prepare(
        "SELECT path, agent_id, expires_at_ms FROM claims
         WHERE agent_id <> ?2 AND expires_at_ms > ?3
           AND (path = ?1
                OR substr(?1, 1, length(path) + 1) = path || '/'
                OR substr(path, 1, length(?1) + 1) = ?1 || '/')
         ORDER BY LENGTH(path) DESC, expires_at_ms DESC, path ASC",
    )?;
    let rows = stmt.query_map(params![path, agent_id, now_ms], |row| {
        Ok(db::ActiveClaim {
            path: row.get(0)?,
            agent_id: row.get(1)?,
            expires_at_ms: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Query open typed blocks inside a transaction.
fn open_blocks_tx(
    tx: &Transaction<'_>,
    except_agent_id: Option<&str>,
    overlap_path: Option<&str>,
    now_ms: i64,
) -> anyhow::Result<Vec<db::TypedBlock>> {
    let rows = match (except_agent_id, overlap_path) {
        (Some(agent_id), Some(path)) => {
            let mut stmt = tx.prepare(
                "SELECT b.id, b.agent_id, b.mode, b.reason, b.created_at_ms, b.expires_at_ms,
                        b.resolved_at_ms, b.resolved_by_agent_id, bp.path
                 FROM blocks b
                 JOIN block_paths bp ON bp.block_id = b.id
                 WHERE b.agent_id <> ?1
                   AND b.resolved_at_ms IS NULL
                   AND (b.expires_at_ms IS NULL OR b.expires_at_ms > ?2)
                   AND (bp.path = ?3
                        OR substr(?3, 1, length(bp.path) + 1) = bp.path || '/'
                        OR substr(bp.path, 1, length(?3) + 1) = ?3 || '/')
                 ORDER BY b.id ASC, bp.path ASC",
            )?;
            stmt.query_map(params![agent_id, now_ms, path], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        }
        (Some(agent_id), None) => {
            let mut stmt = tx.prepare(
                "SELECT b.id, b.agent_id, b.mode, b.reason, b.created_at_ms, b.expires_at_ms,
                        b.resolved_at_ms, b.resolved_by_agent_id, bp.path
                 FROM blocks b
                 JOIN block_paths bp ON bp.block_id = b.id
                 WHERE b.agent_id <> ?1
                   AND b.resolved_at_ms IS NULL
                   AND (b.expires_at_ms IS NULL OR b.expires_at_ms > ?2)
                 ORDER BY b.id ASC, bp.path ASC",
            )?;
            stmt.query_map(params![agent_id, now_ms], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        }
        (None, Some(path)) => {
            let mut stmt = tx.prepare(
                "SELECT b.id, b.agent_id, b.mode, b.reason, b.created_at_ms, b.expires_at_ms,
                        b.resolved_at_ms, b.resolved_by_agent_id, bp.path
                 FROM blocks b
                 JOIN block_paths bp ON bp.block_id = b.id
                 WHERE b.resolved_at_ms IS NULL
                   AND (b.expires_at_ms IS NULL OR b.expires_at_ms > ?1)
                   AND (bp.path = ?2
                        OR substr(?2, 1, length(bp.path) + 1) = bp.path || '/'
                        OR substr(bp.path, 1, length(?2) + 1) = ?2 || '/')
                 ORDER BY b.id ASC, bp.path ASC",
            )?;
            stmt.query_map(params![now_ms, path], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        }
        (None, None) => {
            let mut stmt = tx.prepare(
                "SELECT b.id, b.agent_id, b.mode, b.reason, b.created_at_ms, b.expires_at_ms,
                        b.resolved_at_ms, b.resolved_by_agent_id, bp.path
                 FROM blocks b
                 JOIN block_paths bp ON bp.block_id = b.id
                 WHERE b.resolved_at_ms IS NULL
                   AND (b.expires_at_ms IS NULL OR b.expires_at_ms > ?1)
                 ORDER BY b.id ASC, bp.path ASC",
            )?;
            stmt.query_map(params![now_ms], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        }
    };
    Ok(group_blocks_local(rows))
}

/// Group block rows inside this module.
fn group_blocks_local(rows: Vec<BlockRow>) -> Vec<db::TypedBlock> {
    let mut grouped = BTreeMap::<i64, db::TypedBlock>::new();
    for (
        id,
        agent_id,
        mode,
        reason,
        created_at_ms,
        expires_at_ms,
        resolved_at_ms,
        resolved_by_agent_id,
        path,
    ) in rows
    {
        let entry = grouped.entry(id).or_insert_with(|| db::TypedBlock {
            id,
            agent_id,
            mode,
            reason,
            paths: Vec::new(),
            created_at_ms,
            expires_at_ms,
            resolved_at_ms,
            resolved_by_agent_id,
        });
        entry.paths.push(path);
    }
    grouped.into_values().collect()
}

/// Insert a free-text message.
fn handle_post(conn: &Connection, agent_id: &str, message: &str) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let body_json = json!({ "text": message }).to_string();
    insert_history_message(conn, now_ms, agent_id, "message", &body_json)?;
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
    insert_history_message(conn, now_ms, agent_id, kind, body_json)?;
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
    let evidence_array: Vec<Value> = evidence
        .iter()
        .map(|item| json!({ "detail": item }))
        .collect();
    let payload = json!({
        "title": title,
        "evidence": evidence_array,
        "suggested_action": { "cmd": action },
        "signal": signal.unwrap_or("high"),
    });
    db::validate_discovery_payload(&payload)?;
    insert_history_message(conn, now_ms, agent_id, "discovery", &payload.to_string())?;
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
    let payload = json!({
        "scope": scope,
        "tags": tags,
        "surface": surface,
        "paths": paths,
    });
    db::validate_surface_payload(&payload)?;
    insert_history_message(conn, now_ms, agent_id, kind, &payload.to_string())?;
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
    let normalized = normalized_unique_paths(paths);
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
    let body_json = json!({
        "text": format!("block {block_id}: {reason}"),
        "block_id": block_id,
        "mode": mode_str,
        "reason": reason,
        "paths": normalized,
    })
    .to_string();
    insert_history_message_tx(&tx, now_ms, agent_id, "block", &body_json)?;
    touch_agent_progress_tx(&tx, agent_id, now_ms)?;
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

    let body_json = json!({
        "text": format!("resolved block {block_id}"),
        "block_id": block_id,
        "target_agent_id": target_agent_id,
        "resolved_by_agent_id": agent_id,
    })
    .to_string();
    insert_history_message(conn, now_ms, agent_id, "resolve", &body_json)?;
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
    let normalized = normalized_unique_paths(paths);
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
    let body_json = json!({
        "text": note
            .map(|note| format!("@{target_agent_id}: ack: {note}"))
            .unwrap_or_else(|| format!("@{target_agent_id}: ack")),
        "ack_id": ack_id,
        "target_agent_id": target_agent_id,
        "paths": normalized,
        "note": note,
    })
    .to_string();
    insert_history_message_tx(&tx, now_ms, agent_id, "ack", &body_json)?;
    touch_agent_progress_tx(&tx, agent_id, now_ms)?;
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
        ReadView::Inbox => handle_read_inbox(conn, agent_id),
        ReadView::Full => handle_read_full(conn),
        ReadView::Discoveries => {
            let (discoveries, next_steps) =
                db::load_discoveries_and_next_steps(conn, "discovery", true)?;
            emit(&json!({
                "ok": true,
                "view": "discoveries",
                "discoveries": discoveries,
                "next_steps": next_steps,
            }))
        }
        ReadView::Messages => emit(&json!({
            "ok": true,
            "view": "messages",
            "messages": db::load_messages_since(conn, None, 0)?,
        })),
        ReadView::Claims => emit(&json!({
            "ok": true,
            "view": "claims",
            "claims": db::load_active_claims(conn, None)?,
        })),
        ReadView::Agents => emit(&json!({
            "ok": true,
            "view": "agents",
            "agents": db::load_agent_snapshots(conn)?,
        })),
    }
}

/// Emit the inbox view for a specific agent.
fn handle_read_inbox(conn: &Connection, agent_id: &str) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let my_active_claims = db::load_active_claims_for_agent(conn, agent_id)?;
    let my_claim_paths: Vec<String> = my_active_claims
        .iter()
        .map(|claim| claim.path.clone())
        .collect();
    let open_blocks = db::load_open_blocks(conn, Some(agent_id), None)?;
    let open_blocks_relevant_to_me: Vec<db::TypedBlock> = if my_claim_paths.is_empty() {
        Vec::new()
    } else {
        open_blocks
            .iter()
            .filter(|block| {
                block.paths.iter().any(|block_path| {
                    my_claim_paths
                        .iter()
                        .any(|claim_path| db::paths_overlap(block_path, claim_path))
                })
            })
            .cloned()
            .collect()
    };

    let pending_acks: Vec<db::TypedBlock> = open_blocks_relevant_to_me
        .iter()
        .filter(|block| {
            block.paths.is_empty()
                || block.paths.iter().any(|path| {
                    db::ack_exists_since(conn, agent_id, &block.agent_id, path, block.created_at_ms)
                        .map(|acked| !acked)
                        .unwrap_or(true)
                })
        })
        .cloned()
        .collect();

    let (mentions_or_directed_updates, prev_cursor, new_cursor) =
        db::unread_inbox_updates(conn, agent_id, now_ms)?;
    let dependency_hints_relevant_to_requested_paths =
        db::dependency_hints_for_paths(conn, agent_id, &my_claim_paths)?;
    let stale_agents_holding_relevant_claims = if my_claim_paths.is_empty() {
        Vec::new()
    } else {
        db::stale_agents_for_paths(conn, &my_claim_paths, now_ms)?
    };

    let mut recent_advisories: Vec<Value> = open_blocks
        .iter()
        .filter(|block| block.mode == "advisory")
        .map(block_to_advisory_value)
        .collect();
    if my_claim_paths.is_empty() {
        recent_advisories.clear();
    } else {
        recent_advisories.extend(db::recent_discoveries_for_paths(conn, &my_claim_paths, 10)?);
    }

    emit(&json!({
        "ok": true,
        "view": "inbox",
        "mentions_or_directed_updates": mentions_or_directed_updates,
        "open_blocks_relevant_to_me": open_blocks_relevant_to_me,
        "my_active_claims": my_active_claims,
        "pending_acks": pending_acks,
        "dependency_hints_relevant_to_requested_paths": dependency_hints_relevant_to_requested_paths,
        "stale_agents_holding_relevant_claims": stale_agents_holding_relevant_claims,
        "recent_advisories": recent_advisories,
        "cursor": {
            "topic": "inbox",
            "prev": prev_cursor,
            "next": new_cursor,
        }
    }))
}

/// Emit the full transcript-style snapshot.
fn handle_read_full(conn: &Connection) -> anyhow::Result<()> {
    let (discoveries, next_steps) = db::load_discoveries_and_next_steps(conn, "discovery", true)?;
    emit(&json!({
        "ok": true,
        "view": "full",
        "messages": db::load_messages_since(conn, None, 0)?,
        "discoveries": discoveries,
        "next_steps": next_steps,
        "claims": db::load_active_claims(conn, None)?,
        "agents": db::load_agent_snapshots(conn)?,
        "blocks": db::load_open_blocks(conn, None, None)?,
    }))
}

/// Emit the discovery brief.
fn handle_brief(conn: &Connection, kind: Option<String>, all: bool) -> anyhow::Result<()> {
    let kind = kind.unwrap_or_else(|| "discovery".to_owned());
    let (discoveries, next_steps) = db::load_discoveries_and_next_steps(conn, &kind, all)?;
    emit(&json!({
        "ok": true,
        "mode": "brief",
        "kind": kind,
        "discoveries": discoveries,
        "next_steps": next_steps,
    }))
}

/// Emit the discovery digest.
fn handle_digest(conn: &Connection, kind: Option<String>, all: bool) -> anyhow::Result<()> {
    let kind = kind.unwrap_or_else(|| "discovery".to_owned());
    let (discoveries, next_steps) = db::load_discoveries_and_next_steps(conn, &kind, all)?;
    let discoveries: Vec<Value> = discoveries
        .into_iter()
        .map(|discovery| match discovery {
            Value::Object(mut map) => {
                let title = map.remove("title");
                let agent_id = map.remove("agent_id");
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
    }))
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
    Status,
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

/// Emit agent snapshots.
fn handle_agents(conn: &Connection) -> anyhow::Result<()> {
    emit(&json!({
        "ok": true,
        "agents": db::load_agent_snapshots(conn)?,
    }))
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
    insert_history_message(conn, now_ms, agent_id, "message", &body_json)?;
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};

    use super::*;

    /// Create a temporary repository-like directory for path resolution tests.
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

    #[test]
    fn check_ignores_free_text_blocking_words() -> anyhow::Result<()> {
        let conn = Connection::open_in_memory()?;
        db::init_db(&conn)?;
        handle_post(&conn, "peer", "please avoid src/app.txt while I refactor")?;

        let result = check_result(&conn, "me", "src/app.txt", false)?;
        assert_eq!(result.decision, "allow");
        assert_eq!(result.reason_code, "no_conflict");
        Ok(())
    }

    #[test]
    fn check_respects_typed_blocks() -> anyhow::Result<()> {
        let mut conn = Connection::open_in_memory()?;
        db::init_db(&conn)?;
        handle_block(
            &mut conn,
            "peer",
            &[String::from("src/app.txt")],
            "shared refactor",
            BlockMode::Hard,
            None,
        )?;

        let result = check_result(&conn, "me", "src/app.txt", false)?;
        assert_eq!(result.decision, "deny");
        assert_eq!(result.reason_code, "hard_block");
        Ok(())
    }

    #[test]
    fn acquire_claims_clear_paths_and_skips_blocked_paths() -> anyhow::Result<()> {
        let mut conn = Connection::open_in_memory()?;
        db::init_db(&conn)?;
        handle_block(
            &mut conn,
            "peer",
            &[String::from("src/blocked.txt")],
            "shared refactor",
            BlockMode::Hard,
            None,
        )?;

        handle_acquire_batch(
            &mut conn,
            "me",
            &[
                String::from("src/clear.txt"),
                String::from("src/blocked.txt"),
            ],
            std::time::Duration::from_secs(900),
            false,
            CheckFormat::Full,
        )?;

        let claims = db::load_active_claims_for_agent(&conn, "me")?;
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].path, "src/clear.txt");
        Ok(())
    }

    #[test]
    fn acquire_serializes_concurrent_writers_into_blocked_result() -> anyhow::Result<()> {
        let tempdir = temp_test_dir("acquire-concurrent")?;
        let db_path = tempdir.join("coord.db");

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
                std::time::Duration::from_secs(900),
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
                std::time::Duration::from_secs(900),
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

        let conn = Connection::open(tempdir.join("coord.db"))?;
        db::init_db(&conn)?;
        let claims = db::load_active_claims(&conn, Some("src/app.txt"))?;
        assert_eq!(claims.len(), 1);
        Ok(())
    }
}

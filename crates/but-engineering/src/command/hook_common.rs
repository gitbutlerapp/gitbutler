//! Shared utilities for Claude Code hook subcommands.
//!
//! All hook commands receive JSON on stdin, need DB access, and produce
//! either plain text or JSON output. This module provides the common
//! building blocks.

use std::collections::BTreeMap;
use std::io::Read as _;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::db::DbHandle;
use crate::session;
use crate::types::{Agent, Claim, Message};

/// How far back to look for "active" agents and messages.
/// 60 minutes covers a full working session — agents stay visible
/// even during long tasks with infrequent posts.
pub const ACTIVE_WINDOW_MINUTES: i64 = 60;

/// Default number of message previews to show.
pub const MESSAGE_PREVIEW_COUNT: usize = 3;

/// Max characters per message preview line.
pub const MESSAGE_PREVIEW_LEN: usize = 100;

/// Longer preview for messages containing @mentions so they don't get truncated.
pub const MENTION_PREVIEW_LEN: usize = 300;

/// Read stdin into a String. All hooks receive JSON on stdin;
/// some parse it, others just consume and discard.
pub fn read_stdin() -> String {
    let mut buf = String::new();
    let _ = std::io::stdin().lock().read_to_string(&mut buf);
    buf
}

/// Parse stdin as JSON. Returns `None` on any failure (empty, malformed, etc.).
pub fn parse_stdin_json() -> Option<serde_json::Value> {
    let input = read_stdin();
    serde_json::from_str(&input).ok()
}

/// Fetch active agents and recent messages within the standard window.
pub fn fetch_coordination_state(db: &DbHandle, message_count: usize) -> anyhow::Result<(Vec<Agent>, Vec<Message>)> {
    let now = Utc::now();
    let since = now - chrono::Duration::minutes(ACTIVE_WINDOW_MINUTES);
    let agents = db.list_agents(Some(since))?;
    let messages = db.query_recent_messages(since, message_count)?;
    Ok((agents, messages))
}

/// Source used to resolve the current agent identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IdentitySource {
    Arg,
    Env,
    Session,
    Heuristic,
    None,
}

/// Agent identity and how it was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedIdentity {
    pub agent_id: Option<String>,
    pub source: IdentitySource,
}

fn resolve_session_agent(db: &DbHandle) -> Option<String> {
    if let Some(claude_pid) = session::find_claude_ancestor()
        && let Ok(Some(agent_id)) = db.get_session_agent(claude_pid)
    {
        return Some(agent_id);
    }
    None
}

fn resolve_heuristic_agent(db: &DbHandle) -> Option<String> {
    db.list_agents(None)
        .ok()?
        .into_iter()
        .max_by_key(|a| a.last_active)
        .map(|a| a.id)
}

fn normalize_identity_input(value: Option<&str>) -> Option<String> {
    value.map(str::trim).filter(|s| !s.is_empty()).map(ToOwned::to_owned)
}

fn resolve_identity_from_candidates(
    explicit_agent: Option<&str>,
    env_agent: Option<&str>,
    session_agent: Option<String>,
    heuristic_agent: Option<String>,
) -> ResolvedIdentity {
    if let Some(agent_id) = normalize_identity_input(explicit_agent) {
        return ResolvedIdentity {
            agent_id: Some(agent_id),
            source: IdentitySource::Arg,
        };
    }

    if let Some(agent_id) = normalize_identity_input(env_agent) {
        return ResolvedIdentity {
            agent_id: Some(agent_id),
            source: IdentitySource::Env,
        };
    }

    if let Some(agent_id) = session_agent {
        return ResolvedIdentity {
            agent_id: Some(agent_id),
            source: IdentitySource::Session,
        };
    }

    if let Some(agent_id) = heuristic_agent {
        return ResolvedIdentity {
            agent_id: Some(agent_id),
            source: IdentitySource::Heuristic,
        };
    }

    ResolvedIdentity {
        agent_id: None,
        source: IdentitySource::None,
    }
}

/// Resolve current agent identity.
///
/// Resolution order:
/// 1. Explicit arg (`--agent-id`)
/// 2. `BUT_ENGINEERING_AGENT_ID` env var
/// 3. Session lookup via Claude ancestor PID
/// 4. Most-recently-active heuristic
pub fn resolve_identity(db: &DbHandle, explicit_agent: Option<&str>) -> ResolvedIdentity {
    let env_agent = std::env::var("BUT_ENGINEERING_AGENT_ID").ok();
    let session_agent = resolve_session_agent(db);
    let heuristic_agent = resolve_heuristic_agent(db);
    resolve_identity_from_candidates(explicit_agent, env_agent.as_deref(), session_agent, heuristic_agent)
}

/// Resolve only the agent id for compatibility with existing call sites.
pub fn resolve_self_agent(db: &DbHandle) -> Option<String> {
    resolve_identity(db, None).agent_id
}

/// Check if a single message contains an @mention pattern (`@` followed by a word character).
fn content_has_mention(content: &str) -> bool {
    content
        .as_bytes()
        .windows(2)
        .any(|w| w[0] == b'@' && (w[1].is_ascii_alphanumeric() || w[1] == b'_' || w[1] == b'-'))
}

// -- CTA strings shared across hook commands --

/// Cold-start CTA: no agents or messages, repo uses but-engineering.
pub const CTA_COLD_START: &str = "Use Skill(but-engineering) — run `but status --json` to orient, then post your plan to the channel before starting work.";

/// CTA when @mentions are detected.
pub const CTA_MENTIONS: &str = "Teammates are waiting — use Skill(but-engineering), \
set or refresh your plan, read the channel, and respond to @mentions first.";

/// CTA when agents are active but no messages yet.
pub const CTA_NO_MESSAGES: &str =
    "Use Skill(but-engineering) — set your plan, post what you're about to work on, then read the channel.";

/// CTA when agents are active with messages (no @mentions).
pub const CTA_HAS_MESSAGES: &str =
    "Use Skill(but-engineering) — set your plan, post what you're working on, then read the channel.";

/// Select the appropriate CTA based on coordination state.
/// `extra_cta` is an optional override for non-mention, non-empty-message cases
/// (for hook-specific wording).
pub fn select_cta(mentions: bool, messages_empty: bool, extra_cta: Option<&str>) -> &str {
    if mentions {
        return CTA_MENTIONS;
    }
    if let Some(cta) = extra_cta {
        return cta;
    }
    if messages_empty {
        CTA_NO_MESSAGES
    } else {
        CTA_HAS_MESSAGES
    }
}

/// Build a text summary of coordination state for injection as context.
///
/// Returns `(summary_text, has_mentions)`. The bool indicates whether any
/// message contains an @mention, so callers can adjust their CTA.
/// Returns an empty string (with `false`) if there's nothing to report.
pub fn build_summary(agents: &[Agent], messages: &[Message], now: DateTime<Utc>) -> (String, bool) {
    let mut out = String::with_capacity(512);

    if agents.is_empty() && messages.is_empty() {
        return (out, false);
    }

    let mentions = messages.iter().any(|m| content_has_mention(&m.content));

    out.push_str(&format!("but-engineering: {} agent(s) active", agents.len()));
    if !messages.is_empty() {
        out.push_str(&format!(", {} new msg(s)", messages.len()));
        if mentions {
            out.push_str(" [@mentions detected]");
        }
    }
    out.push('\n');

    for msg in messages {
        let who = &msg.agent_id;
        let t = format_minutes_short((now - msg.timestamp).num_minutes());

        let is_mention = content_has_mention(&msg.content);
        let (prefix, max_len) = if is_mention {
            (">>> ", MENTION_PREVIEW_LEN)
        } else {
            ("  ", MESSAGE_PREVIEW_LEN)
        };

        out.push_str(&format!("{prefix}[{t}] {who}: {}\n", truncate(&msg.content, max_len)));
    }

    for agent in agents.iter().filter(|a| a.status.is_some()) {
        out.push_str(&format!("  {}: {}\n", agent.id, agent.status.as_deref().unwrap()));
    }

    (out, mentions)
}

/// Truncate a string to at most `max` characters, replacing newlines with spaces.
pub fn truncate(s: &str, max: usize) -> String {
    let flat: String = s.chars().map(|c| if c == '\n' { ' ' } else { c }).collect();
    if flat.chars().count() <= max {
        return flat;
    }
    // Find the byte offset of the max-th character.
    let end = flat.char_indices().nth(max).map_or(flat.len(), |(i, _)| i);
    format!("{}...", &flat[..end])
}

/// Format a duration in minutes as a human-readable relative time.
///
/// Returns `"now"` for zero or negative values, otherwise `"{n}m ago"`.
pub fn format_minutes_ago(mins: i64) -> String {
    if mins <= 0 {
        "now".into()
    } else {
        format!("{mins}m ago")
    }
}

/// Format a duration in minutes as a compact relative time label.
///
/// Returns `"now"` for zero or negative values, otherwise `"{n}m"`.
pub fn format_minutes_short(mins: i64) -> String {
    if mins <= 0 { "now".into() } else { format!("{mins}m") }
}

/// Extract the file name from a path, falling back to the full path.
pub fn short_file_name(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

/// Format claims as a one-line summary grouped by agent, using short file names.
///
/// Produces output like: `  claimed: agent1→file1, file2; agent2→file3;\n`
/// The `label` parameter sets the prefix (e.g., `"claimed"` or `"claims"`).
/// Returns an empty string if `claims` is empty.
pub fn format_claims_summary(claims: &[Claim], label: &str) -> String {
    if claims.is_empty() {
        return String::new();
    }
    let mut by_agent: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for c in claims {
        by_agent.entry(&c.agent_id).or_default().push(&c.file_path);
    }
    let mut out = format!("  {label}:");
    for (agent, files) in &by_agent {
        let files_str = files.iter().map(|f| short_file_name(f)).collect::<Vec<_>>().join(", ");
        out.push_str(&format!(" {agent}\u{2192}{files_str};"));
    }
    out.push('\n');
    out
}

/// Find messages that mention a file path (substring match on content).
pub fn messages_mentioning_path<'a>(messages: &'a [Message], path: &str) -> Vec<&'a Message> {
    messages.iter().filter(|m| m.content.contains(path)).collect()
}

/// Configuration for which sections to include in `build_full_context`.
#[derive(Default)]
pub struct ContextOptions<'a> {
    /// Blocks targeting this agent (shown at top with urgency).
    pub blocks: &'a [Message],
    /// Whether to include proactive awareness warnings.
    pub include_proactive_awareness: bool,
    /// Optional CTA override (e.g., for resume/compact or block urgency).
    pub extra_cta: Option<&'a str>,
}

/// Build full coordination context for injection into hooks.
///
/// Combines the summary, blocks, claims, proactive awareness, plans, and
/// discoveries into a single string with the appropriate CTA prepended.
/// Returns `CTA_COLD_START` if there are no agents and no messages.
pub fn build_full_context(
    agents: &[Agent],
    messages: &[Message],
    claims: &[Claim],
    discoveries: &[Message],
    self_agent: Option<&str>,
    now: DateTime<Utc>,
    opts: ContextOptions<'_>,
) -> String {
    if agents.is_empty() && messages.is_empty() {
        return CTA_COLD_START.to_string();
    }

    let (mut summary, mentions) = build_summary(agents, messages, now);

    // --- Blocks targeting us — highest urgency, shown first in summary ---
    let my_blocks: Vec<&Message> = opts
        .blocks
        .iter()
        .filter(|b| {
            if let Some(me) = self_agent {
                b.content.contains(&format!("@{me}"))
            } else {
                false
            }
        })
        .collect();

    if !my_blocks.is_empty() {
        for b in &my_blocks {
            let t = format_minutes_ago((now - b.timestamp).num_minutes());
            summary.insert_str(
                0,
                &format!(
                    "  BLOCKED: {} ({t}) — {}\n",
                    b.agent_id,
                    truncate(&b.content, MESSAGE_PREVIEW_LEN)
                ),
            );
        }
    }

    // Append active claims.
    summary.push_str(&format_claims_summary(claims, "claims"));

    // --- Proactive semantic awareness ---
    if opts.include_proactive_awareness
        && let Some(me) = self_agent
    {
        let awareness = build_proactive_awareness(claims, me);
        if !awareness.is_empty() {
            summary.push_str("  heads-up:\n");
            for warning in &awareness {
                summary.push_str(&format!("    {warning}\n"));
            }
        }
    }

    // Append agent plans if any exist.
    let plans: Vec<_> = agents.iter().filter(|a| a.plan.is_some()).collect();
    if !plans.is_empty() {
        summary.push_str("  plans:\n");
        for agent in &plans {
            let plan = agent.plan.as_deref().unwrap();
            summary.push_str(&format!("    {}: {}\n", agent.id, truncate(plan, MESSAGE_PREVIEW_LEN)));
        }
    }

    // Append recent discoveries.
    if !discoveries.is_empty() {
        summary.push_str("  discoveries:\n");
        for d in discoveries {
            let t = format_minutes_short((now - d.timestamp).num_minutes());
            summary.push_str(&format!(
                "    [{t}] {}: {}\n",
                d.agent_id,
                truncate(&d.content, MESSAGE_PREVIEW_LEN)
            ));
        }
    }

    // CTA — if we have blocks targeting us, override with urgency CTA.
    let cta = if !my_blocks.is_empty() {
        "A teammate is BLOCKED waiting for you — read the channel and release your claim."
    } else {
        select_cta(mentions, messages.is_empty(), opts.extra_cta)
    };
    format!("{cta}\n\n{summary}")
}

/// Proactive semantic awareness: check if any of our claimed files depend
/// on files that teammates are currently editing (claimed by them).
///
/// Reads only our own files (bounded, cheap), checks for references to
/// teammate filenames/stems.
pub fn build_proactive_awareness(claims: &[Claim], self_agent: &str) -> Vec<String> {
    let my_claims: Vec<&Claim> = claims.iter().filter(|c| c.agent_id == self_agent).collect();
    let their_claims: Vec<&Claim> = claims.iter().filter(|c| c.agent_id != self_agent).collect();

    if my_claims.is_empty() || their_claims.is_empty() {
        return Vec::new();
    }

    // Build a lookup: teammate filename/stem → (agent_id, full_path)
    let mut their_files: Vec<(&str, &str, &str)> = Vec::new(); // (filename, stem, agent_id)
    for c in &their_claims {
        if let Some(filename) = std::path::Path::new(&c.file_path).file_name().and_then(|f| f.to_str()) {
            let stem = std::path::Path::new(filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(filename);
            // Skip short/common stems to avoid false positives (e.g., "mod" in mod.rs).
            let use_stem = stem.len() >= 4 && !matches!(stem, "mod" | "lib" | "main" | "test" | "tests");
            their_files.push((filename, if use_stem { stem } else { "" }, &c.agent_id));
        }
    }

    if their_files.is_empty() {
        return Vec::new();
    }

    let mut warnings = Vec::new();

    for my_claim in &my_claims {
        let my_content = match std::fs::read_to_string(&my_claim.file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let my_short = short_file_name(&my_claim.file_path);

        for &(filename, stem, agent_id) in &their_files {
            // Skip self-references (same file claimed by both — shouldn't happen
            // but be safe).
            if my_claim.file_path.ends_with(filename) {
                continue;
            }
            if my_content.contains(filename) || (!stem.is_empty() && my_content.contains(stem)) {
                warnings.push(format!(
                    "Your {my_short} references {filename} which {agent_id} is editing"
                ));
            }
        }
    }

    warnings
}

/// Normalize a file path to be relative to the repo root.
///
/// If the path is absolute and starts with the repo root, strips the prefix.
/// If already relative, returns as-is. Strips trailing slashes.
///
/// This ensures claims use consistent paths regardless of whether the caller
/// passes absolute or relative paths.
pub fn normalize_path(path: &str, repo_root: &std::path::Path) -> String {
    let p = std::path::Path::new(path);

    // Try to make absolute (resolve symlinks, normalize components).
    let absolute = if p.is_absolute() {
        // Try to canonicalize, fall back to the path as-is.
        std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
    } else {
        // For relative paths, try resolving against CWD for canonicalization.
        std::env::current_dir()
            .ok()
            .and_then(|cwd| std::fs::canonicalize(cwd.join(p)).ok())
            .unwrap_or_else(|| p.to_path_buf())
    };

    // Canonicalize repo_root too so the prefix strip works.
    let canonical_root = std::fs::canonicalize(repo_root).unwrap_or_else(|_| repo_root.to_path_buf());

    // Strip repo root prefix.
    if let Ok(relative) = absolute.strip_prefix(&canonical_root) {
        let s = relative.to_string_lossy().to_string();
        // Strip trailing slashes.
        s.trim_end_matches('/').to_string()
    } else {
        // Not under repo root — return the path as-is.
        path.trim_end_matches('/').to_string()
    }
}

/// Discover the git repo root (work tree) from the current directory.
///
/// Returns `None` if not in a git repo. Uses `gix::discover()`.
pub fn find_repo_root() -> Option<std::path::PathBuf> {
    let current_dir = std::env::current_dir().ok()?;
    let repo = gix::discover(&current_dir).ok()?;
    repo.workdir().map(|p| p.to_path_buf())
}

/// Build JSON output in the hook-specific format expected by Claude Code.
///
/// Supported hooks accept JSON with
/// `hookSpecificOutput.additionalContext` for injecting context.
pub fn build_hook_json(hook_event_name: &str, additional_context: &str) -> serde_json::Value {
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": hook_event_name,
            "additionalContext": additional_context
        }
    })
}

/// Print JSON output in the hook-specific format expected by Claude Code.
pub fn print_hook_json(hook_event_name: &str, additional_context: &str) {
    print!("{}", build_hook_json(hook_event_name, additional_context));
}

/// Build a deny decision JSON for PreToolUse hooks.
pub fn build_deny_json(reason: &str) -> serde_json::Value {
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_precedence_arg_over_all() {
        let resolved = resolve_identity_from_candidates(
            Some("arg-agent"),
            Some("env-agent"),
            Some("session-agent".to_string()),
            Some("heur-agent".to_string()),
        );
        assert_eq!(resolved.source, IdentitySource::Arg);
        assert_eq!(resolved.agent_id.as_deref(), Some("arg-agent"));
    }

    #[test]
    fn identity_precedence_env_over_session_and_heuristic() {
        let resolved = resolve_identity_from_candidates(
            None,
            Some("env-agent"),
            Some("session-agent".to_string()),
            Some("heur-agent".to_string()),
        );
        assert_eq!(resolved.source, IdentitySource::Env);
        assert_eq!(resolved.agent_id.as_deref(), Some("env-agent"));
    }

    #[test]
    fn identity_precedence_session_over_heuristic() {
        let resolved = resolve_identity_from_candidates(
            None,
            None,
            Some("session-agent".to_string()),
            Some("heur-agent".to_string()),
        );
        assert_eq!(resolved.source, IdentitySource::Session);
        assert_eq!(resolved.agent_id.as_deref(), Some("session-agent"));
    }

    #[test]
    fn identity_precedence_heuristic_when_only_fallback_available() {
        let resolved = resolve_identity_from_candidates(None, None, None, Some("heur-agent".to_string()));
        assert_eq!(resolved.source, IdentitySource::Heuristic);
        assert_eq!(resolved.agent_id.as_deref(), Some("heur-agent"));
    }

    #[test]
    fn identity_none_when_no_candidates() {
        let resolved = resolve_identity_from_candidates(None, None, None, None);
        assert_eq!(resolved.source, IdentitySource::None);
        assert_eq!(resolved.agent_id, None);
    }
}

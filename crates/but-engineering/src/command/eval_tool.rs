//! Eval-tool: PreToolUse hook for file conflict detection.
//!
//! Fires before every Edit, Write, or MultiEdit tool call (configured with matcher
//! `Edit|Write|MultiEdit`). Three-layer conflict detection:
//!
//! 1. **Claims check (blocking)**: If the file is claimed by another active
//!    agent, outputs `permissionDecision: deny` to block the edit. The agent
//!    must coordinate (ask the claiming agent to release) before retrying.
//!
//! 2. **Message check (advisory)**: If no claim but the file was mentioned
//!    by another agent in recent messages, injects a warning. Advisory only.
//!
//! 3. **Semantic check (advisory)**: If the file is referenced/imported by
//!    files claimed by other agents, injects a warning. Performance-bounded:
//!    reads only file prefixes (8 KB) and caps the number of files scanned.
//!
//! Performance-critical: fires on every Edit/Write/MultiEdit.
//! Delegated from the unified `eval` command.

use chrono::Utc;

use super::conflict::{self, CheckDecision};
use super::hook_common;
use super::hook_common::IdentitySource;
use crate::db::DbHandle;
use crate::session;
use crate::types::MessageKind;

/// Dedup window for hook-generated coordination chatter.
const COORDINATION_POST_WINDOW_MINUTES: i64 = 5;

pub fn execute(db: &DbHandle, input: &Option<serde_json::Value>) -> anyhow::Result<()> {
    // Extract file_path from tool_input.
    // MultiEdit payloads can include either `file_path` or `path`.
    let raw_file_path = input.as_ref().and_then(|v| v.get("tool_input")).and_then(|v| {
        v.get("file_path")
            .and_then(|p| p.as_str())
            .or_else(|| v.get("path").and_then(|p| p.as_str()))
    });

    let raw_file_path = match raw_file_path {
        Some(p) => p,
        None => return Ok(()), // No file_path = nothing to check.
    };

    let now = Utc::now();

    // Explicit-first identity resolver (arg/env/session/heuristic).
    // Hook path provides no explicit arg, so this typically resolves via
    // env/session/heuristic.
    let identity = resolve_hook_identity_for_side_effects(db, now);
    let self_agent = identity.agent_id.as_deref();
    let evaluation = conflict::evaluate_conflict(db, raw_file_path, self_agent)?;

    // Blocking deny decision (unchanged output contract).
    if evaluation.decision == CheckDecision::Deny {
        // If this agent is part of a contended-claim set for the same file,
        // release its local claim proactively to reduce deadlock loops.
        if let Some(ref self_id) = identity.agent_id
            && let Ok(file_claims) = db.get_claims_for_file(&evaluation.file_path)
        {
            let self_has_claim = file_claims.iter().any(|c| c.agent_id == *self_id);
            let has_other_claimants = file_claims.iter().any(|c| c.agent_id != *self_id);
            if self_has_claim
                && has_other_claimants
                && let Err(e) = db.release_files(&[evaluation.file_path.as_str()], self_id)
            {
                eprintln!(
                    "but-engineering: failed to auto-release local claim on {}: {e}",
                    evaluation.file_path
                );
            }
        }

        // Auto-post a block message so the claiming agent sees urgency on
        // their next prompt. Dedup: skip if we already posted a block for
        // the same file within the last 5 minutes.
        if let Some(ref self_id) = identity.agent_id {
            let block_since = now - chrono::Duration::minutes(COORDINATION_POST_WINDOW_MINUTES);
            let recent_blocks = db.query_recent_blocks(block_since, 10).unwrap_or_default();
            let already_blocked = recent_blocks
                .iter()
                .any(|b| b.agent_id == *self_id && b.content.contains(&evaluation.file_path));

            if !already_blocked {
                let mentions = evaluation
                    .blocking_agents
                    .iter()
                    .map(|a| format!("@{a}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let block_msg = format!(
                    "BLOCKED on {} — {} please release it. I attempted the edit and paused to avoid a conflict; I will retry after release.",
                    evaluation.file_path, mentions
                );
                if let Err(e) = super::post::execute_with_kind(db, block_msg, self_id.clone(), MessageKind::Block) {
                    eprintln!("but-engineering: failed to post block message: {e}");
                }
            }
        }

        if let Some(reason) = evaluation.reason.as_deref() {
            print_deny_json(reason);
        }
        return Ok(());
    }

    // Advisory warnings (unchanged output contract).
    if !evaluation.warnings.is_empty() {
        hook_common::print_hook_json("PreToolUse", &evaluation.warnings.join("\n"));

        // Add a concise advisory post so teammates can see this coordination
        // signal in-channel. Dedup by (agent, file, marker) in a short window.
        if let Some(ref self_id) = identity.agent_id {
            let since = now - chrono::Duration::minutes(COORDINATION_POST_WINDOW_MINUTES);
            let recent = db.query_recent_messages(since, 30).unwrap_or_default();
            let already_posted = recent.iter().any(|m| {
                m.agent_id == *self_id
                    && m.kind == MessageKind::Message
                    && m.content.contains("[coordination-check]")
                    && m.content.contains(&evaluation.file_path)
            });

            if !already_posted {
                let first_warning = evaluation
                    .warnings
                    .first()
                    .map(|w| hook_common::truncate(w, 180))
                    .unwrap_or_else(|| "advisory signal detected".to_string());
                let advisory_msg = format!(
                    "[coordination-check] Pre-edit check on {} found advisory signals: {}. I will read/respond in channel before continuing.",
                    evaluation.file_path, first_warning
                );
                if let Err(e) = super::post::execute_with_kind(db, advisory_msg, self_id.clone(), MessageKind::Message)
                {
                    eprintln!("but-engineering: failed to post advisory message: {e}");
                }
            }
        }
    }

    // --- Auto-claim: claim the file for the editing agent ---
    // This is the self-reinforcing fallback: even agents that forget to
    // `claim` explicitly still get their files tracked. The claim refreshes
    // on every edit, keeping it alive while the agent is actively working.
    if let Some(ref self_id) = identity.agent_id {
        if let Err(e) = db.upsert_agent(self_id, now) {
            eprintln!("but-engineering: failed to refresh agent activity for {self_id}: {e}");
        }
        if let Err(e) = db.claim_files(&[&evaluation.file_path], self_id, now) {
            eprintln!("but-engineering: auto-claim failed for {}: {e}", evaluation.file_path);
        }
    }

    Ok(())
}

/// Resolve identity for hook side effects (posts/claims) with collision safety.
///
/// We avoid heuristic attribution because it can mis-assign messages/claims to
/// the wrong agent when multiple agents are active. Preferred sources are:
/// arg/env/session. If unavailable, derive a unique fallback from Claude PID.
/// If that also fails, disable side effects by returning no agent id.
fn resolve_hook_identity_for_side_effects(db: &DbHandle, now: chrono::DateTime<Utc>) -> hook_common::ResolvedIdentity {
    let mut identity = hook_common::resolve_identity(db, None);

    if matches!(
        identity.source,
        IdentitySource::Arg | IdentitySource::Env | IdentitySource::Session
    ) {
        return identity;
    }

    if let Some(claude_pid) = session::find_claude_ancestor() {
        let fallback_agent = format!("claude-{claude_pid}");
        if db.register_session(claude_pid, &fallback_agent, now).is_ok() {
            identity.agent_id = Some(fallback_agent);
            identity.source = IdentitySource::Session;
            return identity;
        }
    }

    identity.agent_id = None;
    identity.source = IdentitySource::None;
    identity
}

/// Print a deny decision that blocks the tool call.
fn print_deny_json(reason: &str) {
    print!("{}", hook_common::build_deny_json(reason));
}

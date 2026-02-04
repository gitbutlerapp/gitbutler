//! Eval-prompt command: UserPromptSubmit hook for forced skill evaluation.
//!
//! Key design: output must contain **novel, changing data** (message previews,
//! agent statuses) on every invocation. Static repeated instructions get
//! habituated and ignored by the model after the first prompt.

use chrono::Utc;

use super::conflict;
use super::hook_common::{self, ContextOptions};
use crate::db::DbHandle;

/// Outputs context to stdout when coordination is relevant.
///
/// Fires when:
/// - Multiple agents are active (coordination needed)
/// - One agent with recent messages (conversation in progress)
/// - No active agents but DB exists (cold start — first prompt of a new session)
///
/// Only stays silent when the DB itself can't be opened (not a but-engineering repo).
/// Claims from agents inactive longer than this are garbage-collected.
/// Longer than the 5-minute conflict-check threshold so claims aren't
/// deleted while they'd still block — this only cleans up truly abandoned ones.
const CLAIM_EXPIRY_MINUTES: i64 = 15;

/// How far back to look for active blocks.
const BLOCK_WINDOW_MINUTES: i64 = 15;

pub fn execute(db: &DbHandle) -> anyhow::Result<()> {
    let (agents, messages) = hook_common::fetch_coordination_state(db, hook_common::MESSAGE_PREVIEW_COUNT)?;

    let now = Utc::now();
    let active_since = now - chrono::Duration::minutes(hook_common::ACTIVE_WINDOW_MINUTES);
    let claims = db.list_claims(Some(active_since)).unwrap_or_default();
    let discoveries = db.query_recent_discoveries(active_since, 5).unwrap_or_default();
    let block_since = now - chrono::Duration::minutes(BLOCK_WINDOW_MINUTES);
    let blocks = db.query_recent_blocks(block_since, 10).unwrap_or_default();

    // Identify ourselves: session lookup with fallback to most-recently-active heuristic.
    let self_agent_owned = hook_common::resolve_self_agent(db);
    let self_agent = self_agent_owned.as_deref();

    // Periodic garbage collection: expire claims and plans from inactive agents.
    let expiry_cutoff = now - chrono::Duration::minutes(CLAIM_EXPIRY_MINUTES);
    let _ = db.expire_stale_claims(expiry_cutoff);
    let lease_cutoff = now - chrono::Duration::minutes(conflict::CLAIM_STALE_MINUTES);
    let _ = db.expire_claims_older_than(lease_cutoff);
    let _ = db.clear_stale_plans(expiry_cutoff);

    let context = hook_common::build_full_context(
        &agents,
        &messages,
        &claims,
        &discoveries,
        self_agent,
        now,
        ContextOptions {
            blocks: &blocks,
            include_proactive_awareness: true,
            ..Default::default()
        },
    );

    print!("{context}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::command::hook_common::{self, ContextOptions};
    use crate::types::{Agent, Claim, Message, MessageKind};

    fn make_agent(id: &str, plan: Option<&str>) -> Agent {
        Agent {
            id: id.to_string(),
            status: None,
            last_active: Utc::now(),
            last_read: None,
            plan: plan.map(|s| s.to_string()),
            plan_updated_at: plan.map(|_| Utc::now()),
        }
    }

    fn make_message(agent_id: &str, content: &str, mins_ago: i64) -> Message {
        Message {
            id: format!("msg-{agent_id}-{mins_ago}"),
            agent_id: agent_id.to_string(),
            content: content.to_string(),
            timestamp: Utc::now() - chrono::Duration::minutes(mins_ago),
            kind: MessageKind::Message,
        }
    }

    fn make_block(agent_id: &str, content: &str, mins_ago: i64) -> Message {
        Message {
            id: format!("block-{agent_id}-{mins_ago}"),
            agent_id: agent_id.to_string(),
            content: content.to_string(),
            timestamp: Utc::now() - chrono::Duration::minutes(mins_ago),
            kind: MessageKind::Block,
        }
    }

    fn make_claim(agent_id: &str, file_path: &str) -> Claim {
        Claim {
            file_path: file_path.to_string(),
            agent_id: agent_id.to_string(),
            claimed_at: Utc::now(),
        }
    }

    /// Helper to build context with eval_prompt defaults (blocks + proactive awareness).
    fn build_context(
        agents: &[Agent],
        messages: &[Message],
        claims: &[Claim],
        discoveries: &[Message],
        blocks: &[Message],
        self_agent: Option<&str>,
        now: chrono::DateTime<Utc>,
    ) -> String {
        hook_common::build_full_context(
            agents,
            messages,
            claims,
            discoveries,
            self_agent,
            now,
            ContextOptions {
                blocks,
                include_proactive_awareness: true,
                ..Default::default()
            },
        )
    }

    #[test]
    fn test_cold_start_returns_cta() {
        let now = Utc::now();
        let result = build_context(&[], &[], &[], &[], &[], None, now);
        assert_eq!(result, hook_common::CTA_COLD_START);
    }

    #[test]
    fn test_active_agents_with_messages() {
        let now = Utc::now();
        let agents = vec![make_agent("agent-1", None)];
        let messages = vec![make_message("agent-1", "Working on auth", 2)];
        let result = build_context(&agents, &messages, &[], &[], &[], Some("agent-1"), now);

        assert!(result.contains("1 agent(s) active"));
        assert!(result.contains("1 new msg(s)"));
        assert!(result.contains("Working on auth"));
    }

    #[test]
    fn test_blocks_shown_at_top_with_urgency_cta() {
        let now = Utc::now();
        let agents = vec![make_agent("agent-1", None), make_agent("agent-2", None)];
        let messages = vec![make_message("agent-2", "need your file", 1)];
        let blocks = vec![make_block(
            "agent-2",
            "BLOCKED on types.rs — @agent-1 please release it",
            1,
        )];

        let result = build_context(&agents, &messages, &[], &[], &blocks, Some("agent-1"), now);

        // Urgency CTA should be present.
        assert!(result.contains("teammate is BLOCKED waiting for you"));
        // Block should appear in the output.
        assert!(result.contains("BLOCKED:"));
        assert!(result.contains("agent-2"));
    }

    #[test]
    fn test_blocks_not_targeting_self_ignored() {
        let now = Utc::now();
        let agents = vec![make_agent("agent-1", None), make_agent("agent-2", None)];
        let messages = vec![make_message("agent-1", "hello", 1)];
        // Block targets agent-3, not agent-1 (self).
        let blocks = vec![make_block(
            "agent-2",
            "BLOCKED on types.rs — @agent-3 please release it",
            1,
        )];

        let result = build_context(&agents, &messages, &[], &[], &blocks, Some("agent-1"), now);

        // Should NOT contain urgency CTA.
        assert!(!result.contains("teammate is BLOCKED waiting for you"));
        assert!(!result.contains("BLOCKED:"));
    }

    #[test]
    fn test_claims_shown_in_output() {
        let now = Utc::now();
        let agents = vec![make_agent("agent-1", None)];
        let messages = vec![make_message("agent-1", "working", 1)];
        let claims = vec![make_claim("agent-1", "src/auth.rs"), make_claim("agent-2", "src/db.rs")];

        let result = build_context(&agents, &messages, &claims, &[], &[], Some("agent-1"), now);

        assert!(result.contains("claims:"));
        assert!(result.contains("auth.rs"));
        assert!(result.contains("db.rs"));
    }

    #[test]
    fn test_plans_shown_in_output() {
        let now = Utc::now();
        let agents = vec![make_agent("agent-1", Some("Refactoring auth module"))];
        let messages = vec![make_message("agent-1", "starting", 1)];

        let result = build_context(&agents, &messages, &[], &[], &[], Some("agent-1"), now);

        assert!(result.contains("plans:"));
        assert!(result.contains("Refactoring auth module"));
    }

    #[test]
    fn test_discoveries_shown_in_output() {
        let now = Utc::now();
        let agents = vec![make_agent("agent-1", None)];
        let messages = vec![make_message("agent-1", "working", 1)];
        let discoveries = vec![make_message("agent-2", "API endpoint moved to /v2", 3)];

        let result = build_context(&agents, &messages, &[], &discoveries, &[], Some("agent-1"), now);

        assert!(result.contains("discoveries:"));
        assert!(result.contains("API endpoint moved to /v2"));
    }

    #[test]
    fn test_mentions_trigger_mention_cta() {
        let now = Utc::now();
        let agents = vec![make_agent("agent-1", None), make_agent("agent-2", None)];
        let messages = vec![make_message("agent-2", "@agent-1 can you review?", 1)];

        let result = build_context(&agents, &messages, &[], &[], &[], Some("agent-1"), now);

        assert!(result.contains("@mentions"));
        assert!(result.contains("Teammates are waiting"));
    }

    #[test]
    fn test_blocks_appear_before_summary() {
        let now = Utc::now();
        let agents = vec![make_agent("agent-1", None), make_agent("agent-2", None)];
        let messages = vec![make_message("agent-2", "working", 1)];
        let blocks = vec![make_block(
            "agent-2",
            "BLOCKED on types.rs — @agent-1 please release it",
            1,
        )];

        let result = build_context(&agents, &messages, &[], &[], &blocks, Some("agent-1"), now);

        // Block line should appear before the "agent(s) active" summary line.
        let block_pos = result.find("BLOCKED:").expect("should contain BLOCKED:");
        let summary_pos = result.find("agent(s) active").expect("should contain agent(s) active");
        assert!(
            block_pos < summary_pos,
            "BLOCKED line at {block_pos} should appear before summary at {summary_pos}"
        );
    }

    #[test]
    fn test_agent_status_shown_in_summary() {
        let now = Utc::now();
        let agents = vec![Agent {
            id: "agent-1".to_string(),
            status: Some("reviewing auth module".to_string()),
            last_active: Utc::now(),
            last_read: None,
            plan: None,
            plan_updated_at: None,
        }];
        let messages = vec![make_message("agent-1", "working", 1)];

        let result = build_context(&agents, &messages, &[], &[], &[], Some("agent-1"), now);

        assert!(result.contains("reviewing auth module"));
    }

    #[test]
    fn test_proactive_awareness_detects_references() {
        let dir = tempfile::tempdir().unwrap();

        // My file references "types.rs" (teammate's file).
        let my_file = dir.path().join("handler.rs");
        std::fs::write(&my_file, "use crate::types;\nfn handle() {}").unwrap();

        let their_file = dir.path().join("types.rs");
        std::fs::write(&their_file, "pub struct Foo {}").unwrap();

        let claims = vec![
            Claim {
                file_path: my_file.to_str().unwrap().to_string(),
                agent_id: "me".to_string(),
                claimed_at: Utc::now(),
            },
            Claim {
                file_path: their_file.to_str().unwrap().to_string(),
                agent_id: "teammate".to_string(),
                claimed_at: Utc::now(),
            },
        ];

        let warnings = hook_common::build_proactive_awareness(&claims, "me");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("types.rs"));
        assert!(warnings[0].contains("teammate"));
    }

    #[test]
    fn test_proactive_awareness_skips_short_stems() {
        let dir = tempfile::tempdir().unwrap();

        // My file contains "pub mod" which would false-match stem "mod" from mod.rs.
        let my_file = dir.path().join("handler.rs");
        std::fs::write(&my_file, "pub mod auth;\nfn handle() {}").unwrap();

        // Teammate's file is mod.rs — stem "mod" should be skipped.
        let their_file = dir.path().join("mod.rs");
        std::fs::write(&their_file, "pub mod handler;").unwrap();

        let claims = vec![
            Claim {
                file_path: my_file.to_str().unwrap().to_string(),
                agent_id: "me".to_string(),
                claimed_at: Utc::now(),
            },
            Claim {
                file_path: their_file.to_str().unwrap().to_string(),
                agent_id: "teammate".to_string(),
                claimed_at: Utc::now(),
            },
        ];

        let warnings = hook_common::build_proactive_awareness(&claims, "me");
        // Should NOT match on stem "mod" — only filename "mod.rs" would match,
        // and "pub mod" doesn't contain the literal string "mod.rs".
        assert!(
            warnings.is_empty(),
            "expected no warnings for short stem, got: {warnings:?}"
        );
    }

    #[test]
    fn test_proactive_awareness_skips_self_reference() {
        let dir = tempfile::tempdir().unwrap();

        // File claimed by both agents (shouldn't happen, but test the guard).
        let shared_file = dir.path().join("shared.rs");
        std::fs::write(&shared_file, "fn shared() {}").unwrap();

        let claims = vec![
            Claim {
                file_path: shared_file.to_str().unwrap().to_string(),
                agent_id: "me".to_string(),
                claimed_at: Utc::now(),
            },
            Claim {
                file_path: shared_file.to_str().unwrap().to_string(),
                agent_id: "teammate".to_string(),
                claimed_at: Utc::now(),
            },
        ];

        let warnings = hook_common::build_proactive_awareness(&claims, "me");
        // Self-reference guard: my file ends_with the teammate's filename.
        assert!(
            warnings.is_empty(),
            "expected no self-reference warnings, got: {warnings:?}"
        );
    }

    #[test]
    fn test_proactive_awareness_empty_when_solo() {
        let claims = vec![Claim {
            file_path: "src/auth.rs".to_string(),
            agent_id: "me".to_string(),
            claimed_at: Utc::now(),
        }];

        // No teammate claims → no warnings.
        let warnings = hook_common::build_proactive_awareness(&claims, "me");
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_proactive_awareness_uses_long_stems() {
        let dir = tempfile::tempdir().unwrap();

        // My file references "authentication" (the stem of teammate's file).
        let my_file = dir.path().join("handler.rs");
        std::fs::write(&my_file, "use crate::authentication;\nfn handle() {}").unwrap();

        let their_file = dir.path().join("authentication.rs");
        std::fs::write(&their_file, "pub fn login() {}").unwrap();

        let claims = vec![
            Claim {
                file_path: my_file.to_str().unwrap().to_string(),
                agent_id: "me".to_string(),
                claimed_at: Utc::now(),
            },
            Claim {
                file_path: their_file.to_str().unwrap().to_string(),
                agent_id: "teammate".to_string(),
                claimed_at: Utc::now(),
            },
        ];

        let warnings = hook_common::build_proactive_awareness(&claims, "me");
        // "authentication" stem is 14 chars, well above the 4-char threshold.
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("authentication.rs"));
    }
}

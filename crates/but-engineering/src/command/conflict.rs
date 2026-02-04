//! Shared conflict evaluation for pre-edit coordination checks.
//!
//! Used by:
//! - `check` command (read-only API)
//! - `eval pre-tool-use` hook (with side effects handled by the caller)

use std::collections::BTreeSet;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::hook_common;
use crate::db::DbHandle;

/// More messages than usual to cast a wider net for file mentions.
const CONFLICT_SCAN_COUNT: usize = 20;

/// Claims from agents inactive longer than this are ignored.
pub const CLAIM_STALE_MINUTES: i64 = 5;

/// Maximum number of other agents' claimed files to scan for semantic
/// dependencies. Beyond this, the cost isn't worth the advisory benefit.
const MAX_SEMANTIC_SCAN_FILES: usize = 15;

/// Maximum bytes to read from each file when scanning for imports/references.
/// Imports are almost always in the first few KB. Reading bounded prefixes
/// avoids spending time on large generated or binary-ish files.
const MAX_SEMANTIC_READ_BYTES: u64 = 8192;

/// High-level result of a pre-edit check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckDecision {
    Allow,
    Deny,
}

/// Structured reason for a pre-edit decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckReasonCode {
    ClaimedByOther,
    MessageMention,
    SemanticDependency,
    StackDependency,
    NoConflict,
    IdentityMissing,
}

/// Canonical actions that wrappers/agents can use to respond to a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredAction {
    ReadChannel,
    PostCoordinationMessage,
    WaitForRelease,
    RetryCheck,
    ProceedWithEdit,
}

/// Output from shared conflict evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictEvaluation {
    pub file_path: String,
    pub decision: CheckDecision,
    pub reason: Option<String>,
    pub blocking_agents: Vec<String>,
    pub warnings: Vec<String>,
    pub reason_code: CheckReasonCode,
    pub required_actions: Vec<RequiredAction>,
    pub exclusive_self_claim: bool,
    pub lock_owner: Option<String>,
}

/// Normalize the file path to match claim storage.
///
/// If repo root can be discovered, absolute paths are converted to repo-relative.
/// Otherwise, returns the raw path.
pub fn normalize_file_path(raw_file_path: &str) -> String {
    if let Some(root) = hook_common::find_repo_root() {
        hook_common::normalize_path(raw_file_path, &root)
    } else {
        raw_file_path.to_string()
    }
}

/// Evaluate whether editing `raw_file_path` should be allowed and what warnings
/// should be shown. This function is side-effect free.
pub fn evaluate_conflict(
    db: &DbHandle,
    raw_file_path: &str,
    self_agent: Option<&str>,
) -> anyhow::Result<ConflictEvaluation> {
    let file_path = normalize_file_path(raw_file_path);
    let now = Utc::now();
    let mut exclusive_self_claim = false;
    let mut lock_owner: Option<String> = None;

    // --- Layer 1: Claims check (blocking) ---
    let claims = db.get_claims_for_file(&file_path)?;
    if !claims.is_empty() {
        let stale_cutoff = now - chrono::Duration::minutes(CLAIM_STALE_MINUTES);

        let agent_active_cutoff = now - chrono::Duration::minutes(hook_common::ACTIVE_WINDOW_MINUTES);

        let active_claims: Vec<_> = claims
            .iter()
            .filter(|c| {
                // Lease semantics: a claim must be fresh by its own timestamp.
                // Agent recency is an additional guard to ignore orphan claims.
                let claim_fresh = c.claimed_at >= stale_cutoff;
                if !claim_fresh {
                    return false;
                }
                // Fail-safe: if agent lookup fails, treat as active so we avoid
                // accidental false-allows during transient DB errors.
                match db.get_agent(&c.agent_id) {
                    Ok(Some(a)) => a.last_active >= agent_active_cutoff,
                    Ok(None) => true,
                    Err(_) => true,
                }
            })
            .collect();

        if !active_claims.is_empty() {
            if active_claims.len() == 1 {
                let only_owner = active_claims[0].agent_id.clone();
                lock_owner = Some(only_owner.clone());
                exclusive_self_claim = Some(only_owner.as_str()) == self_agent;
            }

            // Deadlock avoidance for shared claims:
            // if we are one of multiple active claimants, pick a deterministic owner
            // (oldest claim, then lexicographic agent id) so one agent can proceed.
            if let Some(self_id) = self_agent {
                let has_self = active_claims.iter().any(|c| c.agent_id == self_id);
                let has_others = active_claims.iter().any(|c| c.agent_id != self_id);
                if has_self && has_others {
                    let owner = active_claims
                        .iter()
                        .min_by(|a, b| {
                            a.claimed_at
                                .cmp(&b.claimed_at)
                                .then_with(|| a.agent_id.cmp(&b.agent_id))
                        })
                        .expect("active_claims is non-empty");
                    lock_owner = Some(owner.agent_id.clone());

                    let contenders = active_claims
                        .iter()
                        .map(|c| c.agent_id.clone())
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>();

                    if owner.agent_id == self_id {
                        // Let the designated owner proceed; later layers may still add advisories.
                        let mut warnings = Vec::new();
                        warnings.push(format!(
                            "but-engineering: {} has multiple active claims ({}). \
                             You currently hold priority by claim order; proceed and ask others to release.",
                            file_path,
                            contenders.join(", ")
                        ));

                        let since = now - chrono::Duration::minutes(hook_common::ACTIVE_WINDOW_MINUTES);
                        let messages = db.query_recent_messages(since, CONFLICT_SCAN_COUNT)?;
                        let mut has_message_warning = false;
                        if let Some(message_warning) = message_warning_for_file(&messages, &file_path, self_agent) {
                            warnings.push(message_warning);
                            has_message_warning = true;
                        }

                        let mut has_semantic_warning = false;
                        if let Some(semantic_warning) = check_semantic_dependencies(db, &file_path, self_agent) {
                            warnings.push(semantic_warning);
                            has_semantic_warning = true;
                        }

                        let reason_code = if has_semantic_warning {
                            CheckReasonCode::SemanticDependency
                        } else if has_message_warning {
                            CheckReasonCode::MessageMention
                        } else {
                            CheckReasonCode::NoConflict
                        };
                        let required_actions = if has_message_warning || has_semantic_warning {
                            vec![RequiredAction::ReadChannel, RequiredAction::ProceedWithEdit]
                        } else {
                            vec![RequiredAction::ProceedWithEdit]
                        };

                        return Ok(ConflictEvaluation {
                            file_path,
                            decision: CheckDecision::Allow,
                            reason: None,
                            blocking_agents: Vec::new(),
                            warnings,
                            reason_code,
                            required_actions,
                            exclusive_self_claim: false,
                            lock_owner,
                        });
                    }

                    let reason = format!(
                        "File {} has multiple active claims ({}). \
                         Ownership is currently assigned to {} (earliest claim). \
                         Post '@{} I need {}' and retry after release.",
                        file_path,
                        contenders.join(", "),
                        owner.agent_id,
                        owner.agent_id,
                        file_path
                    );

                    return Ok(ConflictEvaluation {
                        file_path,
                        decision: CheckDecision::Deny,
                        reason: Some(reason),
                        blocking_agents: vec![owner.agent_id.clone()],
                        warnings: Vec::new(),
                        reason_code: CheckReasonCode::ClaimedByOther,
                        required_actions: vec![
                            RequiredAction::PostCoordinationMessage,
                            RequiredAction::WaitForRelease,
                            RequiredAction::RetryCheck,
                        ],
                        exclusive_self_claim: false,
                        lock_owner,
                    });
                }
            }

            let blocking_agents = active_claims
                .iter()
                .filter(|c| Some(c.agent_id.as_str()) != self_agent)
                .map(|c| c.agent_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();

            if !blocking_agents.is_empty() {
                let first_agent = blocking_agents.first().cloned().unwrap_or_default();
                lock_owner = Some(first_agent.clone());
                let reason = format!(
                    "File {} is claimed by {}. Post '@{} I need {}' to ask them to release it, then retry.",
                    file_path,
                    blocking_agents.join(", "),
                    first_agent,
                    file_path
                );

                return Ok(ConflictEvaluation {
                    file_path,
                    decision: CheckDecision::Deny,
                    reason: Some(reason),
                    blocking_agents,
                    warnings: Vec::new(),
                    reason_code: CheckReasonCode::ClaimedByOther,
                    required_actions: vec![
                        RequiredAction::PostCoordinationMessage,
                        RequiredAction::WaitForRelease,
                        RequiredAction::RetryCheck,
                    ],
                    exclusive_self_claim: false,
                    lock_owner,
                });
            }
        }
    }

    // --- Layer 2: Message check (advisory) ---
    let since = now - chrono::Duration::minutes(hook_common::ACTIVE_WINDOW_MINUTES);
    let messages = db.query_recent_messages(since, CONFLICT_SCAN_COUNT)?;
    let mut warnings = Vec::new();
    let mut has_message_warning = false;

    if let Some(message_warning) = message_warning_for_file(&messages, &file_path, self_agent) {
        warnings.push(message_warning);
        has_message_warning = true;
    }

    // --- Layer 3: Semantic conflict detection (advisory) ---
    let mut has_semantic_warning = false;
    if let Some(semantic_warning) = check_semantic_dependencies(db, &file_path, self_agent) {
        warnings.push(semantic_warning);
        has_semantic_warning = true;
    }

    let reason_code = if has_semantic_warning {
        CheckReasonCode::SemanticDependency
    } else if has_message_warning {
        CheckReasonCode::MessageMention
    } else {
        CheckReasonCode::NoConflict
    };

    let required_actions = if has_message_warning || has_semantic_warning {
        vec![RequiredAction::ReadChannel, RequiredAction::ProceedWithEdit]
    } else {
        vec![RequiredAction::ProceedWithEdit]
    };

    Ok(ConflictEvaluation {
        file_path,
        decision: CheckDecision::Allow,
        reason: None,
        blocking_agents: Vec::new(),
        warnings,
        reason_code,
        required_actions,
        exclusive_self_claim,
        lock_owner,
    })
}

fn message_warning_for_file(
    messages: &[crate::types::Message],
    file_path: &str,
    self_agent: Option<&str>,
) -> Option<String> {
    if messages.is_empty() {
        return None;
    }

    let mut matching = hook_common::messages_mentioning_path(messages, file_path);

    // If no full-path match, try filename-only for partial references.
    if matching.is_empty()
        && let Some(filename) = std::path::Path::new(file_path).file_name().and_then(|f| f.to_str())
        && !filename.is_empty()
    {
        matching = hook_common::messages_mentioning_path(messages, filename);
    }

    let agents = matching
        .iter()
        .map(|m| m.agent_id.as_str())
        .filter(|agent| Some(*agent) != self_agent)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    if agents.is_empty() {
        return None;
    }

    Some(format!(
        "but-engineering: file {} was mentioned by {} in the channel. \
         Check for potential conflict before editing.",
        file_path,
        agents.join(", ")
    ))
}

/// Layer 3: Semantic conflict detection.
///
/// Check if the file being edited is referenced/imported by files claimed by
/// other active agents.
fn check_semantic_dependencies(db: &DbHandle, file_path: &str, self_agent: Option<&str>) -> Option<String> {
    let filename = std::path::Path::new(file_path).file_name().and_then(|f| f.to_str())?;

    // Also get the stem (e.g., "types" from "types.rs") for import matching.
    // Skip stems that are too short or are common filenames to avoid false positives.
    let stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    let use_stem = stem.len() >= 4 && !matches!(stem, "mod" | "lib" | "main" | "test" | "tests");

    // Get all active claims from other agents.
    let now = Utc::now();
    let active_since = now - chrono::Duration::minutes(CLAIM_STALE_MINUTES);
    let all_claims = db.list_claims(Some(active_since)).ok()?;

    let other_claims: Vec<_> = all_claims
        .iter()
        .filter(|c| {
            if let Some(me) = self_agent {
                c.agent_id != me
            } else {
                true
            }
        })
        .collect();

    if other_claims.is_empty() {
        return None;
    }

    // Bail out if there are too many files to scan.
    if other_claims.len() > MAX_SEMANTIC_SCAN_FILES {
        return None;
    }

    let mut dependents: Vec<(&str, &str)> = Vec::new(); // (agent_id, their_file)

    for claim in &other_claims {
        let content = match read_file_prefix(&claim.file_path, MAX_SEMANTIC_READ_BYTES) {
            Some(c) => c,
            None => continue,
        };

        if content.contains(filename) || (use_stem && content.contains(stem)) || content.contains(file_path) {
            dependents.push((&claim.agent_id, &claim.file_path));
        }
    }

    if dependents.is_empty() {
        return None;
    }

    let details: Vec<String> = dependents
        .iter()
        .map(|(agent, file)| format!("{agent}'s {}", hook_common::short_file_name(file)))
        .collect();

    Some(format!(
        "but-engineering: {} is referenced by {}. \
         Coordinate changes to shared interfaces.",
        filename,
        details.join(", ")
    ))
}

/// Read at most `max_bytes` from a file.
fn read_file_prefix(path: &str, max_bytes: u64) -> Option<String> {
    use std::io::Read;

    let file = std::fs::File::open(path).ok()?;
    let mut reader = file.take(max_bytes);
    let mut buf = Vec::with_capacity(max_bytes as usize);
    reader.read_to_end(&mut buf).ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tempfile::TempDir;

    use super::*;
    use crate::command;

    fn create_test_db() -> (TempDir, DbHandle) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db = DbHandle::new_at_path(&db_path).unwrap();
        (dir, db)
    }

    #[test]
    fn allow_when_no_claims() {
        let (_dir, db) = create_test_db();
        let result = evaluate_conflict(&db, "src/auth.rs", Some("agent-1")).unwrap();

        assert_eq!(result.decision, CheckDecision::Allow);
        assert_eq!(result.reason, None);
        assert!(result.blocking_agents.is_empty());
        assert_eq!(result.reason_code, CheckReasonCode::NoConflict);
        assert_eq!(result.required_actions, vec![RequiredAction::ProceedWithEdit]);
    }

    #[test]
    fn deny_when_other_active_claims_file() {
        let (_dir, db) = create_test_db();

        command::claim::execute(&db, vec!["src/auth.rs".to_string()], "agent-2".to_string()).unwrap();
        let result = evaluate_conflict(&db, "src/auth.rs", Some("agent-1")).unwrap();

        assert_eq!(result.decision, CheckDecision::Deny);
        assert!(result.reason.unwrap().contains("claimed by agent-2"));
        assert_eq!(result.blocking_agents, vec!["agent-2".to_string()]);
        assert_eq!(result.reason_code, CheckReasonCode::ClaimedByOther);
        assert_eq!(
            result.required_actions,
            vec![
                RequiredAction::PostCoordinationMessage,
                RequiredAction::WaitForRelease,
                RequiredAction::RetryCheck
            ]
        );
    }

    #[test]
    fn allow_when_only_self_claim_exists() {
        let (_dir, db) = create_test_db();

        command::claim::execute(&db, vec!["src/auth.rs".to_string()], "agent-1".to_string()).unwrap();
        let result = evaluate_conflict(&db, "src/auth.rs", Some("agent-1")).unwrap();

        assert_eq!(result.decision, CheckDecision::Allow);
        assert!(result.blocking_agents.is_empty());
        assert_eq!(result.reason_code, CheckReasonCode::NoConflict);
        assert!(result.exclusive_self_claim);
        assert_eq!(result.lock_owner.as_deref(), Some("agent-1"));
    }

    #[test]
    fn stale_claim_is_ignored() {
        let (_dir, db) = create_test_db();

        command::claim::execute(&db, vec!["src/auth.rs".to_string()], "agent-2".to_string()).unwrap();

        let old = Utc::now() - chrono::Duration::minutes(20);
        db.conn()
            .execute(
                "UPDATE agents SET last_active = ?1 WHERE id = ?2",
                rusqlite::params![old, "agent-2"],
            )
            .unwrap();
        db.conn()
            .execute(
                "UPDATE claims SET claimed_at = ?1 WHERE file_path = ?2 AND agent_id = ?3",
                rusqlite::params![old, "src/auth.rs", "agent-2"],
            )
            .unwrap();

        let result = evaluate_conflict(&db, "src/auth.rs", Some("agent-1")).unwrap();
        assert_eq!(result.decision, CheckDecision::Allow);
        assert_eq!(result.reason_code, CheckReasonCode::NoConflict);
    }

    #[test]
    fn stale_claim_ignored_even_when_agent_is_active() {
        let (_dir, db) = create_test_db();

        command::claim::execute(&db, vec!["src/auth.rs".to_string()], "agent-2".to_string()).unwrap();

        // Keep agent active, but age the claim itself beyond lease timeout.
        let old_claim = Utc::now() - chrono::Duration::minutes(CLAIM_STALE_MINUTES + 1);
        db.conn()
            .execute(
                "UPDATE claims SET claimed_at = ?1 WHERE file_path = ?2 AND agent_id = ?3",
                rusqlite::params![old_claim, "src/auth.rs", "agent-2"],
            )
            .unwrap();

        let result = evaluate_conflict(&db, "src/auth.rs", Some("agent-1")).unwrap();
        assert_eq!(result.decision, CheckDecision::Allow);
        assert_eq!(result.reason_code, CheckReasonCode::NoConflict);
    }

    #[test]
    fn warnings_include_message_mentions() {
        let (_dir, db) = create_test_db();

        command::post::execute(
            &db,
            "I am editing src/auth.rs right now".to_string(),
            "agent-2".to_string(),
        )
        .unwrap();

        let result = evaluate_conflict(&db, "src/auth.rs", Some("agent-1")).unwrap();
        assert_eq!(result.decision, CheckDecision::Allow);
        assert!(result.warnings.iter().any(|w| w.contains("mentioned by agent-2")));
    }

    #[test]
    fn self_only_mentions_do_not_emit_advisory_warning() {
        let (_dir, db) = create_test_db();

        command::post::execute(
            &db,
            "I am editing src/auth.rs right now".to_string(),
            "agent-1".to_string(),
        )
        .unwrap();

        let result = evaluate_conflict(&db, "src/auth.rs", Some("agent-1")).unwrap();
        assert_eq!(result.decision, CheckDecision::Allow);
        assert!(result.warnings.is_empty());
        assert_eq!(result.reason_code, CheckReasonCode::NoConflict);
    }

    #[test]
    fn mixed_mentions_exclude_self_from_advisory_agents() {
        let (_dir, db) = create_test_db();

        command::post::execute(
            &db,
            "I am editing src/auth.rs right now".to_string(),
            "agent-1".to_string(),
        )
        .unwrap();
        command::post::execute(&db, "Also touching src/auth.rs".to_string(), "agent-2".to_string()).unwrap();

        let result = evaluate_conflict(&db, "src/auth.rs", Some("agent-1")).unwrap();
        assert_eq!(result.decision, CheckDecision::Allow);
        let warning_text = result.warnings.join("\n");
        assert!(warning_text.contains("agent-2"));
        assert!(!warning_text.contains("agent-1"));
    }

    #[test]
    fn warnings_include_semantic_dependencies() {
        let (_db_dir, db) = create_test_db();
        let files_dir = tempfile::tempdir().unwrap();

        let target = files_dir.path().join("types.rs");
        std::fs::write(&target, "pub struct Foo {}").unwrap();

        let dependent = files_dir.path().join("handler.rs");
        std::fs::write(&dependent, "use crate::types;\nfn handle() {}").unwrap();

        let now = Utc::now();
        db.upsert_agent("agent-2", now).unwrap();
        db.claim_files(&[dependent.to_str().unwrap()], "agent-2", now).unwrap();

        let result = evaluate_conflict(&db, target.to_str().unwrap(), Some("agent-1")).unwrap();
        assert_eq!(result.decision, CheckDecision::Allow);
        assert!(result.warnings.iter().any(|w| w.contains("types.rs")));
    }

    #[test]
    fn contended_claims_allow_single_owner_and_deny_other() {
        let (_dir, db) = create_test_db();

        command::claim::execute(&db, vec!["src/auth.rs".to_string()], "agent-1".to_string()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        command::claim::execute(&db, vec!["src/auth.rs".to_string()], "agent-2".to_string()).unwrap();

        let owner = evaluate_conflict(&db, "src/auth.rs", Some("agent-1")).unwrap();
        assert_eq!(owner.decision, CheckDecision::Allow);
        assert!(owner.blocking_agents.is_empty());
        assert!(owner.warnings.iter().any(|w| w.contains("multiple active claims")));
        assert!(!owner.exclusive_self_claim);
        assert_eq!(owner.lock_owner.as_deref(), Some("agent-1"));

        let loser = evaluate_conflict(&db, "src/auth.rs", Some("agent-2")).unwrap();
        assert_eq!(loser.decision, CheckDecision::Deny);
        assert_eq!(loser.blocking_agents, vec!["agent-1".to_string()]);
        assert!(!loser.exclusive_self_claim);
        assert_eq!(loser.lock_owner.as_deref(), Some("agent-1"));
        assert!(
            loser
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("Ownership is currently assigned to agent-1")
        );
    }

    #[test]
    fn contended_claims_tiebreak_by_agent_id_when_same_timestamp() {
        let (_dir, db) = create_test_db();

        command::claim::execute(&db, vec!["src/auth.rs".to_string()], "agent-b".to_string()).unwrap();
        command::claim::execute(&db, vec!["src/auth.rs".to_string()], "agent-a".to_string()).unwrap();

        let same_time = Utc::now();
        db.conn()
            .execute(
                "UPDATE claims SET claimed_at = ?1 WHERE file_path = ?2",
                rusqlite::params![same_time, "src/auth.rs"],
            )
            .unwrap();

        let a = evaluate_conflict(&db, "src/auth.rs", Some("agent-a")).unwrap();
        assert_eq!(a.decision, CheckDecision::Allow);
        assert_eq!(a.lock_owner.as_deref(), Some("agent-a"));

        let b = evaluate_conflict(&db, "src/auth.rs", Some("agent-b")).unwrap();
        assert_eq!(b.decision, CheckDecision::Deny);
        assert_eq!(b.blocking_agents, vec!["agent-a".to_string()]);
        assert_eq!(b.lock_owner.as_deref(), Some("agent-a"));
    }

    #[test]
    fn ownership_context_classification_clear_advisory_exclusive_blocked() {
        let (_dir, db) = create_test_db();

        // clear
        let clear = evaluate_conflict(&db, "src/clear.rs", Some("agent-1")).unwrap();
        assert_eq!(clear.decision, CheckDecision::Allow);
        assert!(clear.warnings.is_empty());
        assert!(!clear.exclusive_self_claim);
        assert!(clear.lock_owner.is_none());

        // advisory
        command::post::execute(&db, "Working in src/advisory.rs".to_string(), "agent-2".to_string()).unwrap();
        let advisory = evaluate_conflict(&db, "src/advisory.rs", Some("agent-1")).unwrap();
        assert_eq!(advisory.decision, CheckDecision::Allow);
        assert!(!advisory.warnings.is_empty());
        assert!(!advisory.exclusive_self_claim);
        assert!(advisory.lock_owner.is_none());

        // exclusive_owner
        command::claim::execute(&db, vec!["src/exclusive.rs".to_string()], "agent-1".to_string()).unwrap();
        let exclusive = evaluate_conflict(&db, "src/exclusive.rs", Some("agent-1")).unwrap();
        assert_eq!(exclusive.decision, CheckDecision::Allow);
        assert!(exclusive.exclusive_self_claim);
        assert_eq!(exclusive.lock_owner.as_deref(), Some("agent-1"));

        // blocked
        command::claim::execute(&db, vec!["src/blocked.rs".to_string()], "agent-2".to_string()).unwrap();
        let blocked = evaluate_conflict(&db, "src/blocked.rs", Some("agent-1")).unwrap();
        assert_eq!(blocked.decision, CheckDecision::Deny);
        assert!(!blocked.exclusive_self_claim);
        assert_eq!(blocked.lock_owner.as_deref(), Some("agent-2"));
    }
}

//! Stack dependency evaluation for `but-engineering check --include-stack`.
//!
//! This module is advisory-first: it never blocks edits. It detects when the
//! intended branch depends on lower branches that appear to be owned by other
//! active agents, then returns structured hints for coordination.

use std::collections::{BTreeMap, BTreeSet};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::conflict::CheckReasonCode;
use crate::command::hook_common;
use crate::db::DbHandle;

const BUT_STATUS_TIMEOUT_MS: u64 = 1_200;
const MENTION_SCAN_COUNT: usize = 50;

/// Source of dependency signal for coordination hints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencySource {
    None,
    ButStatus,
    Semantic,
    Combined,
}

/// Additive coordination hints returned by `check`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinationHints {
    pub stack_dependency_detected: bool,
    pub dependency_source: DependencySource,
    pub intent_branch: Option<String>,
    pub depends_on_branches: Vec<String>,
    pub dependent_agents: Vec<String>,
    pub suggested_but_commands: Vec<String>,
    pub stack_context_error: Option<String>,
}

impl CoordinationHints {
    pub fn empty() -> Self {
        Self {
            stack_dependency_detected: false,
            dependency_source: DependencySource::None,
            intent_branch: None,
            depends_on_branches: Vec::new(),
            dependent_agents: Vec::new(),
            suggested_but_commands: Vec::new(),
            stack_context_error: None,
        }
    }
}

#[derive(Debug, Clone)]
struct ParsedStack {
    branch_order: Vec<String>,
    branch_files: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ButStatus {
    #[serde(default)]
    stacks: Vec<ButStack>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ButStack {
    #[serde(default)]
    branches: Vec<ButBranch>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ButBranch {
    name: String,
    #[serde(default)]
    commits: Vec<ButCommit>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ButCommit {
    #[serde(default)]
    changes: Option<Vec<ButChange>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ButChange {
    file_path: String,
}

/// Evaluate stack dependency hints.
///
/// `status_json_override` is used for deterministic tests. In normal usage it
/// should be `None` so the evaluator shells out to `but status -f --json`.
pub fn evaluate(
    db: &DbHandle,
    file_path: &str,
    self_agent: Option<&str>,
    intent_branch_arg: Option<&str>,
    conflict_reason_code: CheckReasonCode,
    status_json_override: Option<&str>,
) -> CoordinationHints {
    let semantic_signal = conflict_reason_code == CheckReasonCode::SemanticDependency;

    let mut hints = CoordinationHints::empty();

    let status_json = match status_json_override {
        Some(value) => value.to_string(),
        None => match load_status_json() {
            Ok(value) => value,
            Err(error) => {
                hints.stack_context_error = Some(error.to_string());
                hints.dependency_source = if semantic_signal {
                    DependencySource::Semantic
                } else {
                    DependencySource::None
                };
                return hints;
            }
        },
    };

    let stacks = match parse_stacks_from_status(&status_json) {
        Ok(v) => v,
        Err(error) => {
            hints.stack_context_error = Some(error.to_string());
            hints.dependency_source = if semantic_signal {
                DependencySource::Semantic
            } else {
                DependencySource::None
            };
            return hints;
        }
    };

    let intent_branch = resolve_intent_branch(file_path, intent_branch_arg, &stacks);
    let depends_on_branches = intent_branch
        .as_deref()
        .map(|branch| depends_on_branches(branch, &stacks))
        .unwrap_or_default();
    let dependent_agents = detect_dependent_agents(db, &depends_on_branches, &stacks, self_agent);

    let stack_dependency_detected = !depends_on_branches.is_empty() && !dependent_agents.is_empty();
    let dependency_source = match (stack_dependency_detected, semantic_signal) {
        (true, true) => DependencySource::Combined,
        (true, false) => DependencySource::ButStatus,
        (false, true) => DependencySource::Semantic,
        (false, false) => DependencySource::None,
    };

    let suggested_but_commands = if stack_dependency_detected {
        let mut commands = vec!["but status --json".to_string()];
        commands.push(format!(
            "but-engineering post \"Coordinating dependency on {}\" --agent-id <id>",
            depends_on_branches.join(", ")
        ));
        commands.push(format!("but branch new <child> -a {}", depends_on_branches[0]));
        commands.push("but commit <branch> -m \"<message>\" --json --status-after".to_string());
        commands
    } else {
        Vec::new()
    };

    hints.stack_dependency_detected = stack_dependency_detected;
    hints.dependency_source = dependency_source;
    hints.intent_branch = intent_branch;
    hints.depends_on_branches = depends_on_branches;
    hints.dependent_agents = dependent_agents;
    hints.suggested_but_commands = suggested_but_commands;
    hints
}

fn load_status_json() -> anyhow::Result<String> {
    let repo_root = hook_common::find_repo_root().ok_or_else(|| anyhow::anyhow!("not in a git repository"))?;
    let but_bin = std::env::var("BUT_ENGINEERING_BUT_BIN").unwrap_or_else(|_| "but".to_string());

    let mut child = Command::new(&but_bin)
        .arg("-C")
        .arg(&repo_root)
        .arg("status")
        .arg("-f")
        .arg("--json")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("failed to launch `{but_bin} status --json`: {e}"))?;

    let deadline = Instant::now() + Duration::from_millis(BUT_STATUS_TIMEOUT_MS);
    let mut timed_out = false;

    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    timed_out = true;
                    let _ = child.kill();
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(anyhow::anyhow!("failed while waiting for `{but_bin}`: {e}")),
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| anyhow::anyhow!("failed to collect `{but_bin}` output: {e}"))?;

    if timed_out {
        return Err(anyhow::anyhow!("`{but_bin} status -f --json` timed out"));
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            return Err(anyhow::anyhow!("`{but_bin} status -f --json` failed"));
        }
        return Err(anyhow::anyhow!("`{but_bin} status -f --json` failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_stacks_from_status(json: &str) -> anyhow::Result<Vec<ParsedStack>> {
    let parsed: ButStatus =
        serde_json::from_str(json).map_err(|e| anyhow::anyhow!("failed to parse `but status` JSON: {e}"))?;

    let mut out = Vec::new();
    for stack in parsed.stacks {
        let mut branch_order = Vec::new();
        let mut branch_files: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        for branch in stack.branches {
            branch_order.push(branch.name.clone());
            let mut files = BTreeSet::new();
            for commit in branch.commits {
                if let Some(changes) = commit.changes {
                    for change in changes {
                        files.insert(normalize_file_key(&change.file_path));
                    }
                }
            }
            branch_files.insert(branch.name, files);
        }

        out.push(ParsedStack {
            branch_order,
            branch_files,
        });
    }

    Ok(out)
}

fn normalize_file_key(file_path: &str) -> String {
    file_path.trim().trim_start_matches("./").to_string()
}

fn resolve_intent_branch(file_path: &str, intent_branch_arg: Option<&str>, stacks: &[ParsedStack]) -> Option<String> {
    if let Some(intent) = intent_branch_arg {
        let trimmed = intent.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let file_key = normalize_file_key(file_path);
    let mut matches = Vec::new();
    for stack in stacks {
        for (branch, files) in &stack.branch_files {
            if files.contains(&file_key) {
                matches.push(branch.clone());
            }
        }
    }

    if matches.len() == 1 {
        return matches.pop();
    }

    // Fallback: infer branch by semantic overlap between file path tokens and
    // branch-name tokens. This helps when `but status` has unassigned changes.
    let file_tokens = tokens_for_overlap(file_path);
    if file_tokens.is_empty() {
        return None;
    }

    let mut best_score = 0usize;
    let mut best_branch: Option<String> = None;
    let mut tie = false;
    for stack in stacks {
        for branch in &stack.branch_order {
            let branch_tokens = tokens_for_overlap(branch);
            if branch_tokens.is_empty() {
                continue;
            }
            let score = branch_tokens.intersection(&file_tokens).count();
            if score == 0 {
                continue;
            }
            if score > best_score {
                best_score = score;
                best_branch = Some(branch.clone());
                tie = false;
            } else if score == best_score {
                tie = true;
            }
        }
    }

    if best_score > 0 && !tie { best_branch } else { None }
}

fn tokens_for_overlap(input: &str) -> BTreeSet<String> {
    input
        .split(|c: char| !c.is_ascii_alphanumeric())
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| token.len() >= 3)
        .collect()
}

fn depends_on_branches(intent_branch: &str, stacks: &[ParsedStack]) -> Vec<String> {
    for stack in stacks {
        if let Some(index) = stack.branch_order.iter().position(|b| b == intent_branch) {
            return stack.branch_order.iter().skip(index + 1).cloned().collect();
        }
    }
    Vec::new()
}

fn detect_dependent_agents(
    db: &DbHandle,
    depends_on_branches: &[String],
    stacks: &[ParsedStack],
    self_agent: Option<&str>,
) -> Vec<String> {
    if depends_on_branches.is_empty() {
        return Vec::new();
    }

    let now = Utc::now();
    let active_since = now - chrono::Duration::minutes(hook_common::ACTIVE_WINDOW_MINUTES);

    let mut dependent_files = BTreeSet::new();
    for branch_name in depends_on_branches {
        for stack in stacks {
            if let Some(files) = stack.branch_files.get(branch_name) {
                dependent_files.extend(files.iter().cloned());
            }
        }
    }

    let mut agents = BTreeSet::new();

    let claims = db.list_claims(Some(active_since)).unwrap_or_default();
    for claim in &claims {
        if self_agent.is_some_and(|me| claim.agent_id == me) {
            continue;
        }
        let claim_path = normalize_file_key(&claim.file_path);
        if dependent_files.contains(&claim_path) {
            agents.insert(claim.agent_id.clone());
        }
    }

    let lower_branches = depends_on_branches.iter().map(|b| b.to_lowercase()).collect::<Vec<_>>();

    let messages = db
        .query_recent_messages(active_since, MENTION_SCAN_COUNT)
        .unwrap_or_default();
    for message in &messages {
        if self_agent.is_some_and(|me| message.agent_id == me) {
            continue;
        }
        let content = message.content.to_lowercase();
        if lower_branches.iter().any(|name| content.contains(name)) {
            agents.insert(message.agent_id.clone());
        }
    }

    let agents_state = db.list_agents(Some(active_since)).unwrap_or_default();
    for agent in &agents_state {
        if self_agent.is_some_and(|me| agent.id == me) {
            continue;
        }
        if let Some(plan) = &agent.plan {
            let plan_lower = plan.to_lowercase();
            if lower_branches.iter().any(|name| plan_lower.contains(name)) {
                agents.insert(agent.id.clone());
            }
        }
    }

    agents.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::command;
    use crate::db::DbHandle;

    fn create_test_db() -> (TempDir, DbHandle) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let db = DbHandle::new_at_path(&db_path).unwrap();
        (dir, db)
    }

    #[test]
    fn parse_stack_status_extracts_branches_and_files() {
        let json = r#"
        {
          "stacks": [
            {
              "branches": [
                {
                  "name": "profile-ui",
                  "commits": [{ "changes": [{ "filePath": "src/profile.rs" }] }]
                },
                {
                  "name": "auth-base",
                  "commits": [{ "changes": [{ "filePath": "src/auth.rs" }] }]
                }
              ]
            }
          ]
        }
        "#;
        let stacks = parse_stacks_from_status(json).unwrap();
        assert_eq!(stacks.len(), 1);
        assert_eq!(
            stacks[0].branch_order,
            vec!["profile-ui".to_string(), "auth-base".to_string()]
        );
        assert!(stacks[0].branch_files["profile-ui"].contains("src/profile.rs"));
        assert!(stacks[0].branch_files["auth-base"].contains("src/auth.rs"));
    }

    #[test]
    fn branch_dependency_extraction_from_stack_order() {
        let stacks = vec![ParsedStack {
            branch_order: vec!["child".to_string(), "middle".to_string(), "base".to_string()],
            branch_files: BTreeMap::new(),
        }];
        let deps = depends_on_branches("child", &stacks);
        assert_eq!(deps, vec!["middle".to_string(), "base".to_string()]);
    }

    #[test]
    fn intent_branch_arg_takes_precedence_over_inferred() {
        let mut files = BTreeMap::new();
        files.insert("profile-ui".to_string(), BTreeSet::from(["src/profile.rs".to_string()]));
        files.insert("auth-base".to_string(), BTreeSet::from(["src/auth.rs".to_string()]));

        let stacks = vec![ParsedStack {
            branch_order: vec!["profile-ui".to_string(), "auth-base".to_string()],
            branch_files: files,
        }];

        let resolved = resolve_intent_branch("src/profile.rs", Some("auth-base"), &stacks);
        assert_eq!(resolved.as_deref(), Some("auth-base"));
    }

    #[test]
    fn intent_branch_can_be_inferred_from_file_and_branch_name_overlap() {
        let stacks = vec![
            ParsedStack {
                branch_order: vec!["te-branch-1".to_string()],
                branch_files: BTreeMap::new(),
            },
            ParsedStack {
                branch_order: vec!["profile-ui".to_string(), "auth-base".to_string()],
                branch_files: BTreeMap::new(),
            },
        ];

        let resolved = resolve_intent_branch("src/profile/handler.rs", None, &stacks);
        assert_eq!(resolved.as_deref(), Some("profile-ui"));
    }

    #[test]
    fn evaluate_uses_semantic_source_when_status_unavailable() {
        let (_dir, db) = create_test_db();
        let hints = evaluate(
            &db,
            "src/config.rs",
            Some("agent-1"),
            None,
            CheckReasonCode::SemanticDependency,
            Some("this-is-not-json"),
        );
        assert!(!hints.stack_dependency_detected);
        assert_eq!(hints.dependency_source, DependencySource::Semantic);
        assert!(hints.stack_context_error.is_some());
    }

    #[test]
    fn detects_dependency_agents_from_claims_and_mentions() {
        let (_dir, db) = create_test_db();
        command::claim::execute(&db, vec!["src/auth.rs".to_string()], "peer-a".to_string()).unwrap();
        command::post::execute(&db, "I am moving auth-base internals".to_string(), "peer-a".to_string()).unwrap();

        let json = r#"
        {
          "stacks": [
            {
              "branches": [
                {
                  "name": "profile-ui",
                  "commits": [{ "changes": [{ "filePath": "src/profile.rs" }] }]
                },
                {
                  "name": "auth-base",
                  "commits": [{ "changes": [{ "filePath": "src/auth.rs" }] }]
                }
              ]
            }
          ]
        }
        "#;

        let hints = evaluate(
            &db,
            "src/profile.rs",
            Some("agent-self"),
            Some("profile-ui"),
            CheckReasonCode::NoConflict,
            Some(json),
        );
        assert!(hints.stack_dependency_detected);
        assert_eq!(hints.dependency_source, DependencySource::ButStatus);
        assert_eq!(hints.depends_on_branches, vec!["auth-base".to_string()]);
        assert_eq!(hints.dependent_agents, vec!["peer-a".to_string()]);
        assert!(!hints.suggested_but_commands.is_empty());
    }
}

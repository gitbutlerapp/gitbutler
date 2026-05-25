use std::{
    fmt,
    io::Read as _,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::{
    agent::Agent,
    capture::{prepare_transcript, record_prepared_transcript},
    capture_lock::with_capture_lock,
    gitmeta::{
        RelatedTarget, SessionRecords, SessionTimeline, find_related_sessions_limited,
        get_session_records, get_session_timeline_outline, sync_metadata,
    },
    skim::{self, SkimReport},
};

const DEFAULT_TIMELINE_LIMIT: usize = 20;
const DEFAULT_RECORD_LIMIT: usize = 20;

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Capture an agent transcript from hook input.
    #[clap(hide = true)]
    Hook {
        #[clap(long, value_enum)]
        agent: Option<Agent>,
    },
    /// Show a session, or one turn in detail.
    #[clap(name = "show")]
    Show {
        /// Session key from `skim --json`.
        #[clap(value_name = "SESSION", value_parser = non_empty_value)]
        session_key: String,
        /// Show detailed records for this turn key.
        #[clap(long, value_name = "TURN", value_parser = non_empty_value)]
        turn: Option<String>,
        /// Maximum turns or turn records to return.
        #[clap(long)]
        limit: Option<usize>,
    },
    /// Skim prior agent work.
    #[clap(name = "skim")]
    Skim {
        #[clap(value_enum)]
        target: Option<RelatedSessionTarget>,
        /// Branch, review, or change value to skim.
        #[clap(value_name = "VALUE", value_parser = non_empty_value)]
        value: Option<String>,
    },
    /// Sync GitMeta metadata.
    #[clap(hide = true)]
    Sync,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum CommandOutput {
    Message { message: String },
    Timeline(SessionTimeline),
    Records(SessionRecords),
    Skim(SkimReport),
}

impl fmt::Display for CommandOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandOutput::Message { message } => f.write_str(message),
            CommandOutput::Timeline(timeline) => {
                writeln!(
                    f,
                    "{} of {} turns for {}",
                    timeline.coverage.showing_turns,
                    timeline.coverage.total_turns,
                    timeline.session_key
                )?;
                for turn in &timeline.turns {
                    writeln!(
                        f,
                        "#{} {} {} records={} env={}",
                        turn.turn_index,
                        turn.turn_key,
                        turn.captured_at,
                        turn.record_count,
                        turn.environment_snapshot_status
                    )?;
                    if let Some(preview) = turn.latest_user_preview.as_ref() {
                        writeln!(f, "  user: {}", display_preview(&preview.text))?;
                    }
                    if let Some(preview) = turn.latest_assistant_preview.as_ref() {
                        writeln!(f, "  assistant: {}", display_preview(&preview.text))?;
                    }
                }
                Ok(())
            }
            CommandOutput::Records(records) => {
                writeln!(
                    f,
                    "{} of {} records for {} turn {}",
                    records.coverage.showing_records,
                    records.coverage.total_records,
                    records.session_key,
                    records.turn_key
                )?;
                for record in &records.records {
                    let timestamp = record.timestamp.as_deref().unwrap_or("unknown");
                    let kind = record.kind.as_deref().unwrap_or("unknown");
                    let label = record
                        .role
                        .as_deref()
                        .or(record.tool_name.as_deref())
                        .unwrap_or("-");
                    let preview = record
                        .text
                        .as_deref()
                        .map(display_preview)
                        .filter(|preview| !preview.is_empty());
                    if let Some(preview) = preview {
                        writeln!(
                            f,
                            "#{} {} {} {} {}",
                            record.turn_record_index, timestamp, kind, label, preview
                        )?;
                    } else {
                        writeln!(
                            f,
                            "#{} {} {} {}",
                            record.turn_record_index, timestamp, kind, label
                        )?;
                    }
                }
                Ok(())
            }
            CommandOutput::Skim(report) => report.fmt(f),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum RelatedSessionTarget {
    /// Branch ref target.
    Branch,
    /// GitButler review target, including pull request / merge request style reviews.
    Review,
    /// GitButler change id target.
    Change,
}

impl RelatedSessionTarget {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RelatedSessionTarget::Branch => "branch",
            RelatedSessionTarget::Review => "review",
            RelatedSessionTarget::Change => "change",
        }
    }

    fn related_target<'a>(self, target_key: &'a str) -> RelatedTarget<'a> {
        match self {
            RelatedSessionTarget::Branch => RelatedTarget::Branch(target_key),
            RelatedSessionTarget::Review => RelatedTarget::Review(target_key),
            RelatedSessionTarget::Change => RelatedTarget::Change(target_key),
        }
    }
}

pub fn run_from_dir(dir: &Path, command: Command) -> anyhow::Result<impl Serialize + fmt::Display> {
    match command {
        Command::Hook { agent } => {
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .context("failed to read agent hook input")?;
            if let Some(sync_dir) = run_hook(dir, agent, &input)? {
                spawn_agentlog_sync(&sync_dir);
            }
            Ok(CommandOutput::Message {
                message: String::new(),
            })
        }
        Command::Show {
            session_key,
            turn,
            limit,
        } => {
            let repo_path = resolve_read_repo_path(dir)?;
            match turn {
                Some(turn) => {
                    let records = get_session_records(
                        &repo_path,
                        &session_key,
                        &turn,
                        limit.unwrap_or(DEFAULT_RECORD_LIMIT),
                    )
                    .context("failed to read agent session records")?;
                    Ok(CommandOutput::Records(records))
                }
                None => {
                    let timeline = get_session_timeline_outline(
                        &repo_path,
                        &session_key,
                        Some(limit.unwrap_or(DEFAULT_TIMELINE_LIMIT)),
                    )
                    .context("failed to read agent session timeline")?;
                    Ok(CommandOutput::Timeline(timeline))
                }
            }
        }
        Command::Skim { target, value } => {
            let explicit_target = match (target, value) {
                (Some(target), Some(value)) => Some((target, value)),
                (Some(target), None) => {
                    anyhow::bail!("{} target value is required", target.as_str())
                }
                (None, Some(_)) => {
                    anyhow::bail!("target kind is required when target value is provided")
                }
                (None, None) => None,
            };
            let repo_path = if explicit_target.is_some() {
                resolve_read_repo_path(dir)?
            } else {
                resolve_workdir(dir)?
            };
            let (target, value) = match explicit_target {
                Some(target) => target,
                None => skim::resolve_default_branch_target(&repo_path)?,
            };
            let target_key = related_session_target_key(target, &value);
            let sessions =
                find_related_sessions_limited(&repo_path, target.related_target(&target_key), None)
                    .context("failed to find related agent sessions")?;
            let report = skim::report(&repo_path, target, target_key, sessions)
                .context("failed to build agent skim")?;
            Ok(CommandOutput::Skim(report))
        }
        Command::Sync => {
            let workdir = resolve_workdir(dir)?;
            with_capture_lock(&workdir, || sync_metadata(&workdir))
                .context("failed to sync GitMeta metadata")?;
            Ok(CommandOutput::Message {
                message: "Synced GitMeta metadata".into(),
            })
        }
    }
}

fn run_hook(dir: &Path, agent: Option<Agent>, input: &str) -> anyhow::Result<Option<PathBuf>> {
    let input: HookInput =
        serde_json::from_str(input).context("failed to parse agent hook input")?;
    let Some(transcript_path) = input
        .transcript_path
        .filter(|path| !path.as_os_str().is_empty())
    else {
        return Ok(None);
    };
    let dir = input
        .cwd
        .as_deref()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(dir);
    let agent = agent.context("agent is required")?;

    record_agent_log(dir, agent, &transcript_path)
        .context("failed to capture agent log from hook input")
}

fn spawn_agentlog_sync(dir: &Path) {
    #[cfg(target_os = "linux")]
    let but_path = Path::new("/proc/self/exe");
    #[cfg(not(target_os = "linux"))]
    let but_path = match std::env::current_exe() {
        Ok(path) => path,
        Err(_) => return,
    };

    let _ = ProcessCommand::new(but_path)
        .arg("-C")
        .arg(dir)
        .args(["agentlog", "sync"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn record_agent_log(
    dir: &Path,
    agent: Agent,
    transcript_path: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    let workdir = resolve_workdir(dir)?;
    let transcript_path = if transcript_path.is_absolute() {
        transcript_path.to_path_buf()
    } else {
        dir.join(transcript_path)
    };
    let transcript = prepare_transcript(agent, &transcript_path)?;
    let Some(transcript) = transcript else {
        return Ok(None);
    };

    let metadata_changed = with_capture_lock(&workdir, || {
        let (_records_written, metadata_changed) =
            record_prepared_transcript(&workdir, agent, transcript)?;

        Ok(metadata_changed)
    })?;

    Ok(metadata_changed.then_some(workdir))
}

fn resolve_workdir(dir: &Path) -> anyhow::Result<PathBuf> {
    let repo = discover_repo(dir)?;
    let workdir = repo
        .workdir()
        .context("Bare repositories are not supported.")?;
    std::fs::canonicalize(workdir).context("failed to resolve repository worktree")
}

fn resolve_read_repo_path(dir: &Path) -> anyhow::Result<PathBuf> {
    let repo = discover_repo(dir)?;
    let repo_path = repo.workdir().unwrap_or_else(|| repo.git_dir());
    std::fs::canonicalize(repo_path).context("failed to resolve repository path")
}

fn discover_repo(dir: &Path) -> anyhow::Result<gix::Repository> {
    gix::discover(dir)
        .or_else(|_| gix::open(dir))
        .context("No git repository found. Use -C to choose a repository.")
}

fn non_empty_value(value: &str) -> Result<String, String> {
    if value.is_empty() {
        Err("target value is required".into())
    } else {
        Ok(value.to_owned())
    }
}

fn display_preview(text: &str) -> String {
    const PREVIEW_CHARS: usize = 120;

    let single_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = single_line.chars();
    let mut preview = chars.by_ref().take(PREVIEW_CHARS).collect::<String>();
    if chars.next().is_some() {
        preview.push_str("...");
    }
    preview
}

fn related_session_target_key(target: RelatedSessionTarget, value: &str) -> String {
    match target {
        RelatedSessionTarget::Branch => {
            let value = value.strip_prefix("branch:").unwrap_or(value);
            if value.starts_with("ref:") {
                value.to_owned()
            } else if value.starts_with("refs/") {
                format!("ref:{value}")
            } else {
                format!("ref:refs/heads/{value}")
            }
        }
        RelatedSessionTarget::Review => {
            let value = value.strip_prefix("review:").unwrap_or(value);
            if value.starts_with("gitbutler-review:") || value.starts_with("pull-request:") {
                value.to_owned()
            } else {
                format!("gitbutler-review:{value}")
            }
        }
        RelatedSessionTarget::Change => {
            let value = value.strip_prefix("change-id:").unwrap_or(value);
            if value.starts_with("gitbutler-change:") {
                value.to_owned()
            } else {
                format!("gitbutler-change:{value}")
            }
        }
    }
}

#[derive(Deserialize)]
struct HookInput {
    transcript_path: Option<PathBuf>,
    cwd: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        fs,
        path::{Path, PathBuf},
    };

    use but_core::RepositoryExt as _;
    use git_meta_lib::{MetaValue, Session, Target};
    use tempfile::TempDir;

    use super::{
        Command, RelatedSessionTarget, related_session_target_key, run_from_dir, run_hook,
    };
    use crate::Agent;
    use crate::environment::{EnvironmentObservation, ObservedTargets};
    use crate::gitmeta::write_transcript_batch;
    use crate::transcript::TranscriptBatch;

    const TEST_SESSION_KEY: &str = "sha256-11111111111111111111111111111111";
    const TEST_SOURCE_KEY: &str = "sha256-22222222222222222222222222222222";
    const TEST_BRANCH_KEY: &str = "ref:refs/heads/main";
    const TEST_REVIEW_KEY: &str = "gitbutler-review:review-1";
    const TEST_CHANGE_KEY: &str = "gitbutler-change:change-1";

    #[test]
    fn timeline_outputs_compact_turns() {
        let repo = setup_repo();
        let turn_key = write_turn_with_targets(repo.path());

        let output = run_from_dir(
            repo.path(),
            Command::Show {
                session_key: TEST_SESSION_KEY.to_owned(),
                turn: None,
                limit: None,
            },
        )
        .expect("show timeline");
        let json = serde_json::to_value(&output).expect("serialize command output");

        assert_eq!(json["session_key"], TEST_SESSION_KEY);
        assert_eq!(json["turns"][0]["turn_key"], turn_key);
        assert!(json["turns"][0].get("records").is_none());
        assert!(output.to_string().contains(&format!(
            "1 of 1 turns for {TEST_SESSION_KEY}\n#0 {turn_key}"
        )));
    }

    #[test]
    fn records_outputs_bounded_turn_records() {
        let repo = setup_repo();
        let turn_key = write_turn_with_targets(repo.path());

        let output = run_from_dir(
            repo.path(),
            Command::Show {
                session_key: TEST_SESSION_KEY.to_owned(),
                turn: Some(turn_key.clone()),
                limit: Some(1),
            },
        )
        .expect("show records");
        let json = serde_json::to_value(&output).expect("serialize command output");

        assert_eq!(json["session_key"], TEST_SESSION_KEY);
        assert_eq!(json["turn_key"], turn_key);
        assert_eq!(json["coverage"]["showing_records"], 1);
        assert_eq!(json["records"][0]["text"], "hello");
        assert!(json["records"][0].get("record_hash").is_none());
        assert!(json["records"][0].get("source_key").is_none());
        assert!(output.to_string().contains(&format!(
            "1 of 1 records for {TEST_SESSION_KEY} turn {turn_key}"
        )));
        assert!(output.to_string().contains("message - hello"));
    }

    #[test]
    fn show_reads_bare_repo_metadata() {
        let repo = setup_bare_repo();
        let turn_key = write_turn_with_targets(repo.path());

        let output = run_from_dir(
            repo.path(),
            Command::Show {
                session_key: TEST_SESSION_KEY.to_owned(),
                turn: Some(turn_key.clone()),
                limit: Some(1),
            },
        )
        .expect("show bare repo records");
        let json = serde_json::to_value(&output).expect("serialize command output");

        assert_eq!(json["session_key"], TEST_SESSION_KEY);
        assert_eq!(json["turn_key"], turn_key);
        assert_eq!(json["records"][0]["text"], "hello");
    }

    #[test]
    fn skim_outputs_all_turns_with_drill_down_keys_in_json() {
        let repo = setup_repo();
        let turn_key = write_user_turn_with_targets(repo.path());

        let output = run_from_dir(
            repo.path(),
            Command::Skim {
                target: Some(RelatedSessionTarget::Branch),
                value: Some("main".into()),
            },
        )
        .expect("read skim");
        let json = serde_json::to_value(&output).expect("serialize command output");

        assert_eq!(json["target_kind"], "branch");
        assert_eq!(json["target_key"], TEST_BRANCH_KEY);
        assert_eq!(json["sessions"][0]["session_key"], TEST_SESSION_KEY);
        assert_eq!(json["sessions"][0]["related_turn_count"], 1);
        assert_eq!(json["coverage"]["showing_sessions"], 1);
        assert_eq!(json["coverage"]["showing_turns"], 1);
        assert_eq!(json["coverage"]["related_turn_count"], 1);
        assert_eq!(json["sessions"][0]["turns"][0]["turn_key"], turn_key);
        assert_eq!(json["sessions"][0]["turns"][0]["related"], true);

        let human = output.to_string();
        assert!(human.starts_with(&format!("Skim for branch {TEST_BRANCH_KEY}\n")));
        assert!(human.contains(
            "\nSessions: showing 1 related sessions, 1 turns total, 1 directly related turns."
        ));
        assert!(human.contains("\nSession #1: 1 turns, 1 records, latest "));
        assert!(human.contains("\n- #0 "));
        assert!(human.contains("hello"));
        assert!(human.contains("\nThis is a skim: all related sessions and turns, abbreviated."));
        assert!(
            !human.contains("sha256-"),
            "human output should not print session or turn handles by default"
        );
        assert!(
            !human.contains(TEST_SESSION_KEY),
            "human output should keep full session keys in JSON for drill-down"
        );
        assert!(
            !human.contains(&turn_key),
            "human output should keep full turn keys in JSON for drill-down"
        );
    }

    #[test]
    fn explicit_skim_reads_bare_repo_metadata() {
        let repo = setup_bare_repo();
        let turn_key = write_user_turn_with_targets(repo.path());

        let output = run_from_dir(
            repo.path(),
            Command::Skim {
                target: Some(RelatedSessionTarget::Branch),
                value: Some("main".into()),
            },
        )
        .expect("skim bare repo");
        let json = serde_json::to_value(&output).expect("serialize command output");

        assert_eq!(json["target_key"], TEST_BRANCH_KEY);
        assert_eq!(json["sessions"][0]["session_key"], TEST_SESSION_KEY);
        assert_eq!(json["sessions"][0]["turns"][0]["turn_key"], turn_key);
    }

    #[test]
    fn skim_outputs_related_sessions_chronologically() {
        let repo = setup_repo();
        let probe_session_key = "sha256-33333333333333333333333333333333";
        let probe_source_key = "sha256-44444444444444444444444444444444";
        write_turn_for_session(repo.path(), TEST_SESSION_KEY, TEST_SOURCE_KEY, "first");
        write_turn_for_session(repo.path(), TEST_SESSION_KEY, TEST_SOURCE_KEY, "second");
        write_turn_for_session(repo.path(), TEST_SESSION_KEY, TEST_SOURCE_KEY, "third");
        write_turn_for_session(repo.path(), probe_session_key, probe_source_key, "probe");
        set_session_updated_at(repo.path(), TEST_SESSION_KEY, "2026-05-07T09:00:00.000Z");
        set_session_updated_at(repo.path(), probe_session_key, "2026-05-07T10:00:00.000Z");

        let output = run_from_dir(
            repo.path(),
            Command::Skim {
                target: Some(RelatedSessionTarget::Branch),
                value: Some("main".into()),
            },
        )
        .expect("read skim");
        let json = serde_json::to_value(&output).expect("serialize command output");

        assert_eq!(json["coverage"]["showing_sessions"], 2);
        assert_eq!(json["coverage"]["showing_turns"], 4);
        assert_eq!(json["coverage"]["related_turn_count"], 4);
        assert_eq!(json["sessions"][0]["session_key"], TEST_SESSION_KEY);
        assert_eq!(json["sessions"][0]["related_turn_count"], 3);
        assert_eq!(json["sessions"][1]["session_key"], probe_session_key);
        assert_eq!(json["sessions"][1]["related_turn_count"], 1);
    }

    #[test]
    fn skim_marks_session_associated_follow_up_related() {
        let repo = setup_repo();
        let related_turn_key = write_turn_for_session_with_targets(
            repo.path(),
            TEST_SESSION_KEY,
            TEST_SOURCE_KEY,
            "related setup",
            Some("assistant"),
            ObservedTargets::from_index_keys_for_testing(
                TEST_BRANCH_KEY,
                TEST_REVIEW_KEY,
                TEST_CHANGE_KEY,
            ),
        );
        let unrelated_turn_key = write_turn_for_session_with_targets(
            repo.path(),
            TEST_SESSION_KEY,
            TEST_SOURCE_KEY,
            "unrelated follow-up",
            Some("assistant"),
            ObservedTargets::default(),
        );

        let output = run_from_dir(
            repo.path(),
            Command::Skim {
                target: Some(RelatedSessionTarget::Branch),
                value: Some("main".into()),
            },
        )
        .expect("read skim");
        let json = serde_json::to_value(&output).expect("serialize command output");

        assert_eq!(json["coverage"]["showing_turns"], 2);
        assert_eq!(json["coverage"]["related_turn_count"], 2);
        assert_eq!(
            json["sessions"][0]["turns"][0]["turn_key"],
            related_turn_key
        );
        assert_eq!(json["sessions"][0]["turns"][0]["related"], true);
        assert_eq!(
            json["sessions"][0]["turns"][1]["turn_key"],
            unrelated_turn_key
        );
        assert_eq!(json["sessions"][0]["turns"][1]["related"], true);
        assert_eq!(
            json["sessions"][0]["previews"][0],
            "assistant: unrelated follow-up"
        );
    }

    #[test]
    fn related_session_target_key_normalizes_common_inputs() {
        for (target, value, expected) in [
            (RelatedSessionTarget::Branch, "main", TEST_BRANCH_KEY),
            (RelatedSessionTarget::Branch, "branch:main", TEST_BRANCH_KEY),
            (
                RelatedSessionTarget::Branch,
                "refs/heads/main",
                TEST_BRANCH_KEY,
            ),
            (
                RelatedSessionTarget::Branch,
                "ref:refs/heads/main",
                TEST_BRANCH_KEY,
            ),
            (RelatedSessionTarget::Review, "review-1", TEST_REVIEW_KEY),
            (
                RelatedSessionTarget::Review,
                "review:review-1",
                TEST_REVIEW_KEY,
            ),
            (
                RelatedSessionTarget::Review,
                "gitbutler-review:review-1",
                TEST_REVIEW_KEY,
            ),
            (RelatedSessionTarget::Change, "change-1", TEST_CHANGE_KEY),
            (
                RelatedSessionTarget::Change,
                "change-id:change-1",
                TEST_CHANGE_KEY,
            ),
            (
                RelatedSessionTarget::Change,
                "gitbutler-change:change-1",
                TEST_CHANGE_KEY,
            ),
        ] {
            assert_eq!(related_session_target_key(target, value), expected);
        }
    }

    #[test]
    fn hook_reads_transcript_path_from_payload() {
        let repo = setup_repo();
        write_transcript_with_message(repo.path());
        let payload = serde_json::json!({
            "transcript_path": "session.jsonl",
            "cwd": repo.path().display().to_string(),
        })
        .to_string();

        let sync_dir = run_hook(Path::new("/"), Some(Agent::Codex), &payload).expect("run hook");

        assert_eq!(
            sync_dir,
            Some(fs::canonicalize(repo.path()).expect("canonical repo"))
        );
        assert_eq!(session_keys(repo.path(), &Target::project()).len(), 1);

        let duplicate_sync_dir =
            run_hook(repo.path(), Some(Agent::Codex), &payload).expect("duplicate hook");
        assert_eq!(duplicate_sync_dir, None);
    }

    #[test]
    fn hook_without_transcript_path_noops() {
        let repo = setup_repo();
        let payload = serde_json::json!({
            "cwd": repo.path().display().to_string(),
        })
        .to_string();

        let sync_dir = run_hook(repo.path(), None, &payload).expect("run hook");

        assert_eq!(sync_dir, None);
        assert!(
            !gitbutler_storage_path(repo.path()).exists(),
            "transcriptless hook should not create GitButler storage"
        );
        assert!(
            target_value(repo.path(), &Target::project(), "gitbutler:agent-sessions").is_none()
        );
    }

    fn write_transcript_with_message(repo: &Path) {
        fs::write(
            repo.join("session.jsonl"),
            concat!(
                r#"{"timestamp":"2026-05-07T09:00:00Z","type":"session_meta","payload":{"id":"session-1"}}"#,
                "\n",
                r#"{"timestamp":"2026-05-07T09:00:01Z","type":"response_item","payload":{"type":"message","content":"hello"}}"#,
                "\n",
            ),
        )
        .expect("write transcript");
    }

    fn write_turn_with_targets(repo: &Path) -> String {
        write_turn_for_session(repo, TEST_SESSION_KEY, TEST_SOURCE_KEY, "hello")
    }

    fn write_user_turn_with_targets(repo: &Path) -> String {
        write_turn_for_session_with_role(
            repo,
            TEST_SESSION_KEY,
            TEST_SOURCE_KEY,
            "hello",
            Some("user"),
        )
    }

    fn write_turn_for_session(
        repo: &Path,
        session_key: &str,
        source_key: &str,
        text: &str,
    ) -> String {
        write_turn_for_session_with_role(repo, session_key, source_key, text, None)
    }

    fn write_turn_for_session_with_role(
        repo: &Path,
        session_key: &str,
        source_key: &str,
        text: &str,
        role: Option<&str>,
    ) -> String {
        write_turn_for_session_with_targets(
            repo,
            session_key,
            source_key,
            text,
            role,
            ObservedTargets::from_index_keys_for_testing(
                TEST_BRANCH_KEY,
                TEST_REVIEW_KEY,
                TEST_CHANGE_KEY,
            ),
        )
    }

    fn write_turn_for_session_with_targets(
        repo: &Path,
        session_key: &str,
        source_key: &str,
        text: &str,
        role: Option<&str>,
        observed_targets: ObservedTargets,
    ) -> String {
        let role_fragment = role
            .map(|role| {
                format!(
                    r#","role":{}"#,
                    serde_json::to_string(role).expect("serialize role")
                )
            })
            .unwrap_or_default();
        let batch = TranscriptBatch::parse(
            Agent::Codex,
            format!(
                concat!(
                    r#"{{"timestamp":"2026-05-07T09:00:00Z","type":"session_meta","payload":{{"id":"session-1"}}}}"#,
                    "\n",
                    r#"{{"timestamp":"2026-05-07T09:00:01Z","type":"response_item","payload":{{"type":"message"{},"content":{}}}}}"#,
                    "\n",
                ),
                role_fragment,
                serde_json::to_string(text).expect("serialize message text")
            )
            .as_bytes(),
        )
        .expect("parse transcript");

        write_transcript_batch(repo, Agent::Codex, session_key, source_key, batch, || {
            EnvironmentObservation::from_observed_targets_for_testing(observed_targets)
        })
        .expect("write transcript batch");
        latest_turn_key(repo, session_key)
    }

    fn latest_turn_key(repo: &Path, session_key: &str) -> String {
        let Some(MetaValue::List(entries)) = target_value(
            repo,
            &Target::project(),
            &format!("gitbutler:agent-session:{session_key}:turns"),
        ) else {
            panic!("expected turn summaries");
        };
        let summary: serde_json::Value =
            serde_json::from_str(&entries.last().expect("latest turn").value)
                .expect("turn summary JSON");
        summary["turn_key"].as_str().expect("turn key").to_owned()
    }

    fn set_session_updated_at(repo: &Path, session_key: &str, updated_at: &str) {
        let session = Session::open(repo).expect("open session");
        let target = Target::project();
        let key = format!("gitbutler:agent-session:{session_key}:updated-at");
        let value = MetaValue::String(updated_at.to_owned());
        session
            .target(&target)
            .apply_edits(vec![git_meta_lib::MetaEdit::set_value(&key, &value)])
            .expect("set session updated-at");
    }

    fn setup_repo() -> TempDir {
        let dir = TempDir::new().expect("temp repo");
        gix::ThreadSafeRepository::init_opts(
            dir.path(),
            gix::create::Kind::WithWorktree,
            gix::create::Options::default(),
            gix::open::Options::isolated().config_overrides(["init.defaultBranch=main"]),
        )
        .expect("gitoxide repo init");
        dir
    }

    fn setup_bare_repo() -> TempDir {
        let dir = TempDir::new().expect("temp bare repo");
        gix::init_bare(dir.path()).expect("gitoxide bare repo init");
        dir
    }

    fn session_keys(repo: &Path, target: &Target) -> BTreeSet<String> {
        let Some(MetaValue::Set(keys)) = target_value(repo, target, "gitbutler:agent-sessions")
        else {
            panic!("expected session index set");
        };
        keys
    }

    fn target_value(repo: &Path, target: &Target, key: &str) -> Option<MetaValue> {
        Session::open(repo)
            .expect("open session")
            .target(target)
            .get_value(key)
            .expect("read GitMeta value")
    }

    fn gitbutler_storage_path(repo: &Path) -> PathBuf {
        gix::discover(repo)
            .expect("discover repo")
            .gitbutler_storage_path()
            .expect("storage path")
    }
}

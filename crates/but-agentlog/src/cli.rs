use std::{
    fmt,
    io::Read as _,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
};

use anyhow::Context as _;
use git_meta_lib::Target;
use serde::{Deserialize, Serialize};

use crate::{
    agent::Agent,
    capture::{prepare_transcript, record_prepared_transcript},
    capture_lock::with_capture_lock,
    gitmeta::{
        RelatedSession, RelatedTarget, associate_session, find_related_sessions,
        resolve_association_target, sync_metadata,
    },
};

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    Capture {
        #[clap(long, value_enum)]
        agent: Option<Agent>,
        #[clap(long, value_name = "PATH", value_parser = non_empty_path)]
        transcript_path: PathBuf,
        #[clap(long, value_name = "TARGET", value_parser = Target::parse)]
        associate_target: Option<Target>,
    },
    Hook {
        #[clap(long, value_enum)]
        agent: Option<Agent>,
        #[clap(long, value_name = "TARGET", value_parser = Target::parse)]
        associate_target: Option<Target>,
    },
    #[clap(name = "sessions")]
    Sessions {
        #[clap(value_enum)]
        target: RelatedSessionTarget,
        #[clap(value_name = "VALUE", value_parser = non_empty_value)]
        value: String,
    },
    Sync,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum CommandOutput {
    Message {
        message: String,
    },
    Sessions {
        target_kind: &'static str,
        target_key: String,
        sessions: Vec<RelatedSession>,
    },
}

impl fmt::Display for CommandOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandOutput::Message { message } => f.write_str(message),
            CommandOutput::Sessions {
                target_kind,
                target_key,
                sessions,
            } => {
                let noun = if sessions.len() == 1 {
                    "session"
                } else {
                    "sessions"
                };
                writeln!(
                    f,
                    "{} {noun} related to {target_kind} {target_key}",
                    sessions.len()
                )?;
                for session in sessions {
                    writeln!(f, "{} {}", session.session_key, session.turn_keys.join(","))?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum RelatedSessionTarget {
    Branch,
    Review,
    Change,
}

impl RelatedSessionTarget {
    fn as_str(self) -> &'static str {
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
        Command::Sessions { target, value } => {
            let workdir = resolve_workdir(dir)?;
            let target_key = related_session_target_key(target, &value);
            let sessions = find_related_sessions(&workdir, target.related_target(&target_key))
                .context("failed to find related agent sessions")?;
            Ok(CommandOutput::Sessions {
                target_kind: target.as_str(),
                target_key,
                sessions,
            })
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

fn run_hook(
    dir: &Path,
    agent: Option<Agent>,
    associate_target: Option<&Target>,
    input: &str,
) -> anyhow::Result<Option<PathBuf>> {
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

struct RecordedAgentLog {
    workdir: PathBuf,
    metadata_changed: bool,
}

fn record_agent_log(
    dir: &Path,
    agent: Agent,
    transcript_path: &Path,
    associate_target: Option<&Target>,
) -> anyhow::Result<RecordedAgentLog> {
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
    let repo =
        gix::discover(dir).context("No git repository found. Use -C to choose a repository.")?;
    let workdir = repo
        .workdir()
        .context("Bare repositories are not supported.")?;
    std::fs::canonicalize(workdir).context("failed to resolve repository worktree")
}

fn non_empty_path(value: &str) -> Result<PathBuf, String> {
    if value.is_empty() {
        Err("transcript path is required".into())
    } else {
        Ok(value.into())
    }
}

fn non_empty_value(value: &str) -> Result<String, String> {
    if value.is_empty() {
        Err("target value is required".into())
    } else {
        Ok(value.to_owned())
    }
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
        process::{Command as ProcessCommand, Stdio},
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

    #[derive(Debug, clap::Parser)]
    struct Args {
        #[clap(subcommand)]
        command: Command,
    }

    #[test]
    fn rejects_unsupported_agent_while_parsing_args() {
        use clap::Parser as _;

        Args::try_parse_from([
            "but-agentlog",
            "capture",
            "--agent",
            "cursor",
            "--transcript-path",
            "missing.jsonl",
        ])
        .expect_err("unsupported agent should fail");
    }

    #[test]
    fn rejects_empty_transcript_path_while_parsing_args() {
        use clap::Parser as _;

        Args::try_parse_from([
            "but-agentlog",
            "capture",
            "--agent",
            "codex",
            "--transcript-path",
            "",
        ])
        .expect_err("empty transcript path should fail");
    }

    #[test]
    fn rejects_empty_related_sessions_target_value_while_parsing_args() {
        use clap::Parser as _;

        Args::try_parse_from(["but-agentlog", "sessions", "branch", ""])
            .expect_err("empty sessions target value should fail");
    }

    #[test]
    fn parses_association_target_flag_without_agent() {
        use clap::Parser as _;

        let args = Args::try_parse_from([
            "but-agentlog",
            "capture",
            "--transcript-path",
            "session.jsonl",
            "--associate-target",
            "branch:main",
        ])
        .expect("parse args");

        let Command::Capture {
            agent,
            associate_target,
            ..
        } = args.command
        else {
            panic!("expected capture command");
        };
        assert_eq!(agent, None);
        assert_eq!(
            associate_target.expect("association target").to_string(),
            "branch:main"
        );
    }

    #[test]
    fn parses_sessions_target() {
        use clap::Parser as _;

        let args = Args::try_parse_from(["but-agentlog", "sessions", "branch", "main"])
            .expect("parse args");

        let Command::Sessions { target, value } = args.command else {
            panic!("expected sessions command");
        };
        assert_eq!(target, RelatedSessionTarget::Branch);
        assert_eq!(value, "main");
    }

    #[test]
    fn related_sessions_outputs_verified_sessions() {
        let repo = setup_repo();
        let turn_key = write_turn_with_targets(repo.path());

        let output = run_from_dir(
            repo.path(),
            Command::Sessions {
                target: RelatedSessionTarget::Branch,
                value: "main".into(),
            },
        )
        .expect("find related sessions");
        let json = serde_json::to_value(&output).expect("serialize command output");

        assert_eq!(
            json,
            serde_json::json!({
                "target_kind": "branch",
                "target_key": TEST_BRANCH_KEY,
                "sessions": [{
                    "session_key": TEST_SESSION_KEY,
                    "turn_keys": [turn_key],
                }],
            })
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
    fn capture_can_associate_existing_session_without_new_records() {
        let repo = setup_repo();
        write_transcript_with_message(repo.path());
        run_from_dir(
            repo.path(),
            Command::Capture {
                agent: Some(Agent::Codex),
                transcript_path: "session.jsonl".into(),
                associate_target: None,
            },
        )
        .expect("initial capture");

        let output = run_from_dir(
            repo.path(),
            Command::Capture {
                agent: Some(Agent::Codex),
                transcript_path: "session.jsonl".into(),
                associate_target: Some(Target::branch("main")),
            },
        )
        .expect("associate existing session")
        .to_string();

        assert_eq!(
            output,
            "Captured 0 records and associated session with branch:main"
        );
        assert_eq!(
            session_keys(repo.path(), &Target::branch("main")),
            session_keys(repo.path(), &Target::project())
        );
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

    #[test]
    fn sync_with_empty_metadata_remote_outputs_message() {
        let repo = setup_repo();
        let remote = TempDir::new().expect("temp remote");
        let status = ProcessCommand::new("git")
            .args(["init", "--bare"])
            .current_dir(remote.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("git init bare");
        assert!(status.success());

        let status = ProcessCommand::new("git")
            .args(["remote", "add", "origin", &remote.path().to_string_lossy()])
            .current_dir(repo.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("git remote add");
        assert!(status.success());
        let status = ProcessCommand::new("git")
            .args(["config", "remote.origin.meta", "true"])
            .current_dir(repo.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("git config remote origin meta");
        assert!(status.success());

        let session = Session::open(repo.path()).expect("open session");
        session
            .target(&Target::project())
            .set("gitbutler:test", "value")
            .expect("write metadata");

        let output = run_from_dir(repo.path(), Command::Sync)
            .expect("sync")
            .to_string();

        assert_eq!(output, "Synced GitMeta metadata");
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
        let batch = TranscriptBatch::parse(
            Agent::Codex,
            concat!(
                r#"{"timestamp":"2026-05-07T09:00:00Z","type":"session_meta","payload":{"id":"session-1"}}"#,
                "\n",
                r#"{"timestamp":"2026-05-07T09:00:01Z","type":"response_item","payload":{"type":"message","content":"hello"}}"#,
                "\n",
            )
            .as_bytes(),
        )
        .expect("parse transcript");

        write_transcript_batch(
            repo,
            Agent::Codex,
            TEST_SESSION_KEY,
            TEST_SOURCE_KEY,
            batch,
            || {
                EnvironmentObservation::from_observed_targets_for_testing(
                    ObservedTargets::from_index_keys_for_testing(
                        TEST_BRANCH_KEY,
                        TEST_REVIEW_KEY,
                        TEST_CHANGE_KEY,
                    ),
                )
            },
        )
        .expect("write transcript batch");
        only_turn_key(repo)
    }

    fn only_turn_key(repo: &Path) -> String {
        let Some(MetaValue::List(entries)) = target_value(
            repo,
            &Target::project(),
            &format!("gitbutler:agent-session:{TEST_SESSION_KEY}:turns"),
        ) else {
            panic!("expected turn summaries");
        };
        let summary: serde_json::Value =
            serde_json::from_str(&entries[0].value).expect("turn summary JSON");
        summary["turn_key"].as_str().expect("turn key").to_owned()
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

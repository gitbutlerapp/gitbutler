use std::{
    fmt,
    path::Path,
    process::{Command as ProcessCommand, Stdio},
};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::{
    RelatedSessionTarget,
    gitmeta::{RelatedSession, get_session_timeline_outline},
};

const SKIM_LINE_CHARS: usize = 120;

#[derive(Debug, Serialize)]
pub(crate) struct SkimReport {
    target_kind: &'static str,
    target_key: String,
    coverage: SkimCoverage,
    sessions: Vec<SkimSession>,
}

#[derive(Debug, Default, Serialize)]
struct SkimCoverage {
    showing_sessions: usize,
    showing_turns: usize,
    related_turn_count: usize,
}

#[derive(Debug, Serialize)]
struct SkimSession {
    session_key: String,
    updated_at: String,
    latest_captured_at: Option<String>,
    turn_count: usize,
    record_count: usize,
    related_turn_count: usize,
    previews: Vec<String>,
    turns: Vec<SkimTurn>,
}

#[derive(Clone, Debug, Serialize)]
struct SkimTurn {
    turn_key: String,
    turn_index: usize,
    captured_at: String,
    related: bool,
    record_count: usize,
    tool_names: Vec<String>,
    labels: Vec<&'static str>,
    previews: Vec<String>,
}

impl fmt::Display for SkimReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Skim for {} {}", self.target_kind, self.target_key)?;
        if self.sessions.is_empty() {
            writeln!(f, "\nNo related agent sessions found.")?;
            return Ok(());
        }
        writeln!(
            f,
            "\nSessions: showing {} related sessions, {} turns total, {} directly related turns.",
            self.coverage.showing_sessions,
            self.coverage.showing_turns,
            self.coverage.related_turn_count
        )?;
        for (index, session) in self.sessions.iter().enumerate() {
            writeln!(
                f,
                "\nSession #{}: {} turns, {} records, latest {}",
                index + 1,
                session.turn_count,
                session.record_count,
                session.latest_captured_at.as_deref().unwrap_or("unknown")
            )?;
            for turn in &session.turns {
                write!(f, "- #{}", turn.turn_index)?;
                if !turn.labels.is_empty() {
                    write!(f, " [{}]", turn.labels.join(","))?;
                }
                write!(
                    f,
                    " {} records={}{}",
                    turn.captured_at,
                    turn.record_count,
                    if turn.related { " related" } else { "" }
                )?;
                let line = turn_summary_line(turn);
                if line.is_empty() {
                    writeln!(f)?;
                } else {
                    writeln!(f, " {line}")?;
                }
            }
        }
        writeln!(
            f,
            "\nThis is a skim: all related sessions and turns, abbreviated. For exact detail, use `but agentlog show <session> --turn <turn>` from JSON output."
        )?;
        Ok(())
    }
}

pub(crate) fn report(
    workdir: &Path,
    target: RelatedSessionTarget,
    target_key: String,
    sessions: Vec<RelatedSession>,
) -> anyhow::Result<SkimReport> {
    if sessions.is_empty() {
        return Ok(SkimReport {
            target_kind: target.as_str(),
            target_key,
            coverage: SkimCoverage::default(),
            sessions: Vec::new(),
        });
    }
    let mut sessions = sessions;
    sessions.sort_by(|lhs, rhs| {
        session_sort_timestamp(lhs)
            .cmp(session_sort_timestamp(rhs))
            .then_with(|| lhs.session_key.cmp(&rhs.session_key))
    });

    let mut skim_sessions = Vec::with_capacity(sessions.len());
    let mut showing_turns = 0;
    let mut related_turn_count = 0;
    for session in sessions {
        let turns = skim_session_turns(workdir, &session)?;
        showing_turns += turns.len();
        related_turn_count += session.related_turn_keys.len();
        let previews = session_previews(&session);
        skim_sessions.push(SkimSession {
            session_key: session.session_key,
            updated_at: session.updated_at,
            latest_captured_at: session.latest_captured_at,
            turn_count: session.turn_count,
            record_count: session.record_count,
            related_turn_count: session.related_turn_keys.len(),
            previews,
            turns,
        });
    }
    let coverage = SkimCoverage {
        showing_sessions: skim_sessions.len(),
        showing_turns,
        related_turn_count,
    };

    Ok(SkimReport {
        target_kind: target.as_str(),
        target_key,
        coverage,
        sessions: skim_sessions,
    })
}

fn skim_session_turns(workdir: &Path, session: &RelatedSession) -> anyhow::Result<Vec<SkimTurn>> {
    let timeline = get_session_timeline_outline(workdir, &session.session_key, None)
        .context("failed to read agent session timeline")?;
    let related_turn_keys = &session.related_turn_keys;
    Ok(timeline
        .turns
        .into_iter()
        .map(|turn| {
            let related = related_turn_keys
                .iter()
                .any(|turn_key| turn_key == &turn.turn_key);
            let mut previews = Vec::new();
            if let Some(preview) = turn.latest_user_preview {
                previews.push(format!("user: {}", preview_text(&preview.text)));
            }
            if let Some(preview) = turn.latest_assistant_preview {
                previews.push(format!("assistant: {}", preview_text(&preview.text)));
            }
            let labels = turn_labels(&previews);
            SkimTurn {
                turn_key: turn.turn_key,
                turn_index: turn.turn_index,
                captured_at: turn.captured_at,
                related,
                record_count: turn.record_count,
                tool_names: turn.tool_counts.tool_names,
                labels,
                previews,
            }
        })
        .collect())
}

fn session_sort_timestamp(session: &RelatedSession) -> &str {
    session
        .started_at
        .as_deref()
        .or(session.latest_captured_at.as_deref())
        .unwrap_or(session.updated_at.as_str())
}

fn session_previews(session: &RelatedSession) -> Vec<String> {
    let mut previews = Vec::new();
    if let Some(preview) = &session.latest_user_preview {
        previews.push(format!("user: {}", preview_text(&preview.text)));
    }
    if let Some(preview) = &session.latest_assistant_preview {
        previews.push(format!("assistant: {}", preview_text(&preview.text)));
    }
    previews
}

fn turn_labels(previews: &[String]) -> Vec<&'static str> {
    let text = previews.join(" ").to_ascii_lowercase();
    let mut labels = Vec::new();
    push_label_if(
        &mut labels,
        "implemented",
        has_any(&text, &["implemented", "added ", "created "]),
    );
    push_label_if(
        &mut labels,
        "decided",
        has_any(&text, &["decision", "decided", "pick", "chose"]),
    );
    push_label_if(
        &mut labels,
        "reviewed",
        has_any(&text, &["review", "findings", "subagent"]),
    );
    push_label_if(
        &mut labels,
        "tested",
        has_any(&text, &["test", "check", "validation"]),
    );
    push_label_if(
        &mut labels,
        "renamed",
        has_any(&text, &["renamed", "command is now"]),
    );
    push_label_if(
        &mut labels,
        "installed",
        has_any(&text, &["install", "installed"]),
    );
    push_label_if(
        &mut labels,
        "next",
        has_any(&text, &["what's next", "whats next", "next is"]),
    );
    labels
}

fn push_label_if(labels: &mut Vec<&'static str>, label: &'static str, condition: bool) {
    if condition && !labels.contains(&label) {
        labels.push(label);
    }
}

fn has_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn turn_summary_line(turn: &SkimTurn) -> String {
    let user = prefixed_preview(&turn.previews, "user: ");
    let assistant = prefixed_preview(&turn.previews, "assistant: ");
    match (user, assistant) {
        (Some(user), Some(assistant)) => {
            format!("{} -> {}", short_line(&user), short_line(&assistant))
        }
        (Some(user), None) => short_line(&user),
        (None, Some(assistant)) => short_line(&assistant),
        (None, None) => String::new(),
    }
}

fn prefixed_preview(previews: &[String], prefix: &str) -> Option<String> {
    previews
        .iter()
        .find_map(|preview| preview.strip_prefix(prefix).map(ToOwned::to_owned))
}

fn short_line(text: &str) -> String {
    single_line_preview(text, SKIM_LINE_CHARS)
}

fn preview_text(text: &str) -> String {
    single_line_preview(text, 320)
}

fn single_line_preview(text: &str, char_limit: usize) -> String {
    let single_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = single_line.chars();
    let mut preview = chars.by_ref().take(char_limit).collect::<String>();
    if chars.next().is_some() {
        preview.push_str("...");
    }
    preview
}

#[cfg(test)]
mod tests {
    use super::preview_text;

    #[test]
    fn preview_text_keeps_enough_context_for_skims() {
        let text = "x".repeat(321);

        let preview = preview_text(&text);

        assert_eq!(preview.chars().filter(|ch| *ch == 'x').count(), 320);
        assert!(preview.ends_with("..."));
    }
}

pub(crate) fn resolve_default_branch_target(
    workdir: &Path,
) -> anyhow::Result<(RelatedSessionTarget, String)> {
    Ok((
        RelatedSessionTarget::Branch,
        applied_gitbutler_branch(workdir)?,
    ))
}

#[derive(Deserialize)]
struct StatusReport {
    stacks: Vec<StatusStack>,
}

#[derive(Deserialize)]
struct StatusStack {
    branches: Vec<StatusBranch>,
}

#[derive(Deserialize)]
struct StatusBranch {
    name: String,
}

fn applied_gitbutler_branch(workdir: &Path) -> anyhow::Result<String> {
    let but_path = std::env::current_exe().context("failed to locate current executable")?;
    let output = ProcessCommand::new(&but_path)
        .arg("-C")
        .arg(workdir)
        .args(["--json", "status"])
        .stdin(Stdio::null())
        .output()
        .context("failed to run 'but --json status' for agentlog skim target discovery")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("failed to discover GitButler branch with 'but --json status': {stderr}");
    }
    let status: StatusReport =
        serde_json::from_slice(&output.stdout).context("failed to parse 'but --json status'")?;
    status
        .stacks
        .into_iter()
        .flat_map(|stack| stack.branches)
        .map(|branch| branch.name)
        .next()
        .context("no applied GitButler branch found; pass an explicit skim target")
}

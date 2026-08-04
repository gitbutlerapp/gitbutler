//! The agent-facing skill freshness subsystem: detect whether the driving
//! agent can load a GitButler skill, nudge it to install one,
//! and keep existing installs current.

use std::path::PathBuf;

use super::{
    check_skill_status, find_format_installations, home_dir, is_current_skill_installation,
    skill_format_for_agent, skill_format_for_name, write_skill_files,
};
use crate::{
    theme,
    utils::detect_agent::{self, Agent},
};

pub(crate) enum AgentSkillNotice {
    NotInstalled(String),
    Hint(String),
    Updated(String),
    UpdateFailed(String),
}

impl AgentSkillNotice {
    pub(crate) fn text(&self) -> &str {
        match self {
            Self::NotInstalled(text)
            | Self::Hint(text)
            | Self::Updated(text)
            | Self::UpdateFailed(text) => text,
        }
    }

    fn into_text(self) -> String {
        match self {
            Self::NotInstalled(text)
            | Self::Hint(text)
            | Self::Updated(text)
            | Self::UpdateFailed(text) => text,
        }
    }

    pub(crate) fn is_hint(&self) -> bool {
        matches!(self, Self::NotInstalled(_) | Self::Hint(_))
    }
}

pub(crate) fn agent_skill_notice(current_dir: &std::path::Path) -> Option<AgentSkillNotice> {
    let update = agent_skill_freshness_check();
    let hint = agent_skill_install_hint(current_dir);
    match (hint, update) {
        (Some(missing @ AgentSkillNotice::NotInstalled(_)), _) => Some(missing),
        (_, Some(failed @ AgentSkillNotice::UpdateFailed(_))) => Some(failed),
        (hint, update) => hint.or(update),
    }
}

pub(crate) fn agent_skill_update_notice() -> Option<String> {
    agent_skill_freshness_check().map(AgentSkillNotice::into_text)
}

/// Skill upkeep for agent commands. Returns `None` when no agent is detected.
///
/// Scans skill installations and updates all stale global copies.
///
/// Best-effort by construction: every failure in skill detection or file
/// updates is logged or turned into an agent-facing line.
/// Nothing propagates to the command that triggered the check.
fn agent_skill_freshness_check() -> Option<AgentSkillNotice> {
    detect_agent::detect()?;
    let result = check_skill_status(None, true, false).ok()?;
    let outdated: Vec<_> = result
        .skills
        .into_iter()
        .filter(|skill| !skill.up_to_date)
        .map(|skill| skill.path)
        .collect();
    if outdated.is_empty() {
        return None;
    }
    let mut update_error = None;
    for path in outdated {
        if let Err(err) = write_skill_files(&path) {
            tracing::debug!(?err, ?path, "failed to update the agent skill");
            if update_error.is_none() {
                update_error = Some(err);
            }
        }
    }
    if let Some(err) = update_error {
        return Some(AgentSkillNotice::UpdateFailed(
            agent_skill_update_failed_notice(&err),
        ));
    }
    Some(AgentSkillNotice::Updated(agent_skill_updated_message(
        &result.cli_version,
    )))
}

/// Report a missing skill before normal agent-driven commands.
fn agent_skill_install_hint(current_dir: &std::path::Path) -> Option<AgentSkillNotice> {
    let agent = detect_agent::detect()?;
    let workdir = gix::discover(current_dir)
        .ok()
        .and_then(|repo| repo.workdir().map(std::path::Path::to_path_buf));
    let installations = agent_skill_installations(agent, workdir.as_deref())?;
    let version = option_env!("VERSION").unwrap_or("dev");
    if installations.is_empty() {
        Some(AgentSkillNotice::NotInstalled(
            agent_skill_not_installed_notice(),
        ))
    } else if installations
        .iter()
        .all(|path| is_current_skill_installation(path, version))
    {
        None
    } else {
        Some(AgentSkillNotice::Hint(agent_skill_outdated_notice()))
    }
}

fn agent_skill_installations(
    agent: Agent,
    workdir: Option<&std::path::Path>,
) -> Option<Vec<PathBuf>> {
    let global = skill_format_for_agent(agent, true)?;
    let mut installations = home_dir()
        .map(|home| find_format_installations(global, &home))
        .unwrap_or_default();
    if let Some(workdir) = workdir {
        if let Some(format) = skill_format_for_agent(agent, false) {
            installations.extend(find_format_installations(format, workdir));
        }
        if matches!(agent, Agent::OpenCode | Agent::Devin)
            && let Some(format) = skill_format_for_name("Agent Skills", false)
        {
            installations.extend(find_format_installations(format, workdir));
        }
    }
    Some(installations)
}

fn agent_skill_not_installed_notice() -> String {
    let t = theme::get();
    format!(
        "{} AGENT ACTION REQUIRED: The GitButler skill is not installed for this agent.\n\
         To work effectively with but, run: but skill install\n\
         Then read the installed SKILL.md path printed by that command and continue.\n\
         This notice repeats until the skill is installed. If it still appears \
         after installing, report it instead of retrying.",
        t.sym().warning,
    )
}

fn agent_skill_outdated_notice() -> String {
    let t = theme::get();
    format!(
        "{} AGENT ACTION REQUIRED: The GitButler skill is out of date or incomplete.\n\
         Run once: but skill check --update\n\
         Then reload/use the updated skill.",
        t.sym().warning,
    )
}

/// The default install location for a bare `but skill install` when a detected
/// agent runs it without a terminal to answer the wizard: the agent's own
/// global skill directory, the same location the freshness check considers
/// loadable. The caller decides whether this is an agent-driven invocation.
pub(super) fn agent_default_install_path(agent: Agent) -> Option<PathBuf> {
    Some(skill_format_for_agent(agent, true)?.get_install_path(&home_dir()?))
}

fn agent_skill_updated_message(version: &str) -> String {
    let t = theme::get();
    format!(
        "{} The GitButler skill was out of date and was updated to {version}. \
         Reload/use the GitButler skill to pick up the changes.",
        t.sym().success,
    )
}

fn agent_skill_update_failed_notice(err: &anyhow::Error) -> String {
    let t = theme::get();
    format!(
        "{} AGENT ACTION REQUIRED: The GitButler skill is out of date and auto-update failed: {err:#}.\n\
         Run once: but skill check --update\n\
         Then reload/use the updated skill.\n\
         If this warning repeats, report it instead of retrying.",
        t.sym().warning,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Full notice/hint wording is pinned by the status snapshot test.
    #[test]
    fn not_installed_notice_explains_the_action() {
        let notice = agent_skill_not_installed_notice();
        assert!(
            notice.contains("repeats until the skill is installed"),
            "repetition is expected behavior, so the notice must say so instead \
             of letting the agent read a repeat as a malfunction"
        );
        assert!(notice.contains("run: but skill install"));
        assert!(!notice.contains("but agent setup"));
    }

    #[test]
    fn update_failure_notice_surfaces_the_error() {
        let failed = agent_skill_update_failed_notice(&anyhow::anyhow!("denied"));
        assert!(failed.contains("denied"));
        assert!(failed.contains("but skill check --update"));
    }

    #[test]
    fn agent_global_skill_dirs_match_agent_loaders() {
        let cases = [
            (Agent::ClaudeCode, &[".claude", "skills", "gitbutler"][..]),
            (Agent::GeminiCli, &[".gemini", "skills", "gitbutler"][..]),
            (
                Agent::OpenCode,
                &[".config", "opencode", "skills", "gitbutler"][..],
            ),
            (
                Agent::Devin,
                &[".config", "devin", "skills", "gitbutler"][..],
            ),
            (Agent::Dirac, &[".agents", "skills", "gitbutler"][..]),
            (Agent::Pi, &[".pi", "agent", "skills", "gitbutler"][..]),
            (Agent::KiroCli, &[".kiro", "skills", "gitbutler"][..]),
            (Agent::Junie, &[".junie", "skills", "gitbutler"][..]),
        ];
        for (agent, expected) in cases {
            assert_eq!(
                skill_format_for_agent(agent, true).map(|format| format.path_components),
                Some(expected),
                "global skill dir for {agent:?}"
            );
        }
        assert!(skill_format_for_agent(Agent::Unknown, true).is_none());
    }

    #[test]
    fn kiro_and_junie_skill_dirs_are_available_locally() {
        for (agent, expected) in [
            (Agent::KiroCli, &[".kiro", "skills", "gitbutler"][..]),
            (Agent::Junie, &[".junie", "skills", "gitbutler"][..]),
        ] {
            assert_eq!(
                skill_format_for_agent(agent, false).map(|format| format.path_components),
                Some(expected),
                "local skill dir for {agent:?}"
            );
        }
    }

    #[test]
    fn agent_skill_installations_ignore_local_without_workdir() {
        let dir = tempfile::tempdir().unwrap();
        let local = skill_format_for_agent(Agent::Codex, false)
            .unwrap()
            .get_install_path(dir.path());
        write_skill_files(&local).unwrap();

        assert!(
            !agent_skill_installations(Agent::Codex, None)
                .unwrap()
                .contains(&local)
        );
    }
}

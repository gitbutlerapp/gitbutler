//! Which skill installations a given agent can actually load, so callers can
//! tell whether the agent driving them is working from current instructions.

use std::path::{Path, PathBuf};

use but_path::home_dir;

use crate::{
    detect::Agent,
    format::{skill_format_for_agent, skill_format_for_name},
    status::find_format_installations,
};

/// Every GitButler skill installation the given agent would load: its own
/// global skill directory, plus the repository-local ones when a worktree is
/// known. `None` when the agent has no skill format at all.
pub fn agent_skill_installations(agent: Agent, workdir: Option<&Path>) -> Option<Vec<PathBuf>> {
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

/// The default install location for a bare `but skill install` when a detected
/// agent runs it without a terminal to answer the wizard: the agent's own
/// global skill directory, the same location the freshness check considers
/// loadable. The caller decides whether this is an agent-driven invocation.
pub fn agent_default_install_path(agent: Agent) -> Option<PathBuf> {
    Some(skill_format_for_agent(agent, true)?.get_install_path(&home_dir()?))
}

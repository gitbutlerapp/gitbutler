use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};

use crate::utils::detect_agent;

use super::{RepoInfo, Scope};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum AgentTarget {
    Codex,
    ClaudeCode,
    Cursor,
    GitHubCopilot,
    Windsurf,
    OpenCode,
    AgentSkills,
}

impl AgentTarget {
    pub(super) const ALL: [Self; 7] = [
        Self::Codex,
        Self::ClaudeCode,
        Self::Cursor,
        Self::GitHubCopilot,
        Self::Windsurf,
        Self::OpenCode,
        Self::AgentSkills,
    ];

    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::Cursor => "Cursor",
            Self::GitHubCopilot => "GitHub Copilot",
            Self::Windsurf => "Windsurf / Devin",
            Self::OpenCode => "OpenCode",
            Self::AgentSkills => "Agent Skills",
        }
    }

    pub(super) fn help(self) -> &'static str {
        match self {
            Self::Codex => "Install the Codex skill and write Codex AGENTS.md steering.",
            Self::ClaudeCode => "Install the Claude Code skill and write Claude instruction files.",
            Self::Cursor => "Install the Cursor skill and write supported Cursor project steering.",
            Self::GitHubCopilot => {
                "Install the Copilot skill and write supported Copilot instructions."
            }
            Self::Windsurf => {
                "Install the Windsurf skill and write Cascade-compatible AGENTS.md steering."
            }
            Self::OpenCode => "Install the OpenCode skill and write OpenCode AGENTS.md steering.",
            Self::AgentSkills => {
                "Install the shared .agents skill format and write generic AGENTS.md steering."
            }
        }
    }

    pub(super) fn from_detected(agent: detect_agent::Agent) -> Option<Self> {
        match agent {
            detect_agent::Agent::Codex => Some(Self::Codex),
            detect_agent::Agent::ClaudeCode | detect_agent::Agent::ClaudeCodeCowork => {
                Some(Self::ClaudeCode)
            }
            detect_agent::Agent::Cursor | detect_agent::Agent::CursorCli => Some(Self::Cursor),
            detect_agent::Agent::GitHubCopilot => Some(Self::GitHubCopilot),
            detect_agent::Agent::OpenCode => Some(Self::OpenCode),
            detect_agent::Agent::Devin => Some(Self::Windsurf),
            detect_agent::Agent::GeminiCli
            | detect_agent::Agent::Augment
            | detect_agent::Agent::Antigravity
            | detect_agent::Agent::Replit
            | detect_agent::Agent::V0
            | detect_agent::Agent::Unknown => None,
        }
    }

    /// Whether this agent looks like it is already in use on this machine, so the
    /// picker can pre-select it. Looks for the agent's config directory under
    /// `$HOME`, then for an unambiguous per-repository marker.
    pub(super) fn in_use(self, home: Option<&Path>, repo: Option<&RepoInfo>) -> bool {
        // In use if the agent has config under $HOME, an unambiguous repo marker,
        // or a GitButler skill already installed for it — the last makes a re-run
        // of the wizard re-select agents it (or `but skill`) previously set up.
        if let Some(home) = home
            && (marker_exists(home, self.home_config_marker())
                || marker_exists(home, self.skill_path_components(Scope::Global)))
        {
            return true;
        }
        if let Some(repo) = repo
            && (marker_exists(&repo.root, self.repo_config_marker())
                || marker_exists(&repo.root, self.skill_path_components(Scope::Repository)))
        {
            return true;
        }
        false
    }

    /// The agent's config directory under `$HOME`; its presence means the agent
    /// is set up for this user.
    fn home_config_marker(self) -> Option<&'static [&'static str]> {
        Some(match self {
            Self::Codex => &[".codex"],
            Self::ClaudeCode => &[".claude"],
            Self::Cursor => &[".cursor"],
            Self::GitHubCopilot => &[".copilot"],
            Self::OpenCode => &[".config", "opencode"],
            Self::Windsurf => &[".codeium"],
            // The shared `.agents` format has no agent-specific config to detect.
            Self::AgentSkills => return None,
        })
    }

    /// An unambiguous per-repository marker for this agent. `AGENTS.md` is shared
    /// by several agents, so it is intentionally not treated as a marker.
    fn repo_config_marker(self) -> Option<&'static [&'static str]> {
        Some(match self {
            Self::ClaudeCode => &["CLAUDE.md"],
            Self::GitHubCopilot => &[".github", "copilot-instructions.md"],
            Self::Cursor => &[".cursor"],
            Self::Codex | Self::OpenCode | Self::Windsurf | Self::AgentSkills => return None,
        })
    }

    /// Where this agent's skill installs, relative to a base directory. Derived
    /// from `SKILL_FORMATS` in `crate::command::skill` so the wizard installs to
    /// the exact paths `but skill` discovers. Only the single-location scopes
    /// carry a path; `Both` is expanded into Global + Repository before this is
    /// called.
    pub(super) fn skill_path_components(self, scope: Scope) -> Option<&'static [&'static str]> {
        crate::command::skill::path_components_for(
            self.skill_format_name(),
            matches!(scope, Scope::Global),
        )
    }

    /// This agent's `SKILL_FORMATS` display name. The install paths themselves
    /// are the single source of truth in `crate::command::skill`.
    fn skill_format_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::Cursor => "Cursor",
            Self::GitHubCopilot => "GitHub Copilot",
            Self::Windsurf => "Windsurf",
            Self::OpenCode => "OpenCode",
            Self::AgentSkills => "Agent Skills",
        }
    }

    pub(super) fn shared_instruction_components(self) -> &'static [&'static str] {
        match self {
            // Cursor reads AGENTS.md without rule metadata, so prefer it over a
            // `.cursor/rules/*.mdc` file, which would need YAML frontmatter
            // (e.g. `alwaysApply: true`) to be loaded automatically.
            Self::Codex | Self::OpenCode | Self::AgentSkills | Self::Cursor | Self::Windsurf => {
                &["AGENTS.md"]
            }
            Self::ClaudeCode => &["CLAUDE.md"],
            Self::GitHubCopilot => &[".github", "copilot-instructions.md"],
        }
    }

    pub(super) fn global_instruction_components(self) -> Option<&'static [&'static str]> {
        match self {
            Self::Codex => Some(&[".codex", "AGENTS.md"]),
            Self::ClaudeCode => Some(&[".claude", "rules", "gitbutler.md"]),
            Self::GitHubCopilot => Some(&[".copilot", "copilot-instructions.md"]),
            Self::OpenCode => Some(&[".config", "opencode", "AGENTS.md"]),
            Self::Windsurf => Some(&[".codeium", "windsurf", "memories", "global_rules.md"]),
            Self::Cursor | Self::AgentSkills => None,
        }
    }
}

#[derive(Debug)]
pub(super) struct Plan {
    pub(super) scope: Scope,
    pub(super) policy: String,
    pub(super) skill_installs: Vec<SkillInstallPlan>,
    pub(super) instruction_writes: Vec<InstructionWritePlan>,
    pub(super) print_only_notes: Vec<String>,
    pub(super) setup_needed: bool,
}

impl Plan {
    pub(super) fn new(
        repo: Option<&RepoInfo>,
        scope: Scope,
        agents: Vec<AgentTarget>,
        policy: String,
    ) -> Result<Self> {
        let skill_installs = collect_skill_installs(&agents, scope, repo)?;
        let (instruction_writes, print_only_notes) =
            collect_instruction_writes(&agents, scope, repo)?;
        let setup_needed = repository_setup_needed(repo, scope);
        Ok(Self {
            scope,
            policy,
            skill_installs,
            instruction_writes,
            print_only_notes,
            setup_needed,
        })
    }
}

#[derive(Debug)]
pub(super) struct SkillInstallPlan {
    pub(super) agent: AgentTarget,
    pub(super) path: PathBuf,
}

#[derive(Debug, Clone)]
pub(super) struct InstructionWritePlan {
    pub(super) path: PathBuf,
    pub(super) agents: Vec<AgentTarget>,
}

pub(super) fn collect_skill_installs(
    agents: &[AgentTarget],
    scope: Scope,
    repo: Option<&RepoInfo>,
) -> Result<Vec<SkillInstallPlan>> {
    // Resolve each concrete install location (a single-location scope) to its
    // base directory once, expanding `Both` into global + repository.
    let mut locations: Vec<(Scope, PathBuf)> = Vec::new();
    if matches!(scope, Scope::Global | Scope::Both) {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        locations.push((Scope::Global, home));
    }
    if matches!(scope, Scope::Repository | Scope::Both) {
        let root = repo
            .map(|repo| repo.root.clone())
            .context("Repository skill install requested outside a repository")?;
        locations.push((Scope::Repository, root));
    }

    let mut installs = Vec::new();
    for agent in agents {
        for (location, base_dir) in &locations {
            if let Some(components) = agent.skill_path_components(*location) {
                installs.push(SkillInstallPlan {
                    agent: *agent,
                    path: join_components(base_dir, components),
                });
            }
        }
    }
    Ok(installs)
}

pub(super) fn repository_setup_needed(repo: Option<&RepoInfo>, scope: Scope) -> bool {
    repo.is_some_and(|repo| repo.needs_setup && matches!(scope, Scope::Repository | Scope::Both))
}

pub(super) fn collect_instruction_writes(
    agents: &[AgentTarget],
    scope: Scope,
    repo: Option<&RepoInfo>,
) -> Result<(Vec<InstructionWritePlan>, Vec<String>)> {
    let mut by_path: BTreeMap<PathBuf, Vec<AgentTarget>> = BTreeMap::new();
    let mut print_only_notes = Vec::new();
    let home = if matches!(scope, Scope::Global | Scope::Both) {
        Some(dirs::home_dir().context("Could not determine home directory")?)
    } else {
        None
    };
    for agent in agents {
        if matches!(scope, Scope::Repository | Scope::Both) {
            let repo = repo.context("Repository instructions requested outside a repository")?;
            by_path
                .entry(join_components(
                    &repo.root,
                    agent.shared_instruction_components(),
                ))
                .or_default()
                .push(*agent);
        }

        if let Some(home) = &home {
            if let Some(components) = agent.global_instruction_components() {
                by_path
                    .entry(join_components(home, components))
                    .or_default()
                    .push(*agent);
            } else {
                print_only_notes.push(format!(
                    "{} has no supported global instructions file; copy the generated policy below into it manually.",
                    agent.name()
                ));
            }
        }
    }

    Ok((
        by_path
            .into_iter()
            .map(|(path, agents)| InstructionWritePlan { path, agents })
            .collect(),
        print_only_notes,
    ))
}

fn join_components(base: &Path, components: &[&str]) -> PathBuf {
    components
        .iter()
        .fold(base.to_path_buf(), |path, component| path.join(component))
}

/// Whether `base` joined with `marker`'s components exists on disk. `None` marker
/// (the agent has no such location) is never present.
fn marker_exists(base: &Path, marker: Option<&'static [&'static str]>) -> bool {
    marker.is_some_and(|components| join_components(base, components).exists())
}

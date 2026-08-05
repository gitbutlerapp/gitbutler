//! The coding agents `but agent setup` can configure, and how to tell which of
//! them a user already works with.

use std::path::Path;

use crate::detect::Agent;
use crate::framework::{Framework, framework_by_name};
use crate::plan::{RepoInfo, Scope};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AgentTarget {
    Codex,
    ClaudeCode,
    Cursor,
    GitHubCopilot,
    Windsurf,
    OpenCode,
    Poolside,
    AgentSkills,
}

impl AgentTarget {
    pub const ALL: [Self; 8] = [
        Self::Codex,
        Self::ClaudeCode,
        Self::Cursor,
        Self::GitHubCopilot,
        Self::Windsurf,
        Self::OpenCode,
        Self::Poolside,
        Self::AgentSkills,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::Cursor => "Cursor",
            Self::GitHubCopilot => "GitHub Copilot",
            Self::Windsurf => "Windsurf",
            Self::OpenCode => "OpenCode",
            Self::Poolside => "Poolside",
            Self::AgentSkills => "Agent Skills",
        }
    }

    pub fn help(self) -> &'static str {
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
            Self::Poolside => "Install the Poolside skill and write Poolside AGENTS.md steering.",
            Self::AgentSkills => {
                "Install the shared .agents skill format and write generic AGENTS.md steering."
            }
        }
    }

    pub fn from_detected(agent: Agent) -> Option<Self> {
        match agent {
            Agent::Codex => Some(Self::Codex),
            Agent::ClaudeCode | Agent::ClaudeCodeCowork => Some(Self::ClaudeCode),
            Agent::Cursor | Agent::CursorCli => Some(Self::Cursor),
            Agent::GitHubCopilot => Some(Self::GitHubCopilot),
            Agent::OpenCode => Some(Self::OpenCode),
            Agent::Poolside => Some(Self::Poolside),
            Agent::Devin | Agent::Dirac => Some(Self::AgentSkills),
            Agent::GeminiCli
            | Agent::KiroCli
            | Agent::Junie
            | Agent::QwenCode
            | Agent::GitLabDuoCli
            | Agent::KiloCode
            | Agent::Hermes
            | Agent::Augment
            | Agent::Antigravity
            | Agent::Replit
            | Agent::V0
            | Agent::Crush
            | Agent::PulumiNeo
            | Agent::Goose
            | Agent::Amp
            | Agent::Cline
            | Agent::RooCode
            | Agent::Trae
            | Agent::TabnineCli
            | Agent::Pi
            | Agent::AmazonQ
            | Agent::CodeBuddy
            | Agent::GrokBuild
            | Agent::Warp
            | Agent::OpenHands
            | Agent::OpenClaw
            | Agent::Unknown => None,
        }
    }

    /// Whether this agent looks like it is already in use on this machine, so the
    /// picker can pre-select it. Looks for the agent's config directory under
    /// `$HOME`, then for an unambiguous per-repository marker.
    pub fn in_use(self, home: Option<&Path>, repo: Option<&RepoInfo>) -> bool {
        // In use if the agent has config under $HOME, an unambiguous repo marker,
        // or a GitButler skill already installed for it — the last makes a re-run
        // of the wizard re-select agents it (or `but skill`) previously set up.
        self.framework().in_use(home, repo)
    }

    /// This target's entry in the shared framework table, which owns the
    /// detection markers and instruction paths so the CLI wizard and the
    /// desktop app can never disagree about them.
    pub fn framework(self) -> &'static Framework {
        framework_by_name(self.skill_format_name())
            .expect("every AgentTarget has a framework entry")
    }

    /// Where this agent's skill installs, relative to a base directory. Derived
    /// from `SKILL_FORMATS` in `crate::command::skill` so the wizard installs to
    /// the exact paths `but skill` discovers. Only the single-location scopes
    /// carry a path; `Both` is expanded into Global + Repository before this is
    /// called.
    pub fn skill_path_components(self, scope: Scope) -> Option<&'static [&'static str]> {
        crate::format::path_components_for(self.skill_format_name(), matches!(scope, Scope::Global))
    }

    /// This agent's `SKILL_FORMATS` display name. The install paths themselves
    /// are the single source of truth in `crate::command::skill`.
    pub fn skill_format_name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::Cursor => "Cursor",
            Self::GitHubCopilot => "GitHub Copilot",
            Self::Windsurf => "Windsurf",
            Self::OpenCode => "OpenCode",
            Self::Poolside => "Poolside",
            Self::AgentSkills => "Agent Skills",
        }
    }

    pub fn shared_instruction_components(self) -> &'static [&'static str] {
        self.framework().repo_instructions
    }

    pub fn global_instruction_components(self) -> Option<&'static [&'static str]> {
        self.framework().global_instructions
    }
}

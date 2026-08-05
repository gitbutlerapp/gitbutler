//! The on-disk layout of a GitButler skill: the embedded files it is made of,
//! and every per-agent directory format it can be installed into.

use std::path::PathBuf;

use crate::detect::Agent;

// Embedded skill files
pub(crate) const SKILL_MD: &[u8] = include_bytes!("../skill/SKILL.md");
pub(crate) const CONCEPTS_MD: &[u8] = include_bytes!("../skill/references/concepts.md");
pub(crate) const EXAMPLES_MD: &[u8] = include_bytes!("../skill/references/examples.md");
pub(crate) const REFERENCE_MD: &[u8] = include_bytes!("../skill/references/reference.md");

/// Metadata for a skill file to be installed
pub struct SkillFile {
    /// Relative path components from install directory.
    pub path_components: &'static [&'static str],
    /// Embedded content
    pub content: &'static [u8],
    /// Display name for output
    pub display_name: &'static str,
}

impl SkillFile {
    /// Get the actual installation path given a base directory.
    pub fn get_install_path(&self, base_dir: &std::path::Path) -> PathBuf {
        join_relative_path(base_dir, self.path_components)
    }

    /// Format the relative path for output and JSON.
    pub fn display_path(&self) -> String {
        self.path_components.join("/")
    }

    /// True if this is the main SKILL.md entry point.
    pub fn is_main_skill_file(&self) -> bool {
        self.path_components == ["SKILL.md"]
    }
}

/// All skill files to be installed
pub const SKILL_FILES: &[SkillFile] = &[
    SkillFile {
        path_components: &["SKILL.md"],
        content: SKILL_MD,
        display_name: "SKILL.md",
    },
    SkillFile {
        path_components: &["references", "concepts.md"],
        content: CONCEPTS_MD,
        display_name: "concepts.md",
    },
    SkillFile {
        path_components: &["references", "examples.md"],
        content: EXAMPLES_MD,
        display_name: "examples.md",
    },
    SkillFile {
        path_components: &["references", "reference.md"],
        content: REFERENCE_MD,
        display_name: "reference.md",
    },
];

pub fn skill_files_in_write_order() -> impl Iterator<Item = &'static SkillFile> {
    SKILL_FILES
        .iter()
        .filter(|file| !file.is_main_skill_file())
        .chain(SKILL_FILES.iter().filter(|file| file.is_main_skill_file()))
}

/// Represents a skill installation location format
#[derive(Debug, Clone)]
pub struct SkillFormat {
    /// Display name of the format
    pub name: &'static str,
    /// Description of where this format is used
    pub description: &'static str,
    /// Whether this format should be offered for local and/or global installs.
    pub availability: SkillFormatAvailability,
    /// Relative path components from repository root or home directory.
    pub path_components: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillFormatAvailability {
    LocalAndGlobal,
    LocalOnly,
    GlobalOnly,
}

impl SkillFormat {
    const fn global(name: &'static str, path_components: &'static [&'static str]) -> Self {
        Self {
            name,
            description: "Agent-specific global skill format",
            availability: SkillFormatAvailability::GlobalOnly,
            path_components,
        }
    }

    /// Get the actual installation path given a base directory
    pub fn get_install_path(&self, base_dir: &std::path::Path) -> PathBuf {
        join_relative_path(base_dir, self.path_components)
    }

    /// The skills directory that holds this format's installation folder.
    pub fn skills_parent_dir(&self, base_dir: &std::path::Path) -> PathBuf {
        let (_, parent) = self
            .path_components
            .split_last()
            .expect("skill format path components are never empty");
        join_relative_path(base_dir, parent)
    }

    pub fn is_available_for(&self, global: bool) -> bool {
        matches!(
            (global, self.availability),
            (_, SkillFormatAvailability::LocalAndGlobal)
                | (false, SkillFormatAvailability::LocalOnly)
                | (true, SkillFormatAvailability::GlobalOnly)
        )
    }
}

/// Install-path components (relative to a base directory) for a skill format,
/// selected by its display name and whether the install is global. This keeps
/// callers outside `but skill` — e.g. the `agent setup` wizard — installing to
/// the same locations that `but skill check`/install/update discover, instead of
/// duplicating (and drifting from) these paths.
pub fn path_components_for(name: &str, global: bool) -> Option<&'static [&'static str]> {
    skill_format_for_name(name, global).map(|format| format.path_components)
}

pub fn skill_format_for_name(name: &str, global: bool) -> Option<&'static SkillFormat> {
    SKILL_FORMATS
        .iter()
        .find(|format| format.name == name && format.is_available_for(global))
}

/// Join a relative path from components using platform-native separators.
pub fn join_relative_path(base_dir: &std::path::Path, components: &[&str]) -> PathBuf {
    components
        .iter()
        .fold(base_dir.to_path_buf(), |path, component| {
            path.join(component)
        })
}

// Common skill folder formats
pub const SKILL_FORMATS: &[SkillFormat] = &[
    SkillFormat {
        name: "Agent Skills",
        description: "Shared .agents/skills format",
        availability: SkillFormatAvailability::LocalAndGlobal,
        path_components: &[".agents", "skills", "gitbutler"],
    },
    SkillFormat {
        name: "Claude Code",
        description: "Claude Code CLI skill format",
        availability: SkillFormatAvailability::LocalAndGlobal,
        path_components: &[".claude", "skills", "gitbutler"],
    },
    SkillFormat {
        name: "OpenCode",
        description: "OpenCode local skill format",
        availability: SkillFormatAvailability::LocalOnly,
        path_components: &[".opencode", "skills", "gitbutler"],
    },
    SkillFormat::global("OpenCode", &[".config", "opencode", "skills", "gitbutler"]),
    SkillFormat {
        name: "Codex",
        description: "Codex skill format",
        availability: SkillFormatAvailability::LocalAndGlobal,
        path_components: &[".codex", "skills", "gitbutler"],
    },
    SkillFormat {
        name: "GitHub Copilot",
        description: "GitHub Copilot local (repo) skill format",
        availability: SkillFormatAvailability::LocalOnly,
        path_components: &[".github", "skills", "gitbutler"],
    },
    SkillFormat {
        name: "GitHub Copilot",
        description: "GitHub Copilot global skill format",
        availability: SkillFormatAvailability::GlobalOnly,
        path_components: &[".copilot", "skills", "gitbutler"],
    },
    SkillFormat {
        name: "Cursor",
        description: "Cursor AI skill format",
        availability: SkillFormatAvailability::LocalAndGlobal,
        path_components: &[".cursor", "skills", "gitbutler"],
    },
    SkillFormat {
        name: "Kiro",
        description: "Kiro skill format",
        availability: SkillFormatAvailability::LocalAndGlobal,
        path_components: &[".kiro", "skills", "gitbutler"],
    },
    SkillFormat {
        name: "Junie",
        description: "Junie skill format",
        availability: SkillFormatAvailability::LocalAndGlobal,
        path_components: &[".junie", "skills", "gitbutler"],
    },
    SkillFormat {
        name: "Windsurf",
        description: "Windsurf local skill format",
        availability: SkillFormatAvailability::LocalOnly,
        path_components: &[".windsurf", "skills", "gitbutler"],
    },
    SkillFormat::global("Windsurf", &[".codeium", "windsurf", "skills", "gitbutler"]),
    SkillFormat {
        name: "Poolside",
        description: "Poolside local skill format",
        availability: SkillFormatAvailability::LocalOnly,
        path_components: &[".poolside", "skills", "but"],
    },
    SkillFormat::global("Poolside", &[".config", "poolside", "skills", "but"]),
    SkillFormat::global("Gemini CLI", &[".gemini", "skills", "gitbutler"]),
    SkillFormat::global("Augment", &[".augment", "skills", "gitbutler"]),
    SkillFormat::global(
        "Antigravity",
        &[".gemini", "antigravity", "skills", "gitbutler"],
    ),
    SkillFormat::global(
        "Universal Agents",
        &[".config", "agents", "skills", "gitbutler"],
    ),
    SkillFormat::global("Crush", &[".config", "crush", "skills", "gitbutler"]),
    SkillFormat::global("Goose", &[".config", "goose", "skills", "gitbutler"]),
    SkillFormat::global("Roo Code", &[".roo", "skills", "gitbutler"]),
    SkillFormat::global("Trae", &[".trae", "skills", "gitbutler"]),
    SkillFormat::global("Tabnine CLI", &[".tabnine", "agent", "skills", "gitbutler"]),
    SkillFormat::global("Pi", &[".pi", "agent", "skills", "gitbutler"]),
    SkillFormat::global("Devin", &[".config", "devin", "skills", "gitbutler"]),
];

pub fn skill_format_for_agent(agent: Agent, global: bool) -> Option<&'static SkillFormat> {
    let name = match agent {
        Agent::Codex => "Codex",
        Agent::ClaudeCode | Agent::ClaudeCodeCowork => "Claude Code",
        Agent::Cursor | Agent::CursorCli => "Cursor",
        Agent::GitHubCopilot => "GitHub Copilot",
        Agent::OpenCode => "OpenCode",
        Agent::Poolside => "Poolside",
        Agent::GeminiCli => "Gemini CLI",
        Agent::Augment => "Augment",
        Agent::Antigravity => "Antigravity",
        Agent::Replit | Agent::Amp => "Universal Agents",
        Agent::Crush => "Crush",
        Agent::Goose => "Goose",
        Agent::Cline | Agent::Dirac => "Agent Skills",
        Agent::RooCode => "Roo Code",
        Agent::Trae => "Trae",
        Agent::TabnineCli => "Tabnine CLI",
        Agent::Pi => "Pi",
        Agent::Devin => "Devin",
        Agent::KiroCli => "Kiro",
        Agent::Junie => "Junie",
        Agent::QwenCode
        | Agent::GitLabDuoCli
        | Agent::KiloCode
        | Agent::Hermes
        | Agent::V0
        | Agent::PulumiNeo
        | Agent::AmazonQ
        | Agent::CodeBuddy
        | Agent::GrokBuild
        | Agent::Warp
        | Agent::OpenHands
        | Agent::OpenClaw
        | Agent::Unknown => return None,
    };
    skill_format_for_name(name, global)
}

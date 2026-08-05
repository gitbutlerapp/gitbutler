//! The coding-agent frameworks GitButler knows how to set up.
//!
//! [`SKILL_FORMATS`](crate::format::SKILL_FORMATS) answers "where does a skill
//! install"; this answers "who is this agent, does the user actually use it,
//! and which file holds its steering instructions". They are separate tables
//! because several frameworks have *two* skill formats (a repo-local and a
//! global one) while detection markers and instruction files are per-framework.
//!
//! Markers are written out explicitly rather than derived from install paths.
//! Deriving them looks tempting but is wrong: GitHub Copilot's local skill path
//! is `.github/skills/gitbutler`, so a derived repo marker would be `.github` —
//! present in essentially every repository on GitHub.

use std::path::Path;

use crate::{
    format::{SKILL_FORMATS, SkillFormat},
    plan::{RepoInfo, Scope, join_components},
};

/// A coding agent GitButler can install a skill for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Framework {
    /// Stable kebab-case identifier. This is the key mutating APIs take, so it
    /// must not change once shipped.
    pub id: &'static str,
    /// The [`SkillFormat::name`] this framework installs under.
    pub name: &'static str,
    /// One-line description for a settings row.
    pub description: &'static str,
    /// Config location under `$HOME` whose presence means the user has this
    /// agent set up. `None` for formats with nothing agent-specific to find.
    pub home_marker: Option<&'static [&'static str]>,
    /// An unambiguous per-repository marker. `AGENTS.md` is deliberately never
    /// used: it is shared by many agents, so it proves nothing about any one.
    pub repo_marker: Option<&'static [&'static str]>,
    /// The repository-scoped instruction file the managed policy block goes in.
    pub repo_instructions: &'static [&'static str],
    /// The global instruction file, when this agent has a supported one.
    /// `None` means the policy can only be shown for manual copying.
    pub global_instructions: Option<&'static [&'static str]>,
}

/// Shorthand for the common case: an agent whose steering lives in the shared
/// repo-level `AGENTS.md` and which has no known global instruction file.
const fn agents_md(
    id: &'static str,
    name: &'static str,
    description: &'static str,
    home_marker: Option<&'static [&'static str]>,
) -> Framework {
    Framework {
        id,
        name,
        description,
        home_marker,
        repo_marker: None,
        repo_instructions: &["AGENTS.md"],
        global_instructions: None,
    }
}

/// Every framework, in the order the UI should present them.
///
/// The first eight are the ones `but agent setup` offers interactively; the
/// rest can install skills but are not part of the wizard's curated list.
pub const FRAMEWORKS: &[Framework] = &[
    Framework {
        id: "codex",
        name: "Codex",
        description: "Install the Codex skill and write Codex AGENTS.md steering.",
        home_marker: Some(&[".codex"]),
        repo_marker: None,
        repo_instructions: &["AGENTS.md"],
        global_instructions: Some(&[".codex", "AGENTS.md"]),
    },
    Framework {
        id: "claude-code",
        name: "Claude Code",
        description: "Install the Claude Code skill and write Claude instruction files.",
        home_marker: Some(&[".claude"]),
        repo_marker: Some(&["CLAUDE.md"]),
        repo_instructions: &["CLAUDE.md"],
        global_instructions: Some(&[".claude", "rules", "gitbutler.md"]),
    },
    Framework {
        // Cursor reads AGENTS.md without rule metadata, so prefer it over a
        // `.cursor/rules/*.mdc` file, which would need YAML frontmatter
        // (e.g. `alwaysApply: true`) to be loaded automatically.
        id: "cursor",
        name: "Cursor",
        description: "Install the Cursor skill and write supported Cursor project steering.",
        home_marker: Some(&[".cursor"]),
        repo_marker: Some(&[".cursor"]),
        repo_instructions: &["AGENTS.md"],
        global_instructions: None,
    },
    Framework {
        id: "github-copilot",
        name: "GitHub Copilot",
        description: "Install the Copilot skill and write supported Copilot instructions.",
        home_marker: Some(&[".copilot"]),
        repo_marker: Some(&[".github", "copilot-instructions.md"]),
        repo_instructions: &[".github", "copilot-instructions.md"],
        global_instructions: Some(&[".copilot", "copilot-instructions.md"]),
    },
    Framework {
        id: "windsurf",
        name: "Windsurf",
        description: "Install the Windsurf skill and write Cascade-compatible AGENTS.md steering.",
        home_marker: Some(&[".codeium"]),
        repo_marker: None,
        repo_instructions: &["AGENTS.md"],
        global_instructions: Some(&[".codeium", "windsurf", "memories", "global_rules.md"]),
    },
    Framework {
        id: "opencode",
        name: "OpenCode",
        description: "Install the OpenCode skill and write OpenCode AGENTS.md steering.",
        home_marker: Some(&[".config", "opencode"]),
        repo_marker: None,
        repo_instructions: &["AGENTS.md"],
        global_instructions: Some(&[".config", "opencode", "AGENTS.md"]),
    },
    Framework {
        id: "poolside",
        name: "Poolside",
        description: "Install the Poolside skill and write Poolside AGENTS.md steering.",
        home_marker: Some(&[".config", "poolside"]),
        repo_marker: Some(&[".poolside"]),
        repo_instructions: &["AGENTS.md"],
        global_instructions: Some(&[".config", "poolside", "AGENTS.md"]),
    },
    Framework {
        // The shared `.agents` format has no agent-specific config to detect.
        id: "agent-skills",
        name: "Agent Skills",
        description: "Install the shared .agents skill format and write generic AGENTS.md steering.",
        home_marker: None,
        repo_marker: None,
        repo_instructions: &["AGENTS.md"],
        global_instructions: None,
    },
    // Frameworks below install skills but are not offered by the CLI wizard.
    // None has a documented global instruction file we are confident writing
    // to, so their policy is shown for manual copying rather than guessed at.
    Framework {
        id: "kiro",
        name: "Kiro",
        description: "Install the Kiro skill.",
        home_marker: Some(&[".kiro"]),
        repo_marker: Some(&[".kiro"]),
        repo_instructions: &["AGENTS.md"],
        global_instructions: None,
    },
    Framework {
        id: "junie",
        name: "Junie",
        description: "Install the Junie skill.",
        home_marker: Some(&[".junie"]),
        repo_marker: Some(&[".junie"]),
        repo_instructions: &["AGENTS.md"],
        global_instructions: None,
    },
    agents_md(
        "gemini-cli",
        "Gemini CLI",
        "Install the Gemini CLI skill.",
        Some(&[".gemini"]),
    ),
    agents_md(
        "augment",
        "Augment",
        "Install the Augment skill.",
        Some(&[".augment"]),
    ),
    agents_md(
        "antigravity",
        "Antigravity",
        "Install the Antigravity skill.",
        Some(&[".gemini", "antigravity"]),
    ),
    agents_md(
        "universal-agents",
        "Universal Agents",
        "Install the shared ~/.config/agents skill format.",
        Some(&[".config", "agents"]),
    ),
    agents_md(
        "crush",
        "Crush",
        "Install the Crush skill.",
        Some(&[".config", "crush"]),
    ),
    agents_md(
        "goose",
        "Goose",
        "Install the Goose skill.",
        Some(&[".config", "goose"]),
    ),
    agents_md(
        "roo-code",
        "Roo Code",
        "Install the Roo Code skill.",
        Some(&[".roo"]),
    ),
    agents_md("trae", "Trae", "Install the Trae skill.", Some(&[".trae"])),
    agents_md(
        "tabnine-cli",
        "Tabnine CLI",
        "Install the Tabnine CLI skill.",
        Some(&[".tabnine"]),
    ),
    agents_md("pi", "Pi", "Install the Pi skill.", Some(&[".pi"])),
    agents_md(
        "devin",
        "Devin",
        "Install the Devin skill.",
        Some(&[".config", "devin"]),
    ),
];

/// Look a framework up by its stable id.
pub fn framework_by_id(id: &str) -> Option<&'static Framework> {
    FRAMEWORKS.iter().find(|framework| framework.id == id)
}

/// Look a framework up by its [`SkillFormat::name`].
pub fn framework_by_name(name: &str) -> Option<&'static Framework> {
    FRAMEWORKS.iter().find(|framework| framework.name == name)
}

impl Framework {
    /// The skill formats this framework offers at `global` scope, if any.
    pub fn format(&self, global: bool) -> Option<&'static SkillFormat> {
        SKILL_FORMATS
            .iter()
            .find(|format| format.name == self.name && format.is_available_for(global))
    }

    /// Whether a skill can be installed for this framework at `scope`.
    pub fn supports(&self, scope: Scope) -> bool {
        match scope {
            Scope::Global => self.format(true).is_some(),
            Scope::Repository => self.format(false).is_some(),
            Scope::Both => self.format(true).is_some() || self.format(false).is_some(),
        }
    }

    /// The instruction file for `scope`, relative components only. `None` when
    /// this framework has no supported file at that scope.
    pub fn instruction_components(&self, scope: Scope) -> Option<&'static [&'static str]> {
        match scope {
            Scope::Repository => Some(self.repo_instructions),
            Scope::Global => self.global_instructions,
            // `Both` is expanded into single scopes before this is reached.
            Scope::Both => None,
        }
    }

    /// Whether this agent looks like it is already in use on this machine, so
    /// the UI can pre-select it. Looks for the agent's config under `$HOME`,
    /// then for an unambiguous per-repository marker, then for a GitButler
    /// skill already installed for it — the last makes a re-run re-select
    /// agents that `but skill` previously set up.
    pub fn in_use(&self, home: Option<&Path>, repo: Option<&RepoInfo>) -> bool {
        self.detected_globally(home) || self.detected_in_repo(repo)
    }

    /// Whether the user's home directory shows this agent in use.
    pub fn detected_globally(&self, home: Option<&Path>) -> bool {
        let Some(home) = home else { return false };
        marker_exists(home, self.home_marker)
            || marker_exists(home, self.skill_path_components(Scope::Global))
    }

    /// Whether the repository shows this agent in use.
    pub fn detected_in_repo(&self, repo: Option<&RepoInfo>) -> bool {
        let Some(repo) = repo else { return false };
        marker_exists(&repo.root, self.repo_marker)
            || marker_exists(&repo.root, self.skill_path_components(Scope::Repository))
    }

    /// Where this framework's skill installs, relative to a base directory.
    pub fn skill_path_components(&self, scope: Scope) -> Option<&'static [&'static str]> {
        self.format(matches!(scope, Scope::Global))
            .map(|format| format.path_components)
    }
}

/// Whether `base` joined with `marker`'s components exists on disk. A `None`
/// marker (the agent has no such location) is never present.
fn marker_exists(base: &Path, marker: Option<&'static [&'static str]>) -> bool {
    marker.is_some_and(|components| join_components(base, components).exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_framework_has_a_skill_format_in_some_scope() {
        for framework in FRAMEWORKS {
            assert!(
                framework.supports(Scope::Both),
                "{} should install somewhere",
                framework.id
            );
        }
    }

    #[test]
    fn ids_and_names_are_unique() {
        for (i, framework) in FRAMEWORKS.iter().enumerate() {
            for other in &FRAMEWORKS[i + 1..] {
                assert_ne!(framework.id, other.id, "duplicate id");
                assert_ne!(framework.name, other.name, "duplicate name");
            }
        }
    }

    #[test]
    fn every_skill_format_name_maps_to_a_framework() {
        for format in SKILL_FORMATS {
            assert!(
                framework_by_name(format.name).is_some(),
                "skill format {} has no framework entry",
                format.name
            );
        }
    }

    /// `.github` exists in almost every repository on GitHub, so it must never
    /// on its own imply the user works with Copilot.
    #[test]
    fn a_bare_dot_github_directory_is_not_a_copilot_marker() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".github")).unwrap();
        let repo = RepoInfo {
            root: dir.path().to_path_buf(),
            needs_setup: false,
        };

        let copilot = framework_by_id("github-copilot").unwrap();
        assert!(
            !copilot.detected_in_repo(Some(&repo)),
            "a bare .github directory should not count"
        );

        std::fs::write(
            dir.path().join(".github").join("copilot-instructions.md"),
            "hi",
        )
        .unwrap();
        assert!(
            copilot.detected_in_repo(Some(&repo)),
            "the instructions file is the real marker"
        );
    }

    /// `AGENTS.md` is shared by many agents, so it proves nothing about any one.
    #[test]
    fn a_shared_agents_md_is_not_a_marker_for_anyone() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("AGENTS.md"), "shared").unwrap();
        let repo = RepoInfo {
            root: dir.path().to_path_buf(),
            needs_setup: false,
        };

        for framework in FRAMEWORKS {
            assert!(
                !framework.detected_in_repo(Some(&repo)),
                "{} should not be detected from a shared AGENTS.md",
                framework.id
            );
        }
    }

    #[test]
    fn home_config_directory_marks_an_agent_in_use() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude")).unwrap();

        let claude = framework_by_id("claude-code").unwrap();
        assert!(claude.detected_globally(Some(dir.path())));
        assert!(
            !framework_by_id("codex")
                .unwrap()
                .detected_globally(Some(dir.path())),
            "an unrelated agent's marker must not match"
        );
    }
}

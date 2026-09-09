use std::{fmt::Write as _, path::PathBuf};

use anyhow::{Context as _, Result};
use but_ctx::Context;
// Test-override-aware (`E2E_TEST_APP_DATA_DIR`), so skill discovery and
// installation never touch the real home directory under test.
use but_path::home_dir;
use serde::Serialize;

use crate::{
    args::skill,
    theme::{self, Paint},
    utils::{OutputChannel, detect_agent::Agent},
};

mod freshness;
mod reference;
use freshness::agent_default_install_path;
pub(crate) use freshness::{agent_skill_notice, agent_skill_update_notice};

/// Error type for user-initiated cancellation
#[derive(Debug, Clone, Copy)]
struct UserCancelled;

impl std::fmt::Display for UserCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Installation cancelled by user")
    }
}

impl std::error::Error for UserCancelled {}

// Embedded skill files
const SKILL_MD: &[u8] = include_bytes!("../../../skill/SKILL.md");
const CONCEPTS_MD: &[u8] = include_bytes!("../../../skill/references/concepts.md");
const EXAMPLES_MD: &[u8] = include_bytes!("../../../skill/references/examples.md");
const REFERENCE_MD: &[u8] = include_bytes!("../../../skill/references/reference.md");
/// Body of a stub SKILL.md: the full skill's frontmatter followed by this.
const STUB_MD: &str = include_str!("../../../skill/stub.md");

/// Metadata for a skill file to be installed
struct SkillFile {
    /// Relative path components from install directory.
    path_components: &'static [&'static str],
    /// Embedded content
    content: &'static [u8],
    /// Name of the document as `but skill <name>` prints it.
    name: &'static str,
    /// Renders the text `but skill <name>` prints when it is not the embedded
    /// content: the command reference comes from the clap tree so it always
    /// matches the binary, while the installed file stays hand-written.
    served: Option<fn() -> String>,
}

impl SkillFile {
    /// Get the actual installation path given a base directory.
    fn get_install_path(&self, base_dir: &std::path::Path) -> PathBuf {
        join_relative_path(base_dir, self.path_components)
    }

    /// Format the relative path for output and JSON.
    fn display_path(&self) -> String {
        self.path_components.join("/")
    }

    /// True if this is the main SKILL.md entry point.
    fn is_main_skill_file(&self) -> bool {
        self.path_components == ["SKILL.md"]
    }

    /// The embedded content as text; the sources are Markdown and must be UTF-8.
    fn text(&self) -> Result<&'static str> {
        std::str::from_utf8(self.content)
            .with_context(|| format!("{} is not valid UTF-8", self.display_path()))
    }

    /// The text `but skill <name>` prints.
    fn served_text(&self) -> Result<String> {
        match self.served {
            Some(render) => Ok(render()),
            None => self.text().map(str::to_owned),
        }
    }
}

/// All skill files to be installed, in the order `but skill --full` prints them.
const SKILL_FILES: &[SkillFile] = &[
    SkillFile {
        path_components: &["SKILL.md"],
        content: SKILL_MD,
        name: "core",
        served: None,
    },
    SkillFile {
        path_components: &["references", "reference.md"],
        content: REFERENCE_MD,
        name: "reference",
        served: Some(reference::render),
    },
    SkillFile {
        path_components: &["references", "concepts.md"],
        content: CONCEPTS_MD,
        name: "concepts",
        served: None,
    },
    SkillFile {
        path_components: &["references", "examples.md"],
        content: EXAMPLES_MD,
        name: "examples",
        served: None,
    },
];

/// Which files an installation carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkillLayout {
    /// SKILL.md plus every reference file.
    Full,
    /// A single SKILL.md whose body tells the agent to run `but skill`. Carries
    /// the same frontmatter as the full skill plus `stub: true` and a
    /// pre-approval for the `but skill` read so the redirect never waits on
    /// a permission prompt.
    Stub,
}

/// The files a layout consists of, SKILL.md last so a partial write never
/// looks like a complete installation.
fn skill_files_in_write_order(layout: SkillLayout) -> impl Iterator<Item = &'static SkillFile> {
    SKILL_FILES
        .iter()
        .filter(move |file| layout == SkillLayout::Full && !file.is_main_skill_file())
        .chain(SKILL_FILES.iter().filter(|file| file.is_main_skill_file()))
}

/// Represents a skill installation location format
#[derive(Debug, Clone)]
struct SkillFormat {
    /// Display name of the format
    name: &'static str,
    /// Description of where this format is used
    description: &'static str,
    /// Whether this format should be offered for local and/or global installs.
    availability: SkillFormatAvailability,
    /// Relative path components from repository root or home directory.
    path_components: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillFormatAvailability {
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
    fn get_install_path(&self, base_dir: &std::path::Path) -> PathBuf {
        join_relative_path(base_dir, self.path_components)
    }

    /// The skills directory that holds this format's installation folder.
    fn skills_parent_dir(&self, base_dir: &std::path::Path) -> PathBuf {
        let (_, parent) = self
            .path_components
            .split_last()
            .expect("skill format path components are never empty");
        join_relative_path(base_dir, parent)
    }

    fn is_available_for(&self, global: bool) -> bool {
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
pub(crate) fn path_components_for(name: &str, global: bool) -> Option<&'static [&'static str]> {
    skill_format_for_name(name, global).map(|format| format.path_components)
}

fn skill_format_for_name(name: &str, global: bool) -> Option<&'static SkillFormat> {
    SKILL_FORMATS
        .iter()
        .find(|format| format.name == name && format.is_available_for(global))
}

/// Join a relative path from components using platform-native separators.
fn join_relative_path(base_dir: &std::path::Path, components: &[&str]) -> PathBuf {
    components
        .iter()
        .fold(base_dir.to_path_buf(), |path, component| {
            path.join(component)
        })
}

// Common skill folder formats
const SKILL_FORMATS: &[SkillFormat] = &[
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

fn skill_format_for_agent(agent: Agent, global: bool) -> Option<&'static SkillFormat> {
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
        Agent::Cline | Agent::Dirac | Agent::DeepSeekHarness => "Agent Skills",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallScope {
    Local,
    Global,
}

impl InstallScope {
    fn is_global(self) -> bool {
        matches!(self, Self::Global)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallScopeResolution {
    PromptUser,
    Fixed(InstallScope),
}

fn determine_install_scope_resolution(
    global: bool,
    local_scope_available: bool,
) -> InstallScopeResolution {
    if global {
        InstallScopeResolution::Fixed(InstallScope::Global)
    } else if local_scope_available {
        InstallScopeResolution::PromptUser
    } else {
        InstallScopeResolution::Fixed(InstallScope::Global)
    }
}

#[derive(Debug, Clone, Copy)]
enum InstallScopeOption {
    Local,
    Global,
}

impl From<InstallScopeOption> for String {
    fn from(value: InstallScopeOption) -> Self {
        match value {
            InstallScopeOption::Local => "Local (repository)".to_string(),
            InstallScopeOption::Global => "Global (home directory)".to_string(),
        }
    }
}

/// Status of an installed skill
#[derive(Debug, Clone, Serialize)]
pub struct SkillStatus {
    /// Path to the skill installation directory
    pub path: PathBuf,
    /// The format name (e.g., "Claude Code", "Cursor")
    pub format_name: String,
    /// Scope of the installation ("local" or "global")
    pub scope: String,
    /// Version found in the installed SKILL.md
    pub installed_version: String,
    /// Whether the skill is up to date with the CLI
    pub up_to_date: bool,
}

/// Result of checking all skills
#[derive(Debug, Serialize)]
pub struct SkillCheckResult {
    /// Current CLI version
    pub cli_version: String,
    /// List of all found skill installations with their status
    pub skills: Vec<SkillStatus>,
    /// Number of outdated skills
    pub outdated_count: usize,
}

/// Handle skill subcommands. Bare `but skill` prints the core guide.
pub fn handle(
    current_dir: &std::path::Path,
    out: &mut OutputChannel,
    platform: skill::Platform,
) -> Result<()> {
    if let Some(doc) = platform.doc() {
        return print_doc(out, doc, platform.full);
    }
    match platform.cmd {
        Some(skill::Subcommands::Install {
            global,
            path,
            detect,
            stub,
        }) => {
            let layout = if stub {
                SkillLayout::Stub
            } else {
                SkillLayout::Full
            };
            match install_skill(current_dir, out, global, path, detect, layout) {
                Err(error) if error.downcast_ref::<UserCancelled>().is_some() => Ok(()),
                result => result,
            }
        }
        Some(skill::Subcommands::Check {
            global,
            local,
            update,
        }) => check_skills(current_dir, out, global, local, update),
        None
        | Some(
            skill::Subcommands::Reference
            | skill::Subcommands::Concepts
            | skill::Subcommands::Examples,
        ) => unreachable!("read-only invocations are handled by Platform::doc"),
    }
}

/// Print one embedded skill document, with every reference appended when
/// `full` is set. Content goes straight to the output channel with no pager so
/// an agent can read it in one call.
fn print_doc(out: &mut OutputChannel, name: &str, full: bool) -> Result<()> {
    let file = SKILL_FILES
        .iter()
        .find(|file| file.name == name)
        .expect("every doc subcommand names an embedded skill file");
    let content = file.served_text()?;
    // The frontmatter is trigger text for the harness, not guidance. Only
    // SKILL.md carries one; a `---` rule inside a reference must stay.
    let body = match frontmatter_close(&content) {
        Some(close) if file.is_main_skill_file() => &content[close.end..],
        _ => content.as_str(),
    };
    // Validate every reference before printing anything, so a bad embed
    // fails loudly instead of producing a partial or altered document.
    let references: Vec<(&SkillFile, String)> = if full {
        SKILL_FILES
            .iter()
            .filter(|file| !file.is_main_skill_file())
            .map(|file| Ok((file, file.served_text()?)))
            .collect::<Result<_>>()?
    } else {
        Vec::new()
    };
    if let Some(writer) = out.for_human() {
        write!(writer, "{body}")?;
        for (reference, text) in &references {
            writeln!(writer, "\n--- but skill {} ---\n", reference.name)?;
            write!(writer, "{text}")?;
        }
    }
    if let Some(out) = out.for_json() {
        let docs: Vec<serde_json::Value> = references
            .iter()
            .map(|(reference, text)| serde_json::json!({ "name": reference.name, "content": text }))
            .collect();
        out.write_value(serde_json::json!({ "name": name, "content": body, "references": docs }))?;
    }
    Ok(())
}

/// The repository around `current_dir`, `None` when there is none. Any other
/// discovery failure is an error, so a corrupt repository is reported as such.
fn repository_context(current_dir: &std::path::Path) -> Result<Option<Context>> {
    match Context::discover(current_dir) {
        Ok(ctx) => Ok(Some(ctx)),
        Err(err) if crate::is_not_in_git_repository_error(&err) => Ok(None),
        Err(err) => Err(err),
    }
}

/// Expand tilde in path to home directory
fn expand_tilde(path_str: &str) -> Option<PathBuf> {
    if path_str == "~" || path_str.starts_with("~/") || path_str.starts_with("~\\") {
        home_dir().map(|home| {
            if path_str == "~" {
                home
            } else {
                home.join(&path_str[2..])
            }
        })
    } else {
        None
    }
}

/// Get the base directory for installation (repo root or home directory)
fn get_base_dir(ctx: Option<&mut Context>, global: bool) -> Result<PathBuf> {
    if global {
        home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))
    } else {
        let ctx = ctx.ok_or_else(|| {
            anyhow::anyhow!(
                "Not in a git repository. Use --global to install globally, or run from within a repository."
            )
        })?;
        let repo = ctx.repo.get()?;
        repo.workdir()
            .ok_or_else(|| anyhow::anyhow!("Not in a Git repository"))
            .map(|p| p.to_path_buf())
    }
}

/// Byte range of the frontmatter's closing `---` line plus the blank line
/// after it, for any line ending (Unix, Windows, or old Mac). `None` unless
/// the content opens with a frontmatter block, so a Markdown `---` rule in a
/// document without one is never mistaken for the delimiter.
fn frontmatter_close(content: &str) -> Option<std::ops::Range<usize>> {
    if !(content.starts_with("---\n") || content.starts_with("---\r")) {
        return None;
    }
    let after_opener = "---".len();
    ["---\n\n", "---\r\n\r\n", "---\r\r"]
        .into_iter()
        .find_map(|delimiter| {
            content[after_opener..]
                .find(delimiter)
                .map(|start| after_opener + start..after_opener + start + delimiter.len())
        })
}

/// Replace version in SKILL.md content
fn inject_version(content: &str, version: &str) -> String {
    let replace = |text: &str| text.replace("version: 0.0.0", &format!("version: {version}"));
    match frontmatter_close(content) {
        Some(close) => format!(
            "{}{}",
            replace(&content[..close.start]),
            &content[close.start..]
        ),
        // Fallback if frontmatter format is unexpected
        None => replace(content),
    }
}

/// Resolve custom path with tilde expansion and relative path handling
fn resolve_custom_path(custom: &str, ctx: Option<&mut Context>, global: bool) -> Result<PathBuf> {
    let path = std::path::Path::new(custom);

    // Try tilde expansion first
    let expanded_path = expand_tilde(custom).unwrap_or_else(|| path.to_path_buf());

    if expanded_path.is_absolute() {
        Ok(expanded_path)
    } else {
        // Relative path - join with base directory
        let base_dir = get_base_dir(ctx, global)?;
        Ok(base_dir.join(expanded_path))
    }
}

/// Validate that a SKILL.md file is actually a GitButler skill by requiring
/// `name: but` in its YAML frontmatter.
///
/// The check is deliberately strict: discovery scans every entry inside an
/// agent's `skills` directory, so a looser match (the string `name: but`
/// appearing in prose, or a `# GitButler CLI Skill` header in another skill's
/// docs) could misclassify an unrelated skill and let `--detect`/`--update`
/// overwrite it.
fn is_gitbutler_skill(skill_md_path: &std::path::Path) -> bool {
    std::fs::read_to_string(skill_md_path)
        .ok()
        .and_then(|content| frontmatter_value(&content, "name:"))
        .as_deref()
        == Some("but")
}

/// The layout of the SKILL.md at `skill_md_path`: a stub written by `--stub`
/// carries `stub: true` in its frontmatter; anything else, including no file
/// at all, counts as the full layout.
fn installed_layout(skill_md_path: &std::path::Path) -> SkillLayout {
    let is_stub = std::fs::read_to_string(skill_md_path)
        .ok()
        .and_then(|content| frontmatter_value(&content, "stub:"))
        .as_deref()
        == Some("true");
    if is_stub {
        SkillLayout::Stub
    } else {
        SkillLayout::Full
    }
}

/// Extract the version from an installed SKILL.md file's YAML frontmatter.
/// Returns None if the file doesn't exist, isn't readable, or has no valid version.
fn extract_installed_version(skill_md_path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(skill_md_path).ok()?;
    extract_installed_version_from_content(&content)
}

/// Extract the version from YAML frontmatter content.
/// Returns None if the content has no frontmatter or no version entry.
fn extract_installed_version_from_content(content: &str) -> Option<String> {
    frontmatter_value(content, "version:")
}

/// Read a top-level `key` (e.g. `"name:"`, `"version:"`) from the leading YAML
/// frontmatter and return its parsed value. None if the content has no
/// frontmatter or the key is absent.
fn frontmatter_value(content: &str, key: &str) -> Option<String> {
    let mut lines = content.lines();

    // Frontmatter must open on the very first line.
    if lines.next()? != "---" {
        return None;
    }

    for line in lines {
        if line == "---" {
            break;
        }
        if let Some(value) = line.strip_prefix(key) {
            return Some(parse_yaml_value(value));
        }
    }

    None
}

/// Parse a simple YAML value, handling common cases:
/// - Whitespace trimming
/// - Quoted strings (single or double quotes)
/// - Inline comments
fn parse_yaml_value(value: &str) -> String {
    let value = value.trim();

    // Handle quoted strings
    if value.starts_with('"') || value.starts_with('\'') {
        let quote_char = value.chars().next().unwrap();
        // Find the closing quote
        if let Some(end) = value[1..].find(quote_char) {
            return value[1..1 + end].to_string();
        }
    }

    // Handle inline comments (but not inside quotes, which we already handled)
    let value = if let Some(comment_pos) = value.find(" #") {
        &value[..comment_pos]
    } else {
        value
    };

    value.trim().to_string()
}

/// Find GitButler skill installations for one format under `base_dir`.
///
/// Scans the format's skills directory and accepts any folder name, so custom
/// installs like `.claude/skills/but` are found too - identity comes from the
/// SKILL.md contents, not the folder name.
fn find_format_installations(format: &SkillFormat, base_dir: &std::path::Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(format.skills_parent_dir(base_dir)) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| is_gitbutler_skill(&path.join("SKILL.md")))
        .collect();
    found.sort();
    found
}

fn is_complete_skill_installation(path: &std::path::Path) -> bool {
    let skill_md = path.join("SKILL.md");
    is_gitbutler_skill(&skill_md)
        && skill_files_in_write_order(installed_layout(&skill_md))
            .all(|file| file.get_install_path(path).is_file())
}

fn is_current_skill_installation(path: &std::path::Path, version: &str) -> bool {
    is_complete_skill_installation(path)
        && extract_installed_version(&path.join("SKILL.md")).as_deref() == Some(version)
}

/// Scope labels for a skill installation. Single-sourced here because
/// [`detect_install_paths`] selects installations by scope.
const SCOPE_GLOBAL: &str = "global";
const SCOPE_LOCAL: &str = "local";

/// Find all GitButler skill installations.
///
/// Returns a list of (install_path, format_name, scope) tuples.
fn find_all_installations(
    ctx: Option<&mut Context>,
    check_global: bool,
    check_local: bool,
) -> Result<Vec<(PathBuf, &'static str, &'static str)>> {
    let mut installations = Vec::new();

    // Determine which base directories to check
    let mut base_dirs: Vec<(PathBuf, &str)> = Vec::new();

    if check_global && let Some(home) = home_dir() {
        base_dirs.push((home, SCOPE_GLOBAL));
    }

    if check_local
        && let Some(ctx) = ctx
        && let Ok(repo) = ctx.repo.get()
        && let Some(workdir) = repo.workdir()
    {
        // A repository discovered from a relative working directory reports a
        // relative workdir. Absolutize it so discovered installation paths can
        // be handed to context-free operations (`check --update` reinstalls
        // each outdated path without a repository context).
        let workdir = std::path::absolute(workdir)
            .with_context(|| format!("Could not absolutize repository workdir {workdir:?}"))?;
        base_dirs.push((workdir, SCOPE_LOCAL));
    }

    // Scan each base directory for the formats valid in its scope, so a
    // scope-specific location (e.g. `.github/skills` locally, `.copilot/skills`
    // globally) isn't discovered under the wrong scope.
    for (base_dir, scope) in base_dirs {
        let is_global = scope == SCOPE_GLOBAL;
        for format in SKILL_FORMATS {
            if !format.is_available_for(is_global) {
                continue;
            }
            for path in find_format_installations(format, &base_dir) {
                installations.push((path, format.name, scope));
            }
        }
    }

    Ok(installations)
}

/// Check the status of all installed skills.
pub fn check_skill_status(
    ctx: Option<&mut Context>,
    check_global: bool,
    check_local: bool,
) -> Result<SkillCheckResult> {
    let cli_version = option_env!("VERSION").unwrap_or("dev").to_string();
    let installations = find_all_installations(ctx, check_global, check_local)?;

    let mut skills = Vec::new();
    let mut outdated_count = 0;

    for (path, format_name, scope) in installations {
        let skill_md_path = path.join("SKILL.md");
        let installed_version =
            extract_installed_version(&skill_md_path).unwrap_or_else(|| "unknown".to_string());

        let up_to_date = is_current_skill_installation(&path, &cli_version);
        if !up_to_date {
            outdated_count += 1;
        }

        skills.push(SkillStatus {
            path,
            format_name: format_name.to_string(),
            scope: scope.to_string(),
            installed_version,
            up_to_date,
        });
    }

    Ok(SkillCheckResult {
        cli_version,
        skills,
        outdated_count,
    })
}

/// Check if installed skills are up to date
fn check_skills(
    current_dir: &std::path::Path,
    out: &mut OutputChannel,
    global_only: bool,
    local_only: bool,
    auto_update: bool,
) -> Result<()> {
    let t = theme::get();
    // Determine scope
    let (check_global, check_local) = match (global_only, local_only) {
        (true, false) => (true, false),
        (false, true) => (false, true),
        (false, false) => (true, true), // default: check both
        _ => unreachable!(),            // clap conflicts_with prevents this
    };
    // Only local installations live in a repository.
    let mut ctx = if check_local {
        repository_context(current_dir)?
    } else {
        None
    };

    // Warn if --local was explicitly requested but no repo context is available
    if local_only && ctx.is_none() {
        anyhow::bail!(
            "Cannot check local installations: not in a git repository.\n\
             Use --global to check global installations, or run from within a repository."
        );
    }

    // First check to find outdated skills (reborrow ctx so we can use it again later)
    let initial_result = check_skill_status(ctx.as_mut(), check_global, check_local)?;

    // Collect paths of outdated skills (needed for auto-update)
    let outdated_paths: Vec<String> = initial_result
        .skills
        .iter()
        .filter(|s| !s.up_to_date)
        .map(|s| s.path.display().to_string())
        .collect();

    // Auto-update if requested (do this before displaying results)
    if auto_update && !outdated_paths.is_empty() {
        let mut progress = out.progress_channel();
        writeln!(
            progress,
            "{}",
            t.important.paint("Updating outdated skills...")
        )?;
        writeln!(progress)?;

        for path_str in &outdated_paths {
            // Pass None for ctx since the paths are already absolute and don't require repo context
            install_skill(
                current_dir,
                out,
                false,
                Some(path_str.clone()),
                false,
                SkillLayout::Full,
            )?;
        }
    }

    // Re-check status after updates (or use initial result if no updates)
    let result = if auto_update && !outdated_paths.is_empty() {
        check_skill_status(ctx.as_mut(), check_global, check_local)?
    } else {
        initial_result
    };

    // Output based on format
    if let Some(writer) = out.for_human() {
        print_human_check_output(writer, &result)?;

        if result.outdated_count > 0 && !auto_update {
            writeln!(writer)?;
            writeln!(
                writer,
                "{} Run 'but skill check --update' to update outdated skills",
                t.sym().arrow.attention()
            )?;
        }
    } else if let Some(json_out) = out.for_json() {
        json_out.write_value(&result)?;
    }

    Ok(())
}

fn print_human_check_output(
    writer: &mut dyn std::fmt::Write,
    result: &SkillCheckResult,
) -> Result<(), anyhow::Error> {
    let t = theme::get();
    writeln!(writer)?;
    writeln!(
        writer,
        "CLI version: {}",
        t.config_value.paint(&result.cli_version)
    )?;
    writeln!(writer)?;

    if result.skills.is_empty() {
        writeln!(writer, "No GitButler skill installations found.")?;
        writeln!(writer)?;
        writeln!(writer, "Install with: but skill install")?;
        return Ok(());
    }

    writeln!(
        writer,
        "Found {} skill installation(s):",
        result.skills.len()
    )?;
    writeln!(writer)?;

    for skill in &result.skills {
        let status_icon = if skill.up_to_date {
            t.sym().success.to_string()
        } else {
            t.sym().error.to_string()
        };

        let version_display = if skill.up_to_date {
            t.success.paint(&skill.installed_version).to_string()
        } else {
            format!(
                "{} → {}",
                t.error.paint(&skill.installed_version),
                t.success.paint(&result.cli_version)
            )
        };

        writeln!(
            writer,
            "  {} {} ({}) - {} [{}]",
            status_icon,
            skill.format_name,
            skill.scope,
            t.hint.paint(skill.path.display().to_string()),
            version_display
        )?;
    }

    writeln!(writer)?;

    if result.outdated_count == 0 {
        writeln!(writer, "{} All skills are up to date!", t.sym().success)?;
    } else {
        writeln!(
            writer,
            "{} {} skill(s) are outdated",
            t.sym().warning,
            result.outdated_count
        )?;
    }

    Ok(())
}

/// Detect existing skill installations to refresh in place.
///
/// Returns every GitButler skill in the highest-priority scope that has one
/// (local before global), so `--detect` refreshes them all instead of forcing a
/// choice between them. Filtering to a single scope keeps a repo-local `--detect`
/// from reaching into global installs.
fn detect_install_paths(ctx: Option<&mut Context>, global: bool) -> Result<Vec<PathBuf>> {
    let installations = find_all_installations(ctx, true, !global)?;

    for scope in [SCOPE_LOCAL, SCOPE_GLOBAL] {
        let paths: Vec<PathBuf> = installations
            .iter()
            .filter(|(_, _, s)| *s == scope)
            .map(|(path, _, _)| path.clone())
            .collect();
        if !paths.is_empty() {
            return Ok(paths);
        }
    }

    anyhow::bail!(
        "Could not detect an existing GitButler skill installation.\n\
         Run `but skill install` to create one, or use `--path <dir>` to choose a location."
    )
}

fn prompt_for_install_scope(
    input: &mut crate::utils::InputOutputChannel<'_>,
    progress: &mut impl std::io::Write,
) -> Result<InstallScope> {
    let t = theme::get();
    writeln!(progress)?;
    writeln!(
        progress,
        "{}",
        t.important.paint("Select installation scope:")
    )?;
    writeln!(progress)?;

    let options = nonempty::nonempty![
        ("Local", InstallScopeOption::Local),
        ("Global", InstallScopeOption::Global)
    ];

    match input
        .prompt_select("Where would you like to install the skill?", &options)?
        .copied()
    {
        Some(InstallScopeOption::Local) => Ok(InstallScope::Local),
        Some(InstallScopeOption::Global) => Ok(InstallScope::Global),
        None => Err(UserCancelled.into()),
    }
}

/// Prompt user to select installation scope and format
fn prompt_for_install_path(
    ctx: Option<&mut Context>,
    global: bool,
    out: &mut OutputChannel,
    progress: &mut impl std::io::Write,
) -> Result<PathBuf> {
    let t = theme::get();
    if out.for_human().is_none() {
        anyhow::bail!(
            "No supported agent was detected. In non-interactive mode, specify --path or --detect. Use --path <path> to choose an installation directory, or --detect to update an existing installation."
        );
    }
    if !out.can_prompt() {
        anyhow::bail!(
            "Human input required - run this in a terminal, or specify --path/--detect to avoid interactive prompts."
        );
    }

    let local_scope_available = if !global {
        match ctx.as_ref() {
            Some(ctx) => {
                let repo = ctx.repo.get()?;
                repo.workdir().is_some()
            }
            None => false,
        }
    } else {
        false
    };

    let mut input = out
        .prepare_for_terminal_input()
        .context("Human input required - run this in a terminal, or specify --path/--detect to avoid interactive prompts.")?;

    let scope = match determine_install_scope_resolution(global, local_scope_available) {
        InstallScopeResolution::PromptUser => prompt_for_install_scope(&mut input, progress)?,
        InstallScopeResolution::Fixed(scope) => scope,
    };

    if !global && !local_scope_available {
        writeln!(progress)?;
        if ctx.is_none() {
            writeln!(
                progress,
                "{} Not in a git repository. Installing globally in your home directory.",
                t.info.paint("ℹ")
            )?;
        } else {
            writeln!(
                progress,
                "{} Local installs require a repository workdir. Installing globally in your home directory.",
                t.info.paint("ℹ")
            )?;
        }
        writeln!(progress)?;
    }

    let base_dir = get_base_dir(ctx, scope.is_global())?;

    writeln!(progress)?;
    writeln!(
        progress,
        "{}",
        t.important.paint("Select a skill folder format:")
    )?;
    writeln!(progress)?;

    let available_formats: Vec<&SkillFormat> = SKILL_FORMATS
        .iter()
        .filter(|f| f.is_available_for(scope.is_global()))
        .collect();
    debug_assert!(
        !available_formats.is_empty(),
        "At least one skill format must be available for each install scope"
    );

    let options = available_formats
        .into_iter()
        .map(|format| {
            let full_path = format.get_install_path(&base_dir);
            (
                format!(
                    "{} - {} ({})",
                    format.name,
                    format.description,
                    full_path.display()
                ),
                format,
            )
        })
        .collect::<Vec<_>>();
    let options =
        nonempty::NonEmpty::from_vec(options).context("No skill folder formats available")?;

    let selected_format = input
        .prompt_select("Which format would you like to use?", &options)?
        .ok_or(UserCancelled)?;

    Ok(selected_format.get_install_path(&base_dir))
}

/// Prepare SKILL.md content with version injection and validate all files
fn prepare_skill_content(version: &str) -> Result<String> {
    // Validate all embedded files are valid UTF-8
    let skill_content = std::str::from_utf8(SKILL_MD).context("SKILL.md is not valid UTF-8")?;
    std::str::from_utf8(CONCEPTS_MD).context("concepts.md is not valid UTF-8")?;
    std::str::from_utf8(EXAMPLES_MD).context("examples.md is not valid UTF-8")?;
    std::str::from_utf8(REFERENCE_MD).context("reference.md is not valid UTF-8")?;

    // Inject version into SKILL.md
    Ok(inject_version(skill_content, version))
}

/// Write the bundled skill files into `install_path`, creating the directory
/// structure as needed and injecting the CLI version into SKILL.md. An existing
/// stub stays a stub on every rewrite, so refreshes never upgrade a layout by
/// accident. Returns the version that was written.
pub(crate) fn write_skill_files(
    install_path: &std::path::Path,
    layout: SkillLayout,
) -> Result<&'static str> {
    if SKILL_FILES.iter().any(|f| f.content.is_empty()) {
        anyhow::bail!(
            "Skill files were not properly embedded at build time. Please report this as a bug."
        );
    }

    // Prepare all content before writing (validate UTF-8 and inject version)
    let version = option_env!("VERSION").unwrap_or("dev");
    let full_skill_md = prepare_skill_content(version)?;
    let layout = match installed_layout(&install_path.join("SKILL.md")) {
        SkillLayout::Stub => SkillLayout::Stub,
        SkillLayout::Full => layout,
    };
    let (skill_md_content, content_dir) = match layout {
        SkillLayout::Full => (full_skill_md, install_path.join("references")),
        SkillLayout::Stub => {
            let close = frontmatter_close(&full_skill_md)
                .context("SKILL.md has no frontmatter to build a stub from")?;
            let stub = format!(
                "{}stub: true\nallowed-tools: Bash(but skill:*)\n{}{STUB_MD}",
                &full_skill_md[..close.start],
                &full_skill_md[close]
            );
            // Converting a full install of ours leaves no stale `references`
            // entry behind, whatever it is; only a directory that holds our
            // own SKILL.md is touched, and links are removed, never followed.
            let references = install_path.join("references");
            if is_gitbutler_skill(&install_path.join("SKILL.md"))
                && let Ok(metadata) = std::fs::symlink_metadata(&references)
            {
                if metadata.is_dir() {
                    std::fs::remove_dir_all(&references)
                } else {
                    std::fs::remove_file(&references)
                }
                .with_context(|| format!("Failed to remove {}", references.display()))?;
            }
            (stub, install_path.to_path_buf())
        }
    };
    std::fs::create_dir_all(&content_dir).with_context(|| {
        format!(
            "Failed to create skill directory at {}. Check that you have write permissions for this location.",
            install_path.display()
        )
    })?;

    for file in skill_files_in_write_order(layout) {
        let file_path = file.get_install_path(install_path);
        let content = if file.is_main_skill_file() {
            // Use the version-injected content for SKILL.md
            skill_md_content.as_bytes()
        } else {
            file.content
        };
        write_skill_file(&file_path, content)?;
    }
    Ok(version)
}

/// Write a skill file with proper error context
fn write_skill_file(path: &std::path::Path, content: &[u8]) -> Result<()> {
    std::fs::write(path, content).with_context(|| {
        format!(
            "Failed to write {}. Check write permissions.",
            path.display()
        )
    })
}

/// Install the skill files
fn install_skill(
    current_dir: &std::path::Path,
    out: &mut OutputChannel,
    global: bool,
    custom_path: Option<String>,
    detect: bool,
    layout: SkillLayout,
) -> Result<()> {
    let t = theme::get();
    let driving_agent = (!out.can_prompt())
        .then(crate::utils::detect_agent::detect)
        .flatten();
    // Validate that embedded files are not empty (catches build issues)
    if SKILL_FILES.iter().any(|f| f.content.is_empty()) {
        anyhow::bail!(
            "Skill files were not properly embedded at build time. Please report this as a bug."
        );
    }

    // Validate SKILL_FORMATS configuration (catches development errors)
    debug_assert!(
        !SKILL_FORMATS.is_empty(),
        "SKILL_FORMATS must contain at least one format"
    );
    debug_assert!(
        SKILL_FORMATS
            .iter()
            .all(|f| !f.name.is_empty() && !f.path_components.is_empty()),
        "SkillFormat name and path components must not be empty"
    );
    debug_assert!(
        SKILL_FILES.iter().all(|file| {
            !file.path_components.is_empty()
                && file
                    .path_components
                    .iter()
                    .all(|component| !component.is_empty())
                && file
                    .path_components
                    .iter()
                    .all(|component| !component.contains('/') && !component.contains('\\'))
        }),
        "SkillFile path components must be non-empty and separator-free"
    );

    let mut progress = out.progress_channel();

    // Validate flags
    if detect && custom_path.is_some() {
        anyhow::bail!("Cannot use both --detect and --path options together");
    }
    // Only a local scope needs the repository: global installs resolve against
    // the home directory and absolute or tilde paths need no base at all.
    let custom_is_absolute = custom_path.as_deref().is_some_and(|custom| {
        expand_tilde(custom)
            .unwrap_or_else(|| PathBuf::from(custom))
            .is_absolute()
    });
    let mut ctx = if global || custom_is_absolute {
        None
    } else {
        repository_context(current_dir)?
    };
    let ctx = ctx.as_mut();
    if ctx.is_none() && !global && custom_path.is_some() && !custom_is_absolute {
        anyhow::bail!(
            "Cannot use relative --path outside a git repository unless --global is specified.\n\
             Use --global --path <path> for a global installation, use an absolute path, or run from within a repository for local installation."
        );
    }

    // Determine installation path(s). Only --detect can yield more than one, when
    // several GitButler skills share a scope; they are all refreshed together.
    let agent_default_path = if custom_path.is_none() && !detect {
        driving_agent.and_then(agent_default_install_path)
    } else {
        None
    };
    let installed_for_driving_agent = agent_default_path.is_some();
    let install_paths = if let Some(custom) = custom_path {
        vec![resolve_custom_path(&custom, ctx, global)?]
    } else if detect {
        detect_install_paths(ctx, global)?
    } else if let Some(path) = agent_default_path {
        vec![path]
    } else {
        vec![prompt_for_install_path(ctx, global, out, &mut progress)?]
    };

    let mut version = "";
    for install_path in &install_paths {
        // Validate installation path
        if install_path.exists() && install_path.is_file() {
            anyhow::bail!(
                "Installation path {} is a file, not a directory. Please specify a directory path.",
                install_path.display()
            );
        }

        // Check if files already exist and warn user
        if install_path.join("SKILL.md").exists()
            && let Some(writer) = out.for_human()
        {
            writeln!(writer)?;
            writeln!(
                writer,
                "{} Skill files already exist at {}",
                t.sym().warning,
                t.config_value.paint(install_path.display().to_string())
            )?;
            writeln!(writer, "  Overwriting existing files...")?;
            writeln!(writer)?;
        }

        version = write_skill_files(install_path, layout)?;
    }
    // An existing stub is kept as a stub, so report what each location
    // actually holds rather than the layout that was requested.
    let written: Vec<(&PathBuf, SkillLayout)> = install_paths
        .iter()
        .map(|path| (path, installed_layout(&path.join("SKILL.md"))))
        .collect();

    if let Some(writer) = out.for_human() {
        writeln!(writer)?;
        writeln!(
            writer,
            "{} GitButler skill installed successfully!",
            t.sym().success
        )?;
        writeln!(writer)?;
        if let [(only, layout)] = written.as_slice() {
            writeln!(
                writer,
                "  Location: {}",
                t.config_value.paint(only.display().to_string())
            )?;
            writeln!(writer)?;
            writeln!(writer, "  Files installed:")?;
            for file in skill_files_in_write_order(*layout) {
                writeln!(writer, "    • {}", file.display_path())?;
            }
        } else {
            writeln!(writer, "  Locations and files installed:")?;
            for (install_path, layout) in &written {
                writeln!(
                    writer,
                    "    • {}",
                    t.config_value.paint(install_path.display().to_string())
                )?;
                for file in skill_files_in_write_order(*layout) {
                    writeln!(writer, "        {}", file.display_path())?;
                }
            }
        }
        writeln!(writer)?;
        // A skill installed mid-session is only picked up when the agent's
        // harness next indexes skills, so point the current session at the
        // file directly. The locations all carry the same content, so reading
        // the first one is enough.
        if installed_for_driving_agent && let [first, ..] = install_paths.as_slice() {
            writeln!(
                writer,
                "To use it in this session, read {} now; future sessions load it automatically.",
                first.join("SKILL.md").display()
            )?;
            writeln!(writer)?;
        }
    }

    if let Some(out) = out.for_json() {
        let installations: Vec<serde_json::Value> = written
            .iter()
            .map(|(path, layout)| {
                serde_json::json!({
                    "path": path.display().to_string(),
                    "files": skill_files_in_write_order(*layout)
                        .map(|f| f.display_path())
                        .collect::<Vec<_>>(),
                })
            })
            .collect();
        let paths: Vec<String> = install_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        let mut result = serde_json::json!({
            "success": true,
            "version": version,
            "paths": paths,
            "installations": installations,
        });
        // `files` describes every location only when they share a layout.
        if let Some((_, first)) = written.first()
            && written.iter().all(|(_, layout)| layout == first)
        {
            result["files"] = installations[0]["files"].clone();
        }
        out.write_value(result)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_tilde_handles_home_only() {
        let result = expand_tilde("~");
        assert!(result.is_some());
        let expanded = result.unwrap();
        assert!(expanded.is_absolute());
        assert!(!expanded.to_string_lossy().contains('~'));
    }

    #[test]
    fn expand_tilde_handles_unix_path() {
        let result = expand_tilde("~/Documents/test");
        assert!(result.is_some());
        let expanded = result.unwrap();
        assert!(expanded.is_absolute());
        assert!(expanded.ends_with("Documents/test"));
    }

    #[test]
    fn expand_tilde_handles_windows_path() {
        let result = expand_tilde("~\\Documents\\test");
        assert!(result.is_some());
        let expanded = result.unwrap();
        assert!(expanded.is_absolute());
    }

    #[test]
    fn expand_tilde_returns_none_for_non_tilde_path() {
        let result = expand_tilde("/absolute/path");
        assert!(result.is_none());

        let result = expand_tilde("relative/path");
        assert!(result.is_none());
    }

    #[test]
    fn inject_version_replaces_in_frontmatter() {
        let content = "---\nname: Test\nversion: 0.0.0\n---\n\nContent here with version: 0.0.0";
        let result = inject_version(content, "1.2.3");

        // Should replace the first occurrence in frontmatter
        assert!(result.contains("version: 1.2.3"));
        // The second occurrence should NOT be replaced
        assert!(result.contains("Content here with version: 0.0.0"));
    }

    #[test]
    fn inject_version_handles_windows_line_endings() {
        let content = "---\r\nname: Test\r\nversion: 0.0.0\r\n---\r\n\r\nContent here";
        let result = inject_version(content, "1.2.3");

        assert!(result.contains("version: 1.2.3"));
    }

    #[test]
    fn inject_version_handles_old_mac_line_endings() {
        let content = "---\rname: Test\rversion: 0.0.0\r---\r\rContent here";
        let result = inject_version(content, "1.2.3");

        assert!(result.contains("version: 1.2.3"));
    }

    #[test]
    fn frontmatter_close_ignores_a_rule_in_a_document_without_frontmatter() {
        assert_eq!(
            frontmatter_close("# Title\n\nintro\n\n---\n\nmore\n"),
            None,
            "a horizontal rule is not a frontmatter delimiter"
        );
        assert_eq!(
            frontmatter_close("---\nname: but\n---\n\n# Title\n"),
            Some(14..19),
            "the closing line and its blank line are the delimiter"
        );
    }

    #[test]
    fn inject_version_fallback_without_frontmatter() {
        let content = "Just some content with version: 0.0.0 in it";
        let result = inject_version(content, "2.0.0");

        assert!(result.contains("version: 2.0.0"));
        assert!(!result.contains("version: 0.0.0"));
    }

    #[test]
    fn inject_version_handles_missing_version_field() {
        let content = "---\nname: Test\n---\n\nContent";
        let result = inject_version(content, "1.0.0");

        // Should not crash, and content should be unchanged
        assert_eq!(content, result);
    }

    #[test]
    fn prepare_skill_content_validates_utf8() {
        // This tests that the function checks UTF-8 validity
        // The actual embedded files should be valid, so this should succeed
        let result = prepare_skill_content("1.0.0");
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn prepare_skill_content_injects_version() {
        let result = prepare_skill_content("9.9.9").unwrap();
        assert!(result.contains("version: 9.9.9"));
    }

    #[test]
    fn skill_formats_are_valid() {
        // Validate that all SKILL_FORMATS have non-empty fields
        assert!(!SKILL_FORMATS.is_empty(), "Must have at least one format");

        for format in SKILL_FORMATS {
            assert!(!format.name.is_empty(), "Format name cannot be empty");
            assert!(
                !format.description.is_empty(),
                "Format description cannot be empty"
            );
            assert!(
                !format.path_components.is_empty(),
                "Format path components cannot be empty"
            );
            assert!(
                format
                    .path_components
                    .iter()
                    .all(|component| !component.is_empty()),
                "Format path components must not be empty"
            );
            assert!(
                format
                    .path_components
                    .iter()
                    .all(|component| !component.contains('/') && !component.contains('\\')),
                "Format path components must not contain path separators"
            );
        }
    }

    #[test]
    fn skill_format_get_install_path_joins_correctly() {
        let format = SkillFormat {
            name: "Test",
            description: "Test format",
            availability: SkillFormatAvailability::LocalAndGlobal,
            path_components: &[".test", "skills", "foo"],
        };

        let base = PathBuf::from("home").join("user");
        let result = format.get_install_path(&base);

        assert_eq!(result, base.join(".test").join("skills").join("foo"));
    }

    #[test]
    fn skill_files_are_valid() {
        for file in SKILL_FILES {
            assert!(
                !file.path_components.is_empty(),
                "SkillFile path components cannot be empty"
            );
            assert!(
                file.path_components
                    .iter()
                    .all(|component| !component.is_empty()),
                "SkillFile path components must not be empty"
            );
            assert!(
                file.path_components
                    .iter()
                    .all(|component| !component.contains('/') && !component.contains('\\')),
                "SkillFile path components must not contain path separators"
            );
        }
    }

    #[test]
    fn determine_install_scope_resolution_explicit_global_is_fixed_global() {
        let resolution = determine_install_scope_resolution(true, true);
        assert_eq!(
            resolution,
            InstallScopeResolution::Fixed(InstallScope::Global)
        );
    }

    #[test]
    fn determine_install_scope_resolution_repo_context_prompts_user() {
        let resolution = determine_install_scope_resolution(false, true);
        assert_eq!(resolution, InstallScopeResolution::PromptUser);
    }

    #[test]
    fn determine_install_scope_resolution_no_repo_context_is_fixed_global() {
        let resolution = determine_install_scope_resolution(false, false);
        assert_eq!(
            resolution,
            InstallScopeResolution::Fixed(InstallScope::Global)
        );
    }

    #[test]
    fn embedded_files_are_not_empty() {
        // This catches build issues where files aren't properly embedded
        for file in SKILL_FILES {
            assert!(
                !file.content.is_empty(),
                "{} should be embedded",
                file.display_path()
            );
        }
    }

    #[test]
    fn embedded_files_are_valid_utf8() {
        // Ensure all embedded files are valid UTF-8
        for file in SKILL_FILES {
            assert!(
                std::str::from_utf8(file.content).is_ok(),
                "{} should be valid UTF-8",
                file.display_path()
            );
        }
    }

    #[test]
    fn skill_file_display_path_is_derived_from_components() {
        assert_eq!(SKILL_FILES[0].display_path(), "SKILL.md");
        assert_eq!(SKILL_FILES[1].display_path(), "references/reference.md");
    }

    #[test]
    fn resolve_custom_path_handles_absolute_path() {
        let result = resolve_custom_path("/absolute/path", None, false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("/absolute/path"));
    }

    #[test]
    fn resolve_custom_path_expands_tilde() {
        let result = resolve_custom_path("~/test/path", None, true);
        assert!(result.is_ok());
        let expanded = result.unwrap();
        assert!(expanded.is_absolute());
        assert!(!expanded.to_string_lossy().contains('~'));
        assert!(expanded.ends_with("test/path"));
    }

    #[test]
    fn get_base_dir_global_returns_home() {
        let result = get_base_dir(None, true);
        assert!(result.is_ok());
        let dir = result.unwrap();
        assert!(dir.is_absolute());
    }

    #[test]
    fn get_base_dir_local_without_context_fails() {
        let result = get_base_dir(None, false);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Not in a git repository")
        );
    }

    // NOTE: detect_install_paths is difficult to test in isolation because it depends on
    // the user home directory and git repository context. It's tested indirectly through
    // integration tests and manual testing. The core logic (is_gitbutler_skill validation
    // and per-format discovery) is tested separately.

    #[test]
    fn is_gitbutler_skill_requires_name_but_in_frontmatter() {
        use std::fs;

        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let skill_path = temp_dir.path().join("SKILL.md");

        // Identity comes from `name: but` in the YAML frontmatter.
        fs::write(
            &skill_path,
            "---\nname: but\nversion: 1.0.0\n---\n# GitButler CLI Skill",
        )
        .unwrap();
        assert!(
            is_gitbutler_skill(&skill_path),
            "frontmatter name: but is the identity marker"
        );

        // Another skill that only mentions GitButler in its header or body must not
        // be misclassified - discovery would otherwise overwrite it in place.
        fs::write(
            &skill_path,
            "---\nname: other-skill\n---\n# GitButler CLI Skill\n\nExample: `name: but`",
        )
        .unwrap();
        assert!(
            !is_gitbutler_skill(&skill_path),
            "a sibling skill's declared name wins over body mentions of GitButler"
        );

        // The header alone, with no frontmatter, is not a reliable marker.
        fs::write(&skill_path, "# GitButler CLI Skill\n\nContent here").unwrap();
        assert!(!is_gitbutler_skill(&skill_path));

        // Prose that merely contains the marker string.
        fs::write(
            &skill_path,
            "I was reading about the GitButler CLI and the name: but that's not right",
        )
        .unwrap();
        assert!(!is_gitbutler_skill(&skill_path));

        // Nonexistent file.
        assert!(!is_gitbutler_skill(&temp_dir.path().join("nonexistent.md")));
    }

    #[test]
    fn extract_installed_version_parses_frontmatter() {
        let version = extract_installed_version_from_content(
            "---\nname: but\nversion: 1.2.3\n---\n# Content",
        );
        assert_eq!(version, Some("1.2.3".to_string()));
    }

    #[test]
    fn extract_installed_version_handles_different_order() {
        // version is not the first field
        let version = extract_installed_version_from_content(
            "---\nname: but\nauthor: Test\nversion: 2.0.0\n---\n# Content",
        );
        assert_eq!(version, Some("2.0.0".to_string()));
    }

    #[test]
    fn extract_installed_version_returns_none_for_missing_version() {
        let version = extract_installed_version_from_content("---\nname: but\n---\n# Content");
        assert_eq!(version, None);
    }

    #[test]
    fn extract_installed_version_returns_none_for_no_frontmatter() {
        let version = extract_installed_version_from_content("# Just a regular markdown file");
        assert_eq!(version, None);
    }

    #[test]
    fn extract_installed_version_returns_none_for_nonexistent_file() {
        let nonexistent = PathBuf::from("/nonexistent/path/SKILL.md");
        let version = extract_installed_version(&nonexistent);
        assert_eq!(version, None);
    }

    #[test]
    fn skill_status_serializes_correctly() {
        let status = SkillStatus {
            path: PathBuf::from("/test/path"),
            format_name: "Claude Code".to_string(),
            scope: "global".to_string(),
            installed_version: "1.0.0".to_string(),
            up_to_date: true,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("Claude Code"));
        assert!(json.contains("up_to_date"));
        assert!(json.contains("1.0.0"));
    }

    #[test]
    fn skill_check_result_serializes_correctly() {
        let result = SkillCheckResult {
            cli_version: "2.0.0".to_string(),
            skills: vec![SkillStatus {
                path: PathBuf::from("/test/path"),
                format_name: "Cursor".to_string(),
                scope: "local".to_string(),
                installed_version: "1.0.0".to_string(),
                up_to_date: false,
            }],
            outdated_count: 1,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("cli_version"));
        assert!(json.contains("2.0.0"));
        assert!(json.contains("outdated_count"));
        assert!(json.contains("Cursor"));
    }

    #[test]
    fn write_skill_files_writes_versioned_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let install_path = dir.path().join(".claude").join("skills").join("gitbutler");

        let version = write_skill_files(&install_path, SkillLayout::Full)
            .expect("a writable path accepts the bundle");

        let skill_md = std::fs::read_to_string(install_path.join("SKILL.md")).unwrap();
        assert!(
            skill_md.contains(&format!("version: {version}")),
            "SKILL.md carries the CLI's bundled version"
        );
        assert!(
            install_path
                .join("references")
                .join("reference.md")
                .exists(),
            "reference files are written alongside SKILL.md"
        );
    }

    #[test]
    fn skill_entrypoint_is_written_last() {
        assert!(
            skill_files_in_write_order(SkillLayout::Full)
                .last()
                .is_some_and(SkillFile::is_main_skill_file),
            "a partial bundle must not look installed"
        );
    }

    #[test]
    fn extract_installed_version_trims_whitespace() {
        // Version with extra whitespace
        let version =
            extract_installed_version_from_content("---\nversion:   1.0.0   \n---\n# Content");
        assert_eq!(version, Some("1.0.0".to_string()));
    }

    #[test]
    fn extract_installed_version_handles_empty_version() {
        // Empty version value
        let version = extract_installed_version_from_content("---\nversion:\n---\n# Content");
        assert_eq!(version, Some("".to_string()));
    }

    #[test]
    fn find_all_installations_discovers_skills_in_temp_dir() {
        use std::fs;

        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Create a Claude Code skill installation
        let claude_skill_dir = temp_dir
            .path()
            .join(".claude")
            .join("skills")
            .join("gitbutler");
        fs::create_dir_all(&claude_skill_dir).unwrap();
        let claude_skill_content = "---\nname: but\nversion: 1.0.0\n---\n# GitButler CLI Skill";
        fs::write(claude_skill_dir.join("SKILL.md"), claude_skill_content).unwrap();

        // Create a Cursor skill installation
        let cursor_skill_dir = temp_dir
            .path()
            .join(".cursor")
            .join("skills")
            .join("gitbutler");
        fs::create_dir_all(&cursor_skill_dir).unwrap();
        let cursor_skill_content = "---\nname: but\nversion: 0.9.0\n---\n# GitButler CLI Skill";
        fs::write(cursor_skill_dir.join("SKILL.md"), cursor_skill_content).unwrap();

        // Create a non-GitButler skill (should be ignored)
        let other_skill_dir = temp_dir
            .path()
            .join(".opencode")
            .join("skills")
            .join("gitbutler");
        fs::create_dir_all(&other_skill_dir).unwrap();
        fs::write(other_skill_dir.join("SKILL.md"), "# Some other skill").unwrap();

        // We can't easily test find_all_installations directly since it uses the user home.
        // But we can test the components it uses

        // Verify is_gitbutler_skill correctly identifies our test files
        assert!(is_gitbutler_skill(&claude_skill_dir.join("SKILL.md")));
        assert!(is_gitbutler_skill(&cursor_skill_dir.join("SKILL.md")));
        assert!(!is_gitbutler_skill(&other_skill_dir.join("SKILL.md")));

        // Verify extract_installed_version parsing works on our test content
        assert_eq!(
            extract_installed_version_from_content(claude_skill_content),
            Some("1.0.0".to_string())
        );
        assert_eq!(
            extract_installed_version_from_content(cursor_skill_content),
            Some("0.9.0".to_string())
        );
    }

    #[test]
    fn find_format_installations_accepts_any_folder_name() {
        use std::fs;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let skills_dir = temp_dir.path().join(".claude").join("skills");

        // A GitButler skill under a custom folder name, plus one under the
        // canonical name - both are real installations.
        let gitbutler_skill = "---\nname: but\nversion: 1.0.0\n---\n# GitButler CLI Skill";
        for folder in ["but", "gitbutler"] {
            fs::create_dir_all(skills_dir.join(folder)).unwrap();
            fs::write(skills_dir.join(folder).join("SKILL.md"), gitbutler_skill).unwrap();
        }
        // Another agent's skill is ignored
        fs::create_dir_all(skills_dir.join("other")).unwrap();
        fs::write(
            skills_dir.join("other").join("SKILL.md"),
            "# Some other skill",
        )
        .unwrap();

        let format = SKILL_FORMATS
            .iter()
            .find(|f| f.name == "Claude Code")
            .unwrap();

        assert_eq!(
            find_format_installations(format, temp_dir.path()),
            vec![skills_dir.join("but"), skills_dir.join("gitbutler")],
            "every GitButler skill is found by SKILL.md contents, not folder name, in sorted order"
        );

        let format_without_dir = SKILL_FORMATS.iter().find(|f| f.name == "Cursor").unwrap();
        assert!(
            find_format_installations(format_without_dir, temp_dir.path()).is_empty(),
            "a missing skills directory yields no installations"
        );
    }

    #[test]
    fn skill_installation_requires_every_embedded_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        write_skill_files(temp_dir.path(), SkillLayout::Full).unwrap();
        std::fs::remove_file(temp_dir.path().join("references/concepts.md")).unwrap();

        assert!(!is_complete_skill_installation(temp_dir.path()));
    }

    #[test]
    fn skill_status_up_to_date_logic() {
        // Same version should be up to date
        let status = SkillStatus {
            path: PathBuf::from("/test"),
            format_name: "Test".to_string(),
            scope: "global".to_string(),
            installed_version: "1.0.0".to_string(),
            up_to_date: "1.0.0" == "1.0.0",
        };
        assert!(status.up_to_date);

        // Different version should not be up to date
        let status = SkillStatus {
            path: PathBuf::from("/test"),
            format_name: "Test".to_string(),
            scope: "global".to_string(),
            installed_version: "0.9.0".to_string(),
            up_to_date: "0.9.0" == "1.0.0",
        };
        assert!(!status.up_to_date);

        // "unknown" version should not be up to date (unless CLI is also unknown)
        let status = SkillStatus {
            path: PathBuf::from("/test"),
            format_name: "Test".to_string(),
            scope: "global".to_string(),
            installed_version: "unknown".to_string(),
            up_to_date: "unknown" == "1.0.0",
        };
        assert!(!status.up_to_date);

        // "dev" versions should match
        let status = SkillStatus {
            path: PathBuf::from("/test"),
            format_name: "Test".to_string(),
            scope: "global".to_string(),
            installed_version: "dev".to_string(),
            up_to_date: "dev" == "dev",
        };
        assert!(status.up_to_date);
    }

    #[test]
    fn skill_check_result_outdated_count_accuracy() {
        let result = SkillCheckResult {
            cli_version: "2.0.0".to_string(),
            skills: vec![
                SkillStatus {
                    path: PathBuf::from("/path1"),
                    format_name: "Claude Code".to_string(),
                    scope: "global".to_string(),
                    installed_version: "2.0.0".to_string(),
                    up_to_date: true,
                },
                SkillStatus {
                    path: PathBuf::from("/path2"),
                    format_name: "Cursor".to_string(),
                    scope: "local".to_string(),
                    installed_version: "1.0.0".to_string(),
                    up_to_date: false,
                },
                SkillStatus {
                    path: PathBuf::from("/path3"),
                    format_name: "Windsurf".to_string(),
                    scope: "global".to_string(),
                    installed_version: "1.5.0".to_string(),
                    up_to_date: false,
                },
            ],
            outdated_count: 2,
        };

        // Verify the count matches the actual outdated skills
        let actual_outdated = result.skills.iter().filter(|s| !s.up_to_date).count();
        assert_eq!(result.outdated_count, actual_outdated);
        assert_eq!(result.outdated_count, 2);
    }

    #[test]
    fn skill_check_result_empty_skills() {
        let result = SkillCheckResult {
            cli_version: "1.0.0".to_string(),
            skills: vec![],
            outdated_count: 0,
        };

        assert!(result.skills.is_empty());
        assert_eq!(result.outdated_count, 0);

        // Should serialize correctly even when empty
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"skills\":[]"));
    }

    #[test]
    fn extract_installed_version_stops_at_frontmatter_end() {
        // Version appears both in frontmatter and body - should only get frontmatter version
        let version = extract_installed_version_from_content(
            "---\nversion: 1.0.0\n---\n\nversion: 2.0.0 in the body",
        );
        assert_eq!(version, Some("1.0.0".to_string()));
    }

    #[test]
    fn frontmatter_value_handles_crlf_line_endings() {
        // Windows checkouts use CRLF. `str::lines()` strips the `\r\n` terminator,
        // so the `---` delimiters and keys still match without special handling.
        let content = "---\r\nname: but\r\nversion: 1.2.3\r\n---\r\n# GitButler CLI Skill";
        assert_eq!(frontmatter_value(content, "name:").as_deref(), Some("but"));
        assert_eq!(
            frontmatter_value(content, "version:").as_deref(),
            Some("1.2.3")
        );
    }

    #[test]
    fn parse_yaml_value_handles_plain_values() {
        assert_eq!(parse_yaml_value("1.0.0"), "1.0.0");
        assert_eq!(parse_yaml_value("  1.0.0  "), "1.0.0");
    }

    #[test]
    fn parse_yaml_value_handles_double_quoted_strings() {
        assert_eq!(parse_yaml_value("\"1.0.0\""), "1.0.0");
        assert_eq!(parse_yaml_value("  \"1.0.0\"  "), "1.0.0");
    }

    #[test]
    fn parse_yaml_value_handles_single_quoted_strings() {
        assert_eq!(parse_yaml_value("'1.0.0'"), "1.0.0");
        assert_eq!(parse_yaml_value("  '1.0.0'  "), "1.0.0");
    }

    #[test]
    fn parse_yaml_value_handles_inline_comments() {
        assert_eq!(parse_yaml_value("1.0.0 # this is a comment"), "1.0.0");
        assert_eq!(
            parse_yaml_value("1.0.0  # comment with extra space"),
            "1.0.0"
        );
    }

    #[test]
    fn parse_yaml_value_handles_quoted_with_comment() {
        // Comment after quoted value
        assert_eq!(parse_yaml_value("\"1.0.0\" # comment"), "1.0.0");
    }

    #[test]
    fn extract_installed_version_handles_quoted_version() {
        let version =
            extract_installed_version_from_content("---\nversion: \"1.2.3\"\n---\n# Content");
        assert_eq!(version, Some("1.2.3".to_string()));
    }

    #[test]
    fn extract_installed_version_handles_version_with_comment() {
        let version = extract_installed_version_from_content(
            "---\nversion: 1.2.3 # installed version\n---\n# Content",
        );
        assert_eq!(version, Some("1.2.3".to_string()));
    }
}

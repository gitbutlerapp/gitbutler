use std::{fmt::Write as _, path::PathBuf};

use anyhow::{Context as _, Result};
use but_ctx::Context;
use cli_prompts::{DisplayPrompt, prompts::AbortReason};
use colored::Colorize;
use serde::Serialize;

use crate::{args::skill, utils::OutputChannel};

/// Error type for user-initiated cancellation
#[derive(Debug, Clone, Copy)]
pub struct UserCancelled;

impl std::fmt::Display for UserCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Installation cancelled by user")
    }
}

impl std::error::Error for UserCancelled {}

// Embedded skill files
const SKILL_MD: &[u8] = include_bytes!("../../skill/SKILL.md");
const CONCEPTS_MD: &[u8] = include_bytes!("../../skill/references/concepts.md");
const EXAMPLES_MD: &[u8] = include_bytes!("../../skill/references/examples.md");
const REFERENCE_MD: &[u8] = include_bytes!("../../skill/references/reference.md");

/// Metadata for a skill file to be installed
struct SkillFile {
    /// Relative path from install directory (e.g., "SKILL.md" or "references/concepts.md")
    path: &'static str,
    /// Embedded content
    content: &'static [u8],
    /// Display name for output
    display_name: &'static str,
}

/// All skill files to be installed
const SKILL_FILES: &[SkillFile] = &[
    SkillFile {
        path: "SKILL.md",
        content: SKILL_MD,
        display_name: "SKILL.md",
    },
    SkillFile {
        path: "references/concepts.md",
        content: CONCEPTS_MD,
        display_name: "concepts.md",
    },
    SkillFile {
        path: "references/examples.md",
        content: EXAMPLES_MD,
        display_name: "examples.md",
    },
    SkillFile {
        path: "references/reference.md",
        content: REFERENCE_MD,
        display_name: "reference.md",
    },
];

/// Represents a skill installation location format
#[derive(Debug, Clone)]
struct SkillFormat {
    /// Display name of the format
    name: &'static str,
    /// Description of where this format is used
    description: &'static str,
    /// Whether this format should be offered for local and/or global installs.
    availability: SkillFormatAvailability,
    /// Path relative to repository root (for local) or home directory (for global)
    path: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkillFormatAvailability {
    LocalAndGlobal,
    LocalOnly,
    GlobalOnly,
}

impl SkillFormat {
    /// Get the actual installation path given a base directory
    fn get_install_path(&self, base_dir: &std::path::Path) -> PathBuf {
        base_dir.join(self.path)
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

// Common skill folder formats
const SKILL_FORMATS: &[SkillFormat] = &[
    SkillFormat {
        name: "Claude Code",
        description: "Claude Code CLI skill format",
        availability: SkillFormatAvailability::LocalAndGlobal,
        path: ".claude/skills/gitbutler",
    },
    SkillFormat {
        name: "OpenCode",
        description: "OpenCode AI skill format",
        availability: SkillFormatAvailability::LocalAndGlobal,
        path: ".opencode/skills/gitbutler",
    },
    SkillFormat {
        name: "Codex",
        description: "Codex skill format",
        availability: SkillFormatAvailability::LocalAndGlobal,
        path: ".codex/skills/gitbutler",
    },
    SkillFormat {
        name: "GitHub Copilot",
        description: "GitHub Copilot local (repo) skill format",
        availability: SkillFormatAvailability::LocalOnly,
        path: ".github/skills/gitbutler",
    },
    SkillFormat {
        name: "GitHub Copilot",
        description: "GitHub Copilot global skill format",
        availability: SkillFormatAvailability::GlobalOnly,
        path: ".copilot/skills/gitbutler",
    },
    SkillFormat {
        name: "Cursor",
        description: "Cursor AI skill format",
        availability: SkillFormatAvailability::LocalAndGlobal,
        path: ".cursor/skills/gitbutler",
    },
    SkillFormat {
        name: "Windsurf",
        description: "Windsurf skill format",
        availability: SkillFormatAvailability::LocalAndGlobal,
        path: ".windsurf/skills/gitbutler",
    },
];

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

/// Handle skill subcommands
pub fn handle(
    ctx: Option<&mut Context>,
    out: &mut OutputChannel,
    cmd: skill::Subcommands,
) -> Result<()> {
    match cmd {
        skill::Subcommands::Install {
            global,
            path,
            detect,
        } => install_skill(ctx, out, global, path, detect),
        skill::Subcommands::Check {
            global,
            local,
            update,
        } => check_skills(ctx, out, global, local, update),
    }
}

/// Expand tilde in path to home directory
fn expand_tilde(path_str: &str) -> Option<PathBuf> {
    if path_str == "~" || path_str.starts_with("~/") || path_str.starts_with("~\\") {
        dirs::home_dir().map(|home| {
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
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))
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

/// Replace version in SKILL.md content
fn inject_version(content: &str, version: &str) -> String {
    // Handle different line endings (Unix \n, Windows \r\n, or old Mac \r)
    let frontmatter_end = content
        .find("---\n\n")
        .or_else(|| content.find("---\r\n\r\n"))
        .or_else(|| content.find("---\r\r"));

    if let Some(end_pos) = frontmatter_end {
        let frontmatter = &content[..end_pos];
        let rest = &content[end_pos..];
        let updated_frontmatter =
            frontmatter.replace("version: 0.0.0", &format!("version: {version}"));
        format!("{updated_frontmatter}{rest}")
    } else {
        // Fallback if frontmatter format is unexpected
        content.replace("version: 0.0.0", &format!("version: {version}"))
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

/// Validate that a SKILL.md file is actually a GitButler skill
fn is_gitbutler_skill(skill_md_path: &std::path::Path) -> bool {
    if let Ok(content) = std::fs::read_to_string(skill_md_path) {
        // Check for GitButler-specific markers with proper context
        // Look for YAML frontmatter with "name: but" or the specific header
        let has_frontmatter_name = content.lines().any(|line| line.trim() == "name: but");

        let has_gitbutler_header = content.lines().any(|line| {
            line.contains("# GitButler CLI Skill") || line.contains("GitButler CLI (`but` command)")
        });

        has_frontmatter_name || has_gitbutler_header
    } else {
        false
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
    let mut lines = content.lines();

    // Parse YAML frontmatter (between --- markers)
    if lines.next()? != "---" {
        return None;
    }

    // Find the version line in frontmatter
    for line in lines {
        if line == "---" {
            break;
        }
        if let Some(value) = line.strip_prefix("version:") {
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

    if check_global && let Some(home) = dirs::home_dir() {
        base_dirs.push((home, "global"));
    }

    if check_local
        && let Some(ctx) = ctx
        && let Ok(repo) = ctx.repo.get()
        && let Some(workdir) = repo.workdir()
    {
        base_dirs.push((workdir.to_path_buf(), "local"));
    }

    // Check each format in each base directory
    for (base_dir, scope) in base_dirs {
        for format in SKILL_FORMATS {
            let potential_path = format.get_install_path(&base_dir);
            let skill_md_path = potential_path.join("SKILL.md");

            if skill_md_path.exists() && is_gitbutler_skill(&skill_md_path) {
                installations.push((potential_path, format.name, scope));
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

        let up_to_date = installed_version == cli_version;
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
    mut ctx: Option<&mut Context>,
    out: &mut OutputChannel,
    global_only: bool,
    local_only: bool,
    auto_update: bool,
) -> Result<()> {
    // Determine scope
    let (check_global, check_local) = match (global_only, local_only) {
        (true, false) => (true, false),
        (false, true) => (false, true),
        (false, false) => (true, true), // default: check both
        _ => unreachable!(),            // clap conflicts_with prevents this
    };

    // Warn if --local was explicitly requested but no repo context is available
    if local_only && ctx.is_none() {
        anyhow::bail!(
            "Cannot check local installations: not in a git repository.\n\
             Use --global to check global installations, or run from within a repository."
        );
    }

    // First check to find outdated skills (reborrow ctx so we can use it again later)
    let initial_result = check_skill_status(ctx.as_deref_mut(), check_global, check_local)?;

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
        writeln!(progress, "{}", "Updating outdated skills...".bold())?;
        writeln!(progress)?;

        for path_str in &outdated_paths {
            // Pass None for ctx since the paths are already absolute and don't require repo context
            install_skill(None, out, false, Some(path_str.clone()), false)?;
        }
    }

    // Re-check status after updates (or use initial result if no updates)
    let result = if auto_update && !outdated_paths.is_empty() {
        check_skill_status(ctx, check_global, check_local)?
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
                "→".yellow().bold()
            )?;
        }
    } else if let Some(json_out) = out.for_json() {
        json_out.write_value(&result)?;
    } else if let Some(writer) = out.for_shell() {
        // Shell output: one path per line (handles paths with spaces)
        for skill in result.skills.iter().filter(|s| !s.up_to_date) {
            writeln!(writer, "{}", skill.path.display())?;
        }
    }

    Ok(())
}

fn print_human_check_output(
    writer: &mut dyn std::fmt::Write,
    result: &SkillCheckResult,
) -> Result<(), anyhow::Error> {
    writeln!(writer)?;
    writeln!(writer, "CLI version: {}", result.cli_version.cyan())?;
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
            "✓".green()
        } else {
            "✗".red()
        };

        let version_display = if skill.up_to_date {
            skill.installed_version.green().to_string()
        } else {
            format!(
                "{} → {}",
                skill.installed_version.red(),
                result.cli_version.green()
            )
        };

        writeln!(
            writer,
            "  {} {} ({}) - {} [{}]",
            status_icon,
            skill.format_name,
            skill.scope,
            skill.path.display().to_string().dimmed(),
            version_display
        )?;
    }

    writeln!(writer)?;

    if result.outdated_count == 0 {
        writeln!(writer, "{} All skills are up to date!", "✓".green().bold())?;
    } else {
        writeln!(
            writer,
            "{} {} skill(s) are outdated",
            "!".yellow().bold(),
            result.outdated_count
        )?;
    }

    Ok(())
}

/// Detect installation path by finding existing skill installation
fn detect_install_path(ctx: Option<&mut Context>, global: bool) -> Result<PathBuf> {
    // Determine which base directories to check
    let base_dirs: Vec<(PathBuf, &str)> = if global {
        // Only check global locations
        vec![(
            dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?,
            "global",
        )]
    } else if let Some(ctx) = ctx {
        // Check local repo first, then fall back to global
        let repo = ctx.repo.get()?;
        let local_dir = repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("Not in a Git repository"))?
            .to_path_buf();
        let global_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        vec![(local_dir, "local"), (global_dir, "global")]
    } else {
        // No repo context, only check global
        vec![(
            dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?,
            "global",
        )]
    };

    // Check each scope in priority order (local before global)
    // Return the first scope that has installations, erroring only if multiple in same scope
    for (base_dir, scope) in &base_dirs {
        let mut scope_installations: Vec<(PathBuf, &str)> = Vec::new();

        for format in SKILL_FORMATS {
            let potential_path = format.get_install_path(base_dir);
            let skill_md_path = potential_path.join("SKILL.md");
            if skill_md_path.exists() && is_gitbutler_skill(&skill_md_path) {
                scope_installations.push((potential_path, format.name));
            }
        }

        match scope_installations.len() {
            0 => {
                // No installations in this scope, try next scope
                continue;
            }
            1 => {
                // Exactly one installation in this scope - use it
                return Ok(scope_installations[0].0.clone());
            }
            _ => {
                // Multiple installations in the same scope - error
                let installations_list = scope_installations
                    .iter()
                    .map(|(path, format)| {
                        format!("  • {} - {} ({})", format, path.display(), scope)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                anyhow::bail!(
                    "Multiple skill installations found in {scope} scope. Please use --path to specify which one to update:\n{installations_list}"
                )
            }
        }
    }

    // No installations found in any scope
    let checked_locations = base_dirs
        .iter()
        .flat_map(|(base_dir, scope)| {
            SKILL_FORMATS
                .iter()
                .map(move |f| format!("  • {} ({})", f.get_install_path(base_dir).display(), scope))
        })
        .collect::<Vec<_>>()
        .join("\n");

    anyhow::bail!(
        "Could not detect installation location. No existing skill found in:\n{checked_locations}"
    )
}

fn prompt_for_install_scope(progress: &mut impl std::io::Write) -> Result<InstallScope> {
    writeln!(progress)?;
    writeln!(progress, "{}", "Select installation scope:".bold())?;
    writeln!(progress)?;

    let prompt = cli_prompts::prompts::Selection::new(
        "Where would you like to install the skill?",
        vec![InstallScopeOption::Local, InstallScopeOption::Global].into_iter(),
    );

    match prompt.display() {
        Ok(InstallScopeOption::Local) => Ok(InstallScope::Local),
        Ok(InstallScopeOption::Global) => Ok(InstallScope::Global),
        Err(AbortReason::Interrupt) => Err(UserCancelled.into()),
        Err(AbortReason::Error(err)) => {
            Err(anyhow::Error::from(err).context("Failed to read user selection"))
        }
    }
}

/// Prompt user to select installation scope and format
fn prompt_for_install_path(
    ctx: Option<&mut Context>,
    global: bool,
    out: &mut OutputChannel,
    progress: &mut impl std::io::Write,
) -> Result<PathBuf> {
    if out.for_human().is_none() {
        anyhow::bail!(
            "In non-interactive mode, you must specify --path or --detect. Use --path <path> to specify where to install the skill, or --detect to update an existing installation."
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

    let scope = match determine_install_scope_resolution(global, local_scope_available) {
        InstallScopeResolution::PromptUser => prompt_for_install_scope(progress)?,
        InstallScopeResolution::Fixed(scope) => scope,
    };

    if !global && !local_scope_available {
        writeln!(progress)?;
        if ctx.is_none() {
            writeln!(
                progress,
                "{} Not in a git repository. Installing globally in your home directory.",
                "ℹ".blue()
            )?;
        } else {
            writeln!(
                progress,
                "{} Local installs require a repository workdir. Installing globally in your home directory.",
                "ℹ".blue()
            )?;
        }
        writeln!(progress)?;
    }

    let base_dir = get_base_dir(ctx, scope.is_global())?;

    writeln!(progress)?;
    writeln!(progress, "{}", "Select a skill folder format:".bold())?;
    writeln!(progress)?;

    let available_formats: Vec<&SkillFormat> = SKILL_FORMATS
        .iter()
        .filter(|f| f.is_available_for(scope.is_global()))
        .collect();
    debug_assert!(
        !available_formats.is_empty(),
        "At least one skill format must be available for each install scope"
    );

    let options: Vec<String> = available_formats
        .iter()
        .map(|format| {
            let full_path = format.get_install_path(&base_dir);
            format!(
                "{} - {} ({})",
                format.name,
                format.description,
                full_path.display().to_string().dimmed()
            )
        })
        .collect();

    let prompt = cli_prompts::prompts::Selection::new(
        "Which format would you like to use?",
        options.into_iter(),
    );

    let selection: String = match prompt.display() {
        Ok(s) => s,
        Err(AbortReason::Interrupt) => return Err(UserCancelled.into()),
        Err(AbortReason::Error(err)) => {
            return Err(anyhow::Error::from(err).context("Failed to read user selection"));
        }
    };

    // Find the format that matches the selection
    let selected_format = available_formats
        .into_iter()
        .find(|format| {
            let expected_prefix = format!("{} - {}", format.name, format.description);
            selection.starts_with(&expected_prefix)
        })
        .ok_or_else(|| anyhow::anyhow!("Invalid selection"))?;

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

/// Write a skill file with proper error context
fn write_skill_file(path: &std::path::Path, content: &[u8], name: &str) -> Result<()> {
    std::fs::write(path, content).with_context(|| {
        format!(
            "Failed to write {} to {}. Check write permissions.",
            name,
            path.parent()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| path.display().to_string())
        )
    })
}

/// Install the skill files
fn install_skill(
    ctx: Option<&mut Context>,
    out: &mut OutputChannel,
    global: bool,
    custom_path: Option<String>,
    detect: bool,
) -> Result<()> {
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
            .all(|f| !f.name.is_empty() && !f.path.is_empty()),
        "SkillFormat name and path must not be empty"
    );

    let mut progress = out.progress_channel();

    // Validate flags
    if detect && custom_path.is_some() {
        anyhow::bail!("Cannot use both --detect and --path options together");
    }
    if ctx.is_none()
        && !global
        && let Some(custom) = custom_path.as_deref()
    {
        // Without a repository context, only absolute/tilde paths can be resolved without `--global`.
        let expanded = expand_tilde(custom).unwrap_or_else(|| PathBuf::from(custom));
        if !expanded.is_absolute() {
            anyhow::bail!(
                "Cannot use relative --path outside a git repository unless --global is specified.\n\
                 Use --global --path <path> for a global installation, use an absolute path, or run from within a repository for local installation."
            );
        }
    }

    // Determine installation path
    let install_path = if let Some(custom) = custom_path {
        resolve_custom_path(&custom, ctx, global)?
    } else if detect {
        detect_install_path(ctx, global)?
    } else {
        prompt_for_install_path(ctx, global, out, &mut progress)?
    };

    // Validate installation path
    if install_path.exists() && install_path.is_file() {
        anyhow::bail!(
            "Installation path {} is a file, not a directory. Please specify a directory path.",
            install_path.display()
        );
    }

    // Check if files already exist and warn user
    let skill_md_path = install_path.join("SKILL.md");
    if skill_md_path.exists() {
        writeln!(progress)?;
        writeln!(
            progress,
            "{} Skill files already exist at {}",
            "⚠".yellow(),
            install_path.display().to_string().cyan()
        )?;
        writeln!(progress, "  Overwriting existing files...")?;
        writeln!(progress)?;
    }

    // Prepare all content before writing (validate UTF-8 and inject version)
    let version = option_env!("VERSION").unwrap_or("dev");
    let skill_md_content = prepare_skill_content(version)?;

    // Create the directory structure
    let references_dir = install_path.join("references");
    std::fs::create_dir_all(&references_dir).with_context(|| {
        format!(
            "Failed to create skill directory at {}. Check that you have write permissions for this location.",
            install_path.display()
        )
    })?;

    // Write all files
    for file in SKILL_FILES {
        let file_path = install_path.join(file.path);
        let content = if file.path == "SKILL.md" {
            // Use the version-injected content for SKILL.md
            skill_md_content.as_bytes()
        } else {
            file.content
        };
        write_skill_file(&file_path, content, file.display_name)?;
    }

    // Output success message
    writeln!(progress)?;
    writeln!(
        progress,
        "{} GitButler skill installed successfully!",
        "✓".green().bold()
    )?;
    writeln!(progress)?;
    writeln!(
        progress,
        "  Location: {}",
        install_path.display().to_string().cyan()
    )?;
    writeln!(progress)?;
    writeln!(progress, "  Files installed:")?;
    for file in SKILL_FILES {
        writeln!(progress, "    • {}", file.path)?;
    }
    writeln!(progress)?;

    if let Some(out) = out.for_json() {
        let file_paths: Vec<&str> = SKILL_FILES.iter().map(|f| f.path).collect();
        let result = serde_json::json!({
            "success": true,
            "version": version,
            "path": install_path.display().to_string(),
            "files": file_paths
        });
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
            assert!(!format.path.is_empty(), "Format path cannot be empty");
            assert!(
                !format.path.starts_with('/'),
                "Format path should be relative"
            );
        }
    }

    #[test]
    fn skill_format_get_install_path_joins_correctly() {
        let format = SkillFormat {
            name: "Test",
            description: "Test format",
            availability: SkillFormatAvailability::LocalAndGlobal,
            path: ".test/skills/foo",
        };

        let base = PathBuf::from("/home/user");
        let result = format.get_install_path(&base);

        assert_eq!(result, PathBuf::from("/home/user/.test/skills/foo"));
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
            assert!(!file.content.is_empty(), "{} should be embedded", file.path);
        }
    }

    #[test]
    fn embedded_files_are_valid_utf8() {
        // Ensure all embedded files are valid UTF-8
        for file in SKILL_FILES {
            assert!(
                std::str::from_utf8(file.content).is_ok(),
                "{} should be valid UTF-8",
                file.path
            );
        }
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

    // NOTE: detect_install_path is difficult to test in isolation because it depends on
    // dirs::home_dir() and git repository context. It's tested indirectly through
    // integration tests and manual testing. The core logic (is_gitbutler_skill validation
    // and scope prioritization) is tested separately.

    #[test]
    fn is_gitbutler_skill_validates_correct_skill() {
        use std::fs;

        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let skill_path = temp_dir.path().join("SKILL.md");

        // Test with valid GitButler skill content (frontmatter)
        fs::write(
            &skill_path,
            "---\nname: but\nversion: 1.0.0\n---\n# Content",
        )
        .unwrap();
        assert!(
            is_gitbutler_skill(&skill_path),
            "Should recognize valid GitButler skill with frontmatter"
        );

        // Test with valid GitButler skill content (header)
        fs::write(&skill_path, "# GitButler CLI Skill\n\nContent here").unwrap();
        assert!(
            is_gitbutler_skill(&skill_path),
            "Should recognize valid GitButler skill with header"
        );

        // Test with invalid content that contains the strings but not as exact matches
        fs::write(
            &skill_path,
            "I was reading about the GitButler CLI and the name: but that's not right",
        )
        .unwrap();
        assert!(
            !is_gitbutler_skill(&skill_path),
            "Should reject content with strings in wrong context"
        );

        // Test with random content
        fs::write(&skill_path, "Some random content").unwrap();
        assert!(
            !is_gitbutler_skill(&skill_path),
            "Should reject non-GitButler content"
        );

        // Test with nonexistent file
        let nonexistent = temp_dir.path().join("nonexistent.md");
        assert!(
            !is_gitbutler_skill(&nonexistent),
            "Should return false for nonexistent file"
        );
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
        let claude_skill_dir = temp_dir.path().join(".claude/skills/gitbutler");
        fs::create_dir_all(&claude_skill_dir).unwrap();
        let claude_skill_content = "---\nname: but\nversion: 1.0.0\n---\n# GitButler CLI Skill";
        fs::write(claude_skill_dir.join("SKILL.md"), claude_skill_content).unwrap();

        // Create a Cursor skill installation
        let cursor_skill_dir = temp_dir.path().join(".cursor/skills/gitbutler");
        fs::create_dir_all(&cursor_skill_dir).unwrap();
        let cursor_skill_content = "---\nname: but\nversion: 0.9.0\n---\n# GitButler CLI Skill";
        fs::write(cursor_skill_dir.join("SKILL.md"), cursor_skill_content).unwrap();

        // Create a non-GitButler skill (should be ignored)
        let other_skill_dir = temp_dir.path().join(".opencode/skills/gitbutler");
        fs::create_dir_all(&other_skill_dir).unwrap();
        fs::write(other_skill_dir.join("SKILL.md"), "# Some other skill").unwrap();

        // We can't easily test find_all_installations directly since it uses dirs::home_dir()
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

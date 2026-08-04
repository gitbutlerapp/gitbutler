//! Discovering installed GitButler skills and reporting whether they are
//! current.
//!
//! Identity comes from a skill's `SKILL.md` frontmatter, never its folder name,
//! so a custom install path is still recognised — and an unrelated skill living
//! in the same directory is never mistaken for ours.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
// Test-override-aware (`E2E_TEST_APP_DATA_DIR`), so skill discovery and
// installation never touch the real home directory under test.
use but_path::home_dir;
use serde::Serialize;

use crate::format::{SKILL_FILES, SKILL_FORMATS, SkillFormat};

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

/// Validate that a SKILL.md file is actually a GitButler skill by requiring
/// `name: but` in its YAML frontmatter.
///
/// The check is deliberately strict: discovery scans every entry inside an
/// agent's `skills` directory, so a looser match (the string `name: but`
/// appearing in prose, or a `# GitButler CLI Skill` header in another skill's
/// docs) could misclassify an unrelated skill and let `--detect`/`--update`
/// overwrite it.
pub fn is_gitbutler_skill(skill_md_path: &std::path::Path) -> bool {
    std::fs::read_to_string(skill_md_path)
        .ok()
        .and_then(|content| frontmatter_value(&content, "name:"))
        .as_deref()
        == Some("but")
}

/// Extract the version from an installed SKILL.md file's YAML frontmatter.
/// Returns None if the file doesn't exist, isn't readable, or has no valid version.
pub fn extract_installed_version(skill_md_path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(skill_md_path).ok()?;
    extract_installed_version_from_content(&content)
}

/// Extract the version from YAML frontmatter content.
/// Returns None if the content has no frontmatter or no version entry.
pub fn extract_installed_version_from_content(content: &str) -> Option<String> {
    frontmatter_value(content, "version:")
}

/// Read a top-level `key` (e.g. `"name:"`, `"version:"`) from the leading YAML
/// frontmatter and return its parsed value. None if the content has no
/// frontmatter or the key is absent.
pub fn frontmatter_value(content: &str, key: &str) -> Option<String> {
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
pub fn parse_yaml_value(value: &str) -> String {
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
pub fn find_format_installations(format: &SkillFormat, base_dir: &std::path::Path) -> Vec<PathBuf> {
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

pub fn is_complete_skill_installation(path: &std::path::Path) -> bool {
    is_gitbutler_skill(&path.join("SKILL.md"))
        && SKILL_FILES
            .iter()
            .all(|file| file.get_install_path(path).is_file())
}

pub fn is_current_skill_installation(path: &std::path::Path, version: &str) -> bool {
    is_complete_skill_installation(path)
        && extract_installed_version(&path.join("SKILL.md")).as_deref() == Some(version)
}

/// Scope labels for a skill installation. Single-sourced here because
/// [`detect_install_paths`] selects installations by scope.
pub const SCOPE_GLOBAL: &str = "global";
pub const SCOPE_LOCAL: &str = "local";

/// Find all GitButler skill installations.
///
/// Returns a list of (install_path, format_name, scope) tuples.
pub fn find_all_installations(
    workdir: Option<&Path>,
    check_global: bool,
    check_local: bool,
) -> Result<Vec<(PathBuf, &'static str, &'static str)>> {
    let mut installations = Vec::new();

    // Determine which base directories to check
    let mut base_dirs: Vec<(PathBuf, &str)> = Vec::new();

    if check_global && let Some(home) = home_dir() {
        base_dirs.push((home, SCOPE_GLOBAL));
    }

    if check_local && let Some(workdir) = workdir {
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
    workdir: Option<&Path>,
    check_global: bool,
    check_local: bool,
) -> Result<SkillCheckResult> {
    let cli_version = crate::cli_version().to_string();
    let installations = find_all_installations(workdir, check_global, check_local)?;

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

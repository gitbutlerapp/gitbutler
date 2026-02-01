use std::{fmt::Write as _, path::PathBuf};

use anyhow::{Context as _, Result};
use but_ctx::Context;
use cli_prompts::DisplayPrompt;
use colored::Colorize;

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
    /// Path relative to repository root (for local) or home directory (for global)
    path: &'static str,
}

impl SkillFormat {
    /// Get the actual installation path given a base directory
    fn get_install_path(&self, base_dir: &std::path::Path) -> PathBuf {
        base_dir.join(self.path)
    }
}

// Common skill folder formats
const SKILL_FORMATS: &[SkillFormat] = &[
    SkillFormat {
        name: "Claude Code",
        description: "Claude Code CLI skill format",
        path: ".claude/skills/gitbutler",
    },
    SkillFormat {
        name: "OpenCode",
        description: "OpenCode AI skill format",
        path: ".opencode/skills/gitbutler",
    },
    SkillFormat {
        name: "GitHub Copilot",
        description: "GitHub Copilot skill format",
        path: ".github/copilot/skills/gitbutler",
    },
    SkillFormat {
        name: "Cursor",
        description: "Cursor AI skill format",
        path: ".cursor/skills/gitbutler",
    },
    SkillFormat {
        name: "Windsurf",
        description: "Windsurf skill format",
        path: ".windsurf/skills/gitbutler",
    },
];

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
            infer,
        } => install_skill(ctx, out, global, path, infer),
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
            anyhow::anyhow!("Not in a git repository. Use --global to install globally, or run from within a repository.")
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
            frontmatter.replace("version: 0.0.0", &format!("version: {}", version));
        format!("{}{}", updated_frontmatter, rest)
    } else {
        // Fallback if frontmatter format is unexpected
        content.replace("version: 0.0.0", &format!("version: {}", version))
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

/// Infer installation path by detecting existing skill installation
fn infer_install_path(ctx: Option<&mut Context>, global: bool) -> Result<PathBuf> {
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
                    "Multiple skill installations found in {} scope. Please use --path to specify which one to update:\n{}",
                    scope,
                    installations_list
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
        "Could not infer installation location. No existing skill found in:\n{}",
        checked_locations
    )
}

/// Prompt user to select installation format
fn prompt_for_install_path(
    ctx: Option<&mut Context>,
    global: bool,
    out: &mut OutputChannel,
    progress: &mut impl std::io::Write,
) -> Result<PathBuf> {
    if out.for_human().is_none() {
        anyhow::bail!(
            "In non-interactive mode, you must specify --path. Use --path <path> to specify where to install the skill."
        );
    }

    let base_dir = get_base_dir(ctx, global)?;

    writeln!(progress)?;
    writeln!(progress, "{}", "Select a skill folder format:".bold())?;
    writeln!(progress)?;

    let options: Vec<String> = SKILL_FORMATS
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
        Err(_) => {
            // User cancelled the prompt (e.g., pressed Escape)
            writeln!(progress)?;
            return Err(UserCancelled.into());
        }
    };

    // Find the format that matches the selection
    let selected_format = SKILL_FORMATS
        .iter()
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
    infer: bool,
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
    if infer && custom_path.is_some() {
        anyhow::bail!("Cannot use both --infer and --path options together");
    }

    // Determine installation path
    let install_path = if let Some(custom) = custom_path {
        resolve_custom_path(&custom, ctx, global)?
    } else if infer {
        infer_install_path(ctx, global)?
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
    if skill_md_path.exists() && out.for_human().is_some() {
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
    if out.for_human().is_some() {
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
    } else if let Some(out) = out.for_json() {
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
            path: ".test/skills/foo",
        };

        let base = PathBuf::from("/home/user");
        let result = format.get_install_path(&base);

        assert_eq!(result, PathBuf::from("/home/user/.test/skills/foo"));
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

    // NOTE: infer_install_path is difficult to test in isolation because it depends on
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
}

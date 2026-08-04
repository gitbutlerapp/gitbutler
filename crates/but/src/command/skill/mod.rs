use std::{fmt::Write as _, path::PathBuf};

use anyhow::{Context as _, Result};
use but_ctx::Context;
// Test-override-aware (`E2E_TEST_APP_DATA_DIR`), so skill discovery and
// installation never touch the real home directory under test.
use but_path::home_dir;

use but_skill::{
    format::{SKILL_FILES, SKILL_FORMATS, SkillFormat},
    freshness::agent_default_install_path,
    install::write_skill_files,
    status::{
        SCOPE_GLOBAL, SCOPE_LOCAL, SkillCheckResult, check_skill_status, find_all_installations,
    },
};

use crate::{
    args::skill,
    theme::{self, Paint},
    utils::OutputChannel,
};

mod freshness;
pub(crate) use freshness::{agent_skill_notice, agent_skill_update_notice};

/// Error type for user-initiated cancellation
#[derive(Debug, Clone, Copy)]
pub struct UserCancelled;

impl std::fmt::Display for UserCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Installation cancelled by user")
    }
}

impl std::error::Error for UserCancelled {}

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

/// The repository worktree a `Context` points at, if any.
///
/// Skill discovery only ever needed the worktree path, so `but-skill` takes a
/// plain path and this adapts the CLI's `Context` at the call site — which is
/// what keeps `but-skill` free of a `but-ctx` dependency.
fn workdir_of(ctx: Option<&mut Context>) -> Option<PathBuf> {
    let ctx = ctx?;
    let repo = ctx.repo.get().ok()?;
    repo.workdir().map(std::path::Path::to_path_buf)
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

/// Check if installed skills are up to date
fn check_skills(
    mut ctx: Option<&mut Context>,
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

    // Warn if --local was explicitly requested but no repo context is available
    if local_only && ctx.is_none() {
        anyhow::bail!(
            "Cannot check local installations: not in a git repository.\n\
             Use --global to check global installations, or run from within a repository."
        );
    }

    // First check to find outdated skills (reborrow ctx so we can use it again later)
    let initial_result = check_skill_status(
        workdir_of(ctx.as_deref_mut()).as_deref(),
        check_global,
        check_local,
    )?;

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
            install_skill(None, out, false, Some(path_str.clone()), false)?;
        }
    }

    // Re-check status after updates (or use initial result if no updates)
    let result = if auto_update && !outdated_paths.is_empty() {
        check_skill_status(workdir_of(ctx).as_deref(), check_global, check_local)?
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
    let installations = find_all_installations(workdir_of(ctx).as_deref(), true, !global)?;

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

/// Install the skill files
fn install_skill(
    ctx: Option<&mut Context>,
    out: &mut OutputChannel,
    global: bool,
    custom_path: Option<String>,
    detect: bool,
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

        version = write_skill_files(install_path)?;
    }

    if let Some(writer) = out.for_human() {
        writeln!(writer)?;
        writeln!(
            writer,
            "{} GitButler skill installed successfully!",
            t.sym().success
        )?;
        writeln!(writer)?;
        if let [only] = install_paths.as_slice() {
            writeln!(
                writer,
                "  Location: {}",
                t.config_value.paint(only.display().to_string())
            )?;
        } else {
            writeln!(writer, "  Locations:")?;
            for install_path in &install_paths {
                writeln!(
                    writer,
                    "    • {}",
                    t.config_value.paint(install_path.display().to_string())
                )?;
            }
        }
        writeln!(writer)?;
        writeln!(writer, "  Files installed:")?;
        for file in SKILL_FILES {
            writeln!(writer, "    • {}", file.display_path())?;
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
        let file_paths: Vec<String> = SKILL_FILES.iter().map(|f| f.display_path()).collect();
        let paths: Vec<String> = install_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        let result = serde_json::json!({
            "success": true,
            "version": version,
            "paths": paths,
            "files": file_paths
        });
        out.write_value(result)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use but_skill::status::SkillStatus;

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

        let version = write_skill_files(&install_path).expect("a writable path accepts the bundle");

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
}

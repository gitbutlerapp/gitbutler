//! Shell and PATH configuration

use std::{
    env,
    fs::{self, OpenOptions},
    io::Write as IoWrite,
    path::{Path, PathBuf},
};

use anyhow::Result;

use crate::ui::{self, info, success, warn};

/// Detected shell type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellType {
    Zsh,
    Bash,
    Fish,
}

impl ShellType {
    fn completion_command(&self) -> &'static str {
        match self {
            ShellType::Zsh => "eval \"$(but completions zsh)\"",
            ShellType::Bash => "eval \"$(but completions bash)\"",
            ShellType::Fish => "but completions fish | source",
        }
    }

    fn name(&self) -> &'static str {
        match self {
            ShellType::Zsh => "zsh",
            ShellType::Bash => "bash",
            ShellType::Fish => "fish",
        }
    }
}

pub(crate) fn setup_path(home_dir: &Path) -> Result<()> {
    let bin_dir = home_dir.join(".local/bin");

    // Canonicalize bin_dir for comparison (resolves symlinks, removes .., etc.)
    let bin_dir_canonical = fs::canonicalize(&bin_dir).unwrap_or_else(|_| bin_dir.clone());

    // Check if already in PATH with normalized comparison
    let current_path = env::var("PATH").unwrap_or_default();
    let already_in_path = current_path.split(':').any(|p| {
        if p.is_empty() {
            return false;
        }

        // Expand tilde and $HOME in PATH entries
        let expanded = if p.starts_with("~/") {
            home_dir.join(p.trim_start_matches("~/"))
        } else if p.starts_with("$HOME/") {
            home_dir.join(p.trim_start_matches("$HOME/"))
        } else {
            PathBuf::from(p)
        };

        // Normalize by removing trailing slashes and canonicalizing
        let normalized = fs::canonicalize(&expanded).unwrap_or_else(|_| {
            // If path doesn't exist yet, at least normalize the string
            let path_str = expanded.to_string_lossy();
            PathBuf::from(path_str.trim_end_matches('/'))
        });

        normalized == bin_dir_canonical
    });

    if already_in_path {
        success(&format!("{} is already in your PATH", bin_dir.display()));
    }

    // Detect shell config file
    let fish_config = home_dir.join(".config/fish/config.fish");
    let zshrc = home_dir.join(".zshrc");
    let bash_profile = home_dir.join(".bash_profile");
    let bashrc = home_dir.join(".bashrc");

    if fish_config.exists() {
        setup_fish_config(&fish_config, &bin_dir, already_in_path)?;
    } else if let Some((shell_config, shell_type)) = detect_shell_config(&zshrc, &bash_profile, &bashrc) {
        setup_posix_shell_config(&shell_config, shell_type, &bin_dir, already_in_path)?;
    } else {
        print_manual_setup_instructions(&bin_dir, already_in_path);
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn detect_shell_config(zshrc: &Path, bash_profile: &Path, bashrc: &Path) -> Option<(PathBuf, ShellType)> {
    if zshrc.exists() {
        Some((zshrc.to_path_buf(), ShellType::Zsh))
    } else if bash_profile.exists() {
        Some((bash_profile.to_path_buf(), ShellType::Bash))
    } else if bashrc.exists() {
        Some((bashrc.to_path_buf(), ShellType::Bash))
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn detect_shell_config(
    zshrc: &Path,
    // bash_profile is only sourced when executing a login shell, which is rarely how shells are
    // invoked on Linux distros. Therefore, we ignore it when detecting shell config on Linux.
    _bash_profile: &Path,
    bashrc: &Path,
) -> Option<(PathBuf, ShellType)> {
    if zshrc.exists() {
        Some((zshrc.to_path_buf(), ShellType::Zsh))
    } else if bashrc.exists() {
        Some((bashrc.to_path_buf(), ShellType::Bash))
    } else {
        None
    }
}

fn setup_fish_config(fish_config: &Path, _bin_dir: &Path, already_in_path: bool) -> Result<()> {
    let contents = fs::read_to_string(fish_config).unwrap_or_default();

    // Check for PATH setup - detect various Fish patterns for adding .local/bin to PATH
    let has_path_setup = contents.lines().any(|line| {
        let trimmed = line.trim();
        // Check if line mentions .local/bin and likely configures PATH
        if !trimmed.contains(".local/bin") {
            return false;
        }
        // Common patterns:
        // - fish_add_path $HOME/.local/bin
        // - set -gx PATH $HOME/.local/bin $PATH
        // - set -x PATH ~/.local/bin $PATH
        // - set PATH $HOME/.local/bin $PATH
        trimmed.contains("fish_add_path") || (trimmed.contains("set") && trimmed.contains("PATH"))
    });

    // Check for completions
    let completion_cmd = ShellType::Fish.completion_command();
    let has_completions = contents.contains("but completions");

    let needs_path_setup = !already_in_path && !has_path_setup;
    let needs_completions = !has_completions;

    if needs_path_setup || needs_completions {
        ui::println_empty();
        info("Fish shell detected. Please add the following to your ~/.config/fish/config.fish:");

        if needs_path_setup {
            ui::println("  fish_add_path $HOME/.local/bin");
        }

        if needs_completions {
            ui::println(&format!("  {completion_cmd}"));
        }
    } else {
        success("Fish shell configuration is already set up");
    }

    Ok(())
}

fn setup_posix_shell_config(
    shell_config: &Path,
    shell_type: ShellType,
    bin_dir: &Path,
    already_in_path: bool,
) -> Result<()> {
    let path_cmd = format!("export PATH=\"{}:$PATH\"", bin_dir.display());
    let completion_cmd = shell_type.completion_command();

    let contents = fs::read_to_string(shell_config).unwrap_or_default();

    // Check for PATH setup with more robust detection
    // Look for lines that export PATH and contain .local/bin
    let has_path_setup = contents.lines().any(|line| {
        let trimmed = line.trim();
        // Must be a PATH export line
        if !trimmed.starts_with("export PATH=") && !trimmed.starts_with("export PATH ") {
            return false;
        }
        // Must contain .local/bin (handles ~, $HOME, full paths, etc.)
        trimmed.contains(".local/bin")
    });

    let has_completions = contents.contains("but completions");

    // Determine what needs to be added based on config file contents, not current environment
    let needs_path_in_config = !has_path_setup;
    let needs_completions = !has_completions;
    let needs_update = needs_path_in_config || needs_completions;

    if !needs_update {
        if has_path_setup {
            info(&format!(
                "PATH configuration already exists in {}",
                shell_config.display()
            ));
        }
        if has_completions {
            success(&format!("{} shell completions already configured", shell_type.name()));
        }
        return Ok(());
    }

    // Try to add to config file
    match OpenOptions::new().append(true).open(shell_config) {
        Ok(mut file) => {
            writeln!(file)?;
            writeln!(file, "# Added by GitButler installer")?;

            if needs_path_in_config {
                writeln!(file, "{path_cmd}")?;
            }

            if needs_completions {
                writeln!(file, "{completion_cmd}")?;
            }

            let mut updated_items = Vec::new();
            if needs_path_in_config {
                updated_items.push("PATH");
            }
            if needs_completions {
                updated_items.push("completions");
            }

            success(&format!(
                "Updated {} to include {}",
                shell_config.display(),
                updated_items.join(" and ")
            ));

            // Only show sourcing instructions if PATH was added and not already in current session
            if needs_path_in_config && !already_in_path {
                ui::println_empty();
                info("To use 'but' in this terminal session, run:");
                ui::println(&format!("  source \"{}\"", shell_config.display()));
                ui::println_empty();
                info("Or close and reopen your terminal");
            }
        }
        Err(e) => {
            use std::io::ErrorKind;
            let error_msg = match e.kind() {
                ErrorKind::PermissionDenied => "permission denied".to_string(),
                ErrorKind::NotFound => "file not found".to_string(),
                _ => format!("{e}"),
            };

            warn(&format!("Cannot write to {} ({})", shell_config.display(), error_msg));
            info("Please add the following lines to your shell config file manually:");

            if needs_path_in_config {
                ui::println(&format!("  {path_cmd}"));
            }
            if needs_completions {
                ui::println(&format!("  {completion_cmd}"));
            }
        }
    }

    Ok(())
}

fn print_manual_setup_instructions(bin_dir: &Path, already_in_path: bool) {
    ui::println_empty();
    warn("Could not detect your shell configuration file");

    if already_in_path {
        // PATH is already set up, only need completions
        info("To set up shell completions, add this to your shell config file:");
        ui::println("  eval \"$(but completions <shell>)\"  # Replace <shell> with bash or zsh");
    } else {
        // Need both PATH and completions
        info("Please add the following lines to your shell config file:");
        ui::println(&format!("  export PATH=\"{}:$PATH\"", bin_dir.display()));
        ui::println("  eval \"$(but completions <shell>)\"  # Replace <shell> with bash or zsh");
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn test_shell_type_completion_commands() {
        assert_eq!(ShellType::Zsh.completion_command(), "eval \"$(but completions zsh)\"");
        assert_eq!(ShellType::Bash.completion_command(), "eval \"$(but completions bash)\"");
        assert_eq!(ShellType::Fish.completion_command(), "but completions fish | source");
    }

    #[test]
    fn test_shell_type_names() {
        assert_eq!(ShellType::Zsh.name(), "zsh");
        assert_eq!(ShellType::Bash.name(), "bash");
        assert_eq!(ShellType::Fish.name(), "fish");
    }

    #[test]
    fn test_setup_posix_shell_config_adds_path_and_completions() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_dir = temp_dir.path();
        let bin_dir = home_dir.join(".local/bin");
        let zshrc = home_dir.join(".zshrc");

        // Create empty config file
        std::fs::File::create(&zshrc).unwrap();

        // Setup shell config (PATH not already set)
        setup_posix_shell_config(&zshrc, ShellType::Zsh, &bin_dir, false).unwrap();

        // Verify both PATH and completions were added
        let content = std::fs::read_to_string(&zshrc).unwrap();
        assert!(content.contains("# Added by GitButler installer"));
        assert!(content.contains(&format!("export PATH=\"{}:$PATH\"", bin_dir.display())));
        assert!(content.contains("eval \"$(but completions zsh)\""));
    }

    #[test]
    fn test_setup_posix_shell_config_adds_path_even_when_in_current_env() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_dir = temp_dir.path();
        let bin_dir = home_dir.join(".local/bin");
        let zshrc = home_dir.join(".zshrc");

        // Create empty config file
        std::fs::File::create(&zshrc).unwrap();

        // Setup shell config with already_in_path=true (PATH in current environment)
        // But config file is empty, so PATH should still be added for persistence
        setup_posix_shell_config(&zshrc, ShellType::Zsh, &bin_dir, true).unwrap();

        // Verify both PATH and completions were added
        // (PATH must be persisted even if temporarily in environment)
        let content = std::fs::read_to_string(&zshrc).unwrap();
        assert!(content.contains("# Added by GitButler installer"));
        assert!(content.contains("export PATH"));
        assert!(content.contains("eval \"$(but completions zsh)\""));
    }

    #[test]
    fn test_setup_posix_shell_config_only_adds_completions_when_path_in_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_dir = temp_dir.path();
        let bin_dir = home_dir.join(".local/bin");
        let zshrc = home_dir.join(".zshrc");

        // Create config file with PATH already configured
        let mut file = std::fs::File::create(&zshrc).unwrap();
        writeln!(file, "export PATH=\"{}:$PATH\"", bin_dir.display()).unwrap();
        drop(file);

        // Setup shell config (PATH not in environment, but IS in config)
        setup_posix_shell_config(&zshrc, ShellType::Zsh, &bin_dir, false).unwrap();

        // Verify only completions were added, PATH was not duplicated
        let content = std::fs::read_to_string(&zshrc).unwrap();
        assert_eq!(content.matches("export PATH").count(), 1);
        assert!(content.contains("eval \"$(but completions zsh)\""));
    }

    #[test]
    fn test_setup_posix_shell_config_no_duplicates() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_dir = temp_dir.path();
        let bin_dir = home_dir.join(".local/bin");
        let zshrc = home_dir.join(".zshrc");

        // Create config file with existing setup
        let mut file = std::fs::File::create(&zshrc).unwrap();
        writeln!(file, "export PATH=\"{}:$PATH\"", bin_dir.display()).unwrap();
        writeln!(file, "eval \"$(but completions zsh)\"").unwrap();
        drop(file);

        // Try to setup again
        setup_posix_shell_config(&zshrc, ShellType::Zsh, &bin_dir, false).unwrap();

        // Verify no duplicates were added
        let content = std::fs::read_to_string(&zshrc).unwrap();
        assert_eq!(content.matches("export PATH").count(), 1);
        assert_eq!(content.matches("but completions").count(), 1);
    }

    #[test]
    fn test_setup_fish_config_prints_manual_instructions() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_dir = temp_dir.path();
        let bin_dir = home_dir.join(".local/bin");
        let fish_config_dir = home_dir.join(".config/fish");
        std::fs::create_dir_all(&fish_config_dir).unwrap();
        let fish_config = fish_config_dir.join("config.fish");

        // Create empty config file
        std::fs::File::create(&fish_config).unwrap();

        // Setup fish config (PATH not already set)
        // Fish setup only prints instructions, doesn't modify the file
        setup_fish_config(&fish_config, &bin_dir, false).unwrap();

        // Verify file is unchanged (fish config doesn't auto-modify)
        let content = std::fs::read_to_string(&fish_config).unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn test_setup_fish_config_detects_existing_setup() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_dir = temp_dir.path();
        let bin_dir = home_dir.join(".local/bin");
        let fish_config_dir = home_dir.join(".config/fish");
        std::fs::create_dir_all(&fish_config_dir).unwrap();
        let fish_config = fish_config_dir.join("config.fish");

        // Create config file with existing setup
        let mut file = std::fs::File::create(&fish_config).unwrap();
        writeln!(file, "fish_add_path ~/.local/bin").unwrap();
        writeln!(file, "but completions fish | source").unwrap();
        drop(file);

        // Setup should detect existing configuration
        setup_fish_config(&fish_config, &bin_dir, false).unwrap();

        // Verify file is unchanged (existing setup detected)
        let content = std::fs::read_to_string(&fish_config).unwrap();
        assert_eq!(content.matches("fish_add_path").count(), 1);
        assert_eq!(content.matches("but completions").count(), 1);
    }

    #[test]
    fn test_setup_posix_shell_detects_path_variations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_dir = temp_dir.path();
        let bin_dir = home_dir.join(".local/bin");
        let zshrc = home_dir.join(".zshrc");

        // Test various PATH export formats that should all be detected
        let variations = [
            "export PATH=\"$HOME/.local/bin:$PATH\"",
            "export PATH='$HOME/.local/bin:$PATH'",
            "export PATH=~/.local/bin:$PATH",
            "export PATH=\"/Users/test/.local/bin:$PATH\"",
            "  export PATH=\"$HOME/.local/bin:$PATH\"  # with whitespace",
        ];

        for (i, path_line) in variations.iter().enumerate() {
            // Create config with this variation
            std::fs::write(&zshrc, path_line).unwrap();

            // Try to setup - should detect existing PATH
            setup_posix_shell_config(&zshrc, ShellType::Zsh, &bin_dir, false).unwrap();

            // Verify no duplicate PATH line was added
            let content = std::fs::read_to_string(&zshrc).unwrap();
            let path_count = content.lines().filter(|l| l.contains("export PATH=")).count();
            assert_eq!(path_count, 1, "Variation {i} should not add duplicate PATH entry");
        }
    }

    #[test]
    fn test_setup_fish_config_with_path_already_in_env() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_dir = temp_dir.path();
        let bin_dir = home_dir.join(".local/bin");
        let fish_config = home_dir.join("config.fish");

        // Create empty config file
        std::fs::File::create(&fish_config).unwrap();

        // PATH is already in environment but NOT in config file
        // Should NOT print PATH instructions (already_in_path=true)
        // Should print completion instructions (has_completions=false)
        setup_fish_config(&fish_config, &bin_dir, true).unwrap();

        // Verify file is unchanged (we don't auto-write to Fish configs)
        let content = std::fs::read_to_string(&fish_config).unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn test_setup_fish_config_detects_path_variations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_dir = temp_dir.path();
        let bin_dir = home_dir.join(".local/bin");
        let fish_config_dir = home_dir.join(".config/fish");
        std::fs::create_dir_all(&fish_config_dir).unwrap();
        let fish_config = fish_config_dir.join("config.fish");

        // Test various Fish PATH configuration formats
        let variations = [
            "fish_add_path $HOME/.local/bin",
            "fish_add_path ~/.local/bin",
            "set -gx PATH $HOME/.local/bin $PATH",
            "set -x PATH ~/.local/bin $PATH",
            "set PATH $HOME/.local/bin $PATH",
            "  set -gx PATH $HOME/.local/bin $PATH  # with comment",
        ];

        for (i, path_line) in variations.iter().enumerate() {
            // Create config with this variation plus completions
            let config_content = format!("{path_line}\nbut completions fish | source");
            std::fs::write(&fish_config, &config_content).unwrap();

            // Try to setup - should detect existing PATH and completions
            setup_fish_config(&fish_config, &bin_dir, false).unwrap();

            // Verify no changes were made (already configured)
            let content = std::fs::read_to_string(&fish_config).unwrap();
            assert_eq!(
                content, config_content,
                "Variation {i} should not modify already-configured Fish config"
            );
        }
    }
}

//! Shell and PATH configuration

use std::{
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

    fn path_command(&self) -> &'static str {
        match self {
            ShellType::Zsh | ShellType::Bash => "export PATH=\"$HOME/.local/bin:$PATH\"",
            ShellType::Fish => "fish_add_path $HOME/.local/bin",
        }
    }

    fn name(&self) -> &'static str {
        match self {
            ShellType::Zsh => "zsh",
            ShellType::Bash => "bash",
            ShellType::Fish => "fish",
        }
    }

    fn detect_config(&self, home_dir: &Path) -> Option<ShellConfig> {
        match self {
            ShellType::Fish => ShellConfig::detect_fish_shell_config(home_dir),
            ShellType::Bash | ShellType::Zsh => {
                ShellConfig::detect_posix_shell_config(home_dir, *self)
            }
        }
    }

    /// Find the most likely config path for the shell, if it exists
    fn config_path(&self, home_dir: &Path) -> Option<PathBuf> {
        match self {
            ShellType::Zsh => some_if_is_file(home_dir.join(".zshrc")),
            ShellType::Bash => {
                #[cfg(target_os = "macos")]
                {
                    // On macOS, iTerm starts as a login shell and therefore only sources
                    // bash_profile, so we look for that first.
                    let bash_profile = home_dir.join(".bash_profile");
                    if bash_profile.is_file() {
                        return Some(bash_profile);
                    }
                }

                some_if_is_file(home_dir.join(".bashrc"))
            }
            ShellType::Fish => some_if_is_file(home_dir.join(".config/fish/config.fish")),
        }
    }
}

fn some_if_is_file(path: PathBuf) -> Option<PathBuf> {
    if path.is_file() { Some(path) } else { None }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShellConfig {
    shell: ShellType,
    config_path: PathBuf,
    has_completions: bool,
    has_path: bool,
}

impl ShellConfig {
    fn setup_config(&self) -> Result<()> {
        setup_shell_config(self)
    }

    fn is_fully_configured(&self) -> bool {
        self.has_path && self.has_completions
    }

    fn generate_missing_shell_config_lines(&self) -> Vec<&'static str> {
        let mut missing_shell_config_lines: Vec<&'static str> = vec![];
        if !self.has_path {
            missing_shell_config_lines.push(self.shell.path_command());
        }
        if !self.has_completions {
            missing_shell_config_lines.push(self.shell.completion_command());
        }

        missing_shell_config_lines
    }

    /// Create an empty [`ShellConfig`] for `shell` with a bogus `config_path`.
    fn empty_config(shell: ShellType) -> Self {
        ShellConfig {
            shell,
            config_path: PathBuf::from("/dev/null"),
            has_completions: false,
            has_path: false,
        }
    }

    fn detect_posix_shell_config(home_dir: &Path, shell: ShellType) -> Option<ShellConfig> {
        let config_path = shell.config_path(home_dir)?;
        let contents = fs::read_to_string(&config_path).unwrap_or_default();

        // Check for PATH setup with more robust detection
        // Look for lines that export PATH and contain .local/bin
        let has_path = contents.lines().any(|line| {
            let trimmed = line.trim();
            // Must be a PATH export line
            if !trimmed.starts_with("export PATH=") && !trimmed.starts_with("export PATH ") {
                return false;
            }
            // Must contain .local/bin (handles ~, $HOME, full paths, etc.)
            trimmed.contains(".local/bin")
        });

        let has_completions = contents.contains("but completions");

        Some(ShellConfig {
            shell,
            config_path,
            has_completions,
            has_path,
        })
    }

    fn detect_fish_shell_config(home_dir: &Path) -> Option<ShellConfig> {
        let config_path = ShellType::Fish.config_path(home_dir)?;
        let contents = fs::read_to_string(&config_path).unwrap_or_default();

        // Check for PATH setup - detect various Fish patterns for adding .local/bin to PATH
        let has_path = contents.lines().any(|line| {
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
            trimmed.contains("fish_add_path")
                || (trimmed.contains("set") && trimmed.contains("PATH"))
        });

        let has_completions = contents.contains("but completions");
        Some(ShellConfig {
            shell: ShellType::Fish,
            config_path,
            has_completions,
            has_path,
        })
    }
}

/// Interactively add shell configurations for the user.
///
/// This is designed to run interactively and errors if [`ui::is_connected_to_terminal`] returns `false`.
pub(crate) fn configure_shell(home_dir: &Path) -> Result<()> {
    if !ui::is_connected_to_terminal() {
        anyhow::bail!("Shell configuration cannot be performed without a connected terminal");
    }

    let detected_shell_configs: Vec<ShellConfig> =
        [ShellType::Bash, ShellType::Zsh, ShellType::Fish]
            .map(|config| config.detect_config(home_dir))
            .into_iter()
            .flatten()
            .collect();
    let num_detected_shells = detected_shell_configs.len();

    let unconfigured_shells: Vec<ShellConfig> = detected_shell_configs
        .into_iter()
        .filter(|cfg| !cfg.is_fully_configured())
        .collect();

    for cfg in &unconfigured_shells {
        ui::println_empty();
        info(&format!(
            "Detected {} config file: '{}'",
            cfg.shell.name(),
            cfg.config_path.display()
        ));

        let mut missing = Vec::new();
        if !cfg.has_path {
            missing.push("PATH setup");
        }
        if !cfg.has_completions {
            missing.push("shell completions");
        }
        info(&format!("Config file lacks: {}", missing.join(", ")));

        if ui::prompt_for_confirmation("Automatically add missing config?") {
            if let Err(err) = cfg.setup_config() {
                warn(&format!("{err}"));
                warn(&format!(
                    "Failed to add shell configuration for {}, skipping...",
                    cfg.shell.name()
                ));
            }
        } else {
            info(&format!(
                "Skipping auto-configuration for {}",
                cfg.shell.name()
            ));
            info(&format!(
                "For manual setup, add the following to '{}'",
                cfg.config_path.display(),
            ));
            ui::println_empty();
            for line in cfg.generate_missing_shell_config_lines() {
                ui::println(&format!("  {line}"));
            }
        }
    }

    if num_detected_shells == 0 {
        print_fallback_setup_instructions();
    }

    Ok(())
}

fn setup_shell_config(cfg: &ShellConfig) -> Result<()> {
    let path_cmd = cfg.shell.path_command();
    let completion_cmd = cfg.shell.completion_command();

    let needs_path = !cfg.has_path;
    let needs_completions = !cfg.has_completions;

    if cfg.is_fully_configured() {
        // We shouldn't hit this case in practice as we should never ask the user to setup
        // the shell if there is nothing to do, so this is just being extra defensive.
        info(&format!(
            "Shell already configured in '{}', there is nothing to do.",
            cfg.config_path.display()
        ));
        return Ok(());
    }

    // Try to add to config file
    match OpenOptions::new().append(true).open(&cfg.config_path) {
        Ok(mut file) => {
            writeln!(file)?;
            writeln!(file, "# Added by GitButler installer")?;

            for line in cfg.generate_missing_shell_config_lines() {
                writeln!(file, "{line}")?;
            }

            let mut updated_items = Vec::new();
            if needs_path {
                updated_items.push("PATH");
            }
            if needs_completions {
                updated_items.push("completions");
            }

            success(&format!(
                "Updated {} to include {}",
                cfg.config_path.display(),
                updated_items.join(" and ")
            ));

            if needs_path {
                ui::println_empty();
                info("To use 'but' in this terminal session, you may need to run:");
                ui::println(&format!("  source \"{}\"", cfg.config_path.display()));
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

            warn(&format!(
                "Cannot write to {} ({})",
                cfg.config_path.display(),
                error_msg
            ));
            info("Please add the following lines to your shell config file manually:");

            if needs_path {
                ui::println(&format!("  {path_cmd}"));
            }
            if needs_completions {
                ui::println(&format!("  {completion_cmd}"));
            }
        }
    }

    Ok(())
}

/// Instructions to print if we cannot detect any supported shells
fn print_fallback_setup_instructions() {
    ui::println_empty();
    warn("Could not detect your shell configuration file");

    for shell in [ShellType::Bash, ShellType::Zsh, ShellType::Fish] {
        ui::println_empty();
        info(&format!(
            "For {}, add the following to your config:",
            shell.name()
        ));
        for line in ShellConfig::empty_config(shell).generate_missing_shell_config_lines() {
            ui::println(&format!("  {line}"));
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn tmpdir_with_config_at(
        config_relpath: &Path,
        config_content: Option<&str>,
    ) -> tempfile::TempDir {
        let temp_dir = tempfile::tempdir()
            .expect("must be able to create temporary directory for tests to work");
        let config_abspath = temp_dir.path().join(config_relpath);
        fs::create_dir_all(temp_dir.path().join(config_relpath.parent().unwrap()))
            .expect("must be able to create parent dir of config");

        let config_content = config_content.unwrap_or_default();
        fs::write(&config_abspath, config_content).expect("must be able to write to config file");

        temp_dir
    }

    #[test]
    fn test_shell_type_path_commands() {
        assert_eq!(
            ShellType::Zsh.path_command(),
            "export PATH=\"$HOME/.local/bin:$PATH\""
        );
        assert_eq!(
            ShellType::Bash.path_command(),
            "export PATH=\"$HOME/.local/bin:$PATH\""
        );
        assert_eq!(
            ShellType::Fish.path_command(),
            "fish_add_path $HOME/.local/bin"
        );
    }

    #[test]
    fn test_shell_type_completion_commands() {
        assert_eq!(
            ShellType::Zsh.completion_command(),
            "eval \"$(but completions zsh)\""
        );
        assert_eq!(
            ShellType::Bash.completion_command(),
            "eval \"$(but completions bash)\""
        );
        assert_eq!(
            ShellType::Fish.completion_command(),
            "but completions fish | source"
        );
    }

    #[test]
    fn test_shell_type_names() {
        assert_eq!(ShellType::Zsh.name(), "zsh");
        assert_eq!(ShellType::Bash.name(), "bash");
        assert_eq!(ShellType::Fish.name(), "fish");
    }

    #[test]
    fn test_detect_posix_config_no_config() {
        for shell in &[ShellType::Bash, ShellType::Zsh] {
            let temp_dir = tempfile::tempdir().unwrap();
            let cfg = shell.detect_config(temp_dir.path());
            assert!(cfg.is_none(), "Should not detect config");
        }
    }

    #[test]
    fn test_detect_empty_bash_config() {
        let config_path = Path::new(".bashrc");
        let temp_dir = tmpdir_with_config_at(config_path, None);
        let home_dir = temp_dir.path();

        let cfg = ShellType::Bash
            .detect_config(home_dir)
            .expect("Should detect config");

        assert_eq!(cfg.shell, ShellType::Bash);
        assert_eq!(cfg.config_path, home_dir.join(config_path));
        assert!(!cfg.has_path);
        assert!(!cfg.has_completions);
    }

    #[test]
    fn test_detect_empty_zsh_config() {
        let config_path = Path::new(".zshrc");
        let temp_dir = tmpdir_with_config_at(config_path, None);
        let home_dir = temp_dir.path();

        let cfg = ShellType::Zsh
            .detect_config(home_dir)
            .expect("Should detect config");

        assert_eq!(cfg.shell, ShellType::Zsh);
        assert_eq!(cfg.config_path, home_dir.join(config_path));
        assert!(!cfg.has_path);
        assert!(!cfg.has_completions);
    }

    #[test]
    fn test_detect_posix_config_with_path_variations() {
        let variations = [
            "export PATH=\"$HOME/.local/bin:$PATH\"",
            "export PATH='$HOME/.local/bin:$PATH'",
            "export PATH=~/.local/bin:$PATH",
            "export PATH=\"/Users/test/.local/bin:$PATH\"",
            "  export PATH=\"$HOME/.local/bin:$PATH\"  # with whitespace",
        ];

        for (i, export_statement) in variations.iter().enumerate() {
            let config_path = Path::new(".zshrc");
            let temp_dir = tmpdir_with_config_at(config_path, Some(export_statement));
            let home_dir = temp_dir.path();

            let cfg = ShellType::Zsh
                .detect_config(home_dir)
                .expect("Should detect config");

            assert_eq!(cfg.shell, ShellType::Zsh);
            assert_eq!(cfg.config_path, home_dir.join(config_path));
            assert!(
                cfg.has_path,
                "Variation {i}='{export_statement}' should be detected"
            );
            assert!(!cfg.has_completions);
        }
    }

    #[test]
    fn test_detect_posix_config_with_completions() {
        let config_path = Path::new(".zshrc");
        let temp_dir =
            tmpdir_with_config_at(config_path, Some(ShellType::Zsh.completion_command()));
        let home_dir = temp_dir.path();

        let cfg = ShellType::Zsh
            .detect_config(home_dir)
            .expect("Should detect config");

        assert_eq!(cfg.shell, ShellType::Zsh);
        assert_eq!(cfg.config_path, home_dir.join(config_path));
        assert!(!cfg.has_path);
        assert!(cfg.has_completions);
    }

    #[test]
    fn test_detect_fish_config_no_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let shell_config = ShellType::Fish.detect_config(temp_dir.path());
        assert!(shell_config.is_none(), "Should not detect config");
    }

    #[test]
    fn test_detect_fish_config_empty_config() -> Result<()> {
        let config_path = Path::new(".config/fish/config.fish");
        let temp_dir = tmpdir_with_config_at(config_path, None);
        let home_dir = temp_dir.path();

        let cfg = ShellType::Fish
            .detect_config(home_dir)
            .expect("Should detect config");

        assert_eq!(cfg.shell, ShellType::Fish);
        assert_eq!(cfg.config_path, home_dir.join(config_path));
        assert!(!cfg.has_path);
        assert!(!cfg.has_completions);

        Ok(())
    }

    #[test]
    fn test_detect_fish_config_with_completions() -> Result<()> {
        let config_path = Path::new(".config/fish/config.fish");
        let temp_dir =
            tmpdir_with_config_at(config_path, Some(ShellType::Fish.completion_command()));
        let home_dir = temp_dir.path();

        let cfg = ShellType::Fish
            .detect_config(home_dir)
            .expect("Should detect config");

        assert_eq!(cfg.shell, ShellType::Fish);
        assert_eq!(cfg.config_path, home_dir.join(config_path));
        assert!(!cfg.has_path);
        assert!(cfg.has_completions);

        Ok(())
    }

    #[test]
    fn test_detect_fish_config_detects_path_variations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let home_dir = temp_dir.path();
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

            let cfg = ShellType::Fish
                .detect_config(home_dir)
                .expect("should detect config");

            // Verify no changes were made (already configured)
            assert!(
                cfg.has_path,
                "Variation {i} should not modify already-configured Fish config"
            );
        }
    }

    #[test]
    fn test_setup_fish_shell_config_adds_path_and_completions() -> Result<()> {
        let config_path = Path::new(".config/fish/config.fish");
        let temp_dir = tmpdir_with_config_at(config_path, None);
        let home_dir = temp_dir.path();

        let cfg = ShellType::Fish
            .detect_config(home_dir)
            .expect("Should detect config");

        cfg.setup_config()?;

        let content = std::fs::read_to_string(home_dir.join(config_path)).unwrap();
        assert!(content.contains("# Added by GitButler installer"));
        assert!(content.contains("fish_add_path $HOME/.local/bin"));
        assert!(content.contains("but completions fish | source"));
        Ok(())
    }

    #[test]
    fn test_setup_fish_shell_config_only_adds_completions_when_path_in_config() -> Result<()> {
        let config_path = Path::new(".config/fish/config.fish");
        let temp_dir = tmpdir_with_config_at(config_path, Some("fish_add_path $HOME/.local/bin"));
        let home_dir = temp_dir.path();

        let cfg = ShellType::Fish
            .detect_config(home_dir)
            .expect("Should detect config");

        cfg.setup_config()?;

        let content = std::fs::read_to_string(home_dir.join(config_path)).unwrap();
        assert_eq!(content.matches("fish_add_path").count(), 1);
        assert!(content.contains("but completions fish | source"));
        Ok(())
    }

    #[test]
    fn test_setup_fish_shell_config_no_duplicates() -> Result<()> {
        let config_path = Path::new(".config/fish/config.fish");
        let temp_dir = tmpdir_with_config_at(config_path, None);
        let home_dir = temp_dir.path();

        // detecting and setting up twice should have the same effect as doing it once
        ShellType::Fish
            .detect_config(home_dir)
            .expect("Should detect config")
            .setup_config()?;
        ShellType::Fish
            .detect_config(home_dir)
            .expect("Should detect config")
            .setup_config()?;

        let content = std::fs::read_to_string(home_dir.join(config_path)).unwrap();
        assert_eq!(content.matches("fish_add_path").count(), 1);
        assert_eq!(content.matches("but completions").count(), 1);

        Ok(())
    }

    #[test]
    fn test_setup_posix_shell_config_adds_path_and_completions() -> Result<()> {
        let config_path = Path::new(".zshrc");
        let temp_dir = tmpdir_with_config_at(config_path, None);
        let home_dir = temp_dir.path();

        let cfg = ShellType::Zsh
            .detect_config(home_dir)
            .expect("Should detect config");

        cfg.setup_config()?;

        // Verify both PATH and completions were added
        let content = std::fs::read_to_string(home_dir.join(config_path)).unwrap();
        assert!(content.contains("# Added by GitButler installer"));
        assert!(content.contains("export PATH=\"$HOME/.local/bin:$PATH\""));
        assert!(content.contains("eval \"$(but completions zsh)\""));
        Ok(())
    }

    #[test]
    fn test_setup_posix_shell_config_only_adds_completions_when_path_in_config() -> Result<()> {
        let config_path = Path::new(".zshrc");
        let temp_dir = tmpdir_with_config_at(config_path, Some("export PATH=\"~/.local/bin\""));
        let home_dir = temp_dir.path();

        let cfg = ShellType::Zsh
            .detect_config(home_dir)
            .expect("Should detect config");

        cfg.setup_config()?;

        // Verify only completions were added, PATH was not duplicated
        let content = std::fs::read_to_string(home_dir.join(config_path)).unwrap();
        assert_eq!(content.matches("export PATH").count(), 1);
        assert!(content.contains("eval \"$(but completions zsh)\""));
        Ok(())
    }

    #[test]
    fn test_setup_posix_shell_config_no_duplicates() -> Result<()> {
        let config_path = Path::new(".zshrc");
        let temp_dir = tmpdir_with_config_at(config_path, None);
        let home_dir = temp_dir.path();

        // detecting and setting up twice should have the same effect as doing it once
        ShellType::Zsh
            .detect_config(home_dir)
            .expect("Should detect config")
            .setup_config()?;
        ShellType::Zsh
            .detect_config(home_dir)
            .expect("Should detect config")
            .setup_config()?;

        // Verify no duplicates were added
        let content = std::fs::read_to_string(home_dir.join(config_path)).unwrap();
        assert_eq!(content.matches("export PATH").count(), 1);
        assert_eq!(content.matches("but completions").count(), 1);

        Ok(())
    }
}

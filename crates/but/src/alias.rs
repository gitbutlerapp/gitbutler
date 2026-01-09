//! Git-style alias support for the `but` CLI.
//!
//! This module provides functionality to expand command aliases defined in git config,
//! similar to how `git` handles aliases like `git co` -> `git checkout`.
//!
//! Aliases are read from git config under the `but.alias.<name>` key.
//!
//! ## Examples
//!
//! ```bash
//! # Set up aliases
//! git config but.alias.st status
//! git config but.alias.stv "status --verbose"
//! git config but.alias.co "commit --only"
//!
//! # Use them
//! but st           # Expands to: but status
//! but stv          # Expands to: but status --verbose
//! but co -m "fix"  # Expands to: but commit --only -m "fix"
//! ```

use std::ffi::OsString;

use anyhow::Result;

/// Expands command aliases before argument parsing.
///
/// If the first argument after "but" is an alias defined in git config
/// (under `but.alias.<name>`), it will be expanded to its definition.
/// Additional arguments are preserved and appended after the expansion.
///
/// # Arguments
///
/// * `args` - The raw command-line arguments including the program name
///
/// # Returns
///
/// The expanded arguments, or the original arguments if no alias was found
pub fn expand_aliases(args: Vec<OsString>) -> Result<Vec<OsString>> {
    // Skip if no subcommand (just "but" or "but --help", etc.)
    if args.len() < 2 {
        return Ok(args);
    }

    // Check if the first argument (after "but") might be an alias
    let potential_alias = match args[1].to_str() {
        Some(s) => s,
        None => return Ok(args), // Non-UTF8, not an alias
    };

    // Skip if it's a flag or a known subcommand
    if potential_alias.starts_with('-') || is_known_subcommand(potential_alias) {
        return Ok(args);
    }

    // Try to read from git config: but.alias.<name>
    let alias_value = match read_git_config_alias(potential_alias) {
        Some(value) => value,
        None => {
            // Check for default aliases that can be overridden
            match get_default_alias(potential_alias) {
                Some(default) => default,
                None => return Ok(args), // No alias found
            }
        }
    };

    // Parse the alias value (may contain multiple words/args)
    let alias_parts: Vec<OsString> = shell_words::split(&alias_value)?
        .into_iter()
        .map(OsString::from)
        .collect();

    // Reconstruct args: [but, ...alias_parts, ...remaining_args]
    let mut expanded = vec![args[0].clone()]; // Keep "but"
    expanded.extend(alias_parts);
    expanded.extend(args[2..].iter().cloned()); // Remaining args after the alias

    Ok(expanded)
}

/// Checks if a command is a known subcommand (not an alias).
///
/// This prevents known commands from being treated as aliases.
fn is_known_subcommand(cmd: &str) -> bool {
    matches!(
        cmd,
        "status"
            | "st"
            | "rub"
            | "diff"
            | "init"
            | "pull"
            | "branch"
            | "worktree"
            | "mark"
            | "unmark"
            | "gui"
            | "."
            | "commit"
            | "push"
            | "new"
            | "reword"
            | "oplog"
            | "restore"
            | "undo"
            | "absorb"
            | "discard"
            | "forge"
            | "pr"
            | "review"
            | "mcp"
            | "claude"
            | "cursor"
            | "actions"
            | "metrics"
            | "completions"
            | "resolve"
            | "fetch"
            | "alias"
    )
}

/// Gets a default alias value for built-in aliases that can be overridden.
///
/// These are convenience aliases that ship with `but` but can be overridden
/// by setting them in git config.
///
/// # Arguments
///
/// * `alias_name` - The name of the alias to look up
///
/// # Returns
///
/// The default alias value if one exists, or `None`
fn get_default_alias(alias_name: &str) -> Option<String> {
    match alias_name {
        "stf" => Some("status --files".to_string()),
        _ => None,
    }
}

/// Reads a git config alias value using gix.
///
/// Looks for the config key `but.alias.<name>` in the git configuration.
///
/// # Arguments
///
/// * `alias_name` - The name of the alias to look up
///
/// # Returns
///
/// The alias value if found, or `None` if not found or on error
fn read_git_config_alias(alias_name: &str) -> Option<String> {
    // Try to discover a git repository from the current directory
    let repo = gix::discover(".").ok()?;

    // Get the config snapshot and look for but.alias.<name>
    let config_key = format!("but.alias.{}", alias_name);
    let config = repo.config_snapshot();

    // Try to read the string value from config
    config.string(&config_key).map(|v| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_known_subcommand() {
        assert!(is_known_subcommand("status"));
        assert!(is_known_subcommand("st"));
        assert!(is_known_subcommand("commit"));
        assert!(is_known_subcommand("push"));
        assert!(is_known_subcommand("gui"));
        assert!(!is_known_subcommand("unknown"));
        assert!(!is_known_subcommand("co"));
    }

    #[test]
    fn test_expand_no_args() {
        let args = vec![OsString::from("but")];
        let result = expand_aliases(args.clone()).unwrap();
        assert_eq!(result, args);
    }

    #[test]
    fn test_expand_known_command() {
        let args = vec![OsString::from("but"), OsString::from("status")];
        let result = expand_aliases(args.clone()).unwrap();
        assert_eq!(result, args);
    }

    #[test]
    fn test_expand_with_flag() {
        let args = vec![OsString::from("but"), OsString::from("--help")];
        let result = expand_aliases(args.clone()).unwrap();
        assert_eq!(result, args);
    }

    #[test]
    fn test_expand_unknown_alias_no_config() {
        // This test will pass through since there's no git config set
        let args = vec![OsString::from("but"), OsString::from("unknownalias")];
        let result = expand_aliases(args.clone()).unwrap();
        assert_eq!(result, args);
    }

    #[test]
    fn test_default_alias_stf() {
        assert_eq!(get_default_alias("stf"), Some("status --files".to_string()));
        assert_eq!(get_default_alias("other"), None);
    }

    #[test]
    fn test_expand_default_alias() {
        // Test that the default stf alias expands correctly
        let args = vec![
            OsString::from("but"),
            OsString::from("stf"),
            OsString::from("--verbose"),
        ];
        let result = expand_aliases(args).unwrap();

        // Should expand to: but status --files --verbose
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], OsString::from("but"));
        assert_eq!(result[1], OsString::from("status"));
        assert_eq!(result[2], OsString::from("--files"));
        assert_eq!(result[3], OsString::from("--verbose"));
    }
}

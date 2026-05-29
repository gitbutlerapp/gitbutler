//! Git-style alias support for the `but` CLI.
//!
//! This module provides functionality to expand command aliases defined in git config,
//! similar to how `git` handles aliases like `git co` -> `git checkout`.
//!
//! Aliases are read from git config under the `but.alias.<name>` key.

use std::ffi::OsString;

use anyhow::Result;

/// Default aliases that ship with `but` and can be overridden by git config.
const DEFAULT_ALIASES: &[(&str, &str)] = &[
    ("default", "status"),
    ("st", "status"),
    ("stf", "status --files"),
];

/// Attempts to expand `potential_alias`.
///
/// Passing a known subcommand is considered an error.
///
/// Note that at present, aliases are resolved from the real working directory (".").
///
/// # Arguments
///
/// * `potential_alias` - A potential alias that we try to expand
///
/// # Returns
///
/// A vector containing the expansion of `potential_alias`. If `potential_alias` did not match any
/// existing alias, it expands into itself (i.e. `vec![potential_alias]`).
///
/// An `Ok(..)` value does _not_ indicate that an alias was found!
pub fn expand_alias(potential_alias: &str) -> Result<Vec<OsString>> {
    if is_known_subcommand(potential_alias) {
        anyhow::bail!(
            "BUG: Tried to expand known subcommand {potential_alias} as an alias - parsing order should not allow this!"
        )
    }

    // Try to read from git config: but.alias.<name>
    // And try to discover a git repository from the current directory, way before we have a context.
    let repo = gix::discover(".").ok();
    let alias_value = match repo
        .as_ref()
        .and_then(|repo| read_git_config_alias(repo, potential_alias))
    {
        Some(value) => value,
        None => {
            // Check for default aliases that can be overridden
            match get_default_alias(potential_alias) {
                Some(default) => default,
                None => return Ok(vec![potential_alias.into()]), // No alias found
            }
        }
    };

    // Parse the alias value (may contain multiple words/args)
    let alias_parts: Vec<OsString> = shell_words::split(&alias_value)?
        .into_iter()
        .map(OsString::from)
        .collect();

    Ok(alias_parts)
}

/// Checks if a command is a known subcommand (not an alias).
///
/// This prevents known commands from being treated as aliases.
/// Extracts subcommand names directly from clap's Command structure.
pub fn is_known_subcommand(cmd: &str) -> bool {
    use clap::CommandFactory;

    let command = crate::args::Args::command();
    command.get_subcommands().any(|subcmd| {
        subcmd.get_name() == cmd || subcmd.get_all_aliases().any(|alias| alias == cmd)
    })
}

/// Gets all default aliases as a vector of (name, value) tuples.
///
/// These are convenience aliases that ship with `but` but can be overridden
/// by setting them in git config.
pub fn get_all_default_aliases() -> Vec<(String, String)> {
    DEFAULT_ALIASES
        .iter()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
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
pub fn get_default_alias(alias_name: &str) -> Option<String> {
    DEFAULT_ALIASES
        .iter()
        .find(|(name, _)| *name == alias_name)
        .map(|(_, value)| value.to_string())
}

/// Reads a git config alias value from `repo`.
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
fn read_git_config_alias(repo: &gix::Repository, alias_name: &str) -> Option<String> {
    // Get the config snapshot and look for but.alias.<name>
    let config_key = format!("but.alias.{alias_name}");
    let config = repo.config_snapshot();

    // Try to read the string value from config
    config.string(&config_key).map(|v| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "legacy")] // only works when these commands are implemented.
    fn is_known_subcommand_() {
        assert!(is_known_subcommand("status"));
        assert!(is_known_subcommand("commit"));
        assert!(is_known_subcommand("push"));
        assert!(is_known_subcommand("gui"));
        assert!(!is_known_subcommand("unknown"));
        assert!(!is_known_subcommand("co"));
        assert!(!is_known_subcommand("st"));
    }

    #[test]
    fn default_alias_stf() {
        assert_eq!(get_default_alias("stf"), Some("status --files".to_string()));
        assert_eq!(get_default_alias("other"), None);
    }
}

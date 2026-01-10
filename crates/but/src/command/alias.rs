//! Command implementation for managing `but` aliases.
//!
//! Provides subcommands to list, add, and remove aliases stored in git config.

use anyhow::{Context, Result};
use colored::Colorize;

use crate::utils::OutputChannel;

/// List all configured `but` aliases
pub fn list(out: &mut OutputChannel) -> Result<()> {
    // Use gix to read aliases from git config
    let mut user_aliases: Vec<(String, String)> = Vec::new();

    if let Ok(repo) = gix::discover(".") {
        let config = repo.config_snapshot();

        // Get all sections with name "but" and subsection starting with "alias."
        // We iterate through sections and check their subsections
        let sections: Vec<_> = config.sections().filter(|s| s.header().name() == "but").collect();

        for section in sections {
            if let Some(subsection) = section.header().subsection_name() {
                let subsection_str: &str = subsection.to_str().unwrap_or("");
                if let Some(alias_name) = subsection_str.strip_prefix("alias.") {
                    // Get the value for this alias
                    let key = format!("but.alias.{}", alias_name);
                    if let Some(value) = config.string(&key) {
                        user_aliases.push((alias_name.to_string(), value.to_string()));
                    }
                }
            }
        }
    }

    // Get default aliases
    let default_aliases = get_default_aliases();

    // Check if we have any aliases to show
    if user_aliases.is_empty() && default_aliases.is_empty() {
        if let Some(out) = out.for_human() {
            writeln!(out, "No aliases configured.")?;
            writeln!(out)?;
            writeln!(out, "Create an alias with:")?;
            writeln!(out, "  but alias add st status")?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({
                "user": {},
                "default": {}
            }))?;
        }
        return Ok(());
    }

    // Sort aliases by name
    user_aliases.sort_by(|a, b| a.0.cmp(&b.0));

    if let Some(out) = out.for_human() {
        // Calculate max name length for alignment
        let max_name_len = user_aliases
            .iter()
            .chain(default_aliases.iter())
            .map(|(name, _)| name.len())
            .max()
            .unwrap_or(0);

        // Show user-configured aliases first
        if !user_aliases.is_empty() {
            writeln!(out, "{}:", "User aliases".bold())?;
            writeln!(out)?;

            for (name, value) in &user_aliases {
                writeln!(
                    out,
                    "  {:<width$}  {}  {}",
                    name.green(),
                    "→".dimmed(),
                    value.cyan(),
                    width = max_name_len
                )?;
            }
            writeln!(out)?;
        }

        // Show default aliases
        if !default_aliases.is_empty() {
            writeln!(
                out,
                "{} {}:",
                "Default aliases".bold(),
                "(overridable)".dimmed()
            )?;
            writeln!(out)?;

            for (name, value) in &default_aliases {
                // Check if this default is overridden
                let is_overridden = user_aliases.iter().any(|(n, _)| n == name);

                if is_overridden {
                    writeln!(
                        out,
                        "  {:<width$}  {}  {}  {}",
                        name.dimmed(),
                        "→".dimmed(),
                        value.dimmed(),
                        "(overridden)".dimmed(),
                        width = max_name_len
                    )?;
                } else {
                    writeln!(
                        out,
                        "  {:<width$}  {}  {}",
                        name.green(),
                        "→".dimmed(),
                        value.cyan(),
                        width = max_name_len
                    )?;
                }
            }
        }
    } else if let Some(out) = out.for_json() {
        let user_json: serde_json::Map<String, serde_json::Value> = user_aliases
            .into_iter()
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect();

        let default_json: serde_json::Map<String, serde_json::Value> = default_aliases
            .into_iter()
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect();

        out.write_value(serde_json::json!({
            "user": user_json,
            "default": default_json
        }))?;
    }

    Ok(())
}

/// Get all default aliases
fn get_default_aliases() -> Vec<(String, String)> {
    crate::alias::get_all_default_aliases()
}

/// Add a new alias
pub fn add(out: &mut OutputChannel, name: &str, value: &str, global: bool) -> Result<()> {
    // Validate alias name doesn't conflict with known commands
    if crate::alias::is_known_subcommand(name) {
        anyhow::bail!(
            "Cannot create alias '{}': it conflicts with a built-in command",
            name
        );
    }

    // Use git config command to set the alias
    // This is more reliable and simpler than using gix's config mutation APIs
    let config_key = format!("but.alias.{}", name);
    let mut cmd = std::process::Command::new("git");
    cmd.args(["config"]);

    if global {
        cmd.arg("--global");
    }

    cmd.args([&config_key, value]);

    let output = cmd.output()
        .context("Failed to execute git config")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to set alias: {}", stderr);
    }

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{} Alias '{}' {} '{}'",
            "✓".green(),
            name.green(),
            "→".dimmed(),
            value.cyan()
        )?;
        if global {
            writeln!(out, "  (configured globally)")?;
        }
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "name": name,
            "value": value,
            "scope": if global { "global" } else { "local" }
        }))?;
    }

    Ok(())
}

/// Remove an alias
pub fn remove(out: &mut OutputChannel, name: &str, global: bool) -> Result<()> {
    // Use git config command to unset the alias
    // This is more reliable and simpler than using gix's config mutation APIs
    let config_key = format!("but.alias.{}", name);
    let mut cmd = std::process::Command::new("git");
    cmd.args(["config", "--unset"]);

    if global {
        cmd.arg("--global");
    }

    cmd.arg(&config_key);

    let output = cmd.output()
        .context("Failed to execute git config")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to remove alias: {}", stderr);
    }

    if let Some(out) = out.for_human() {
        writeln!(out, "{} Removed alias '{}'", "✓".green(), name.green())?;
        if global {
            writeln!(out, "  (from global config)")?;
        }
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "name": name,
            "removed": true,
            "scope": if global { "global" } else { "local" }
        }))?;
    }

    Ok(())
}

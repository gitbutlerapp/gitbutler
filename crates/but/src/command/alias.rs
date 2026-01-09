//! Command implementation for managing `but` aliases.
//!
//! Provides subcommands to list, add, and remove aliases stored in git config.

use anyhow::{Context, Result};
use colored::Colorize;

use crate::utils::OutputChannel;

/// List all configured `but` aliases
pub fn list(out: &mut OutputChannel) -> Result<()> {
    // Use git config command to list aliases since gix config API is complex
    let output = std::process::Command::new("git")
        .args(["config", "--get-regexp", "^but\\.alias\\."])
        .output()
        .context("Failed to execute git config")?;

    let mut aliases = Vec::new();

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some((key, value)) = line.split_once(' ') {
                // key is like "but.alias.st", we want just "st"
                if let Some(name) = key.strip_prefix("but.alias.") {
                    aliases.push((name.to_string(), value.to_string()));
                }
            }
        }
    }

    if aliases.is_empty() {
        if let Some(out) = out.for_human() {
            writeln!(out, "No aliases configured.")?;
            writeln!(out)?;
            writeln!(out, "Create an alias with:")?;
            writeln!(out, "  but alias add st status")?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({}))?;
        }
        return Ok(());
    }

    // Sort aliases by name
    aliases.sort_by(|a, b| a.0.cmp(&b.0));

    if let Some(out) = out.for_human() {
        writeln!(out, "{}:", "Configured aliases".bold())?;
        writeln!(out)?;

        let max_name_len = aliases
            .iter()
            .map(|(name, _)| name.len())
            .max()
            .unwrap_or(0);

        for (name, value) in &aliases {
            writeln!(
                out,
                "  {:<width$}  {}  {}",
                name.green(),
                "→".dimmed(),
                value.cyan(),
                width = max_name_len
            )?;
        }
    } else if let Some(out) = out.for_json() {
        let json_aliases: serde_json::Map<String, serde_json::Value> = aliases
            .into_iter()
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect();
        out.write_value(serde_json::json!(json_aliases))?;
    }

    Ok(())
}

/// Add a new alias
pub fn add(out: &mut OutputChannel, name: &str, value: &str, global: bool) -> Result<()> {
    // Validate alias name doesn't conflict with known commands
    if is_known_subcommand(name) {
        anyhow::bail!(
            "Cannot create alias '{}': it conflicts with a built-in command",
            name
        );
    }

    // Use git config command to set the alias
    let config_key = format!("but.alias.{}", name);
    let scope = if global { "--global" } else { "--local" };

    let status = std::process::Command::new("git")
        .args(["config", scope, &config_key, value])
        .status()
        .context("Failed to execute git config")?;

    if !status.success() {
        anyhow::bail!("Failed to set alias in git config");
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
    let config_key = format!("but.alias.{}", name);
    let scope = if global { "--global" } else { "--local" };

    let status = std::process::Command::new("git")
        .args(["config", scope, "--unset", &config_key])
        .status()
        .context("Failed to execute git config")?;

    if !status.success() {
        anyhow::bail!("Alias '{}' not found", name);
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

/// Check if a name conflicts with a known subcommand
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
            | "alias" // Don't allow aliasing the alias command itself!
    )
}

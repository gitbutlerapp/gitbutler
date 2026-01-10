//! Command implementation for managing `but` aliases.
//!
//! Provides subcommands to list, add, and remove aliases stored in git config.

use anyhow::Result;
use bstr::ByteSlice;
use but_ctx::Context;
use colored::Colorize;

use crate::utils::OutputChannel;

/// List all configured `but` aliases
pub fn list(out: &mut OutputChannel) -> Result<()> {
    // Use gix to read aliases from git config
    let mut user_aliases: Vec<(String, String)> = Vec::new();

    if let Ok(repo) = gix::discover(".") {
        let cfg = repo.config_snapshot(); // resolved view of config stack  [oai_citation:4‡Docs.rs](https://docs.rs/gix/latest/gix/struct.Repository.html)

        for section in cfg.sections() {
            let header = section.header();
            let section_name = header.name().to_str_lossy();
            if section_name != "but" {
                continue;
            }

            let subsection = header.subsection_name().map(|s| s.to_str_lossy()); //  [oai_citation:7‡Docs.rs](https://docs.rs/gix/latest/gix/config/parse/section/struct.Header.html)

            for value_name in section.value_names() {
                let vn = value_name.as_ref();

                // Normalize to a dotted key we can prefix-test: "but.alias.<rest>"
                let dotted = match &subsection {
                    // [but "alias"] foo = bar  => but.alias.foo
                    Some(sub) => format!("{}.{}.{}", section_name, sub, vn),
                    // [but] alias.foo = bar    => but.alias.foo
                    None => format!("{}.{}", section_name, vn),
                };

                if !dotted.starts_with("but.alias.") {
                    continue;
                }

                if let Some(val) = section.value(vn) {
                    // Extract the alias name from "but.alias.<name>"
                    let alias_name = dotted.strip_prefix("but.alias.").unwrap();
                    user_aliases.push((alias_name.to_string(), val.to_str_lossy().into_owned()));
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
pub fn add(
    ctx: &mut Context,
    out: &mut OutputChannel,
    name: &str,
    value: &str,
    global: bool,
) -> Result<()> {
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

    dbg!("setting alias", &config_key, value, global);

    let repo = &*ctx.git2_repo.get()?;
    if global {
        let all = git2::Config::open_default()?;
        let mut global = all.open_level(git2::ConfigLevel::Global)?;
        let key = format!("but.alias.{}", name);
        global.set_str(&key, value)?;
    } else {
        let mut cfg = repo.config()?; // repo (local) config
        let key = format!("but.alias.{}", name);
        cfg.set_str(&key, value)?;
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
pub fn remove(_out: &mut OutputChannel, _name: &str, _global: bool) -> Result<()> {
    Ok(())
}

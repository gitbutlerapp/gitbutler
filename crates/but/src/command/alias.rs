//! Command implementation for managing `but` aliases.
//!
//! Provides subcommands to list, add, and remove aliases stored in git config.

use std::collections::HashMap;

use anyhow::Result;
use bstr::ByteSlice;
use but_ctx::Context;
use colored::Colorize;
use serde::Serialize;

use crate::utils::OutputChannel;

/// Represents where an alias is configured
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AliasScope {
    Local,
    Global,
    Both,
}

impl std::fmt::Display for AliasScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AliasScope::Local => write!(f, "local"),
            AliasScope::Global => write!(f, "global"),
            AliasScope::Both => write!(f, "both"),
        }
    }
}

/// An alias entry with its name, value, and scope
#[derive(Debug, Clone, Serialize)]
pub struct AliasEntry {
    pub name: String,
    pub value: String,
    pub scope: AliasScope,
}

/// List all configured `but` aliases
pub fn list(out: &mut OutputChannel) -> Result<()> {
    let user_aliases = get_all_aliases()?;

    // Get default aliases
    let default_aliases = get_default_aliases();

    // Check if we have any aliases to show
    if user_aliases.is_empty() && default_aliases.is_empty() {
        if let Some(out) = out.for_human() {
            writeln!(out, "No aliases configured.")?;
            writeln!(out)?;
            writeln!(out, "Create an alias with:")?;
            writeln!(out, "  but alias add stup 'status --upstream'")?;
        } else if let Some(out) = out.for_json() {
            out.write_value(serde_json::json!({
                "user": {},
                "default": {}
            }))?;
        }
        return Ok(());
    }

    if let Some(out) = out.for_human() {
        // Calculate max name length for alignment
        let max_name_len = user_aliases
            .iter()
            .map(|a| a.name.len())
            .chain(default_aliases.iter().map(|(name, _)| name.len()))
            .max()
            .unwrap_or(0);

        // Show user-configured aliases first
        if !user_aliases.is_empty() {
            writeln!(out, "{}:", "User aliases".bold())?;
            writeln!(out)?;

            for alias in &user_aliases {
                let scope_indicator = match alias.scope {
                    AliasScope::Local => "(local)".dimmed(),
                    AliasScope::Global => "(global)".dimmed(),
                    AliasScope::Both => "(local+global)".dimmed(),
                };
                writeln!(
                    out,
                    "  {:<width$}  {}  {} {}",
                    alias.name.green(),
                    "→".dimmed(),
                    alias.value.cyan(),
                    scope_indicator,
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
                let is_overridden = user_aliases.iter().any(|a| &a.name == name);

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
        let user_json: Vec<serde_json::Value> = user_aliases
            .iter()
            .map(|a| {
                serde_json::json!({
                    "name": a.name,
                    "value": a.value,
                    "scope": a.scope
                })
            })
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

/// Get all user-configured aliases from local and global git config and defaults
fn get_all_aliases() -> Result<Vec<AliasEntry>> {
    // Track aliases by name with their scopes
    let mut alias_map: HashMap<String, (String, bool, bool)> = HashMap::new(); // name -> (value, is_local, is_global)

    if let Ok(repo) = gix::discover(".") {
        let cfg = repo.config_snapshot();

        for section in cfg.sections() {
            let header = section.header();
            let section_name = header.name().to_str_lossy();
            if section_name != "but" {
                continue;
            }

            // Determine if this section is from local or global config
            let source = section.meta().source;
            let is_local = matches!(
                source,
                gix::config::Source::Local | gix::config::Source::Worktree
            );
            let is_global = matches!(source, gix::config::Source::User | gix::config::Source::Git);

            let subsection = header.subsection_name().map(|s| s.to_str_lossy());

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
                    let alias_name = dotted.strip_prefix("but.alias.").unwrap().to_string();
                    let value = val.to_str_lossy().into_owned();

                    alias_map
                        .entry(alias_name)
                        .and_modify(|(v, local, global)| {
                            *v = value.clone(); // Last value wins
                            if is_local {
                                *local = true;
                            }
                            if is_global {
                                *global = true;
                            }
                        })
                        .or_insert((value, is_local, is_global));
                }
            }
        }
    }

    let mut user_aliases: Vec<AliasEntry> = alias_map
        .into_iter()
        .map(|(name, (value, is_local, is_global))| {
            let scope = match (is_local, is_global) {
                (true, true) => AliasScope::Both,
                (true, false) => AliasScope::Local,
                (false, true) => AliasScope::Global,
                (false, false) => AliasScope::Local, // Shouldn't happen, but default to local
            };
            AliasEntry { name, value, scope }
        })
        .collect();

    user_aliases.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(user_aliases)
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

    // ok, let's set the value in git config (using git2)
    let key = format!("but.alias.{}", name);
    let repo = &*ctx.git2_repo.get()?;
    if global {
        let all = git2::Config::open_default()?;
        let mut global = all.open_level(git2::ConfigLevel::Global)?;
        global.set_str(&key, value)?;
    } else {
        let mut cfg = repo.config()?; // repo (local) config
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
pub fn remove(ctx: &mut Context, out: &mut OutputChannel, name: &str, global: bool) -> Result<()> {
    // ok, let's try to remove the value in git config (using git2)
    let mut success = false;
    let key = format!("but.alias.{}", name);
    let repo = &*ctx.git2_repo.get()?;
    if global {
        let all = git2::Config::open_default()?;
        let mut global = all.open_level(git2::ConfigLevel::Global)?;
        if global.get_entry(&key).is_ok() {
            global.remove(&key)?;
            success = true;
        }
    } else {
        let mut cfg = repo.config()?; // repo (local) config
        if cfg.get_entry(&key).is_ok() {
            cfg.remove(&key)?;
            success = true;
        }
    }

    if let Some(out) = out.for_human() {
        if !success {
            writeln!(out, "{} Alias '{}' not found", "✗".red(), name.green())?;
            return Ok(());
        } else {
            writeln!(out, "Alias '{}' removed", name.green())?;
            if global {
                writeln!(out, "  (globally)")?;
            }
        }
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "name": name,
            "scope": if global { "global" } else { "local" },
            "removed": success
        }))?;
    }

    Ok(())
}

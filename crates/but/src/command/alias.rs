//! Command implementation for managing `but` aliases.
//!
//! Provides subcommands to list, add, and remove aliases stored in git config.

use std::collections::HashMap;

use anyhow::{Context as _, Result};
use bstr::ByteSlice;
use but_ctx::Context;
use serde::Serialize;

use super::git_config::{EditGlobalConfig, edit_git_config};
use crate::{
    theme::{self, Paint},
    utils::OutputChannel,
};

/// An alias entry with its name, effective value, and all contributing scopes.
#[derive(Debug, Clone, Serialize)]
pub struct AliasEntry {
    pub name: String,
    pub value: String,
    /// Unique scopes in configuration traversal order; user and XDG config share `global`.
    pub scopes: Vec<&'static str>,
}

/// List all configured `but` aliases
pub fn list(ctx: Option<&Context>, out: &mut OutputChannel) -> Result<()> {
    let repo = ctx.map(|ctx| ctx.repo.get()).transpose()?;
    let user_aliases = match repo.as_deref() {
        Some(repo) => get_all_aliases(&repo.config_snapshot()),
        None => get_all_aliases(&gix::config(None, &gix::open::Options::default())?),
    };

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
        let t = theme::get();
        // Calculate max name length for alignment
        let max_name_len = user_aliases
            .iter()
            .map(|a| a.name.len())
            .chain(default_aliases.iter().map(|(name, _)| name.len()))
            .max()
            .unwrap_or(0);

        // Show user-configured aliases first
        if !user_aliases.is_empty() {
            writeln!(out, "{}:", t.important.paint("User aliases"))?;
            writeln!(out)?;

            for alias in &user_aliases {
                let scope_indicator = t.hint.paint(format!("({})", alias.scopes.join("+")));
                writeln!(
                    out,
                    "  {:<width$}  {}  {} {}",
                    t.config_key.paint(&alias.name),
                    t.sym().arrow,
                    t.config_value.paint(&alias.value),
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
                t.important.paint("Default aliases"),
                t.hint.paint("(overridable)")
            )?;
            writeln!(out)?;

            for (name, value) in &default_aliases {
                // Check if this default is overridden
                let is_overridden = user_aliases.iter().any(|a| &a.name == name);

                if is_overridden {
                    writeln!(
                        out,
                        "  {:<width$}  {}  {}  {}",
                        t.hint.paint(name),
                        t.sym().arrow,
                        t.hint.paint(value),
                        t.hint.paint("(overridden)"),
                        width = max_name_len
                    )?;
                } else {
                    writeln!(
                        out,
                        "  {:<width$}  {}  {}",
                        t.config_key.paint(name),
                        t.sym().arrow,
                        t.config_value.paint(value),
                        width = max_name_len
                    )?;
                }
            }
        }
    } else if let Some(out) = out.for_json() {
        let default_json: serde_json::Map<String, serde_json::Value> = default_aliases
            .into_iter()
            .map(|(k, v)| (k, serde_json::Value::String(v)))
            .collect();

        out.write_value(serde_json::json!({
            "user": user_aliases,
            "default": default_json
        }))?;
    }

    Ok(())
}

/// Get configured aliases from every source, preserving the last value for each name.
fn get_all_aliases(cfg: &gix::config::File) -> Vec<AliasEntry> {
    // Track aliases by name with their scopes
    let mut alias_map: HashMap<String, (String, Vec<&'static str>)> = HashMap::new();

    for section in cfg.sections() {
        let header = section.header();
        let section_name = header.name().to_str_lossy();
        if section_name != "but" {
            continue;
        }

        let scope = match section.meta().source {
            gix::config::Source::GitInstallation => "git-installation",
            gix::config::Source::System => "system",
            gix::config::Source::Git | gix::config::Source::User => "global",
            gix::config::Source::Local => "local",
            gix::config::Source::Worktree => "worktree",
            gix::config::Source::Env => "env",
            gix::config::Source::Cli => "cli",
            gix::config::Source::Api => "api",
            gix::config::Source::EnvOverride => "env-override",
        };

        let subsection = header.subsection_name().map(|s| s.to_str_lossy());

        for value_name in section.value_names() {
            let vn = value_name;

            // Normalize to a dotted key we can prefix-test: "but.alias.<rest>"
            let dotted = match &subsection {
                // [but "alias"] foo = bar  => but.alias.foo
                Some(sub) => format!("{section_name}.{sub}.{vn}"),
                // [but] alias.foo = bar    => but.alias.foo
                None => format!("{section_name}.{vn}"),
            };

            if !dotted.starts_with("but.alias.") {
                continue;
            }

            if let Some(val) = section.value(vn) {
                let alias_name = dotted.strip_prefix("but.alias.").unwrap().to_string();
                let value = val.to_str_lossy().into_owned();

                alias_map
                    .entry(alias_name)
                    .and_modify(|(v, scopes)| {
                        *v = value.clone(); // Last value wins
                        if !scopes.contains(&scope) {
                            scopes.push(scope);
                        }
                    })
                    .or_insert((value, vec![scope]));
            }
        }
    }

    let mut user_aliases: Vec<AliasEntry> = alias_map
        .into_iter()
        .map(|(name, (value, scopes))| AliasEntry {
            name,
            value,
            scopes,
        })
        .collect();

    user_aliases.sort_by(|a, b| a.name.cmp(&b.name));
    user_aliases
}

/// Get all default aliases
fn get_default_aliases() -> Vec<(String, String)> {
    crate::alias::get_all_default_aliases()
}

/// Add a new alias
pub fn add(
    ctx: Option<&Context>,
    out: &mut OutputChannel,
    name: &str,
    value: &str,
    global: EditGlobalConfig,
) -> Result<()> {
    // Validate alias name doesn't conflict with known commands
    if crate::alias::is_known_subcommand(name) {
        anyhow::bail!("Cannot create alias '{name}': it conflicts with a built-in command");
    }

    let is_global: bool = global.into();
    let repo = ctx.map(|ctx| ctx.repo.get()).transpose()?;
    edit_git_config(repo.as_deref(), global, |config| {
        set_alias(config, name, value)?;
        Ok(())
    })?;

    if let Some(out) = out.for_human() {
        let t = theme::get();
        writeln!(
            out,
            "{} Alias '{}' {} '{}'",
            t.sym().success,
            t.config_key.paint(name),
            t.sym().arrow,
            t.config_value.paint(value)
        )?;
        if is_global {
            writeln!(out, "  (configured globally)")?;
        }
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "name": name,
            "value": value,
            "scope": if is_global { "global" } else { "local" }
        }))?;
    }

    Ok(())
}

/// Remove an alias
pub fn remove(
    ctx: Option<&Context>,
    out: &mut OutputChannel,
    name: &str,
    global: EditGlobalConfig,
) -> Result<()> {
    let is_global: bool = global.into();
    let repo = ctx.map(|ctx| ctx.repo.get()).transpose()?;
    let success = edit_git_config(repo.as_deref(), global, |config| {
        remove_alias(config, name);
        Ok(())
    })?;

    if let Some(out) = out.for_human() {
        let t = theme::get();
        if !success {
            writeln!(
                out,
                "{} Alias '{}' not found",
                t.sym().error,
                t.config_key.paint(name)
            )?;
            return Ok(());
        } else {
            writeln!(out, "Alias '{}' removed", t.config_key.paint(name))?;
            if is_global {
                writeln!(out, "  (globally)")?;
            }
        }
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "name": name,
            "scope": if is_global { "global" } else { "local" },
            "removed": success
        }))?;
    }

    Ok(())
}

fn remove_alias(config: &mut gix::config::File, name: &str) {
    config
        .section_mut("but", Some("alias".into()))
        .ok()
        .and_then(|mut section| section.remove(name));
}

fn set_alias(config: &mut gix::config::File, name: &str, value: &str) -> Result<()> {
    let mut section = config.section_mut_or_create_new("but", Some("alias".into()))?;
    section
        .set(name, value)
        .with_context(|| format!("invalid alias name for git config: {name}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_scopes_preserve_all_sources_and_precedence() -> Result<()> {
        use gix::config::{File, Source, file::Metadata};

        let mut combined = File::new(Metadata::from(Source::Api));
        for (source, scope) in [
            (Source::GitInstallation, "git-installation"),
            (Source::System, "system"),
            (Source::Git, "global"),
            (Source::User, "global"),
            (Source::Local, "local"),
            (Source::Worktree, "worktree"),
            (Source::Env, "env"),
            (Source::Cli, "cli"),
            (Source::Api, "api"),
            (Source::EnvOverride, "env-override"),
        ] {
            let mut config = File::new(Metadata::from(source));
            set_alias(&mut config, "example", scope)?;
            // Every source is reported accurately even when it is the only source.
            snapbox::assert_data_eq!(
                serde_json::to_string(&get_all_aliases(&config))?,
                serde_json::json!([{"name": "example", "value": scope, "scopes": [scope]}])
                    .to_string()
            );
            combined.append(config)?;
        }
        // Repeated scopes are deduplicated, and the last source still supplies the value.
        snapbox::assert_data_eq!(
            serde_json::to_string(&get_all_aliases(&combined))?,
            serde_json::json!([{
                "name": "example",
                "value": "env-override",
                "scopes": ["git-installation", "system", "global", "local", "worktree", "env", "cli", "api", "env-override"]
            }]).to_string()
        );
        Ok(())
    }
}

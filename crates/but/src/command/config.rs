//! Command implementation for managing `but` configuration.
//!
//! Provides subcommands to view and modify configuration settings including
//! user information, AI provider, forge accounts, and target branch.

use anyhow::Result;
use but_ctx::Context;
use colored::Colorize;
use serde::Serialize;

use crate::args::config::{Subcommands, UserSubcommand};
use crate::tui;
use crate::utils::OutputChannel;

/// Main entry point for config command
pub async fn exec(
    ctx: &mut Context,
    out: &mut OutputChannel,
    cmd: Option<Subcommands>,
) -> Result<()> {
    match cmd {
        Some(Subcommands::User { cmd }) => user_config(ctx, out, cmd).await,
        Some(Subcommands::Target { branch }) => target_config(ctx, out, branch).await,
        Some(Subcommands::Forge) => forge_config(ctx, out).await,
        None => show_overview(ctx, out).await,
    }
}

/// Show overview of important settings
async fn show_overview(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    #[derive(Serialize)]
    struct ConfigOverview {
        name: Option<String>,
        email: Option<String>,
        editor: Option<String>,
        target_branch: Option<String>,
        forge_configured: bool,
    }

    // Get user info from git config
    let user_info = {
        let git2_repo = &*ctx.git2_repo.get()?;
        let git2_config = git2_repo.config()?;
        get_user_config_info(&git2_config)?
    };

    // Get target branch
    let target_branch =
        but_api::legacy::virtual_branches::get_base_branch_data(ctx.legacy_project.id)?;

    let forge_configured = false;

    if let Some(out) = out.for_human() {
        writeln!(out, "\n{}", "GitButler Configuration".bold())?;
        writeln!(out)?;

        // User section
        writeln!(out, "{}:", "User".bold())?;
        write_user_config_human(out, &user_info)?;

        // Target branch
        writeln!(out, "{}:", "Target Branch".bold())?;
        if let Some(branch) = &target_branch {
            writeln!(out, "  {}", branch.branch_name.cyan())?;
        } else {
            writeln!(out, "  {}", "(not set)".dimmed())?;
        }
        writeln!(out)?;

        // Forge
        writeln!(out, "{}:", "Forge".bold())?;
        if true {
            writeln!(out, "  {}", "✓ Configured".green())?;
        } else {
            writeln!(
                out,
                "  {} Run {} to authenticate",
                "✗ Not configured".red(),
                "but forge auth".cyan()
            )?;
        }
        writeln!(out)?;

        // Hints
        writeln!(out, "{}", "Available subcommands:".dimmed())?;
        writeln!(
            out,
            "  {}   - View/set user settings (name, email, editor)",
            "but config user".blue().dimmed()
        )?;
        writeln!(
            out,
            "  {}     - View AI configuration",
            "but config ai".blue().dimmed()
        )?;
        writeln!(
            out,
            "  {}  - View forge configuration",
            "but config forge".blue().dimmed()
        )?;
        writeln!(
            out,
            "  {} - View/set target branch",
            "but config target".blue().dimmed()
        )?;
    } else if let Some(out) = out.for_json() {
        let target_branch_name = target_branch.map(|b| b.branch_name.to_string());
        out.write_value(serde_json::json!(ConfigOverview {
            name: user_info.name,
            email: user_info.email,
            editor: Some(user_info.editor),
            target_branch: target_branch_name,
            forge_configured,
        }))?;
    }

    Ok(())
}

/// User configuration information
#[derive(serde::Serialize)]
struct UserConfigInfo {
    name: Option<String>,
    email: Option<String>,
    editor: String,
    name_scope: Option<String>,
    email_scope: Option<String>,
    editor_scope: Option<String>,
}

/// Get user configuration info from git config
fn get_user_config_info(config: &git2::Config) -> Result<UserConfigInfo> {
    let name = config.get_string("user.name").ok();
    let email = config.get_string("user.email").ok();
    let name_scope = get_config_scope(config, "user.name");
    let email_scope = get_config_scope(config, "user.email");
    let editor_scope = get_config_scope(config, "core.editor");
    let editor = tui::get_text::get_editor_command()?;

    Ok(UserConfigInfo {
        name,
        email,
        editor,
        name_scope,
        email_scope,
        editor_scope,
    })
}

/// Write user config info in human-readable format
fn write_user_config_human(out: &mut dyn std::fmt::Write, info: &UserConfigInfo) -> Result<()> {
    writeln!(
        out,
        "    {}: {} {}",
        "Name".dimmed(),
        info.name
            .as_deref()
            .map(|n| n.cyan())
            .unwrap_or_else(|| "(not set)".red()),
        format_scope(&info.name_scope)
    )?;
    writeln!(
        out,
        "   {}: {} {}",
        "Email".dimmed(),
        info.email
            .as_deref()
            .map(|e| e.cyan())
            .unwrap_or_else(|| "(not set)".red()),
        format_scope(&info.email_scope)
    )?;
    writeln!(
        out,
        "  {}: {} {}",
        "Editor".dimmed(),
        info.editor.cyan(),
        format_scope(&info.editor_scope)
    )?;
    writeln!(out)?;
    Ok(())
}

/// Handle user config subcommand
async fn user_config(
    ctx: &mut Context,
    out: &mut OutputChannel,
    cmd: Option<UserSubcommand>,
) -> Result<()> {
    let repo = &*ctx.git2_repo.get()?;

    match cmd {
        // View user config
        None => {
            let config = repo.config()?;
            let user_info = get_user_config_info(&config)?;

            if let Some(out) = out.for_human() {
                writeln!(out, "{}:", "\nUser Configuration".bold())?;
                writeln!(out)?;
                write_user_config_human(out, &user_info)?;
                writeln!(out, "{}:", "To set values".dimmed())?;
                writeln!(
                    out,
                    "  {}",
                    "but config user set name \"Your Name\"".blue().dimmed()
                )?;
                writeln!(
                    out,
                    "  {}",
                    "but config user set --global email your@email.com"
                        .blue()
                        .dimmed()
                )?;
            } else if let Some(out) = out.for_json() {
                out.write_value(serde_json::json!(user_info))?;
            }
        }
        // Set user config
        Some(UserSubcommand::Set { key, value, global }) => {
            let git_key = key.to_git_key();

            let mut config = if global {
                let all = git2::Config::open_default()?;
                all.open_level(git2::ConfigLevel::Global)?
            } else {
                repo.config()?
            };

            config.set_str(git_key, &value)?;

            if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "{} Set {} {} {}",
                    "✓".green(),
                    git_key.green(),
                    "→".dimmed(),
                    value.cyan()
                )?;
                if global {
                    writeln!(out, "  (configured globally)")?;
                }
            } else if let Some(out) = out.for_json() {
                out.write_value(serde_json::json!({
                    "key": git_key,
                    "value": value,
                    "scope": if global { "global" } else { "local" }
                }))?;
            }
        }
        // Unset user config
        Some(UserSubcommand::Unset { key, global }) => {
            let git_key = key.to_git_key();

            let mut config = if global {
                let all = git2::Config::open_default()?;
                all.open_level(git2::ConfigLevel::Global)?
            } else {
                repo.config()?
            };

            config.remove(git_key)?;

            if let Some(out) = out.for_human() {
                writeln!(out, "{} Removed {}", "✓".green(), git_key.green(),)?;
                if global {
                    writeln!(out, "  (removed from global config)")?;
                }
            } else if let Some(out) = out.for_json() {
                out.write_value(serde_json::json!({
                    "key": git_key,
                    "action": "unset",
                    "scope": if global { "global" } else { "local" }
                }))?;
            }
        }
    }

    Ok(())
}

/// Handle forge config subcommand
async fn forge_config(_ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    #[derive(Serialize)]
    struct ForgeAccount {
        provider: String,
        username: String,
        account_type: String,
    }

    let known_accounts = but_api::github::list_known_github_accounts().await?;
    let accounts: Vec<ForgeAccount> = known_accounts
        .iter()
        .map(|account| {
            let (username, account_type) = match account {
                but_github::GithubAccountIdentifier::OAuthUsername { username } => {
                    (username.clone(), "OAuth".to_string())
                }
                but_github::GithubAccountIdentifier::PatUsername { username } => {
                    (username.clone(), "Personal Access Token".to_string())
                }
                but_github::GithubAccountIdentifier::Enterprise { username, host } => (
                    format!("{}@{}", username, host),
                    "GitHub Enterprise".to_string(),
                ),
            };
            ForgeAccount {
                provider: "GitHub".to_string(),
                username,
                account_type,
            }
        })
        .collect();

    if let Some(out) = out.for_human() {
        writeln!(out, "{}:", "Forge Configuration".bold())?;
        writeln!(out)?;

        if accounts.is_empty() {
            writeln!(out, "  {}", "✗ No forge accounts configured".red())?;
            writeln!(out)?;
            writeln!(
                out,
                "  Run {} to authenticate with a forge",
                "but forge auth".cyan()
            )?;
        } else {
            writeln!(out, "  {}:", "Configured Accounts".green())?;
            for account in &accounts {
                writeln!(
                    out,
                    "    • {} {} ({})",
                    account.provider.cyan(),
                    account.username.bold(),
                    account.account_type.dimmed()
                )?;
            }
        }
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({ "accounts": accounts }))?;
    }

    Ok(())
}

/// Handle target config subcommand
async fn target_config(
    ctx: &mut Context,
    out: &mut OutputChannel,
    branch: Option<String>,
) -> Result<()> {
    match branch {
        None => {
            let target =
                but_api::legacy::virtual_branches::get_base_branch_data(ctx.legacy_project.id)?;

            if let Some(target_branch) = target {
                if let Some(out) = out.for_human() {
                    writeln!(out, "{}:", "Target Branch".bold())?;
                    writeln!(out, "  {}", target_branch.branch_name.to_string().cyan())?;
                    writeln!(out)?;
                    writeln!(out, "  {}: {}", "Remote".dimmed(), target_branch.remote_url)?;
                    writeln!(out, "  {}: {}", "SHA".dimmed(), target_branch.base_sha)?;
                } else if let Some(out) = out.for_json() {
                    out.write_value(serde_json::json!({
                        "branch": target_branch.branch_name.to_string(),
                        "remote_url": target_branch.remote_url,
                        "sha": target_branch.base_sha.to_string(),
                    }))?;
                } // View current target
            }
        }
        Some(_new_branch) => {
            anyhow::bail!(
                "Setting target branch is not yet implemented. Use the GitButler GUI to change the target branch."
            );
        }
    }

    Ok(())
}

/// Get the scope (local/global) where a config key is set
fn get_config_scope(config: &git2::Config, key: &str) -> Option<String> {
    // Try to get value at local level first
    if let Ok(local_config) = config.open_level(git2::ConfigLevel::Local)
        && local_config.get_string(key).is_ok()
    {
        return Some("local".to_string());
    }

    // Check global level
    if let Ok(global_config) = config.open_level(git2::ConfigLevel::Global)
        && global_config.get_string(key).is_ok()
    {
        return Some("global".to_string());
    }

    // Check system level
    if let Ok(system_config) = config.open_level(git2::ConfigLevel::System)
        && system_config.get_string(key).is_ok()
    {
        return Some("system".to_string());
    }
    None
}

/// Format the scope for display
fn format_scope(scope: &Option<String>) -> String {
    match scope {
        Some(s) => format!("({})", s).dimmed().to_string(),
        None => String::new(),
    }
}

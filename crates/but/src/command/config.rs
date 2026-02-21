//! Command implementation for managing `but` configuration.
//!
//! Provides subcommands to view and modify configuration settings including
//! user information, AI provider, forge accounts, and target branch.

use std::fmt::{Display, Write};

use anyhow::{Context as _, Result};
use but_ctx::Context;
use but_settings::{AppSettingsWithDiskSync, api::TelemetryUpdate};
use cfg_if::cfg_if;
use colored::Colorize;
use serde::Serialize;

use crate::{
    args::config::{ForgeSubcommand, MetricsStatus, Subcommands, UiSubcommand, UserSubcommand},
    tui,
    utils::{ConfirmOrEmpty, InputOutputChannel, OutputChannel},
};

/// Main entry point for config command
pub async fn exec(ctx: &mut Context, out: &mut OutputChannel, cmd: Option<Subcommands>) -> Result<()> {
    match cmd {
        Some(Subcommands::User { cmd }) => user_config(ctx, out, cmd).await,
        Some(Subcommands::Target { branch }) => target_config(ctx, out, branch).await,
        Some(Subcommands::Forge { cmd }) => forge_config(out, cmd).await,
        Some(Subcommands::Metrics { status }) => metrics_config(out, status).await,
        Some(Subcommands::Ui { cmd }) => ui_config(ctx, out, cmd),
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
        forge_accounts: Vec<ForgeAccountInfo>,
    }

    #[derive(Serialize)]
    struct ForgeAccountInfo {
        provider: String,
        username: String,
    }

    // Get user info from git config
    let user_info = {
        let git2_repo = &*ctx.git2_repo.get()?;
        let git2_config = git2_repo.config()?;
        get_user_config_info(&git2_config)?
    };

    // Get target branch
    cfg_if! {
        if #[cfg(feature = "legacy")] {
            let target_branch = but_api::legacy::virtual_branches::get_base_branch_data(ctx)?
                                    .map(|b| b.branch_name);
        } else {
            let target_branch = None::<String>;
        }
    };

    // Get forge accounts
    let known_accounts = but_api::github::list_known_github_accounts().await?;
    let forge_accounts: Vec<ForgeAccountInfo> = known_accounts
        .iter()
        .map(|account| ForgeAccountInfo {
            provider: "GitHub".to_string(),
            username: account.username().to_string(),
        })
        .collect();

    if let Some(out) = out.for_human() {
        writeln!(out, "\n{}", "GitButler Configuration".bold())?;
        writeln!(out)?;

        // User section
        writeln!(out, "{}:", "User".bold())?;
        write_user_config_human(out, &user_info)?;

        // Target branch
        writeln!(out, "{}:", "Target Branch".bold())?;
        if let Some(branch) = &target_branch {
            writeln!(out, "    {}", branch.cyan())?;
        } else {
            writeln!(out, "    {}", "(not set)".dimmed())?;
        }
        writeln!(out)?;

        // Forge
        writeln!(out, "{}:", "Forge".bold())?;
        if forge_accounts.is_empty() {
            writeln!(
                out,
                "  {}    Run {} to authenticate to a forge",
                "✗ Not configured\n".red(),
                "but config forge auth".blue()
            )?;
        } else {
            for account in &forge_accounts {
                writeln!(out, "  • {} {}", account.provider.cyan(), account.username)?;
            }
        }
        writeln!(out)?;

        // UI section
        {
            let git2_repo = &*ctx.git2_repo.get()?;
            let git2_config = git2_repo.config()?;
            let tui_enabled = get_tui_enabled(&git2_config);
            writeln!(out, "{}:", "UI".bold())?;
            writeln!(
                out,
                "  {}: {}",
                "TUI mode".dimmed(),
                if tui_enabled {
                    "enabled".green()
                } else {
                    "disabled".dimmed()
                }
            )?;
            writeln!(out)?;
        }

        // Hints
        writeln!(out, "{}", "Available subcommands:".dimmed())?;
        writeln!(
            out,
            "  {}    - User settings (name, email, editor)",
            "but config user".blue().dimmed()
        )?;
        writeln!(
            out,
            "  {}   - Forge settings (GitHub, etc)",
            "but config forge".blue().dimmed()
        )?;
        writeln!(
            out,
            "  {}  - Target branch settings",
            "but config target".blue().dimmed()
        )?;
        writeln!(out, "  {} - Metrics settings", "but config metrics".blue().dimmed())?;
        writeln!(
            out,
            "  {}      - UI preferences (TUI mode)",
            "but config ui".blue().dimmed()
        )?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!(ConfigOverview {
            name: user_info.name,
            email: user_info.email,
            editor: user_info.editor,
            target_branch,
            forge_accounts,
        }))?;
    }

    Ok(())
}

/// Handle metrics config subcommand (doesn't require repo context)
pub(crate) async fn metrics_config(out: &mut OutputChannel, status: Option<MetricsStatus>) -> Result<()> {
    let app_settings_sync = load_app_settings_sync()?;

    match status {
        None => {
            let enabled = app_settings_sync.get()?.telemetry.app_metrics_enabled;
            if let Some(out) = out.for_human() {
                writeln!(out, "\n{}:", "Metrics Configuration".bold())?;
                writeln!(out)?;
                writeln!(
                    out,
                    "  {}",
                    "GitButler uses metrics to help us know what is useful and improve it.".dimmed()
                )?;
                writeln!(
                    out,
                    "  {} {}",
                    "Privacy policy:".dimmed(),
                    "https://gitbutler.com/privacy".dimmed()
                )?;
                writeln!(out)?;
                writeln!(
                    out,
                    "  {}: {}",
                    "Metrics".dimmed(),
                    if enabled { "enabled".green() } else { "disabled".red() }
                )?;
                writeln!(out)?;
                writeln!(out, "{}:", "To change metrics".dimmed())?;
                writeln!(out, "  {}", "but config metrics enable".blue().dimmed())?;
                writeln!(out, "  {}", "but config metrics disable".blue().dimmed())?;
            } else if let Some(out) = out.for_shell() {
                writeln!(out, "{enabled}")?;
            } else if let Some(out) = out.for_json() {
                out.write_value(serde_json::json!({ "app_metrics_enabled": enabled }))?;
            }
        }
        Some(status) => {
            let enabled = status.enabled();
            let update = TelemetryUpdate {
                app_metrics_enabled: Some(enabled),
                app_error_reporting_enabled: None,
                app_non_anon_metrics_enabled: None,
            };

            but_api::legacy::settings::update_telemetry(
                &app_settings_sync,
                but_api::legacy::settings::UpdateTelemetryParams { update },
            )?;

            if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "{} Metrics are now {}",
                    "✓".green(),
                    if enabled { "enabled".green() } else { "disabled".red() }
                )?;
            } else if let Some(out) = out.for_shell() {
                writeln!(out, "{enabled}")?;
            } else if let Some(out) = out.for_json() {
                out.write_value(serde_json::json!({ "app_metrics_enabled": enabled }))?;
            }
        }
    }

    Ok(())
}

pub(crate) fn load_app_settings_sync() -> Result<AppSettingsWithDiskSync> {
    let config_dir = but_path::app_config_dir()?;
    std::fs::create_dir_all(&config_dir)?;
    AppSettingsWithDiskSync::new_with_customization(config_dir, None)
}

/// User configuration information
#[derive(serde::Serialize)]
struct UserConfigInfo {
    name: Option<String>,
    email: Option<String>,
    editor: Option<String>,
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
    let editor = tui::get_text::get_editor_command();

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
        info.editor.as_deref().unwrap_or("(built-in)").cyan(),
        format_scope(&info.editor_scope)
    )?;
    writeln!(out)?;
    Ok(())
}

/// Handle user config subcommand
async fn user_config(ctx: &mut Context, out: &mut OutputChannel, cmd: Option<UserSubcommand>) -> Result<()> {
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
                writeln!(out, "  {}", "but config user set name \"Your Name\"".blue().dimmed())?;
                writeln!(
                    out,
                    "  {}",
                    "but config user set --global email your@email.com".blue().dimmed()
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

/// Handle forge config subcommand (doesn't require repo context)
pub(crate) async fn forge_config(out: &mut OutputChannel, cmd: Option<ForgeSubcommand>) -> Result<()> {
    match cmd {
        Some(ForgeSubcommand::Auth) => forge_auth(out).await,
        Some(ForgeSubcommand::ListUsers) => forge_show_overview(out).await,
        Some(ForgeSubcommand::Forget { username }) => forge_forget(username, out).await,
        None => forge_show_overview(out).await,
    }
}

/// Show overview of forge configuration (same as list-users)
async fn forge_show_overview(out: &mut OutputChannel) -> Result<()> {
    let known_gh_accounts = but_api::github::list_known_github_accounts().await?;
    let known_gl_accounts = but_api::gitlab::list_known_gitlab_accounts().await?;

    if let Some(out) = out.for_human() {
        if known_gh_accounts.is_empty() && known_gl_accounts.is_empty() {
            writeln!(out, "\n{}", "No forge accounts configured".dimmed())?;
            writeln!(out)?;
            writeln!(
                out,
                "Run {} to authenticate with GitHub or GitLab.",
                "but config forge auth".cyan()
            )?;
        } else {
            let mut some_accounts_invalid = display_authenticated_github_accounts(&known_gh_accounts, out).await?;
            some_accounts_invalid |= display_authenticated_gitlab_accounts(&known_gl_accounts, out).await?;

            if some_accounts_invalid {
                writeln!(out, "{}", "Some accounts have invalid or missing credentials.".yellow())?;
                writeln!(out, "Re-authenticate using: {}", "but config forge auth".cyan())?;
                writeln!(out)?;
            }

            writeln!(out, "{}:", "Available commands".dimmed())?;
            writeln!(
                out,
                "  {} - Authenticate with a forge",
                "but config forge auth".blue().dimmed()
            )?;
            writeln!(
                out,
                "  {} - Forget an authenticated account",
                "but config forge forget [username]".blue().dimmed()
            )?;
        }
    } else if let Some(out) = out.for_shell() {
        if known_gh_accounts.is_empty() && known_gl_accounts.is_empty() {
            writeln!(out, "No forge accounts configured")?;
            return Ok(());
        }

        if !known_gh_accounts.is_empty() {
            writeln!(out, "GitHub accounts:")?;
            for account in &known_gh_accounts {
                writeln!(out, "  {}", account.username())?;
            }
        }
        if !known_gl_accounts.is_empty() {
            writeln!(out, "GitLab accounts:")?;
            for account in known_gl_accounts {
                writeln!(out, "  {}", account.username())?;
            }
        }
    } else if let Some(out) = out.for_json() {
        let accounts = extract_account_details(known_gh_accounts, known_gl_accounts);

        out.write_value(serde_json::json!({ "accounts": accounts }))?;
    }

    Ok(())
}

#[derive(Serialize)]
struct ForgeAccount {
    provider: String,
    username: String,
    account_type: String,
}

/// Extract account details for JSON output, combining GitHub and GitLab accounts into a unified format
fn extract_account_details(
    known_gh_accounts: Vec<but_github::GithubAccountIdentifier>,
    known_gl_accounts: Vec<but_gitlab::GitlabAccountIdentifier>,
) -> Vec<ForgeAccount> {
    let mut accounts: Vec<ForgeAccount> = Vec::new();

    // Add GitHub accounts
    for account in &known_gh_accounts {
        let (username, account_type) = match account {
            but_github::GithubAccountIdentifier::OAuthUsername { username } => (username.clone(), "OAuth".to_string()),
            but_github::GithubAccountIdentifier::PatUsername { username } => {
                (username.clone(), "Personal Access Token".to_string())
            }
            but_github::GithubAccountIdentifier::Enterprise { username, host } => {
                (format!("{username}@{host}"), "GitHub Enterprise".to_string())
            }
        };
        accounts.push(ForgeAccount {
            provider: "GitHub".to_string(),
            username,
            account_type,
        });
    }

    // Add GitLab accounts
    for account in &known_gl_accounts {
        let (username, account_type) = match account {
            but_gitlab::GitlabAccountIdentifier::PatUsername { username } => {
                (username.clone(), "Personal Access Token".to_string())
            }
            but_gitlab::GitlabAccountIdentifier::SelfHosted { username, host } => {
                (format!("{username}@{host}"), "GitLab Self-Hosted".to_string())
            }
        };
        accounts.push(ForgeAccount {
            provider: "GitLab".to_string(),
            username,
            account_type,
        });
    }
    accounts
}

/// Authenticate with a forge provider (GitHub, GitLab, etc)
async fn forge_auth(out: &mut OutputChannel) -> Result<()> {
    use cli_prompts::DisplayPrompt;

    #[derive(Debug, Clone)]
    enum ForgeProvider {
        GitHub,
        GitLab,
    }

    impl From<ForgeProvider> for String {
        fn from(provider: ForgeProvider) -> String {
            match provider {
                ForgeProvider::GitHub => "GitHub".to_string(),
                ForgeProvider::GitLab => "GitLab".to_string(),
            }
        }
    }

    let auth_options = vec![ForgeProvider::GitHub, ForgeProvider::GitLab];

    let auth_prompt =
        cli_prompts::prompts::Selection::new("Select a forge provider to authenticate with", auth_options.into_iter());

    let selected_option = auth_prompt
        .display()
        .map_err(|_| anyhow::anyhow!("Could not determine which forge provider to authenticate with"))?;

    match selected_option {
        ForgeProvider::GitHub => github_auth(out).await,
        ForgeProvider::GitLab => gitlab_auth(out).await,
    }
}

/// Authenticate with GitLab
async fn gitlab_auth(out: &mut OutputChannel) -> Result<()> {
    use cli_prompts::DisplayPrompt;
    #[derive(Debug, Clone)]
    enum AuthMethod {
        Pat,
        SelfHosted,
    }

    impl From<AuthMethod> for String {
        fn from(method: AuthMethod) -> String {
            match method {
                AuthMethod::Pat => "Personal Access Token (PAT)".to_string(),
                AuthMethod::SelfHosted => "Self-Hosted GitLab".to_string(),
            }
        }
    }

    let input = out
        .prepare_for_terminal_input()
        .context("Human input required - run this in a terminal")?;
    let auth_method_prompt = cli_prompts::prompts::Selection::new(
        "Select an authentication method",
        vec![AuthMethod::Pat, AuthMethod::SelfHosted].into_iter(),
    );

    let selected_method = auth_method_prompt
        .display()
        .map_err(|_| anyhow::anyhow!("Could not determine authentication method"))?;

    match selected_method {
        AuthMethod::Pat => gitlab_pat(input).await,
        AuthMethod::SelfHosted => gitlab_self_hosted(input).await,
    }
}

/// Authenticate with GitLab using a Personal Access Token (PAT)
async fn gitlab_pat(mut inout: InputOutputChannel<'_>) -> Result<()> {
    use but_gitlab::AuthStatusResponse;
    use but_secret::Sensitive;

    let input = inout
        .prompt("Please enter your GitLab Personal Access Token (PAT) and hit enter:")?
        .context("No PAT provided. Aborting authentication.")?;

    let pat = Sensitive(input);
    let AuthStatusResponse { username, .. } = but_api::gitlab::store_gitlab_pat(pat)
        .await
        .map_err(|err| err.context("Authentication failed"))?;

    writeln!(inout, "Authentication successful! Welcome, {username}.")?;
    Ok(())
}

/// Authenticate with self-hosted GitLab
async fn gitlab_self_hosted(mut inout: InputOutputChannel<'_>) -> Result<()> {
    use but_gitlab::AuthStatusResponse;
    use but_secret::Sensitive;

    let base_url = inout
        .prompt("Please enter your GitLab instance URL (e.g., https://gitlab.mycompany.com) and hit enter:")?
        .context("No host provided. Aborting authentication.")?;

    let input = inout
        .prompt("Now, please enter your GitLab Personal Access Token (PAT) and hit enter:")?
        .context("No PAT provided. Aborting authentication.")?;
    let pat = Sensitive(input);
    let AuthStatusResponse { username, .. } = but_api::gitlab::store_gitlab_selfhosted_pat(pat, base_url)
        .await
        .map_err(|err| err.context("Authentication failed"))?;

    writeln!(inout, "Authentication successful! Welcome, {username}.")?;
    Ok(())
}

/// Authenticate with GitHub
async fn github_auth(out: &mut OutputChannel) -> Result<()> {
    use cli_prompts::DisplayPrompt;

    #[derive(Debug, Clone)]
    enum AuthMethod {
        DeviceFlow,
        Pat,
        Enterprise,
    }

    impl From<AuthMethod> for String {
        fn from(method: AuthMethod) -> String {
            match method {
                AuthMethod::DeviceFlow => "Device flow (OAuth)".to_string(),
                AuthMethod::Pat => "Personal Access Token (PAT)".to_string(),
                AuthMethod::Enterprise => "GitHub Enterprise".to_string(),
            }
        }
    }

    let input = out
        .prepare_for_terminal_input()
        .context("Human input required - run this in a terminal")?;
    let auth_method_prompt = cli_prompts::prompts::Selection::new(
        "Select an authentication method",
        vec![AuthMethod::DeviceFlow, AuthMethod::Pat, AuthMethod::Enterprise].into_iter(),
    );

    let selected_method = auth_method_prompt
        .display()
        .map_err(|_| anyhow::anyhow!("Could not determine authentication method"))?;

    match selected_method {
        AuthMethod::Pat => github_pat(input).await,
        AuthMethod::Enterprise => github_enterprise(input).await,
        AuthMethod::DeviceFlow => github_oauth(input).await,
    }
}

/// Authenticate with GitHub using a Personal Access Token (PAT)
async fn github_pat(mut inout: InputOutputChannel<'_>) -> Result<()> {
    use but_github::AuthStatusResponse;
    use but_secret::Sensitive;

    let input = inout
        .prompt("Please enter your GitHub Personal Access Token (PAT) and hit enter:")?
        .context("No PAT provided. Aborting authentication.")?;

    let pat = Sensitive(input);
    let AuthStatusResponse { login, .. } = but_api::github::store_github_pat(pat)
        .await
        .map_err(|err| err.context("Authentication failed"))?;

    writeln!(inout, "Authentication successful! Welcome, {login}.")?;
    Ok(())
}

/// Authenticate with GitHub Enterprise
async fn github_enterprise(mut inout: InputOutputChannel<'_>) -> Result<()> {
    use but_github::AuthStatusResponse;
    use but_secret::Sensitive;

    let base_url = inout.prompt("Please enter your GitHub Enterprise API base URL (e.g., https://github.mycompany.com/api/v3) and hit enter:")?.context("No host provided. Aborting authentication.")?;

    let input = inout
        .prompt("Now, please enter your GitHub Enterprise Personal Access Token (PAT) and hit enter:")?
        .context("No PAT provided. Aborting authentication.")?;
    let pat = Sensitive(input);
    let AuthStatusResponse { login, .. } = but_api::github::store_github_enterprise_pat(pat, base_url)
        .await
        .map_err(|err| err.context("Authentication failed"))?;

    writeln!(inout, "Authentication successful! Welcome, {login}.")?;
    Ok(())
}

/// Authenticate with GitHub using the device OAuth flow
async fn github_oauth(mut inout: InputOutputChannel<'_>) -> Result<()> {
    let code = but_api::github::init_github_device_oauth().await?;
    writeln!(
        inout,
        "Device authorization initiated. Please visit the following URL and enter the code:\n\nhttps://github.com/login/device\n\nCode: {}\n\n",
        code.user_code
    )?;

    if inout.confirm_no_default("After completing authorization in your browser, press 'y' to continue.")?
        != ConfirmOrEmpty::Yes
    {
        anyhow::bail!("Authorization process aborted by user.")
    }

    let status = but_api::github::check_github_auth_status(code.device_code)
        .await
        .map_err(|err| err.context("Authentication failed"))?;

    writeln!(inout, "Authentication successful! Welcome, {}.", status.login)?;

    Ok(())
}

async fn display_authenticated_github_accounts(
    known_gh_accounts: &Vec<but_github::GithubAccountIdentifier>,
    out: &mut (dyn Write + 'static),
) -> Result<bool, anyhow::Error> {
    writeln!(out, "\n{}:", "Authenticated GitHub accounts".bold())?;
    writeln!(out)?;

    let mut some_accounts_invalid = false;

    for account in known_gh_accounts {
        let account_status = but_api::github::check_github_credentials(account.clone()).await.ok();

        let message = match account_status {
            Some(but_github::CredentialCheckResult::Valid) => "(valid credentials)".green().bold(),
            Some(but_github::CredentialCheckResult::Invalid) => {
                some_accounts_invalid = true;
                "(invalid credentials)".bold().yellow()
            }
            Some(but_github::CredentialCheckResult::NoCredentials) => {
                some_accounts_invalid = true;
                "(no credentials)".bold().yellow()
            }
            None => "(unknown status)".bold().red(),
        };

        writeln!(out, "  • {account} {message}")?;
    }
    writeln!(out)?;
    Ok(some_accounts_invalid)
}

async fn display_authenticated_gitlab_accounts(
    known_gl_accounts: &Vec<but_gitlab::GitlabAccountIdentifier>,
    out: &mut (dyn Write + 'static),
) -> Result<bool, anyhow::Error> {
    if known_gl_accounts.is_empty() {
        return Ok(false);
    }

    writeln!(out, "\n{}:", "Authenticated GitLab accounts".bold())?;
    writeln!(out)?;

    let mut some_accounts_invalid = false;

    for account in known_gl_accounts {
        let account_status = but_api::gitlab::check_gitlab_credentials(account.clone()).await.ok();

        let message = match account_status {
            Some(but_gitlab::CredentialCheckResult::Valid) => "(valid credentials)".green().bold(),
            Some(but_gitlab::CredentialCheckResult::Invalid) => {
                some_accounts_invalid = true;
                "(invalid credentials)".bold().yellow()
            }
            Some(but_gitlab::CredentialCheckResult::NoCredentials) => {
                some_accounts_invalid = true;
                "(no credentials)".bold().yellow()
            }
            None => "(unknown status)".bold().red(),
        };

        writeln!(out, "  • {account} {message}")?;
    }
    writeln!(out)?;
    Ok(some_accounts_invalid)
}

#[derive(Debug, Clone)]
enum AccountToForget {
    GitHub(but_github::GithubAccountIdentifier),
    GitLab(but_gitlab::GitlabAccountIdentifier),
}

impl Display for AccountToForget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountToForget::GitHub(account) => write!(f, "GitHub account '{account}'"),
            AccountToForget::GitLab(account) => write!(f, "GitLab account '{account}'"),
        }
    }
}

fn forget_account(account: &AccountToForget) -> Result<()> {
    match account {
        AccountToForget::GitHub(gh_account) => but_api::github::forget_github_account(gh_account.clone()),
        AccountToForget::GitLab(gl_account) => but_api::gitlab::forget_gitlab_account(gl_account.clone()),
    }
}

/// Forget a GitHub account
async fn forge_forget(username: Option<String>, out: &mut OutputChannel) -> Result<()> {
    use cli_prompts::DisplayPrompt;

    let known_gh_accounts = but_api::github::list_known_github_accounts().await?;
    let known_gl_accounts = but_api::gitlab::list_known_gitlab_accounts().await?;

    // Gather all potential accounts to delete based on the provided username (or all if no username provided)
    let mut accounts_to_delete: Vec<AccountToForget> = Vec::new();

    for account in known_gh_accounts {
        if username.as_ref().is_none_or(|u| account.username() == u) {
            accounts_to_delete.push(AccountToForget::GitHub(account.clone()));
        }
    }

    for account in known_gl_accounts {
        if username.as_ref().is_none_or(|u| account.username() == u) {
            accounts_to_delete.push(AccountToForget::GitLab(account.clone()));
        }
    }

    // Handle case where no matching account was found
    if accounts_to_delete.is_empty() {
        if let Some((username, out)) = username.zip(out.for_human()) {
            writeln!(out, "No known GitHub account with username '{username}'")?;
        }
        return Ok(());
    }

    // Handle different scenarios based on number of accounts
    match accounts_to_delete.as_slice() {
        [single_account] => {
            // Single account: delete automatically
            forget_account(single_account)?;
            if let Some(out) = out.for_human() {
                writeln!(out, "Forgot forge account '{single_account}'")?;
            }
        }
        _ => {
            // Multiple accounts: prompt user to select
            if let Some(out) = out.for_human() {
                let account_prompt = cli_prompts::prompts::Multiselect::new_transformed(
                    "Which of the following accounts do you want to forget?",
                    accounts_to_delete.into_iter(),
                    |acc| acc.to_string(),
                );

                let selected_accounts = account_prompt
                    .display()
                    .map_err(|_| anyhow::anyhow!("Could not determine which accounts to delete"))?;

                if selected_accounts.is_empty() {
                    writeln!(out, "No accounts were selected to forget.")?;
                    return Ok(());
                }

                for account in selected_accounts {
                    forget_account(&account)?;
                    writeln!(out, "Forgot forge account '{account}'")?;
                }
            } else {
                anyhow::bail!("Username ambiguous, got {accounts_to_delete:?}");
            }
        }
    }

    Ok(())
}

/// Handle target config subcommand
async fn target_config(ctx: &mut Context, out: &mut OutputChannel, branch: Option<String>) -> Result<()> {
    match branch {
        None => {
            #[cfg(feature = "legacy")]
            {
                let target = but_api::legacy::virtual_branches::get_base_branch_data(ctx)?;

                if let Some(target_branch) = target {
                    if let Some(out) = out.for_human() {
                        writeln!(out, "{}", "Used to determine common base to calculate commits unique to each branch (not yet integrated)\n".dimmed())?;
                        writeln!(out, "{}:", "Target Branch".bold())?;
                        writeln!(out, "\n  {}", target_branch.branch_name.to_string().cyan())?;
                        writeln!(out)?;
                        writeln!(out, "  {}: {}", "Remote".dimmed(), target_branch.remote_url)?;
                        writeln!(out, "  {}:    {}", "SHA".dimmed(), target_branch.base_sha)?;
                        writeln!(out, "\n{}:", "To change target branch".dimmed())?;
                        writeln!(out, "  {}", "but config target <branch_name>".blue().dimmed())?;
                    } else if let Some(out) = out.for_json() {
                        out.write_value(serde_json::json!({
                            "branch": target_branch.branch_name.to_string(),
                            "remote_url": target_branch.remote_url,
                            "sha": target_branch.base_sha.to_string(),
                        }))?;
                    } // View current target
                }
            }
        }
        Some(new_branch) => {
            // refuse to run if there are any applied branches. if so, ask user to unapply first.
            let (guard, _, ws, _) = ctx.workspace_and_db()?;
            if !ws.stacks.is_empty() {
                // list the applied branches
                if let Some(out) = out.for_human() {
                    writeln!(out, "{}", "\nThe following branches are currently applied:\n".bold())?;
                    ws.stacks.iter().for_each(|stack| {
                        {
                            writeln!(
                                out,
                                "{} Applied branch: {}",
                                "•".dimmed(),
                                stack
                                    .ref_name()
                                    .map_or_else(|| "ANONYMOUS".to_string(), |rn| rn.shorten().to_string())
                                    .cyan()
                            )
                            .ok();
                        };
                    });
                    writeln!(
                        out,
                        "\n{}\n",
                        "Please unapply all branches before changing the target branch.".yellow()
                    )
                    .ok();
                }
                anyhow::bail!(
                    "Cannot change target branch while there are applied branches. Please unapply all branches first."
                );
            }

            if let Some(out) = out.for_human() {
                writeln!(out, "{} Changing target branch to '{}'", "✓".green(), new_branch.cyan())?;
            }

            // from the new_branch string, we need to parse out the remote name and branch name
            cfg_if! {
                if #[cfg(feature = "legacy")] {
                    drop((guard, ws));
                    but_api::legacy::virtual_branches::set_base_branch(
                        ctx,
                        new_branch.clone(),
                        None,
                    )?;
                } else {
                    anyhow::bail!("Cannot yet set the base-branch without legacy functions - needs port")
                }
            };
        }
    }

    Ok(())
}

/// Handle UI config subcommand
fn ui_config(ctx: &mut Context, out: &mut OutputChannel, cmd: Option<UiSubcommand>) -> Result<()> {
    let repo = &*ctx.git2_repo.get()?;

    match cmd {
        None => {
            let config = repo.config()?;
            let tui_enabled = get_tui_enabled(&config);
            let tui_scope = get_config_scope(&config, "but.ui.tui");

            if let Some(out) = out.for_human() {
                writeln!(out, "{}:", "\nUI Configuration".bold())?;
                writeln!(out)?;
                writeln!(
                    out,
                    "  {}: {} {}",
                    "Prefer TUI mode".dimmed(),
                    if tui_enabled {
                        "enabled".green()
                    } else {
                        "disabled".red()
                    },
                    format_scope(&tui_scope)
                )?;
                writeln!(out)?;
                writeln!(out, "{}:", "To change".dimmed())?;
                writeln!(out, "  {}", "but config ui set tui true".blue().dimmed())?;
                writeln!(out, "  {}", "but config ui set tui false".blue().dimmed())?;
            } else if let Some(out) = out.for_shell() {
                writeln!(out, "{tui_enabled}")?;
            } else if let Some(out) = out.for_json() {
                out.write_value(serde_json::json!({ "tui": tui_enabled }))?;
            }
        }
        Some(UiSubcommand::Set { key, value, global }) => {
            let git_key = key.to_git_key();
            let bool_value = parse_bool_value(&value)
                .ok_or_else(|| anyhow::anyhow!("Invalid value '{value}'. Use true/false or 1/0."))?;

            let mut config = if global {
                let all = git2::Config::open_default()?;
                all.open_level(git2::ConfigLevel::Global)?
            } else {
                repo.config()?
            };

            config.set_bool(git_key, bool_value)?;

            if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "{} Set {} {} {}",
                    "✓".green(),
                    git_key.green(),
                    "→".dimmed(),
                    if bool_value { "true".cyan() } else { "false".cyan() }
                )?;
                if global {
                    writeln!(out, "  (configured globally)")?;
                }
            } else if let Some(out) = out.for_json() {
                out.write_value(serde_json::json!({
                    "key": git_key,
                    "value": bool_value,
                    "scope": if global { "global" } else { "local" }
                }))?;
            }
        }
        Some(UiSubcommand::Unset { key, global }) => {
            let git_key = key.to_git_key();

            let mut config = if global {
                let all = git2::Config::open_default()?;
                all.open_level(git2::ConfigLevel::Global)?
            } else {
                repo.config()?
            };

            config.remove(git_key)?;

            if let Some(out) = out.for_human() {
                writeln!(out, "{} Removed {}", "✓".green(), git_key.green())?;
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

/// Check if TUI mode is enabled in git config. Defaults to false.
pub(crate) fn get_tui_enabled(config: &git2::Config) -> bool {
    config.get_bool("but.ui.tui").unwrap_or(false)
}

/// Parse a string as a boolean value (true/false, 1/0, yes/no, on/off).
fn parse_bool_value(value: &str) -> Option<bool> {
    match value.to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
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
        Some(s) => format!("({s})").dimmed().to_string(),
        None => String::new(),
    }
}

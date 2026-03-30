//! Command implementation for managing `but` configuration.
//!
//! Provides subcommands to view and modify configuration settings including
//! user information, AI provider, forge accounts, and target branch.

use std::fmt::{Display, Write};

use anyhow::{Context as _, Result};
use but_core::git_config::{remove_config_value, set_config_value};
use but_ctx::Context;
use but_llm::{
    AI_ANTHROPIC_KEY_OPTION_KEY, AI_ANTHROPIC_MODEL_NAME_KEY, AI_ANTHROPIC_SECRET_HANDLE,
    AI_LMSTUDIO_ENDPOINT_KEY, AI_LMSTUDIO_MODEL_NAME_KEY, AI_MODEL_PROVIDER_KEY,
    AI_OLLAMA_ENDPOINT_KEY, AI_OLLAMA_MODEL_NAME_KEY, AI_OPENAI_CUSTOM_ENDPOINT_KEY,
    AI_OPENAI_KEY_OPTION_KEY, AI_OPENAI_MODEL_NAME_KEY, AI_OPENAI_SECRET_HANDLE, LLMProviderKind,
};
use but_secret::{Sensitive, secret};
use but_settings::{AppSettingsWithDiskSync, api::TelemetryUpdate};
use cfg_if::cfg_if;
use colored::Colorize;
use gix::bstr::ByteSlice as _;
use serde::Serialize;

use super::git_config::edit_git_config;
use crate::{
    args::config::{
        AiKeyOption, AiSubcommand, ForgeSubcommand, MetricsStatus, Subcommands, UiSubcommand,
        UserSubcommand,
    },
    tui,
    utils::{ConfirmOrEmpty, InputOutputChannel, OutputChannel},
};

impl From<AiKeyOption> for String {
    fn from(value: AiKeyOption) -> Self {
        match value {
            AiKeyOption::BringYourOwn => "Bring your own key".to_string(),
            AiKeyOption::ButlerApi => "Use GitButler API".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum AiScope {
    Global,
    Local,
}

impl AiScope {
    fn from_flags(local: bool, global: bool) -> Result<Self> {
        if local && global {
            anyhow::bail!("Cannot pass both --local and --global")
        }
        if local {
            Ok(AiScope::Local)
        } else {
            Ok(AiScope::Global)
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            AiScope::Global => "global",
            AiScope::Local => "local",
        }
    }
}

#[derive(Debug, Serialize)]
struct AiConfigInfo {
    provider: Option<String>,
    openai_key_option: Option<String>,
    openai_model: Option<String>,
    openai_endpoint: Option<String>,
    anthropic_key_option: Option<String>,
    anthropic_model: Option<String>,
    ollama_endpoint: Option<String>,
    ollama_model: Option<String>,
    lmstudio_endpoint: Option<String>,
    lmstudio_model: Option<String>,
}

/// Main entry point for config command
pub async fn exec(
    ctx: &mut Context,
    out: &mut OutputChannel,
    cmd: Option<Subcommands>,
) -> Result<()> {
    match cmd {
        Some(Subcommands::User { cmd }) => user_config(ctx, out, cmd).await,
        Some(Subcommands::Target { branch }) => target_config(ctx, out, branch).await,
        Some(Subcommands::Forge { cmd }) => forge_config(out, cmd).await,
        Some(Subcommands::Metrics { status }) => metrics_config(out, status).await,
        Some(Subcommands::Ai { local, global, cmd }) => {
            ai_config_with_repo(ctx, out, cmd, local, global)
        }
        Some(Subcommands::Ui { cmd }) => ui_config(ctx, out, cmd),
        None => show_overview(ctx, out).await,
    }
}

/// Handle AI config subcommand without repository context.
///
/// This supports global configuration from outside of a git repository.
pub(crate) fn ai_config(
    out: &mut OutputChannel,
    cmd: Option<AiSubcommand>,
    local: bool,
    global: bool,
) -> Result<()> {
    if local {
        anyhow::bail!("Local AI configuration requires running inside a git repository")
    }
    ai_config_inner(None, out, cmd, local, global)
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
        let repo = ctx.repo.get()?;
        let config = repo.config_snapshot();
        get_user_config_info(&config)
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
    let known_accounts = but_api::github::list_known_github_accounts()?;
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
            let repo = ctx.repo.get()?;
            let config = repo.config_snapshot();
            let tui_enabled = get_tui_enabled(&config);
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
        writeln!(
            out,
            "  {} - Metrics settings",
            "but config metrics".blue().dimmed()
        )?;
        writeln!(
            out,
            "  {}      - AI provider settings",
            "but config ai".blue().dimmed()
        )?;
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
pub(crate) async fn metrics_config(
    out: &mut OutputChannel,
    status: Option<MetricsStatus>,
) -> Result<()> {
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
                    "GitButler uses metrics to help us know what is useful and improve it."
                        .dimmed()
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
                    if enabled {
                        "enabled".green()
                    } else {
                        "disabled".red()
                    }
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

            app_settings_sync.update_telemetry(update)?;

            if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "{} Metrics are now {}",
                    "✓".green(),
                    if enabled {
                        "enabled".green()
                    } else {
                        "disabled".red()
                    }
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
    #[serde(serialize_with = "serialize_config_source")]
    name_scope: Option<gix::config::Source>,
    #[serde(serialize_with = "serialize_config_source")]
    email_scope: Option<gix::config::Source>,
    #[serde(serialize_with = "serialize_config_source")]
    editor_scope: Option<gix::config::Source>,
}

/// Get user configuration info from git config
fn get_user_config_info(config: &gix::config::Snapshot<'_>) -> UserConfigInfo {
    let (name, name_scope) = get_config_string_and_scope(config, "user.name");
    let (email, email_scope) = get_config_string_and_scope(config, "user.email");
    let (_editor, editor_scope) = get_config_string_and_scope(config, "core.editor");
    let editor = tui::get_text::get_editor_command();

    UserConfigInfo {
        name,
        email,
        editor,
        name_scope,
        email_scope,
        editor_scope,
    }
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
        format_scope(info.name_scope)
    )?;
    writeln!(
        out,
        "   {}: {} {}",
        "Email".dimmed(),
        info.email
            .as_deref()
            .map(|e| e.cyan())
            .unwrap_or_else(|| "(not set)".red()),
        format_scope(info.email_scope)
    )?;
    writeln!(
        out,
        "  {}: {} {}",
        "Editor".dimmed(),
        info.editor.as_deref().unwrap_or("(built-in)").cyan(),
        format_scope(info.editor_scope)
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
    let repo = ctx.repo.get()?;

    match cmd {
        // View user config
        None => {
            let config = repo.config_snapshot();
            let user_info = get_user_config_info(&config);

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
            edit_git_config(&repo, global.into(), |config| {
                set_config_value(config, git_key, &value)?;
                Ok(true)
            })?;

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
            edit_git_config(&repo, global.into(), |config| {
                remove_config_value(config, git_key)?;
                Ok(true)
            })?;

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
pub(crate) async fn forge_config(
    out: &mut OutputChannel,
    cmd: Option<ForgeSubcommand>,
) -> Result<()> {
    match cmd {
        Some(ForgeSubcommand::Auth) => forge_auth(out).await,
        Some(ForgeSubcommand::ListUsers) => forge_show_overview(out).await,
        Some(ForgeSubcommand::Forget { username }) => forge_forget(username, out).await,
        None => forge_show_overview(out).await,
    }
}

/// Show overview of forge configuration (same as list-users)
async fn forge_show_overview(out: &mut OutputChannel) -> Result<()> {
    let known_gh_accounts = but_api::github::list_known_github_accounts()?;
    let known_gl_accounts = but_api::gitlab::list_known_gitlab_accounts()?;

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
            let mut some_accounts_invalid =
                display_authenticated_github_accounts(&known_gh_accounts, out).await?;
            some_accounts_invalid |=
                display_authenticated_gitlab_accounts(&known_gl_accounts, out).await?;

            if some_accounts_invalid {
                writeln!(
                    out,
                    "{}",
                    "Some accounts have invalid or missing credentials.".yellow()
                )?;
                writeln!(
                    out,
                    "Re-authenticate using: {}",
                    "but config forge auth".cyan()
                )?;
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
            but_github::GithubAccountIdentifier::OAuthUsername { username } => {
                (username.clone(), "OAuth".to_string())
            }
            but_github::GithubAccountIdentifier::PatUsername { username } => {
                (username.clone(), "Personal Access Token".to_string())
            }
            but_github::GithubAccountIdentifier::Enterprise { username, host } => (
                format!("{username}@{host}"),
                "GitHub Enterprise".to_string(),
            ),
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
            but_gitlab::GitlabAccountIdentifier::SelfHosted { username, host } => (
                format!("{username}@{host}"),
                "GitLab Self-Hosted".to_string(),
            ),
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

    let auth_prompt = cli_prompts::prompts::Selection::new(
        "Select a forge provider to authenticate with",
        auth_options.into_iter(),
    );

    let selected_option = auth_prompt.display().map_err(|_| {
        anyhow::anyhow!("Could not determine which forge provider to authenticate with")
    })?;

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

    let input = inout
        .prompt_secret("Please enter your GitLab Personal Access Token (PAT) and hit enter:")?
        .context("No PAT provided. Aborting authentication.")?;

    let AuthStatusResponse { username, .. } = but_api::gitlab::store_gitlab_pat(input)
        .await
        .map_err(|err| err.context("Authentication failed"))?;

    writeln!(inout, "Authentication successful! Welcome, {username}.")?;
    Ok(())
}

/// Authenticate with self-hosted GitLab
async fn gitlab_self_hosted(mut inout: InputOutputChannel<'_>) -> Result<()> {
    use but_gitlab::AuthStatusResponse;

    let base_url = inout
        .prompt("Please enter your GitLab instance URL (e.g., https://gitlab.mycompany.com) and hit enter:")?
        .context("No host provided. Aborting authentication.")?;

    let input = inout
        .prompt_secret("Now, please enter your GitLab Personal Access Token (PAT) and hit enter:")?
        .context("No PAT provided. Aborting authentication.")?;
    let AuthStatusResponse { username, .. } =
        but_api::gitlab::store_gitlab_selfhosted_pat(input, base_url)
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
        vec![
            AuthMethod::DeviceFlow,
            AuthMethod::Pat,
            AuthMethod::Enterprise,
        ]
        .into_iter(),
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

    let input = inout
        .prompt_secret("Please enter your GitHub Personal Access Token (PAT) and hit enter:")?
        .context("No PAT provided. Aborting authentication.")?;

    let AuthStatusResponse { login, .. } = but_api::github::store_github_pat(input)
        .await
        .map_err(|err| err.context("Authentication failed"))?;

    writeln!(inout, "Authentication successful! Welcome, {login}.")?;
    Ok(())
}

/// Authenticate with GitHub Enterprise
async fn github_enterprise(mut inout: InputOutputChannel<'_>) -> Result<()> {
    use but_github::AuthStatusResponse;

    let base_url = inout.prompt("Please enter your GitHub Enterprise API base URL (e.g., https://github.mycompany.com/api/v3) and hit enter:")?.context("No host provided. Aborting authentication.")?;

    let input = inout
        .prompt_secret(
            "Now, please enter your GitHub Enterprise Personal Access Token (PAT) and hit enter:",
        )?
        .context("No PAT provided. Aborting authentication.")?;
    let AuthStatusResponse { login, .. } =
        but_api::github::store_github_enterprise_pat(input, base_url)
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

    if inout.confirm_no_default(
        "After completing authorization in your browser, press 'y' to continue.",
    )? != ConfirmOrEmpty::Yes
    {
        anyhow::bail!("Authorization process aborted by user.")
    }

    let status = but_api::github::check_github_auth_status(code.device_code)
        .await
        .map_err(|err| err.context("Authentication failed"))?;

    writeln!(
        inout,
        "Authentication successful! Welcome, {}.",
        status.login
    )?;

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
        let account_status = but_api::github::check_github_credentials(account.clone())
            .await
            .ok();

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
        let account_status = but_api::gitlab::check_gitlab_credentials(account.clone())
            .await
            .ok();

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
        AccountToForget::GitHub(gh_account) => {
            but_api::github::forget_github_account(gh_account.clone())
        }
        AccountToForget::GitLab(gl_account) => {
            but_api::gitlab::forget_gitlab_account(gl_account.clone())
        }
    }
}

/// Forget a GitHub account
async fn forge_forget(username: Option<String>, out: &mut OutputChannel) -> Result<()> {
    use cli_prompts::DisplayPrompt;

    let known_gh_accounts = but_api::github::list_known_github_accounts()?;
    let known_gl_accounts = but_api::gitlab::list_known_gitlab_accounts()?;

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

fn ai_config_with_repo(
    ctx: &mut Context,
    out: &mut OutputChannel,
    cmd: Option<AiSubcommand>,
    local: bool,
    global: bool,
) -> Result<()> {
    let repo = ctx.repo.get()?;
    ai_config_inner(Some(&*repo), out, cmd, local, global)
}

fn ai_config_inner(
    repo: Option<&gix::Repository>,
    out: &mut OutputChannel,
    cmd: Option<AiSubcommand>,
    local: bool,
    global: bool,
) -> Result<()> {
    let scope = AiScope::from_flags(local, global)?;

    match cmd {
        None => {
            if out.for_human().is_some() {
                return ai_config_interactive(repo, out, scope);
            }
            show_ai_config(repo, out, scope)
        }
        Some(AiSubcommand::Show) => show_ai_config(repo, out, scope),
        Some(cmd) => ai_config_non_interactive(repo, out, scope, cmd),
    }
}

fn show_ai_config(
    repo: Option<&gix::Repository>,
    out: &mut OutputChannel,
    scope: AiScope,
) -> Result<()> {
    let info = get_ai_config_info(repo, scope)?;

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{} ({})",
            "AI Configuration".bold(),
            scope.as_str().dimmed()
        )?;
        writeln!(out)?;
        writeln!(
            out,
            "  {}: {}",
            "Provider".dimmed(),
            info.provider
                .as_deref()
                .map(|provider| provider.cyan().to_string())
                .unwrap_or_else(|| "(not set)".red().to_string())
        )?;
        writeln!(
            out,
            "  {}: {}",
            "OpenAI key option".dimmed(),
            info.openai_key_option
                .as_deref()
                .unwrap_or("(not set)")
                .cyan()
        )?;
        writeln!(
            out,
            "  {}: {}",
            "OpenAI model".dimmed(),
            info.openai_model.as_deref().unwrap_or("(not set)").cyan()
        )?;
        writeln!(
            out,
            "  {}: {}",
            "OpenAI endpoint".dimmed(),
            info.openai_endpoint
                .as_deref()
                .unwrap_or("(not set)")
                .cyan()
        )?;
        writeln!(
            out,
            "  {}: {}",
            "Anthropic key option".dimmed(),
            info.anthropic_key_option
                .as_deref()
                .unwrap_or("(not set)")
                .cyan()
        )?;
        writeln!(
            out,
            "  {}: {}",
            "Anthropic model".dimmed(),
            info.anthropic_model
                .as_deref()
                .unwrap_or("(not set)")
                .cyan()
        )?;
        writeln!(
            out,
            "  {}: {}",
            "Ollama endpoint".dimmed(),
            info.ollama_endpoint
                .as_deref()
                .unwrap_or("(not set)")
                .cyan()
        )?;
        writeln!(
            out,
            "  {}: {}",
            "Ollama model".dimmed(),
            info.ollama_model.as_deref().unwrap_or("(not set)").cyan()
        )?;
        writeln!(
            out,
            "  {}: {}",
            "LM Studio endpoint".dimmed(),
            info.lmstudio_endpoint
                .as_deref()
                .unwrap_or("(not set)")
                .cyan()
        )?;
        writeln!(
            out,
            "  {}: {}",
            "LM Studio model".dimmed(),
            info.lmstudio_model.as_deref().unwrap_or("(not set)").cyan()
        )?;
    } else if let Some(out) = out.for_shell() {
        writeln!(out, "{}", info.provider.as_deref().unwrap_or(""))?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!(info))?;
    }

    Ok(())
}

fn ai_config_non_interactive(
    repo: Option<&gix::Repository>,
    out: &mut OutputChannel,
    scope: AiScope,
    cmd: AiSubcommand,
) -> Result<()> {
    match cmd {
        AiSubcommand::Show => {
            return show_ai_config(repo, out, scope);
        }
        AiSubcommand::Openai {
            key_option,
            model,
            endpoint,
            api_key,
            api_key_env,
        } => {
            let selected_key_option = key_option.unwrap_or(AiKeyOption::ButlerApi);
            let secret = resolve_secret_input(api_key, api_key_env)?;
            require_non_interactive_secret_if_byok(selected_key_option, secret.as_ref(), "OpenAI")?;
            apply_openai_config(repo, scope, selected_key_option, model, endpoint, secret)?;
            write_ai_config_success(out, scope, LLMProviderKind::OpenAi)?;
        }
        AiSubcommand::Anthropic {
            key_option,
            model,
            api_key,
            api_key_env,
        } => {
            let selected_key_option = key_option.unwrap_or(AiKeyOption::ButlerApi);
            let secret = resolve_secret_input(api_key, api_key_env)?;
            require_non_interactive_secret_if_byok(
                selected_key_option,
                secret.as_ref(),
                "Anthropic",
            )?;
            apply_anthropic_config(repo, scope, selected_key_option, model, secret)?;
            write_ai_config_success(out, scope, LLMProviderKind::Anthropic)?;
        }
        AiSubcommand::Ollama { endpoint, model } => {
            apply_ollama_config(repo, scope, endpoint, model)?;
            write_ai_config_success(out, scope, LLMProviderKind::Ollama)?;
        }
        AiSubcommand::Lmstudio { endpoint, model } => {
            apply_lmstudio_config(repo, scope, endpoint, model)?;
            write_ai_config_success(out, scope, LLMProviderKind::LMStudio)?;
        }
    }

    Ok(())
}

fn ai_config_interactive(
    repo: Option<&gix::Repository>,
    out: &mut OutputChannel,
    scope: AiScope,
) -> Result<()> {
    use cli_prompts::DisplayPrompt;

    let mut inout = out
        .prepare_for_terminal_input()
        .context("Human input required - run this in a terminal")?;

    let provider_prompt = cli_prompts::prompts::Selection::new(
        "Select an AI provider",
        vec!["OpenAI", "Anthropic", "Ollama", "LM Studio"].into_iter(),
    );

    let provider_label = provider_prompt
        .display()
        .map_err(|_| anyhow::anyhow!("Could not determine selected AI provider"))?;
    let provider = match provider_label {
        "OpenAI" => LLMProviderKind::OpenAi,
        "Anthropic" => LLMProviderKind::Anthropic,
        "Ollama" => LLMProviderKind::Ollama,
        "LM Studio" => LLMProviderKind::LMStudio,
        _ => anyhow::bail!("Unsupported AI provider selection: {provider_label}"),
    };

    match provider {
        LLMProviderKind::OpenAi => {
            let key_option_prompt = cli_prompts::prompts::Selection::new(
                "Select OpenAI credential source",
                vec![AiKeyOption::ButlerApi, AiKeyOption::BringYourOwn].into_iter(),
            );
            let key_option = key_option_prompt
                .display()
                .map_err(|_| anyhow::anyhow!("Could not determine OpenAI credential source"))?;

            let model = inout.prompt("Preferred OpenAI model (leave empty for default):")?;
            let endpoint = inout.prompt("Custom endpoint URL (optional):")?;

            let secret = if matches!(key_option, AiKeyOption::BringYourOwn) {
                Some(
                    inout
                        .prompt_secret("Enter OpenAI API key:")?
                        .context("No API key provided. Aborting configuration.")?,
                )
            } else {
                None
            };

            apply_openai_config(repo, scope, key_option, model, endpoint, secret)?;
        }
        LLMProviderKind::Anthropic => {
            let key_option_prompt = cli_prompts::prompts::Selection::new(
                "Select Anthropic credential source",
                vec![AiKeyOption::ButlerApi, AiKeyOption::BringYourOwn].into_iter(),
            );
            let key_option = key_option_prompt
                .display()
                .map_err(|_| anyhow::anyhow!("Could not determine Anthropic credential source"))?;

            let model = inout.prompt("Preferred Anthropic model (leave empty for default):")?;
            let secret = if matches!(key_option, AiKeyOption::BringYourOwn) {
                Some(
                    inout
                        .prompt_secret("Enter Anthropic API key:")?
                        .context("No API key provided. Aborting configuration.")?,
                )
            } else {
                None
            };

            apply_anthropic_config(repo, scope, key_option, model, secret)?;
        }
        LLMProviderKind::Ollama => {
            let endpoint = inout.prompt("Ollama endpoint host:port (optional):")?;
            let model = inout.prompt("Preferred Ollama model (optional):")?;
            apply_ollama_config(repo, scope, endpoint, model)?;
        }
        LLMProviderKind::LMStudio => {
            let endpoint = inout.prompt("LM Studio endpoint URL (optional):")?;
            let model = inout.prompt("Preferred LM Studio model (optional):")?;
            apply_lmstudio_config(repo, scope, endpoint, model)?;
        }
    }

    writeln!(
        inout,
        "{} AI provider set to {} ({})",
        "✓".green(),
        provider.display_name().cyan(),
        scope.as_str().dimmed()
    )?;
    Ok(())
}

fn write_ai_config_success(
    out: &mut OutputChannel,
    scope: AiScope,
    provider: LLMProviderKind,
) -> Result<()> {
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{} AI provider set to {} ({})",
            "✓".green(),
            provider.display_name().cyan(),
            scope.as_str().dimmed()
        )?;
    } else if let Some(out) = out.for_shell() {
        writeln!(out, "{}", provider.as_git_config_value())?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "provider": provider.as_git_config_value(),
            "scope": scope.as_str(),
        }))?;
    }
    Ok(())
}

fn resolve_secret_input(
    api_key: Option<String>,
    api_key_env: Option<String>,
) -> Result<Option<Sensitive<String>>> {
    if api_key.is_some() && api_key_env.is_some() {
        anyhow::bail!("Pass either --api-key or --api-key-env, not both")
    }

    if let Some(value) = api_key {
        return Ok(Some(Sensitive(value)));
    }

    if let Some(env_name) = api_key_env {
        let value = std::env::var(&env_name)
            .with_context(|| format!("Environment variable '{env_name}' is not set"))?;
        return Ok(Some(Sensitive(value)));
    }

    Ok(None)
}

fn require_non_interactive_secret_if_byok(
    key_option: AiKeyOption,
    secret: Option<&Sensitive<String>>,
    provider: &str,
) -> Result<()> {
    if matches!(key_option, AiKeyOption::BringYourOwn) && secret.is_none() {
        anyhow::bail!(
            "{provider} with --key-option bring-your-own requires --api-key or --api-key-env"
        );
    }
    Ok(())
}

fn maybe_set_secret(handle: &str, secret_value: Option<Sensitive<String>>) -> Result<()> {
    if let Some(secret_value) = secret_value {
        secret::persist(handle, &secret_value, secret::Namespace::Global)?;
    }
    Ok(())
}

fn edit_ai_git_config(
    repo: Option<&gix::Repository>,
    scope: AiScope,
    edit: impl FnOnce(&mut gix::config::File<'static>) -> Result<bool>,
) -> Result<()> {
    match scope {
        AiScope::Global => {
            let (mut config, path) = but_core::git_config::open_user_global_config_for_editing()?;
            let changed = edit(&mut config)?;
            if changed {
                but_core::git_config::write_config(&path, &config)?;
            }
            Ok(())
        }
        AiScope::Local => {
            let repo = repo.context("Local AI configuration requires a git repository")?;
            edit_git_config(repo, false.into(), edit)?;
            Ok(())
        }
    }
}

fn set_optional_config_value(
    config: &mut gix::config::File<'static>,
    key: &str,
    value: Option<String>,
) -> Result<()> {
    match value {
        Some(value) if !value.trim().is_empty() => set_config_value(config, key, &value),
        _ => remove_config_value(config, key),
    }
}

fn apply_openai_config(
    repo: Option<&gix::Repository>,
    scope: AiScope,
    key_option: AiKeyOption,
    model: Option<String>,
    endpoint: Option<String>,
    api_key: Option<Sensitive<String>>,
) -> Result<()> {
    edit_ai_git_config(repo, scope, |config| {
        set_config_value(
            config,
            AI_MODEL_PROVIDER_KEY,
            LLMProviderKind::OpenAi.as_git_config_value(),
        )?;
        set_config_value(config, AI_OPENAI_KEY_OPTION_KEY, key_option.as_git_value())?;
        set_optional_config_value(config, AI_OPENAI_MODEL_NAME_KEY, model)?;
        set_optional_config_value(config, AI_OPENAI_CUSTOM_ENDPOINT_KEY, endpoint)?;
        Ok(true)
    })?;

    if matches!(key_option, AiKeyOption::BringYourOwn) {
        maybe_set_secret(AI_OPENAI_SECRET_HANDLE, api_key)?;
    }
    Ok(())
}

fn apply_anthropic_config(
    repo: Option<&gix::Repository>,
    scope: AiScope,
    key_option: AiKeyOption,
    model: Option<String>,
    api_key: Option<Sensitive<String>>,
) -> Result<()> {
    edit_ai_git_config(repo, scope, |config| {
        set_config_value(
            config,
            AI_MODEL_PROVIDER_KEY,
            LLMProviderKind::Anthropic.as_git_config_value(),
        )?;
        set_config_value(
            config,
            AI_ANTHROPIC_KEY_OPTION_KEY,
            key_option.as_git_value(),
        )?;
        set_optional_config_value(config, AI_ANTHROPIC_MODEL_NAME_KEY, model)?;
        Ok(true)
    })?;

    if matches!(key_option, AiKeyOption::BringYourOwn) {
        maybe_set_secret(AI_ANTHROPIC_SECRET_HANDLE, api_key)?;
    }
    Ok(())
}

fn apply_ollama_config(
    repo: Option<&gix::Repository>,
    scope: AiScope,
    endpoint: Option<String>,
    model: Option<String>,
) -> Result<()> {
    edit_ai_git_config(repo, scope, |config| {
        set_config_value(
            config,
            AI_MODEL_PROVIDER_KEY,
            LLMProviderKind::Ollama.as_git_config_value(),
        )?;
        set_optional_config_value(config, AI_OLLAMA_ENDPOINT_KEY, endpoint)?;
        set_optional_config_value(config, AI_OLLAMA_MODEL_NAME_KEY, model)?;
        Ok(true)
    })
}

fn apply_lmstudio_config(
    repo: Option<&gix::Repository>,
    scope: AiScope,
    endpoint: Option<String>,
    model: Option<String>,
) -> Result<()> {
    edit_ai_git_config(repo, scope, |config| {
        set_config_value(
            config,
            AI_MODEL_PROVIDER_KEY,
            LLMProviderKind::LMStudio.as_git_config_value(),
        )?;
        set_optional_config_value(config, AI_LMSTUDIO_ENDPOINT_KEY, endpoint)?;
        set_optional_config_value(config, AI_LMSTUDIO_MODEL_NAME_KEY, model)?;
        Ok(true)
    })
}

fn get_ai_config_info(repo: Option<&gix::Repository>, scope: AiScope) -> Result<AiConfigInfo> {
    match scope {
        AiScope::Global => {
            let file = gix::config::File::from_globals()?;
            Ok(AiConfigInfo {
                provider: file.string(AI_MODEL_PROVIDER_KEY).map(|v| v.to_string()),
                openai_key_option: file.string(AI_OPENAI_KEY_OPTION_KEY).map(|v| v.to_string()),
                openai_model: file.string(AI_OPENAI_MODEL_NAME_KEY).map(|v| v.to_string()),
                openai_endpoint: file
                    .string(AI_OPENAI_CUSTOM_ENDPOINT_KEY)
                    .map(|v| v.to_string()),
                anthropic_key_option: file
                    .string(AI_ANTHROPIC_KEY_OPTION_KEY)
                    .map(|v| v.to_string()),
                anthropic_model: file
                    .string(AI_ANTHROPIC_MODEL_NAME_KEY)
                    .map(|v| v.to_string()),
                ollama_endpoint: file.string(AI_OLLAMA_ENDPOINT_KEY).map(|v| v.to_string()),
                ollama_model: file.string(AI_OLLAMA_MODEL_NAME_KEY).map(|v| v.to_string()),
                lmstudio_endpoint: file.string(AI_LMSTUDIO_ENDPOINT_KEY).map(|v| v.to_string()),
                lmstudio_model: file
                    .string(AI_LMSTUDIO_MODEL_NAME_KEY)
                    .map(|v| v.to_string()),
            })
        }
        AiScope::Local => {
            let repo = repo.context("Local AI configuration requires a git repository")?;
            let config = repo.config_snapshot();
            Ok(AiConfigInfo {
                provider: config.string(AI_MODEL_PROVIDER_KEY).map(|v| v.to_string()),
                openai_key_option: config
                    .string(AI_OPENAI_KEY_OPTION_KEY)
                    .map(|v| v.to_string()),
                openai_model: config
                    .string(AI_OPENAI_MODEL_NAME_KEY)
                    .map(|v| v.to_string()),
                openai_endpoint: config
                    .string(AI_OPENAI_CUSTOM_ENDPOINT_KEY)
                    .map(|v| v.to_string()),
                anthropic_key_option: config
                    .string(AI_ANTHROPIC_KEY_OPTION_KEY)
                    .map(|v| v.to_string()),
                anthropic_model: config
                    .string(AI_ANTHROPIC_MODEL_NAME_KEY)
                    .map(|v| v.to_string()),
                ollama_endpoint: config.string(AI_OLLAMA_ENDPOINT_KEY).map(|v| v.to_string()),
                ollama_model: config
                    .string(AI_OLLAMA_MODEL_NAME_KEY)
                    .map(|v| v.to_string()),
                lmstudio_endpoint: config
                    .string(AI_LMSTUDIO_ENDPOINT_KEY)
                    .map(|v| v.to_string()),
                lmstudio_model: config
                    .string(AI_LMSTUDIO_MODEL_NAME_KEY)
                    .map(|v| v.to_string()),
            })
        }
    }
}

/// Handle target config subcommand
async fn target_config(
    ctx: &mut Context,
    out: &mut OutputChannel,
    branch: Option<String>,
) -> Result<()> {
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
                        writeln!(
                            out,
                            "  {}",
                            "but config target <branch_name>".blue().dimmed()
                        )?;
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
                    writeln!(
                        out,
                        "{}",
                        "\nThe following branches are currently applied:\n".bold()
                    )?;
                    ws.stacks.iter().for_each(|stack| {
                        {
                            writeln!(
                                out,
                                "{} Applied branch: {}",
                                "•".dimmed(),
                                stack
                                    .ref_name()
                                    .map_or_else(
                                        || "ANONYMOUS".to_string(),
                                        |rn| rn.shorten().to_string()
                                    )
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
                writeln!(
                    out,
                    "{} Changing target branch to '{}'",
                    "✓".green(),
                    new_branch.cyan()
                )?;
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
    let repo = ctx.repo.get()?;

    match cmd {
        None => {
            let config = repo.config_snapshot();
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
                    format_scope(tui_scope)
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
            let bool_value = gix::config::Boolean::try_from(value.as_bytes().as_bstr())
                .with_context(|| {
                    anyhow::anyhow!("Invalid value '{value}'. Use true/false or 1/0.")
                })?
                .0;
            let serialized = if bool_value { "true" } else { "false" };
            edit_git_config(&repo, global.into(), |config| {
                set_config_value(config, git_key, serialized)?;
                Ok(true)
            })?;

            if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "{} Set {} {} {}",
                    "✓".green(),
                    git_key.green(),
                    "→".dimmed(),
                    if bool_value {
                        "true".cyan()
                    } else {
                        "false".cyan()
                    }
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
            edit_git_config(&repo, global.into(), |config| {
                remove_config_value(config, git_key)?;
                Ok(true)
            })?;

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
pub(crate) fn get_tui_enabled(config: &gix::config::Snapshot<'_>) -> bool {
    config.boolean("but.ui.tui").unwrap_or(false)
}

/// Get the scope (local/global) where a config key is set
fn get_config_scope(config: &gix::config::Snapshot<'_>, key: &str) -> Option<gix::config::Source> {
    get_config_string_and_scope(config, key).1
}

fn get_config_string_and_scope(
    config: &gix::config::Snapshot<'_>,
    key: &str,
) -> (Option<String>, Option<gix::config::Source>) {
    let mut scope = None;
    let value_opt = config.string_filter(key, |meta| {
        scope = Some(meta.source);
        true
    });
    (value_opt.map(|s| s.to_string()), scope)
}

fn config_source_scope(source: gix::config::Source) -> &'static str {
    use gix::config::Source;
    match source {
        Source::Local | Source::Worktree => "local",
        Source::User | Source::Git => "global",
        Source::System => "system",
        Source::GitInstallation => "git-installation",
        Source::Env => "env",
        Source::Cli => "cli",
        Source::Api => "api",
        Source::EnvOverride => "env-override",
    }
}

/// Format the scope for display
fn format_scope(scope: Option<gix::config::Source>) -> String {
    match scope {
        Some(source) => format!("({})", config_source_scope(source))
            .dimmed()
            .to_string(),
        None => String::new(),
    }
}

/// Serialize an optional config source using the user-facing scope labels.
fn serialize_config_source<S>(
    scope: &Option<gix::config::Source>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match scope {
        Some(source) => serializer.serialize_some(config_source_scope(*source)),
        None => serializer.serialize_none(),
    }
}

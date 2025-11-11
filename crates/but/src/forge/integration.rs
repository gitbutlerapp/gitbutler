use cli_prompts::DisplayPrompt;
use colored::Colorize;
use std::io::Write;

use crate::forge::auth::auth_github;

#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Subcommands,
}
#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Authenticate with your forge provider (at the moment, only GitHub is supported)
    Auth,
    /// List authenticated forge accounts known to GitButler
    ListUsers,
    /// Forget a previously authenticated forge account
    Forget {
        /// The username of the forge account to forget
        /// If not provided, you'll be prompted to select which account(s) to forget. If only one account exists, it will be forgotten automatically.
        username: Option<String>,
    },
}

pub async fn handle(cmd: Subcommands) -> anyhow::Result<()> {
    match cmd {
        Subcommands::Auth => auth_github().await,
        Subcommands::ListUsers => list_github_users().await,
        Subcommands::Forget { username } => forget_github_username(username).await,
    }
}

async fn forget_github_username(username: Option<String>) -> anyhow::Result<()> {
    let known_accounts = but_api::github::list_known_github_accounts().await?;
    let mut stdout = std::io::stdout();

    let accounts_to_delete: Vec<_> = if let Some(username) = &username {
        known_accounts
            .into_iter()
            .filter(|account| account.username() == username)
            .collect()
    } else {
        known_accounts
    };

    // Handle case where no matching account was found
    if accounts_to_delete.is_empty() {
        if let Some(username) = &username {
            writeln!(
                stdout,
                "No known GitHub account with username '{}'",
                username
            )
            .ok();
        }
        return Ok(());
    }

    // Handle different scenarios based on number of accounts
    match accounts_to_delete.as_slice() {
        [single_account] => {
            // Single account: delete automatically
            but_api::github::forget_github_account(single_account.clone())?;
            writeln!(stdout, "Forgot GitHub account '{}'", single_account).ok();
        }
        _ => {
            // Multiple accounts: prompt user to select
            let account_prompt = cli_prompts::prompts::Multiselect::new_transformed(
                "Which of the following accounts do you want to forget?",
                accounts_to_delete.into_iter(),
                |acc| acc.to_string(),
            );

            let selected_accounts = account_prompt
                .display()
                .map_err(|_| anyhow::anyhow!("Could not determine which accounts to delete"))?;

            if selected_accounts.is_empty() {
                writeln!(stdout, "No accounts were selected to forget.").ok();
                return Ok(());
            }

            for account in selected_accounts {
                but_api::github::forget_github_account(account.clone())?;
                writeln!(stdout, "Forgot GitHub account '{}'", account).ok();
            }
        }
    }

    Ok(())
}

async fn list_github_users() -> anyhow::Result<()> {
    let known_accounts = but_api::github::list_known_github_accounts().await?;
    let mut stdout = std::io::stdout();
    writeln!(stdout, "Known GitHub usernames:").ok();
    let mut some_accounts_invalid = false;
    for account in known_accounts {
        let account_status = but_api::github::check_github_credentials(&account)
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
            None => " (unknown status)".bold().red(),
        };

        writeln!(stdout, "- {} {}", account, message).ok();
    }

    if some_accounts_invalid {
        writeln!(
            stdout,
            "\nSome accounts have invalid or missing credentials.\nYou may want to re-authenticate with those accounts using the '{}' command.",
            "but forge auth".bold()
        ).ok();
    }

    Ok(())
}

use anyhow::bail;
use cli_prompts::DisplayPrompt;
use colored::Colorize;

use crate::forge::auth::auth_github;
use crate::utils::OutputChannel;

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

pub async fn handle(cmd: Subcommands, out: &mut OutputChannel) -> anyhow::Result<()> {
    match cmd {
        Subcommands::Auth => auth_github(out).await,
        Subcommands::ListUsers => list_github_users(out).await,
        Subcommands::Forget { username } => forget_github_username(username, out).await,
    }
}

async fn forget_github_username(
    username: Option<String>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let known_accounts = but_api::github::list_known_github_accounts().await?;
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
        if let Some((username, out)) = username.zip(out.for_human()) {
            writeln!(out, "No known GitHub account with username '{username}'")?;
        }
        return Ok(());
    }

    // Handle different scenarios based on number of accounts
    match accounts_to_delete.as_slice() {
        [single_account] => {
            // Single account: delete automatically
            but_api::github::forget_github_account(single_account.clone())?;
            if let Some(out) = out.for_human() {
                writeln!(out, "Forgot GitHub account '{}'", single_account)?;
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
                    but_api::github::forget_github_account(account.clone())?;
                    writeln!(out, "Forgot GitHub account '{}'", account)?;
                }
            } else {
                bail!("Username ambiguous, got {accounts_to_delete:?}");
            }
        }
    }

    Ok(())
}

async fn list_github_users(out: &mut OutputChannel) -> anyhow::Result<()> {
    let known_accounts = but_api::github::list_known_github_accounts().await?;
    if let Some(out) = out.for_human() {
        writeln!(out, "Known GitHub usernames:")?;
        let mut some_accounts_invalid = false;
        for account in known_accounts {
            let account_status = but_api::github::check_github_credentials(&account)
                .await
                .ok();

            let message = match account_status {
                Some(but_github::CredentialCheckResult::Valid) => {
                    "(valid credentials)".green().bold()
                }
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

            writeln!(out, "- {} {}", account, message)?;
        }

        if some_accounts_invalid {
            writeln!(
                out,
                "\nSome accounts have invalid or missing credentials.\nYou may want to re-authenticate with those accounts using the '{}' command.",
                "but forge auth".bold()
            )?;
        }
    } else if let Some(out) = out.for_shell() {
        for account in known_accounts {
            writeln!(out, "{}", account.username())?;
        }
    }
    Ok(())
}

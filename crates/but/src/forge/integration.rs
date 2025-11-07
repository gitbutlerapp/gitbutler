use but_api::NoParams;
use colored::Colorize;
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
        username: String,
    },
}

pub async fn handle(cmd: Subcommands) -> anyhow::Result<()> {
    match cmd {
        Subcommands::Auth => auth_github().await,
        Subcommands::ListUsers => list_github_users().await,
        Subcommands::Forget { username } => forget_github_username(&username).await,
    }
}

async fn forget_github_username(username: &String) -> anyhow::Result<()> {
    let known_accounts = but_api::github::list_known_github_accounts().await?;
    if let Some(account_to_delete) = known_accounts
        .into_iter()
        .find(|account| account.username() == username)
    {
        let message = format!("Forgot GitHub account '{}'", &account_to_delete);
        but_api::github::forget_github_account(account_to_delete)?;
        println!("{}", message);
        Ok(())
    } else {
        println!("No known GitHub account with username '{}'", username);
        Ok(())
    }
}

async fn list_github_users() -> anyhow::Result<()> {
    let known_accounts = but_api::github::list_known_github_accounts().await?;
    println!("Known GitHub usernames:");
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

        println!("- {} {}", account, message);
    }

    if some_accounts_invalid {
        println!(
            "\nSome accounts have invalid or missing credentials.\nYou may want to re-authenticate with those accounts using the '{}' command.",
            "but forge auth".bold()
        );
    }

    Ok(())
}

/// Helper function to extract properties for metrics
async fn auth_github() -> anyhow::Result<()> {
    let code = but_api::github::init_device_oauth(NoParams {}).await?;
    println!(
        "Device authorization initiated. Please visit the following URL and enter the code:\n\nhttps://github.com/login/device\n\nCode: {}\n\n",
        code.user_code
    );

    println!("Type 'y' and press Enter after you have successfully authorized the device:");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() != "y" {
        println!("Authorization process aborted by user.");
        return Ok(());
    }

    let auth_outcome = but_api::github::check_auth_status(but_github::CheckAuthStatusParams {
        device_code: code.device_code,
    })
    .await;

    match auth_outcome {
        Ok(status) => {
            println!("Authentication successful! Welcome, {}.", status.login);
        }
        Err(e) => {
            eprintln!("Authentication failed: {}", anyhow::format_err!(e));
        }
    }

    Ok(())
}

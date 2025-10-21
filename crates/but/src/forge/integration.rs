use but_api::NoParams;
#[derive(Debug, clap::Parser)]
pub struct Platform {
    #[clap(subcommand)]
    pub cmd: Subcommands,
}
#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Authenticat with your forge provider (at the moment, only GitHub is supported)
    Auth,
    /// List authenticated forge accounts known to GitButler
    ListUsers,
    /// Forget a previously authenticated forge account
    Forget {
        /// The username of the forge account to forget
        username: String,
    },
}

pub async fn handle(
    cmd: &Subcommands,
    _: &gitbutler_project::Project,
    _: bool,
) -> anyhow::Result<()> {
    match cmd {
        Subcommands::Auth => auth_github().await,
        Subcommands::ListUsers => list_github_users().await,
        Subcommands::Forget { username } => forget_github_username(username),
    }
}

fn forget_github_username(username: &str) -> anyhow::Result<()> {
    but_api::github::forget_github_username(username.to_string())?;
    println!("Forgot GitHub username '{}'", username);
    Ok(())
}

async fn list_github_users() -> anyhow::Result<()> {
    let known_usernames = but_github::list_known_github_usernames().await?;
    println!("Known GitHub usernames:");
    for username in known_usernames {
        println!("- {}", username);
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

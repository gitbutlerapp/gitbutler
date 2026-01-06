use anyhow::{Context, bail};
use but_github::AuthStatusResponse;
use but_secret::Sensitive;
use cli_prompts::DisplayPrompt;
use std::fmt::Write;

use crate::utils::{InputOutputChannel, OutputChannel};

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

/// Authenticate with GitHub
pub async fn auth_github(out: &mut OutputChannel) -> anyhow::Result<()> {
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
async fn github_pat(mut inout: InputOutputChannel<'_>) -> anyhow::Result<()> {
    let input = inout
        .prompt("Please enter your GitHub Personal Access Token (PAT) and hit enter:")?
        .context("No PAT provided. Aborting authentication.")?;

    let pat = Sensitive(input);
    let AuthStatusResponse { login, .. } = but_api::github::store_github_pat(pat)
        .await
        .map_err(|err| err.context("Authentication failed"))?;

    writeln!(inout, "Authentication successful! Welcome, {}.", login)?;
    Ok(())
}

/// Authenticate with GitHub Enterprise
async fn github_enterprise(mut inout: InputOutputChannel<'_>) -> anyhow::Result<()> {
    let base_url = inout.prompt("Please enter your GitHub Enterprise API base URL (e.g., https://github.mycompany.com/api/v3) and hit enter:")?.context("No host provided. Aborting authentication.")?;

    let input = inout
        .prompt(
            "Now, please enter your GitHub Enterprise Personal Access Token (PAT) and hit enter:",
        )?
        .context("No PAT provided. Aborting authentication.")?;
    let pat = Sensitive(input);
    let AuthStatusResponse { login, .. } =
        but_api::github::store_github_enterprise_pat(pat, base_url)
            .await
            .map_err(|err| err.context("Authentication failed"))?;

    writeln!(inout, "Authentication successful! Welcome, {}.", login)?;
    Ok(())
}

/// Authenticate with GitHub using the device OAuth flow
async fn github_oauth(mut inout: InputOutputChannel<'_>) -> anyhow::Result<()> {
    let code = but_api::github::init_device_oauth().await?;
    writeln!(
        inout,
        "Device authorization initiated. Please visit the following URL and enter the code:\n\nhttps://github.com/login/device\n\nCode: {}\n\n",
        code.user_code
    )?;

    let aborted_msg = "Authorization process aborted by user.";
    let input = inout
        .prompt("Type 'y' and press Enter after you have successfully authorized the device:")?
        .context(aborted_msg)?;

    if input.to_lowercase() != "y" {
        bail!(aborted_msg)
    }

    let status = but_api::github::check_auth_status(code.device_code)
        .await
        .map_err(|err| err.context("Authentication failed"))?;

    writeln!(
        inout,
        "Authentication successful! Welcome, {}.",
        status.login
    )?;

    Ok(())
}

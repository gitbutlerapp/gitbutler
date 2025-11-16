use crate::utils::OutputChannel;
use anyhow::bail;
use but_github::AuthStatusResponse;
use but_secret::Sensitive;
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

/// Authenticate with GitHub
pub async fn auth_github(out: &mut OutputChannel) -> anyhow::Result<()> {
    let Some(out) = out.for_human() else {
        bail!("Human input required - run this in a terminal")
    };
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
        AuthMethod::Pat => github_pat(out).await,
        AuthMethod::Enterprise => github_enterprise(out).await,
        AuthMethod::DeviceFlow => github_oauth(out).await,
    }
}

/// Authenticate with GitHub using a Personal Access Token (PAT)
async fn github_pat(out_for_humans: &mut dyn std::fmt::Write) -> anyhow::Result<()> {
    writeln!(
        out_for_humans,
        "Please enter your GitHub Personal Access Token (PAT) and hit enter:"
    )?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().is_empty() {
        bail!("No PAT provided. Aborting authentication.");
    }

    let pat = Sensitive(input.trim().to_string());
    let AuthStatusResponse { login, .. } = but_api::github::store_github_pat(pat)
        .await
        .map_err(|err| anyhow::Error::from(err).context("Authentication failed"))?;

    writeln!(
        out_for_humans,
        "Authentication successful! Welcome, {}.",
        login
    )?;

    Ok(())
}

/// Authenticate with GitHub Enterprise
async fn github_enterprise(out_for_humans: &mut dyn std::fmt::Write) -> anyhow::Result<()> {
    writeln!(
        out_for_humans,
        "Please enter your GitHub Enterprise API base URL (e.g., https://github.mycompany.com/api/v3) and hit enter:"
    )?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let base_url = input.trim().to_string();
    if base_url.is_empty() {
        bail!("No host provided. Aborting authentication.")
    }

    writeln!(
        out_for_humans,
        "Now, please enter your GitHub Enterprise Personal Access Token (PAT) and hit enter:"
    )?;

    input.clear();
    std::io::stdin().read_line(&mut input)?;
    let pat = Sensitive(input.trim().to_string());
    if pat.is_empty() {
        bail!("No PAT provided. Aborting authentication.");
    }

    let AuthStatusResponse { login, .. } =
        but_api::github::store_github_enterprise_pat(pat, base_url)
            .await
            .map_err(|err| anyhow::Error::from(err).context("Authentication failed"))?;

    writeln!(
        out_for_humans,
        "Authentication successful! Welcome, {}.",
        login
    )?;

    Ok(())
}

/// Authenticate with GitHub usgin the device OAuth flow
async fn github_oauth(out_for_humans: &mut dyn std::fmt::Write) -> anyhow::Result<()> {
    let code = but_api::github::init_device_oauth().await?;
    writeln!(
        out_for_humans,
        "Device authorization initiated. Please visit the following URL and enter the code:\n\nhttps://github.com/login/device\n\nCode: {}\n\n",
        code.user_code
    )?;

    writeln!(
        out_for_humans,
        "Type 'y' and press Enter after you have successfully authorized the device:"
    )?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() != "y" {
        bail!("Authorization process aborted by user.")
    }

    let status = but_api::github::check_auth_status(code.device_code)
        .await
        .map_err(|err| anyhow::Error::from(err).context("Authentication failed"))?;

    writeln!(
        out_for_humans,
        "Authentication successful! Welcome, {}.",
        status.login
    )?;

    Ok(())
}

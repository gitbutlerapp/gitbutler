use but_api::{
    NoParams,
    github::{AuthStatusResponseSensitive, StoreGitHubPatParams},
};
use cli_prompts::DisplayPrompt;
use std::io::Write;

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
pub async fn auth_github() -> anyhow::Result<()> {
    let auth_method_prompt = cli_prompts::prompts::Selection::new(
        "Select an authentication method:",
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
        AuthMethod::Pat => github_pat().await,
        AuthMethod::Enterprise => github_enterprise().await,
        AuthMethod::DeviceFlow => github_oauth().await,
    }
}

/// Authenticate with GitHub using a Personal Access Token (PAT)
async fn github_pat() -> anyhow::Result<()> {
    let mut stdout = std::io::stdout();
    writeln!(
        stdout,
        "Please enter your GitHub Personal Access Token (PAT) and hit enter:"
    )
    .ok();

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().is_empty() {
        writeln!(stdout, "No PAT provided. Aborting authentication.").ok();
        return Ok(());
    }

    let pat = input.trim().to_string();
    match but_api::github::strore_github_pat(StoreGitHubPatParams { access_token: pat }).await {
        Ok(AuthStatusResponseSensitive { login, .. }) => {
            writeln!(stdout, "Authentication successful! Welcome, {}.", login).ok();
        }
        Err(e) => {
            writeln!(stdout, "Authentication failed: {}", anyhow::format_err!(e)).ok();
        }
    }

    Ok(())
}

/// Authenticate with GitHub Enterprise
async fn github_enterprise() -> anyhow::Result<()> {
    let mut stdout = std::io::stdout();
    writeln!(
        stdout,
        "Please enter your GitHub Enterprise API base URL (e.g., https://github.mycompany.com/api/v3) and hit enter:"
    )
    .ok();

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let base_url = input.trim().to_string();
    if base_url.is_empty() {
        writeln!(stdout, "No host provided. Aborting authentication.").ok();
        return Ok(());
    }

    writeln!(
        stdout,
        "Now, please enter your GitHub Enterprise Personal Access Token (PAT) and hit enter:"
    )
    .ok();

    input.clear();
    std::io::stdin().read_line(&mut input)?;
    let pat = input.trim().to_string();
    if pat.is_empty() {
        writeln!(stdout, "No PAT provided. Aborting authentication.").ok();
        return Ok(());
    }

    match but_api::github::store_github_enterprise_pat(
        but_api::github::StoreGitHubEnterprisePatParams {
            access_token: pat,
            host: base_url,
        },
    )
    .await
    {
        Ok(AuthStatusResponseSensitive { login, .. }) => {
            writeln!(stdout, "Authentication successful! Welcome, {}.", login).ok();
        }
        Err(e) => {
            writeln!(stdout, "Authentication failed: {}", anyhow::format_err!(e)).ok();
        }
    }

    Ok(())
}

/// Authenticate with GitHub usgin the device OAuth flow
async fn github_oauth() -> anyhow::Result<()> {
    let code = but_api::github::init_device_oauth(NoParams {}).await?;
    let mut stdout = std::io::stdout();
    writeln!(
        stdout,
        "Device authorization initiated. Please visit the following URL and enter the code:\n\nhttps://github.com/login/device\n\nCode: {}\n\n",
        code.user_code
    ).ok();

    writeln!(
        stdout,
        "Type 'y' and press Enter after you have successfully authorized the device:"
    )
    .ok();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() != "y" {
        writeln!(stdout, "Authorization process aborted by user.").ok();
        return Ok(());
    }

    let auth_outcome = but_api::github::check_auth_status(but_github::CheckAuthStatusParams {
        device_code: code.device_code,
    })
    .await;

    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    match auth_outcome {
        Ok(status) => {
            writeln!(
                stdout,
                "Authentication successful! Welcome, {}.",
                status.login
            )
            .ok();
        }
        Err(e) => {
            writeln!(stderr, "Authentication failed: {}", anyhow::format_err!(e)).ok();
        }
    }

    Ok(())
}

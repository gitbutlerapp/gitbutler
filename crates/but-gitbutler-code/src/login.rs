//! GitButler Code's device login, the one `grit login` uses: start it, have the user approve
//! the code in a browser, and poll until they have.

use anyhow::{Context as _, Result};
use but_secret::Sensitive;
use serde::Deserialize;

use crate::{AuthStatusResponse, base_url, client};

/// A started login.
#[derive(Debug, Clone)]
pub struct LoginStart {
    /// The secret to poll with. Never shown to the user.
    pub device_code: String,
    /// The code the user checks in the browser, like `ABCD-2345`.
    pub user_code: String,
    /// Seconds to wait between polls.
    pub interval: u64,
    /// Seconds until the codes expire.
    pub expires_in: u64,
    /// The page where the user approves `user_code`. It needs a browser signed in to GitButler.
    pub verification_url: String,
}

/// Where a login stands.
#[derive(Debug, Clone)]
pub enum LoginStatus {
    /// The user hasn't approved the code yet.
    Pending,
    /// The user declined the code.
    Denied,
    /// The code expired, was used already, or never existed.
    Expired,
    /// The user approved: the account and its token are stored, and Git has the token.
    Authorized(AuthStatusResponse),
}

#[derive(Deserialize)]
struct StartBody {
    device_code: String,
    user_code: String,
    interval: u64,
    expires_in: u64,
}

#[derive(Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum PollBody {
    Pending,
    Denied,
    Expired,
    /// `email` holds the username, as GitButler Code names it on the wire.
    Authorized {
        email: String,
        token: String,
    },
}

/// Start a login.
pub async fn start_login() -> Result<LoginStart> {
    let base = base_url();
    let body: StartBody = client::http()?
        .post(format!("{base}/.grit/login"))
        .send()
        .await
        .context("Failed to reach GitButler Code")?
        .error_for_status()
        .context("GitButler Code refused to start a login")?
        .json()
        .await
        .context("GitButler Code answered the login with something unexpected")?;
    let verification_url = url::Url::parse_with_params(
        &format!("{base}/.grit/authorize"),
        [("code", &body.user_code)],
    )
    .context("GitButler Code's URL is not a URL")?
    .to_string();
    Ok(LoginStart {
        device_code: body.device_code,
        user_code: body.user_code,
        interval: body.interval,
        expires_in: body.expires_in,
        verification_url,
    })
}

/// Ask whether the login of `device_code` was approved. Once it is, the account and its token
/// are stored in `storage`, and Git's credential store has the token for pushes.
pub async fn check_login(
    device_code: &str,
    storage: &but_forge_storage::Controller,
) -> Result<LoginStatus> {
    let body: PollBody = client::http()?
        .post(format!("{}/.grit/login/poll", base_url()))
        .json(&serde_json::json!({ "device_code": device_code }))
        .send()
        .await
        .context("Failed to reach GitButler Code")?
        .error_for_status()
        .context("GitButler Code refused the login poll")?
        .json()
        .await
        .context("GitButler Code answered the login poll with something unexpected")?;
    Ok(match body {
        PollBody::Pending => LoginStatus::Pending,
        PollBody::Denied => LoginStatus::Denied,
        PollBody::Expired => LoginStatus::Expired,
        PollBody::Authorized {
            email: username,
            token,
        } => {
            crate::sign_in(&username, Sensitive(token), storage)?;
            LoginStatus::Authorized(AuthStatusResponse { username })
        }
    })
}

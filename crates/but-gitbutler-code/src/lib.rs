//! GitButler Code (`code.gitbutler.com`) as a forge: signing in with its device login, keeping
//! the account and its token, handing the token to Git, and the project calls that move a
//! repository there.
//!
//! GitButler Code has no pull request or CI API yet, so this crate has none either.

use anyhow::{Context as _, Result};
use but_secret::Sensitive;

mod account;
mod client;
mod credential;
mod login;

pub use account::GitbutlerCodeAccountIdentifier;
pub use client::{Client, HttpStatusError, Space};
pub use login::{LoginStart, LoginStatus, check_login, start_login};

/// Where GitButler Code is served.
pub const DEFAULT_BASE_URL: &str = "https://code.gitbutler.com";

/// The environment variable that points GitButler Code calls at another server, as a local one.
pub const BASE_URL_ENV: &str = "BUT_GITBUTLER_CODE_URL";

/// The server's URL, without a trailing slash: [`BASE_URL_ENV`] when set, else
/// [`DEFAULT_BASE_URL`].
pub fn base_url() -> String {
    std::env::var(BASE_URL_ENV)
        .ok()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_owned())
        .trim_end_matches('/')
        .to_owned()
}

/// The host of [`base_url`], with its port when it has one.
pub fn host() -> Result<String> {
    let url = url::Url::parse(&base_url()).context("GitButler Code's URL is not a URL")?;
    let host = url
        .host_str()
        .context("GitButler Code's URL has no host")?
        .to_owned();
    Ok(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host,
    })
}

/// Whether `host` is GitButler Code's.
pub fn is_gitbutler_code_host(host: &str) -> bool {
    self::host().is_ok_and(|ours| ours.eq_ignore_ascii_case(host))
        || host.eq_ignore_ascii_case("code.gitbutler.com")
}

/// Who signed in, as the login hands it back.
#[derive(Debug, Clone)]
pub struct AuthStatusResponse {
    pub username: String,
}

/// The stored GitButler Code accounts.
pub fn list_known_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<GitbutlerCodeAccountIdentifier>> {
    account::list_known_accounts(storage).context("Failed to list known GitButler Code accounts")
}

/// A client signed in as `account`, or `None` when its token is gone.
pub fn client(
    account: &GitbutlerCodeAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<Client>> {
    Ok(account::access_token(account, storage)?.map(|token| Client::new(base_url(), token)))
}

/// Forget `account`: its stored token, and the credential Git keeps for pushes.
pub fn forget_account(
    account: &GitbutlerCodeAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let token = account::access_token(account, storage).ok().flatten();
    account::delete(account, storage).context("Failed to delete the GitButler Code account")?;
    if let Some(token) = token
        && let Err(err) = credential::reject(account.username(), &token)
    {
        tracing::warn!("Failed to remove the GitButler Code credential from Git: {err:#}");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialCheckResult {
    Valid,
    Invalid,
    NoCredentials,
}

/// Whether `account`'s stored token still signs in.
pub async fn check_credentials(
    account: &GitbutlerCodeAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<CredentialCheckResult> {
    let Some(client) = client(account, storage)? else {
        return Ok(CredentialCheckResult::NoCredentials);
    };
    Ok(match client.user().await {
        Ok(_) => CredentialCheckResult::Valid,
        Err(err) if client::is_unauthorized(&err) => CredentialCheckResult::Invalid,
        Err(err) => return Err(err),
    })
}

/// Keep `token` for `username`: as the account's token, and in Git's credential store so pushes
/// to GitButler Code use it.
fn sign_in(
    username: &str,
    token: Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let account = GitbutlerCodeAccountIdentifier::device_login(username);
    account::persist(&account, &token, storage)
        .context("Failed to store the GitButler Code account")?;
    credential::approve(username, &token).context("Failed to give Git the GitButler Code token")
}

//! The accounts GitButler talks to forges and its own service with. They're shared with
//! GitButler's other apps: what's added here works there and the other way round.

use anyhow::{Result, bail};
use but_bitbucket::BitbucketAccountIdentifier as Bitbucket;
use but_github::GithubAccountIdentifier as GitHub;
use but_gitlab::GitlabAccountIdentifier as GitLab;
use but_secret::Sensitive;

use super::runtime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forge {
    GitHub,
    GitLab,
    Bitbucket,
}

impl Forge {
    pub fn name(self) -> &'static str {
        match self {
            Forge::GitHub => "GitHub",
            Forge::GitLab => "GitLab",
            Forge::Bitbucket => "Bitbucket",
        }
    }
}

#[derive(Debug, Clone)]
pub enum AccountId {
    GitHub(GitHub),
    GitLab(GitLab),
    Bitbucket(Bitbucket),
}

/// A stored forge account, as "octocat", with how it signed in when that tells two apart.
#[derive(Debug, Clone)]
pub struct Account {
    pub forge: Forge,
    pub label: String,
    pub id: AccountId,
}

pub fn accounts() -> Vec<Account> {
    let github = but_api::github::list_known_github_accounts().unwrap_or_default();
    let gitlab = but_api::gitlab::list_known_gitlab_accounts().unwrap_or_default();
    let bitbucket = but_api::bitbucket::list_known_bitbucket_accounts().unwrap_or_default();
    let github = github.into_iter().map(|id| {
        let label = match &id {
            GitHub::OAuthUsername { username } => username.clone(),
            GitHub::PatUsername { username } => format!("{username} (token)"),
            GitHub::Enterprise { username, host } => format!("{username} on {host}"),
        };
        Account {
            forge: Forge::GitHub,
            label,
            id: AccountId::GitHub(id),
        }
    });
    let gitlab = gitlab.into_iter().map(|id| {
        let label = match &id {
            GitLab::PatUsername { username } => username.clone(),
            GitLab::SelfHosted { username, host } => format!("{username} on {host}"),
        };
        Account {
            forge: Forge::GitLab,
            label,
            id: AccountId::GitLab(id),
        }
    });
    let bitbucket = bitbucket.into_iter().map(|id| {
        let Bitbucket::ApiToken { email } = &id;
        Account {
            forge: Forge::Bitbucket,
            label: email.clone(),
            id: AccountId::Bitbucket(id),
        }
    });
    github.chain(gitlab).chain(bitbucket).collect()
}

pub fn forget(account: AccountId) -> Result<()> {
    match account {
        AccountId::GitHub(id) => but_api::github::forget_github_account(id),
        AccountId::GitLab(id) => but_api::gitlab::forget_gitlab_account(id),
        AccountId::Bitbucket(id) => but_api::bitbucket::forget_bitbucket_account(id),
    }
}

/// Check `token` with the forge and store it. Bitbucket also needs the account's `email`.
/// Returns who it signed in as.
pub fn add_token(forge: Forge, token: String, email: Option<String>) -> Result<String> {
    let token = Sensitive(token.trim().to_owned());
    let handle = runtime().handle();
    Ok(match forge {
        Forge::GitHub => {
            handle
                .block_on(but_api::github::store_github_pat(token))?
                .login
        }
        Forge::GitLab => {
            handle
                .block_on(but_api::gitlab::store_gitlab_pat(token))?
                .username
        }
        Forge::Bitbucket => {
            let Some(email) = email.filter(|e| !e.trim().is_empty()) else {
                bail!("Bitbucket needs the account's email too");
            };
            let email = email.trim().to_owned();
            handle.block_on(but_api::bitbucket::store_bitbucket_api_token(
                email.clone(),
                token,
            ))?;
            email
        }
    })
}

/// Start signing in to GitHub in the browser. Returns the code to enter there, and the
/// device code to wait for the sign-in with.
pub fn start_github_sign_in() -> Result<(String, String)> {
    let verification = runtime()
        .handle()
        .block_on(but_api::github::init_github_device_oauth())?;
    Ok((verification.user_code, verification.device_code))
}

/// Whether the browser sign-in for `device_code` has finished; it stores the account when
/// it has. GitHub answers with an error while it's still waiting.
pub fn github_signed_in(device_code: &str) -> Result<String> {
    let status = runtime()
        .handle()
        .block_on(but_api::github::check_github_auth_status(
            device_code.to_owned(),
        ))?;
    Ok(status.login)
}

/// Who is signed in to GitButler, if anyone.
pub fn gitbutler_user() -> Option<String> {
    let profile = but_api::legacy::users::get_user_profile_local().ok()??;
    profile.name.or(profile.login).or(profile.email)
}

/// The page to sign in to GitButler on. It shows a token to paste back here.
pub fn gitbutler_login_url() -> Result<String> {
    Ok(but_api::legacy::users::get_login_token()?.url)
}

pub fn gitbutler_log_in(token: &str) -> Result<String> {
    let profile = but_api::legacy::users::login_and_persist(token.trim().to_owned())?;
    Ok(profile
        .name
        .or(profile.login)
        .unwrap_or_else(|| "your account".into()))
}

pub fn gitbutler_log_out() -> Result<()> {
    but_api::legacy::users::delete_user()
}

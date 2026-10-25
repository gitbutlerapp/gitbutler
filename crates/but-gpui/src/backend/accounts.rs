//! The accounts GitButler talks to forges and its own service with. They're shared with
//! GitButler's other apps: what's added here works there and the other way round.

use anyhow::{Result, bail};
use but_bitbucket::BitbucketAccountIdentifier as Bitbucket;
use but_github::GithubAccountIdentifier as GitHub;
use but_gitlab::GitlabAccountIdentifier as GitLab;
use but_secret::Sensitive;

use super::{Forge, runtime};

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

/// Check `token` with the forge and store it. Bitbucket also needs the account's `email`, which
/// the form asks for first. Returns who it signed in as.
pub fn add_token(forge: Forge, token: String, email: String) -> Result<String> {
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
            let email = email.trim().to_owned();
            handle.block_on(but_api::bitbucket::store_bitbucket_api_token(
                email.clone(),
                token,
            ))?;
            email
        }
        Forge::Azure => bail!("GitButler can't sign in to Azure yet"),
    })
}

/// Check `token` with the GitHub Enterprise or self-hosted GitLab server whose API is at `url`
/// and store it. Returns who it signed in as.
pub fn add_self_hosted_token(forge: Forge, url: String, token: String) -> Result<String> {
    let (url, token) = (url.trim().to_owned(), Sensitive(token.trim().to_owned()));
    let handle = runtime().handle();
    Ok(match forge {
        Forge::GitHub => {
            handle
                .block_on(but_api::github::store_github_enterprise_pat(token, url))?
                .login
        }
        Forge::GitLab => {
            handle
                .block_on(but_api::gitlab::store_gitlab_selfhosted_pat(token, url))?
                .username
        }
        Forge::Bitbucket | Forge::Azure => {
            bail!(
                "GitButler can't sign in to a self-hosted {} yet",
                forge.name()
            )
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

/// Who signed in, once the browser sign-in for `device_code` has finished; it stores the
/// account then. `None` while GitHub is still waiting, an error when it never will.
pub fn github_signed_in(device_code: &str) -> Result<Option<String>> {
    match runtime()
        .handle()
        .block_on(but_api::github::check_github_auth_status(
            device_code.to_owned(),
        )) {
        Ok(status) => Ok(Some(status.login)),
        // Expired, denied and rejected sign-ins carry a code; waiting, or a network blip,
        // doesn't, and is asked about again.
        Err(err) if err.downcast_ref::<but_error::Context>().is_some() => Err(err),
        Err(_) => Ok(None),
    }
}

/// Who is signed in to GitButler.
#[derive(Debug, Clone, PartialEq)]
pub struct GitButlerUser {
    /// Their name, else their login, else their email.
    pub name: String,
    pub login: Option<String>,
    pub email: Option<String>,
    /// Where their picture is, if they have one.
    pub picture: Option<String>,
}

impl From<but_api::legacy::users::UserProfile> for GitButlerUser {
    fn from(profile: but_api::legacy::users::UserProfile) -> Self {
        let name = (profile.name.clone())
            .or_else(|| profile.login.clone())
            .or_else(|| profile.email.clone())
            .unwrap_or_else(|| "your account".into());
        Self {
            name,
            login: profile.login,
            email: profile.email,
            picture: Some(profile.picture).filter(|url| !url.is_empty()),
        }
    }
}

/// Who is signed in to GitButler, if anyone. Reads the profile on disk only: no network,
/// no keychain.
pub fn gitbutler_user() -> Option<GitButlerUser> {
    Some(
        but_api::legacy::users::get_user_profile_local()
            .ok()??
            .into(),
    )
}

/// The picture at `url`, as an image to draw. A network that never answers gives up after
/// ten seconds rather than holding a thread.
pub fn avatar(url: &str) -> Result<std::sync::Arc<gpui::Image>> {
    picture(fetch(url)?)
}

/// What `url` serves, with [`avatar`]'s ten seconds to answer.
pub fn fetch(url: &str) -> Result<Vec<u8>> {
    // One client, reusing its connections, for every picture and check.
    static CLIENT: std::sync::LazyLock<reqwest::Client> =
        std::sync::LazyLock::new(reqwest::Client::new);
    runtime().handle().block_on(async {
        let request = CLIENT.get(url);
        let response = request.timeout(std::time::Duration::from_secs(10)).send();
        anyhow::Ok(response.await?.error_for_status()?.bytes().await?.to_vec())
    })
}

/// `bytes` as an image to draw, when they're a picture in a format GitButler draws.
pub fn picture(bytes: Vec<u8>) -> Result<std::sync::Arc<gpui::Image>> {
    use gpui::ImageFormat as F;
    let format = match bytes.as_slice() {
        [0x89, b'P', b'N', b'G', ..] => F::Png,
        [0xFF, 0xD8, ..] => F::Jpeg,
        [b'G', b'I', b'F', ..] => F::Gif,
        b if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP".as_slice()) => F::Webp,
        _ => anyhow::bail!("Not a picture GitButler can draw"),
    };
    Ok(std::sync::Arc::new(gpui::Image::from_bytes(format, bytes)))
}

/// The page to sign in to GitButler on. It shows a token to paste back here.
pub fn gitbutler_login_url() -> Result<String> {
    Ok(but_api::legacy::users::get_login_token()?.url)
}

pub fn gitbutler_log_in(token: &str) -> Result<GitButlerUser> {
    let user = but_api::legacy::users::login_and_persist(token.trim().to_owned())?;
    super::metrics::signed_in(&user);
    Ok(user.into())
}

pub fn gitbutler_log_out() -> Result<()> {
    but_api::legacy::users::delete_user()?;
    super::metrics::signed_out();
    Ok(())
}

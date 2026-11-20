//! GitButler Code: signing in, and the calls `but config forge _move` makes to move a
//! repository there. Plain functions, not `#[but_api]`: only the nightly CLI calls them, and
//! they stay out of the SDK until the app needs them.

use anyhow::{Context as _, Result, bail};
use but_core::git_config::{edit_repo_config, set_config_value};
use but_ctx::Context;
use but_gitbutler_code::{GitbutlerCodeAccountIdentifier, LoginStart, LoginStatus, Space};
use gitbutler_git::GitContextExt as _;

fn storage() -> Result<but_forge_storage::Controller> {
    Ok(but_forge_storage::Controller::from_path(
        but_path::app_data_dir()?,
    ))
}

/// Start GitButler Code's device login. The user approves `verification_url` in a browser,
/// then [`check_login`] finishes it.
pub async fn start_login() -> Result<LoginStart> {
    but_gitbutler_code::start_login().await
}

/// Ask whether the login of `device_code` was approved. Once it is, the account is stored and
/// Git's credential store has its token, so pushes to GitButler Code need no prompt.
pub async fn check_login(device_code: String) -> Result<LoginStatus> {
    but_gitbutler_code::check_login(&device_code, &storage()?).await
}

/// The GitButler Code accounts `but` signed in to.
pub fn list_known_accounts() -> Result<Vec<GitbutlerCodeAccountIdentifier>> {
    but_gitbutler_code::list_known_accounts(&storage()?)
}

/// Forget `account`: its stored token and the credential Git keeps for it.
pub fn forget_account(account: GitbutlerCodeAccountIdentifier) -> Result<()> {
    but_gitbutler_code::forget_account(&account, &storage()?)
}

/// Whether `account`'s stored token still signs in.
pub async fn check_credentials(
    account: GitbutlerCodeAccountIdentifier,
) -> Result<but_gitbutler_code::CredentialCheckResult> {
    but_gitbutler_code::check_credentials(&account, &storage()?).await
}

fn client(account: &GitbutlerCodeAccountIdentifier) -> Result<but_gitbutler_code::Client> {
    but_gitbutler_code::client(account, &storage()?)?.with_context(|| {
        format!("The GitButler Code token of {account} is gone; run `but config forge auth`")
    })
}

/// The username `account`'s token signs in as. Fails when the token was revoked.
pub async fn user(account: GitbutlerCodeAccountIdentifier) -> Result<String> {
    client(&account)?.user().await
}

/// `account`'s copy of the organization project `<org>/<name>`, made on the server when they
/// have none. `None` when `<org>/<name>` doesn't exist or they can't read it.
pub async fn space(
    account: GitbutlerCodeAccountIdentifier,
    org: String,
    name: String,
) -> Result<Option<Space>> {
    client(&account)?.space(&org, &name).await
}

/// Push `commit` as the `main` branch of `remote_name`, without force. Pushing `main` alone
/// to an organization project that doesn't exist yet creates it.
pub fn push_main(ctx: &Context, remote_name: &str, commit: gix::ObjectId) -> Result<()> {
    ctx.push(
        commit,
        format!("refs/remotes/{remote_name}/main"),
        false,
        false,
        Some(format!("{commit}:refs/heads/main")),
        Some(None),
        vec![],
    )?;
    Ok(())
}

/// Fetch `remote_name`.
pub fn fetch(ctx: &Context, remote_name: &str) -> Result<()> {
    ctx.fetch(remote_name, Some("forge move".into()))
}

/// Rename the remote `from` to `to` with Git's own `git remote rename`, which also moves its
/// remote-tracking refs and every branch's upstream setting. gix has no remote rename.
pub fn rename_remote(ctx: &Context, from: &str, to: &str) -> Result<()> {
    let output = std::process::Command::new(gix::path::env::exe_invocation())
        .current_dir(ctx.workdir_or_gitdir()?)
        .args(["remote", "rename", from, to])
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .context("Failed to run git remote rename")?;
    if !output.status.success() {
        bail!(
            "git remote rename {from} {to} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Remove the remote `name` with Git's own `git remote remove`, which also drops its
/// remote-tracking refs.
pub fn remove_remote(ctx: &Context, name: &str) -> Result<()> {
    let output = std::process::Command::new(gix::path::env::exe_invocation())
        .current_dir(ctx.workdir_or_gitdir()?)
        .args(["remote", "remove", name])
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .context("Failed to run git remote remove")?;
    if !output.status.success() {
        bail!(
            "git remote remove {name} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// Make the local branch `branch` track the branch of the same name on `remote_name`.
pub fn track(repo: &gix::Repository, branch: &str, remote_name: &str) -> Result<()> {
    edit_repo_config(repo, gix::config::Source::Local, |config| {
        set_config_value(config, &format!("branch.{branch}.remote"), remote_name)?;
        set_config_value(
            config,
            &format!("branch.{branch}.merge"),
            format!("refs/heads/{branch}"),
        )
    })?;
    Ok(())
}

/// No GitButler Code account to set up a copy with.
#[derive(Debug)]
pub struct NotSignedIn;

impl std::fmt::Display for NotSignedIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Not signed in to GitButler Code")
    }
}

impl std::error::Error for NotSignedIn {}

/// The one signed-in GitButler Code account. Fails with [`NotSignedIn`] when there is none.
pub fn signed_in_account() -> Result<GitbutlerCodeAccountIdentifier> {
    let mut accounts = list_known_accounts()?;
    match accounts.len() {
        0 => Err(NotSignedIn.into()),
        1 => Ok(accounts.remove(0)),
        _ => bail!(
            "Signed in to GitButler Code as more than one user; forget all but one with `but config forge forget <username>`"
        ),
    }
}

/// A copy set up as the push remote.
#[derive(Debug, Clone)]
pub struct Copy {
    /// The user's copy, as the server describes it.
    pub space: Space,
    /// The remote that points at it, named after the user.
    pub remote: String,
}

/// When the target's remote is a GitButler Code organization project and no push remote is set,
/// as in a fresh clone, set up the signed-in user's copy as the push remote with [`setup_copy`].
/// `None` when there's nothing to set up.
pub async fn ensure_copy(ctx: &mut Context) -> Result<Option<Copy>> {
    let (org, name) = {
        let meta = ctx.project_meta()?;
        if meta.push_remote.is_some() || meta.target_ref.is_none() {
            return Ok(None);
        }
        // A target without a remote has nothing to set up; `but push` says what's missing.
        let Ok(url) = meta.target_remote_url(&*ctx.repo.get()?) else {
            return Ok(None);
        };
        match but_forge::derive_forge_repo_info(&url, None) {
            Some(info)
                if info.forge == but_forge::ForgeName::GitButler && info.owner.contains('.') =>
            {
                (info.owner, info.repo)
            }
            _ => return Ok(None),
        }
    };
    let account = signed_in_account()?;
    setup_copy(ctx, &account, &org, &name).await.map(Some)
}

/// Make sure `account` has a copy of the organization project `<org>/<name>`, made on the
/// server with no upload, add a remote named after the user at its URL, and make that remote
/// the push remote, so `but push` publishes branches to the copy.
pub async fn setup_copy(
    ctx: &mut Context,
    account: &GitbutlerCodeAccountIdentifier,
    org: &str,
    name: &str,
) -> Result<Copy> {
    let client = client(account)?;
    let you = client.user().await?;
    let space = client.space(org, name).await?.with_context(|| {
        format!("{org}/{name} doesn't exist on GitButler Code, or you can't read it")
    })?;
    if space.upstream.as_deref() != Some(format!("{org}/{name}").as_str()) {
        bail!(
            "You have a project {} on GitButler Code that isn't a copy of {org}/{name}; rename or delete it there",
            space.project
        );
    }
    {
        let repo = ctx.repo.get()?;
        let existing = repo.find_remote(you.as_str()).ok().and_then(|remote| {
            remote
                .url(gix::remote::Direction::Fetch)
                .map(|url| url.to_bstring().to_string())
        });
        match existing {
            Some(url) if same_url(&url, &space.url) => {}
            Some(url) => bail!(
                "The remote {you} points at {url}, not your copy {}; rename or remove it",
                space.url
            ),
            None => crate::legacy::forge::add_remote_to_config(&repo, &you, &space.url)?,
        }
    }
    ctx.repo.get_mut()?.reload()?;
    crate::workspace::set_push_remote(ctx, you.clone())?;
    fetch(ctx, &you)?;
    Ok(Copy { space, remote: you })
}

/// Whether two remote URLs name the same repository, ignoring a trailing `/` or `.git`.
pub fn same_url(a: &str, b: &str) -> bool {
    let normalize = |url: &str| {
        url.trim_end_matches('/')
            .trim_end_matches(".git")
            .to_owned()
    };
    normalize(a) == normalize(b)
}

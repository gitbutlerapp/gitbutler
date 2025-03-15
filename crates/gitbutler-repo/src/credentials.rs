use std::{path::PathBuf, str::FromStr, vec};

use anyhow::Context;
use gitbutler_command_context::CommandContext;
use gitbutler_project::AuthKey;
use gitbutler_url::{ConvertError, Scheme, Url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshCredential {
    Keyfile {
        key_path: PathBuf,
        passphrase: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpsCredential {
    CredentialHelper { username: String, password: String },
    GitHubToken(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credential {
    Noop,
    Ssh(SshCredential),
    Https(HttpsCredential),
}

impl From<Credential> for git2::RemoteCallbacks<'_> {
    fn from(value: Credential) -> Self {
        let mut remote_callbacks = git2::RemoteCallbacks::new();
        match value {
            Credential::Noop => {}
            Credential::Ssh(SshCredential::Keyfile {
                key_path,
                passphrase,
            }) => {
                remote_callbacks.credentials(move |url, username_from_url, _allowed_types| {
                    use resolve_path::PathResolveExt;
                    let key_path = key_path.resolve();
                    tracing::info!(
                        "authenticating with {} using key {}",
                        url,
                        key_path.display()
                    );
                    git2::Cred::ssh_key(
                        username_from_url.unwrap_or("git"),
                        None,
                        &key_path,
                        passphrase.as_deref(),
                    )
                });
            }
            Credential::Https(HttpsCredential::CredentialHelper { username, password }) => {
                remote_callbacks.credentials(move |url, _username_from_url, _allowed_types| {
                    tracing::info!("authenticating with {url} as '{username}' with password using credential helper");
                    git2::Cred::userpass_plaintext(&username, &password)
                });
            }
            Credential::Https(HttpsCredential::GitHubToken(token)) => {
                remote_callbacks.credentials(move |url, _username_from_url, _allowed_types| {
                    tracing::info!("authenticating with {url} using github token");
                    git2::Cred::userpass_plaintext("git", &token)
                });
            }
        };
        remote_callbacks
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HelpError {
    #[error("no url set for remote")]
    NoUrlSet,
    #[error("failed to convert url: {0}")]
    UrlConvertError(#[from] ConvertError),
    #[error(transparent)]
    Git(#[from] git2::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub fn help<'a>(
    ctx: &'a CommandContext,
    remote_name: &str,
) -> Result<Vec<(git2::Remote<'a>, Vec<Credential>)>, HelpError> {
    let remote = ctx.repo().find_remote(remote_name)?;
    let remote_url = Url::from_str(remote.url().ok_or(HelpError::NoUrlSet)?)
        .context("failed to parse remote url")?;

    // if file, no auth needed.
    if remote_url.scheme == Scheme::File {
        return Ok(vec![(remote, vec![Credential::Noop])]);
    }

    match &ctx.project().preferred_key {
        AuthKey::Local { private_key_path } => {
            let ssh_remote = if remote_url.scheme == Scheme::Ssh {
                Ok(remote)
            } else {
                let ssh_url = remote_url.as_ssh()?;
                ctx.repo().remote_anonymous(&ssh_url.to_string())
            }?;

            Ok(vec![(
                ssh_remote,
                vec![Credential::Ssh(SshCredential::Keyfile {
                    key_path: private_key_path.clone(),
                    passphrase: None,
                })],
            )])
        }
        AuthKey::GitCredentialsHelper => {
            let https_remote = if remote_url.scheme == Scheme::Https {
                Ok(remote)
            } else {
                let url = remote_url.as_https()?;
                ctx.repo().remote_anonymous(&url.to_string())
            }?;
            let flow = https_flow(ctx, &remote_url)?
                .into_iter()
                .map(Credential::Https)
                .collect::<Vec<_>>();
            Ok(vec![(https_remote, flow)])
        }
        AuthKey::SystemExecutable => {
            tracing::error!(
                "WARNING: FIXME: this codepath should NEVER be hit. Something is seriously wrong."
            );
            Ok(vec![])
        }
    }
}

fn https_flow(ctx: &CommandContext, remote_url: &Url) -> Result<Vec<HttpsCredential>, HelpError> {
    let mut flow = vec![];

    let mut helper = git2::CredentialHelper::new(&remote_url.to_string());
    let config = ctx.repo().config()?;
    helper.config(&config);
    if let Some((username, password)) = helper.execute() {
        flow.push(HttpsCredential::CredentialHelper { username, password });
    }

    Ok(flow)
}

use std::str::FromStr;
use std::{path::PathBuf, vec};

use anyhow::Context;

use crate::{keys, project_repository, projects};

use super::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshCredential {
    Keyfile {
        key_path: PathBuf,
        passphrase: Option<String>,
    },
    GitButlerKey(Box<keys::PrivateKey>),
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
                remote_callbacks.credentials(move |url, _username_from_url, _allowed_types| {
                    use resolve_path::PathResolveExt;
                    let key_path = key_path.resolve();
                    tracing::info!(
                        "authenticating with {} using key {}",
                        url,
                        key_path.display()
                    );
                    git2::Cred::ssh_key("git", None, &key_path, passphrase.as_deref())
                });
            }
            Credential::Ssh(SshCredential::GitButlerKey(key)) => {
                remote_callbacks.credentials(move |url, _username_from_url, _allowed_types| {
                    tracing::info!("authenticating with {} using gitbutler's key", url);
                    git2::Cred::ssh_key_from_memory("git", None, &key.to_string(), None)
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

#[derive(Clone)]
pub struct Helper {
    keys: keys::Controller,
}

#[derive(Debug, thiserror::Error)]
pub enum HelpError {
    #[error("no url set for remote")]
    NoUrlSet,
    #[error("failed to convert url: {0}")]
    UrlConvertError(#[from] super::ConvertError),
    #[error(transparent)]
    Git(#[from] git2::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Helper {
    pub fn new(keys: keys::Controller) -> Self {
        Self { keys }
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let keys = keys::Controller::from_path(path);
        Self::new(keys)
    }

    pub fn help<'a>(
        &'a self,
        project_repository: &'a project_repository::Repository,
        remote_name: &str,
    ) -> Result<Vec<(git2::Remote, Vec<Credential>)>, HelpError> {
        let remote = project_repository.repo().find_remote(remote_name)?;
        let remote_url = Url::from_str(remote.url().ok_or(HelpError::NoUrlSet)?)
            .context("failed to parse remote url")?;

        // if file, no auth needed.
        if remote_url.scheme == super::Scheme::File {
            return Ok(vec![(remote, vec![Credential::Noop])]);
        }

        match &project_repository.project().preferred_key {
            projects::AuthKey::Local { private_key_path } => {
                let ssh_remote = if remote_url.scheme == super::Scheme::Ssh {
                    Ok(remote)
                } else {
                    let ssh_url = remote_url.as_ssh()?;
                    project_repository
                        .repo()
                        .remote_anonymous(&ssh_url.to_string())
                }?;

                Ok(vec![(
                    ssh_remote,
                    vec![Credential::Ssh(SshCredential::Keyfile {
                        key_path: private_key_path.clone(),
                        passphrase: None,
                    })],
                )])
            }
            projects::AuthKey::GitCredentialsHelper => {
                let https_remote = if remote_url.scheme == super::Scheme::Https {
                    Ok(remote)
                } else {
                    let url = remote_url.as_https()?;
                    project_repository.repo().remote_anonymous(&url.to_string())
                }?;
                let flow = Self::https_flow(project_repository, &remote_url)?
                    .into_iter()
                    .map(Credential::Https)
                    .collect::<Vec<_>>();
                Ok(vec![(https_remote, flow)])
            }
            projects::AuthKey::Generated => {
                let generated_flow = self.generated_flow(remote, project_repository)?;

                Ok(vec![generated_flow].into_iter().flatten().collect())
            }
            projects::AuthKey::SystemExecutable => {
                tracing::error!("WARNING: FIXME: this codepath should NEVER be hit. Something is seriously wrong.");
                Ok(vec![])
            }
        }
    }

    fn generated_flow<'a>(
        &'a self,
        remote: git2::Remote<'a>,
        project_repository: &'a project_repository::Repository,
    ) -> Result<Vec<(git2::Remote, Vec<Credential>)>, HelpError> {
        let remote_url = Url::from_str(remote.url().ok_or(HelpError::NoUrlSet)?)
            .context("failed to parse remote url")?;

        let ssh_remote = if remote_url.scheme == super::Scheme::Ssh {
            Ok(remote)
        } else {
            let ssh_url = remote_url.as_ssh()?;
            project_repository
                .repo()
                .remote_anonymous(&ssh_url.to_string())
        }?;

        let key = self.keys.get_or_create()?;
        Ok(vec![(
            ssh_remote,
            vec![Credential::Ssh(SshCredential::GitButlerKey(Box::new(key)))],
        )])
    }

    fn https_flow(
        project_repository: &project_repository::Repository,
        remote_url: &super::Url,
    ) -> Result<Vec<HttpsCredential>, HelpError> {
        let mut flow = vec![];

        let mut helper = git2::CredentialHelper::new(&remote_url.to_string());
        let config = project_repository.repo().config()?;
        helper.config(&config);
        if let Some((username, password)) = helper.execute() {
            flow.push(HttpsCredential::CredentialHelper { username, password });
        }

        Ok(flow)
    }
}

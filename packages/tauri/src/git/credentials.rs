use std::{env, path};

use tauri::AppHandle;

use crate::{keys, paths, project_repository, projects, users};

#[derive(Debug, Clone)]
pub enum SshCredential {
    Keyfile {
        key_path: path::PathBuf,
        passphrase: Option<String>,
    },
    GitButlerKey(Box<keys::PrivateKey>),
}

#[derive(Debug, Clone)]
pub enum HttpsCredential {
    UsernamePassword { username: String, password: String },
    CredentialHelper { username: String, password: String },
    GitHubToken(String),
}

#[derive(Debug, Clone)]
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
            Credential::Https(HttpsCredential::UsernamePassword { username, password }) => {
                remote_callbacks.credentials(move |url, _username_from_url, _allowed_types| {
                    tracing::info!("authenticating with {url} as '{username}' with password");
                    git2::Cred::userpass_plaintext(&username, &password)
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
    users: users::Controller,
}

impl From<&AppHandle> for Helper {
    fn from(value: &AppHandle) -> Self {
        Self {
            keys: keys::Controller::from(value),
            users: users::Controller::from(value),
        }
    }
}

impl From<&paths::DataDir> for Helper {
    fn from(value: &paths::DataDir) -> Self {
        Self {
            keys: keys::Controller::from(value),
            users: users::Controller::from(value),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HelpError {
    #[error("no url set for remote")]
    NoUrlSet,
    #[error("failed to convert url: {0}")]
    UrlConvertError(#[from] super::ConvertError),
    #[error(transparent)]
    Users(#[from] users::GetError),
    #[error(transparent)]
    Key(#[from] keys::GetOrCreateError),
    #[error(transparent)]
    Git(#[from] super::Error),
}

impl From<HelpError> for crate::error::Error {
    fn from(value: HelpError) -> Self {
        match value {
            HelpError::NoUrlSet => Self::UserError {
                code: crate::error::Code::ProjectGitRemote,
                message: "no url set for remote".to_string(),
            },
            HelpError::UrlConvertError(error) => Self::UserError {
                code: crate::error::Code::ProjectGitRemote,
                message: error.to_string(),
            },
            HelpError::Users(error) => error.into(),
            HelpError::Key(error) => error.into(),
            HelpError::Git(error) => {
                tracing::error!(?error, "failed to create auth credentials");
                Self::Unknown
            }
        }
    }
}

impl Helper {
    /// returns all possible credentials for a remote, without trying to be smart.
    pub fn enumerate<'a>(
        &'a self,
        project_repository: &'a project_repository::Repository,
        remote: &str,
    ) -> Result<Vec<(super::Remote, Vec<Credential>)>, HelpError> {
        let remote = project_repository.git_repository.find_remote(remote)?;
        let remote_url = remote.url()?.ok_or(HelpError::NoUrlSet)?;

        let mut flow = vec![];

        if let projects::AuthKey::Local {
            private_key_path,
            passphrase,
        } = &project_repository.project().preferred_key
        {
            let ssh_remote = if remote_url.scheme == super::Scheme::Ssh {
                project_repository
                    .git_repository
                    .remote_anonymous(&remote_url)
            } else {
                let ssh_url = remote_url.as_ssh()?;
                project_repository.git_repository.remote_anonymous(&ssh_url)
            }?;
            flow.push((
                ssh_remote,
                vec![Credential::Ssh(SshCredential::Keyfile {
                    key_path: private_key_path
                        .canonicalize()
                        .unwrap_or(private_key_path.clone()),
                    passphrase: passphrase.clone(),
                })],
            ));
        }

        // is github is authenticated, only try github.
        if remote_url.is_github() {
            if let Some(github_access_token) = self
                .users
                .get_user()?
                .and_then(|user| user.github_access_token)
            {
                let https_remote = if remote_url.scheme == super::Scheme::Https {
                    project_repository
                        .git_repository
                        .remote_anonymous(&remote_url)
                } else {
                    let ssh_url = remote_url.as_ssh()?;
                    project_repository.git_repository.remote_anonymous(&ssh_url)
                }?;
                flow.push((
                    https_remote,
                    vec![Credential::Https(HttpsCredential::GitHubToken(
                        github_access_token,
                    ))],
                ));
            }
        }

        if let Ok(https_url) = remote_url.as_https() {
            flow.push((
                project_repository
                    .git_repository
                    .remote_anonymous(&https_url)?,
                Self::https_flow(project_repository, &https_url)?
                    .into_iter()
                    .map(Credential::Https)
                    .collect(),
            ));
        }

        if let Ok(ssh_url) = remote_url.as_ssh() {
            flow.push((
                project_repository
                    .git_repository
                    .remote_anonymous(&ssh_url)?,
                self.ssh_flow()?.into_iter().map(Credential::Ssh).collect(),
            ));
        }

        Ok(flow)
    }

    pub fn help<'a>(
        &'a self,
        project_repository: &'a project_repository::Repository,
        remote: &str,
    ) -> Result<Vec<(super::Remote, Vec<Credential>)>, HelpError> {
        let remote = project_repository.git_repository.find_remote(remote)?;
        let remote_url = remote.url()?.ok_or(HelpError::NoUrlSet)?;

        // if file, no auth needed.
        if remote_url.scheme == super::Scheme::File {
            return Ok(vec![(remote, vec![Credential::Noop])]);
        }

        // if prefernce set, only try that.
        if let projects::AuthKey::Local {
            private_key_path,
            passphrase,
        } = &project_repository.project().preferred_key
        {
            let ssh_remote = if remote_url.scheme == super::Scheme::Ssh {
                Ok(remote)
            } else {
                let ssh_url = remote_url.as_ssh()?;
                project_repository.git_repository.remote_anonymous(&ssh_url)
            }?;

            return Ok(vec![(
                ssh_remote,
                vec![Credential::Ssh(SshCredential::Keyfile {
                    key_path: private_key_path
                        .canonicalize()
                        .unwrap_or(private_key_path.clone()),
                    passphrase: passphrase.clone(),
                })],
            )]);
        }

        // is github is authenticated, only try github.
        if remote_url.is_github() {
            if let Some(github_access_token) = self
                .users
                .get_user()?
                .and_then(|user| user.github_access_token)
            {
                let https_remote = if remote_url.scheme == super::Scheme::Https {
                    Ok(remote)
                } else {
                    let ssh_url = remote_url.as_ssh()?;
                    project_repository.git_repository.remote_anonymous(&ssh_url)
                }?;
                return Ok(vec![(
                    https_remote,
                    vec![Credential::Https(HttpsCredential::UsernamePassword {
                        username: "git".to_string(),
                        password: github_access_token,
                    })],
                )]);
            }
        }

        match remote_url.scheme {
            super::Scheme::Https => {
                let mut flow = vec![(
                    remote,
                    Self::https_flow(project_repository, &remote_url)?
                        .into_iter()
                        .map(Credential::Https)
                        .collect(),
                )];

                if let Ok(ssh_url) = remote_url.as_ssh() {
                    flow.push((
                        project_repository
                            .git_repository
                            .remote_anonymous(&ssh_url)?,
                        self.ssh_flow()?.into_iter().map(Credential::Ssh).collect(),
                    ));
                }

                Ok(flow)
            }
            super::Scheme::Ssh => {
                let mut flow = vec![(
                    remote,
                    self.ssh_flow()?.into_iter().map(Credential::Ssh).collect(),
                )];

                if let Ok(https_url) = remote_url.as_https() {
                    flow.push((
                        project_repository
                            .git_repository
                            .remote_anonymous(&https_url)?,
                        Self::https_flow(project_repository, &https_url)?
                            .into_iter()
                            .map(Credential::Https)
                            .collect(),
                    ));
                }

                Ok(flow)
            }
            _ => {
                let mut flow = vec![];

                if let Ok(https_url) = remote_url.as_https() {
                    flow.push((
                        project_repository
                            .git_repository
                            .remote_anonymous(&https_url)?,
                        Self::https_flow(project_repository, &https_url)?
                            .into_iter()
                            .map(Credential::Https)
                            .collect(),
                    ));
                }

                if let Ok(ssh_url) = remote_url.as_ssh() {
                    flow.push((
                        project_repository
                            .git_repository
                            .remote_anonymous(&ssh_url)?,
                        self.ssh_flow()?.into_iter().map(Credential::Ssh).collect(),
                    ));
                }

                Ok(flow)
            }
        }
    }

    fn https_flow(
        project_repository: &project_repository::Repository,
        remote_url: &super::Url,
    ) -> Result<Vec<HttpsCredential>, HelpError> {
        let mut flow = vec![];

        let mut helper = git2::CredentialHelper::new(&remote_url.to_string());
        let config = project_repository.git_repository.config()?;
        helper.config(&git2::Config::from(config));
        if let Some((username, password)) = helper.execute() {
            flow.push(HttpsCredential::CredentialHelper { username, password });
        }

        Ok(flow)
    }

    fn ssh_flow(&self) -> Result<Vec<SshCredential>, HelpError> {
        let mut flow = vec![];
        if let Ok(home_path) = env::var("HOME") {
            let home_path = std::path::Path::new(&home_path);

            let id_rsa_path = home_path.join(".ssh").join("id_rsa");
            let id_rsa_path = id_rsa_path.canonicalize().unwrap_or(id_rsa_path);
            if id_rsa_path.exists() {
                flow.push(SshCredential::Keyfile {
                    key_path: id_rsa_path.clone(),
                    passphrase: None,
                });
            }

            let id_ed25519_path = home_path.join(".ssh").join("id_ed25519");
            let id_ed25519_path = id_ed25519_path.canonicalize().unwrap_or(id_ed25519_path);
            if id_ed25519_path.exists() {
                flow.push(SshCredential::Keyfile {
                    key_path: id_ed25519_path.clone(),
                    passphrase: None,
                });
            }

            let id_ecdsa_path = home_path.join(".ssh").join("id_ecdsa");
            let id_ecdsa_path = id_ecdsa_path.canonicalize().unwrap_or(id_ecdsa_path);
            if id_ecdsa_path.exists() {
                flow.push(SshCredential::Keyfile {
                    key_path: id_ecdsa_path.clone(),
                    passphrase: None,
                });
            }
        }

        let key = self.keys.get_or_create()?;
        flow.push(SshCredential::GitButlerKey(Box::new(key)));
        Ok(flow)
    }
}

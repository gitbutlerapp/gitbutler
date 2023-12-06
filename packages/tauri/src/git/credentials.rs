use std::{env, path};

use tauri::AppHandle;

use crate::{keys, paths, project_repository, projects, users};

pub enum Credential {
    Noop,
    SshKey {
        key_path: path::PathBuf,
        passphrase: Option<String>,
    },
    GitButlerKey(Box<keys::PrivateKey>),
    Password {
        username: String,
        password: String,
    },
}

impl From<Credential> for git2::RemoteCallbacks<'_> {
    fn from(value: Credential) -> Self {
        let mut remote_callbacks = git2::RemoteCallbacks::new();
        match value {
            Credential::Noop => {}
            Credential::SshKey {
                key_path,
                passphrase,
            } => {
                remote_callbacks.credentials(move |url, _username_from_url, _allowed_types| {
                    tracing::info!(
                        "authenticating with {} using key {}",
                        url,
                        key_path.display()
                    );
                    git2::Cred::ssh_key("git", None, &key_path, passphrase.as_deref())
                });
            }
            Credential::GitButlerKey(key) => {
                remote_callbacks.credentials(move |url, _username_from_url, _allowed_types| {
                    tracing::info!("authenticating with {} using gitbutler's key", url);
                    git2::Cred::ssh_key_from_memory("git", None, &key.to_string(), None)
                });
            }
            Credential::Password { username, password } => {
                remote_callbacks.credentials(move |url, _username_from_url, _allowed_types| {
                    tracing::info!("authenticating with {} using username / password", url);
                    git2::Cred::userpass_plaintext(&username, &password)
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
    pub fn help<'a>(
        &'a self,
        project_repository: &'a project_repository::Repository,
        remote: &str,
    ) -> Result<Vec<(super::Remote, Vec<Credential>)>, HelpError> {
        let remote = project_repository.git_repository.find_remote(remote)?;
        let remote_url = remote.url()?.ok_or(HelpError::NoUrlSet)?;

        match remote_url.scheme {
            super::Scheme::File => Ok(vec![(remote, vec![Credential::Noop])]),
            super::Scheme::Https => {
                let mut flow = vec![(remote, self.https_flow(project_repository, &remote_url)?)];

                if let Ok(ssh_url) = remote_url.as_ssh() {
                    flow.push((
                        project_repository
                            .git_repository
                            .remote_anonymous(&ssh_url)?,
                        self.gitbutler_key_flow()?,
                    ));
                }

                Ok(flow)
            }
            super::Scheme::Ssh => Ok(vec![(
                remote,
                Self::user_keys_flow(project_repository)
                    .into_iter()
                    .chain(self.gitbutler_key_flow()?)
                    .collect(),
            )]),
            _ => {
                let mut flow = vec![];

                if let Ok(https_url) = remote_url.as_https() {
                    flow.push((
                        project_repository
                            .git_repository
                            .remote_anonymous(&https_url)?,
                        self.https_flow(project_repository, &https_url)?,
                    ));
                }

                if let Ok(ssh_url) = remote_url.as_ssh() {
                    flow.push((
                        project_repository
                            .git_repository
                            .remote_anonymous(&ssh_url)?,
                        Self::user_keys_flow(project_repository)
                            .into_iter()
                            .chain(self.gitbutler_key_flow()?)
                            .collect(),
                    ));
                }

                Ok(flow)
            }
        }
    }

    fn https_flow(
        &self,
        project_repository: &project_repository::Repository,
        remote_url: &super::Url,
    ) -> Result<Vec<Credential>, HelpError> {
        let mut flow = vec![];

        if remote_url.is_github() {
            if let Some(github_access_token) = self
                .users
                .get_user()?
                .and_then(|user| user.github_access_token)
            {
                flow.push(Credential::Password {
                    username: "git".to_string(),
                    password: github_access_token,
                });
            }
        }

        let mut helper = git2::CredentialHelper::new(&remote_url.to_string());
        let config = project_repository.git_repository.config()?;
        helper.config(&git2::Config::from(config));
        if let Some((username, password)) = helper.execute() {
            flow.push(Credential::Password { username, password });
        }

        Ok(flow)
    }

    fn user_keys_flow(project_repository: &project_repository::Repository) -> Vec<Credential> {
        match &project_repository.project().preferred_key {
            projects::AuthKey::Local {
                private_key_path,
                passphrase,
            } => vec![Credential::SshKey {
                key_path: private_key_path.clone(),
                passphrase: passphrase.clone(),
            }],
            projects::AuthKey::Generated => {
                let mut flow = vec![];
                if let Ok(home_path) = env::var("HOME") {
                    let home_path = std::path::Path::new(&home_path);

                    let id_rsa_path = home_path.join(".ssh").join("id_rsa");
                    if id_rsa_path.exists() {
                        flow.push(Credential::SshKey {
                            key_path: id_rsa_path.clone(),
                            passphrase: None,
                        });
                    }

                    let id_ed25519_path = home_path.join(".ssh").join("id_ed25519");
                    if id_ed25519_path.exists() {
                        flow.push(Credential::SshKey {
                            key_path: id_ed25519_path.clone(),
                            passphrase: None,
                        });
                    }

                    let id_ecdsa_path = home_path.join(".ssh").join("id_ecdsa");
                    if id_ecdsa_path.exists() {
                        flow.push(Credential::SshKey {
                            key_path: id_ecdsa_path.clone(),
                            passphrase: None,
                        });
                    }
                }
                flow
            }
        }
    }

    fn gitbutler_key_flow(&self) -> Result<Vec<Credential>, HelpError> {
        let key = self.keys.get_or_create()?;
        Ok(vec![Credential::GitButlerKey(Box::new(key))])
    }
}

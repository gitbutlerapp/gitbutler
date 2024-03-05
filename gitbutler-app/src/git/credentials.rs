use std::{env, path};

use tauri::{AppHandle, Manager};

use crate::{keys, project_repository, projects, users};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshCredential {
    Keyfile {
        key_path: path::PathBuf,
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
    users: users::Controller,
    home_dir: Option<path::PathBuf>,
}

impl TryFrom<&AppHandle> for Helper {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(value.state::<Self>().inner().clone())
    }
}

impl TryFrom<&std::path::PathBuf> for Helper {
    type Error = anyhow::Error;

    fn try_from(value: &std::path::PathBuf) -> Result<Self, Self::Error> {
        let keys = keys::Controller::try_from(value)?;
        let users = users::Controller::try_from(value)?;
        let home_dir = env::var_os("HOME").map(path::PathBuf::from);
        Ok(Helper::new(keys, users, home_dir))
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
    pub fn new(
        keys: keys::Controller,
        users: users::Controller,
        home_dir: Option<path::PathBuf>,
    ) -> Self {
        Self {
            keys,
            users,
            home_dir,
        }
    }

    pub fn help<'a>(
        &'a self,
        project_repository: &'a project_repository::Repository,
        remote_name: &str,
    ) -> Result<Vec<(super::Remote, Vec<Credential>)>, HelpError> {
        let remote = project_repository.git_repository.find_remote(remote_name)?;
        let remote_url = remote.url()?.ok_or(HelpError::NoUrlSet)?;

        // if file, no auth needed.
        if remote_url.scheme == super::Scheme::File {
            return Ok(vec![(remote, vec![Credential::Noop])]);
        }

        match &project_repository.project().preferred_key {
            projects::AuthKey::Local {
                private_key_path,
                passphrase,
            } => {
                let ssh_remote = if remote_url.scheme == super::Scheme::Ssh {
                    Ok(remote)
                } else {
                    let ssh_url = remote_url.as_ssh()?;
                    project_repository.git_repository.remote_anonymous(&ssh_url)
                }?;

                Ok(vec![(
                    ssh_remote,
                    vec![Credential::Ssh(SshCredential::Keyfile {
                        key_path: private_key_path.clone(),
                        passphrase: passphrase.clone(),
                    })],
                )])
            }
            projects::AuthKey::GitCredentialsHelper => {
                let https_remote = if remote_url.scheme == super::Scheme::Https {
                    Ok(remote)
                } else {
                    let url = remote_url.as_https()?;
                    project_repository.git_repository.remote_anonymous(&url)
                }?;
                let flow = Self::https_flow(project_repository, &remote_url)?
                    .into_iter()
                    .map(Credential::Https)
                    .collect::<Vec<_>>();
                Ok(vec![(https_remote, flow)])
            }
            projects::AuthKey::Generated => {
                let generated_flow = self.generated_flow(remote, project_repository)?;

                let remote = project_repository.git_repository.find_remote(remote_name)?;
                let default_flow = self.default_flow(remote, project_repository)?;

                Ok(vec![generated_flow, default_flow]
                    .into_iter()
                    .flatten()
                    .collect())
            }
            projects::AuthKey::Default => self.default_flow(remote, project_repository),
        }
    }

    fn generated_flow<'a>(
        &'a self,
        remote: super::Remote<'a>,
        project_repository: &'a project_repository::Repository,
    ) -> Result<Vec<(super::Remote, Vec<Credential>)>, HelpError> {
        let remote_url = remote.url()?.ok_or(HelpError::NoUrlSet)?;

        let ssh_remote = if remote_url.scheme == super::Scheme::Ssh {
            Ok(remote)
        } else {
            let ssh_url = remote_url.as_ssh()?;
            project_repository.git_repository.remote_anonymous(&ssh_url)
        }?;

        let key = self.keys.get_or_create()?;
        Ok(vec![(
            ssh_remote,
            vec![Credential::Ssh(SshCredential::GitButlerKey(Box::new(key)))],
        )])
    }

    fn default_flow<'a>(
        &'a self,
        remote: super::Remote<'a>,
        project_repository: &'a project_repository::Repository,
    ) -> Result<Vec<(super::Remote, Vec<Credential>)>, HelpError> {
        let remote_url = remote.url()?.ok_or(HelpError::NoUrlSet)?;

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
                    let url = remote_url.as_https()?;
                    project_repository.git_repository.remote_anonymous(&url)
                }?;
                return Ok(vec![(
                    https_remote,
                    vec![Credential::Https(HttpsCredential::GitHubToken(
                        github_access_token,
                    ))],
                )]);
            }
        }

        match remote_url.scheme {
            super::Scheme::Https => {
                let mut flow = vec![];

                let https_flow = Self::https_flow(project_repository, &remote_url)?
                    .into_iter()
                    .map(Credential::Https)
                    .collect::<Vec<_>>();

                if !https_flow.is_empty() {
                    flow.push((remote, https_flow));
                }

                if let Ok(ssh_url) = remote_url.as_ssh() {
                    let ssh_flow = self
                        .ssh_flow()?
                        .into_iter()
                        .map(Credential::Ssh)
                        .collect::<Vec<_>>();
                    if !ssh_flow.is_empty() {
                        flow.push((
                            project_repository
                                .git_repository
                                .remote_anonymous(&ssh_url)?,
                            ssh_flow,
                        ));
                    }
                }

                Ok(flow)
            }
            super::Scheme::Ssh => {
                let mut flow = vec![];

                let ssh_flow = self
                    .ssh_flow()?
                    .into_iter()
                    .map(Credential::Ssh)
                    .collect::<Vec<_>>();
                if !ssh_flow.is_empty() {
                    flow.push((remote, ssh_flow));
                }

                if let Ok(https_url) = remote_url.as_https() {
                    let https_flow = Self::https_flow(project_repository, &https_url)?
                        .into_iter()
                        .map(Credential::Https)
                        .collect::<Vec<_>>();
                    if !https_flow.is_empty() {
                        flow.push((
                            project_repository
                                .git_repository
                                .remote_anonymous(&https_url)?,
                            https_flow,
                        ));
                    }
                }

                Ok(flow)
            }
            _ => {
                let mut flow = vec![];

                if let Ok(https_url) = remote_url.as_https() {
                    let https_flow = Self::https_flow(project_repository, &https_url)?
                        .into_iter()
                        .map(Credential::Https)
                        .collect::<Vec<_>>();

                    if !https_flow.is_empty() {
                        flow.push((
                            project_repository
                                .git_repository
                                .remote_anonymous(&https_url)?,
                            https_flow,
                        ));
                    }
                }

                if let Ok(ssh_url) = remote_url.as_ssh() {
                    let ssh_flow = self
                        .ssh_flow()?
                        .into_iter()
                        .map(Credential::Ssh)
                        .collect::<Vec<_>>();
                    if !ssh_flow.is_empty() {
                        flow.push((
                            project_repository
                                .git_repository
                                .remote_anonymous(&ssh_url)?,
                            ssh_flow,
                        ));
                    }
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
        if let Some(home_path) = self.home_dir.as_ref() {
            let id_rsa_path = home_path.join(".ssh").join("id_rsa");
            if id_rsa_path.exists() {
                flow.push(SshCredential::Keyfile {
                    key_path: id_rsa_path.clone(),
                    passphrase: None,
                });
            }

            let id_ed25519_path = home_path.join(".ssh").join("id_ed25519");
            if id_ed25519_path.exists() {
                flow.push(SshCredential::Keyfile {
                    key_path: id_ed25519_path.clone(),
                    passphrase: None,
                });
            }

            let id_ecdsa_path = home_path.join(".ssh").join("id_ecdsa");
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::tests::{self, test_repository};

    #[derive(Default)]
    struct TestCase<'a> {
        remote_url: &'a str,
        github_access_token: Option<&'a str>,
        preferred_key: projects::AuthKey,
        home_dir: Option<path::PathBuf>,
    }

    impl TestCase<'_> {
        fn run(&self) -> Vec<(String, Vec<Credential>)> {
            let local_app_data = tests::temp_dir();

            let users = users::Controller::try_from(&local_app_data).unwrap();
            let user = users::User {
                github_access_token: self.github_access_token.map(ToString::to_string),
                ..Default::default()
            };
            users.set_user(&user).unwrap();

            let keys = keys::Controller::try_from(&local_app_data).unwrap();
            let helper = Helper::new(keys, users, self.home_dir.clone());

            let repo = test_repository();
            repo.remote(
                "origin",
                &self.remote_url.parse().expect("failed to parse remote url"),
            )
            .unwrap();
            let project = projects::Project {
                path: repo.workdir().unwrap().to_path_buf(),
                preferred_key: self.preferred_key.clone(),
                ..Default::default()
            };
            let project_repository = project_repository::Repository::open(&project).unwrap();

            let flow = helper.help(&project_repository, "origin").unwrap();
            flow.into_iter()
                .map(|(remote, credentials)| {
                    (
                        remote.url().unwrap().as_ref().unwrap().to_string(),
                        credentials,
                    )
                })
                .collect::<Vec<_>>()
        }
    }

    mod not_github {
        use super::*;

        mod with_preferred_key {
            use super::*;

            #[test]
            fn https() {
                let test_case = TestCase {
                    remote_url: "https://gitlab.com/test-gitbutler/test.git",
                    github_access_token: Some("token"),
                    preferred_key: projects::AuthKey::Local {
                        private_key_path: path::PathBuf::from("/tmp/id_rsa"),
                        passphrase: None,
                    },
                    ..Default::default()
                };
                let flow = test_case.run();
                assert_eq!(flow.len(), 1);
                assert_eq!(
                    flow[0].0,
                    "git@gitlab.com:test-gitbutler/test.git".to_string(),
                );
                assert_eq!(
                    flow[0].1,
                    vec![Credential::Ssh(SshCredential::Keyfile {
                        key_path: path::PathBuf::from("/tmp/id_rsa"),
                        passphrase: None,
                    })]
                );
            }

            #[test]
            fn ssh() {
                let test_case = TestCase {
                    remote_url: "git@gitlab.com:test-gitbutler/test.git",
                    github_access_token: Some("token"),
                    preferred_key: projects::AuthKey::Local {
                        private_key_path: path::PathBuf::from("/tmp/id_rsa"),
                        passphrase: None,
                    },
                    ..Default::default()
                };
                let flow = test_case.run();
                assert_eq!(flow.len(), 1);
                assert_eq!(
                    flow[0].0,
                    "git@gitlab.com:test-gitbutler/test.git".to_string(),
                );
                assert_eq!(
                    flow[0].1,
                    vec![Credential::Ssh(SshCredential::Keyfile {
                        key_path: path::PathBuf::from("/tmp/id_rsa"),
                        passphrase: None,
                    })]
                );
            }
        }

        mod with_github_token {
            use super::*;

            #[test]
            fn https() {
                let test_case = TestCase {
                    remote_url: "https://gitlab.com/test-gitbutler/test.git",
                    github_access_token: Some("token"),
                    ..Default::default()
                };
                let flow = test_case.run();

                assert_eq!(flow.len(), 1);

                assert_eq!(
                    flow[0].0,
                    "git@gitlab.com:test-gitbutler/test.git".to_string(),
                );
                assert_eq!(flow[0].1.len(), 1);
                assert!(matches!(
                    flow[0].1[0],
                    Credential::Ssh(SshCredential::GitButlerKey(_))
                ));
            }

            #[test]
            fn ssh() {
                let test_case = TestCase {
                    remote_url: "git@gitlab.com:test-gitbutler/test.git",
                    github_access_token: Some("token"),
                    ..Default::default()
                };
                let flow = test_case.run();

                assert_eq!(flow.len(), 1);

                assert_eq!(
                    flow[0].0,
                    "git@gitlab.com:test-gitbutler/test.git".to_string(),
                );
                assert_eq!(flow[0].1.len(), 1);
                assert!(matches!(
                    flow[0].1[0],
                    Credential::Ssh(SshCredential::GitButlerKey(_))
                ));
            }
        }
    }

    mod github {
        use super::*;

        mod with_github_token {
            use super::*;

            #[test]
            fn https() {
                let test_case = TestCase {
                    remote_url: "https://github.com/gitbutlerapp/gitbutler.git",
                    github_access_token: Some("token"),
                    ..Default::default()
                };
                let flow = test_case.run();
                assert_eq!(flow.len(), 1);
                assert_eq!(
                    flow[0].0,
                    "https://github.com/gitbutlerapp/gitbutler.git".to_string(),
                );
                assert_eq!(
                    flow[0].1,
                    vec![Credential::Https(HttpsCredential::GitHubToken(
                        "token".to_string()
                    ))]
                );
            }

            #[test]
            fn ssh() {
                let test_case = TestCase {
                    remote_url: "git@github.com:gitbutlerapp/gitbutler.git",
                    github_access_token: Some("token"),
                    ..Default::default()
                };
                let flow = test_case.run();
                assert_eq!(flow.len(), 1);
                assert_eq!(
                    flow[0].0,
                    "https://github.com/gitbutlerapp/gitbutler.git".to_string(),
                );
                assert_eq!(
                    flow[0].1,
                    vec![Credential::Https(HttpsCredential::GitHubToken(
                        "token".to_string()
                    ))]
                );
            }
        }

        mod without_github_token {
            use super::*;

            mod without_preferred_key {
                use super::*;

                #[test]
                fn https() {
                    let test_case = TestCase {
                        remote_url: "https://github.com/gitbutlerapp/gitbutler.git",
                        ..Default::default()
                    };
                    let flow = test_case.run();

                    assert_eq!(flow.len(), 1);

                    assert_eq!(
                        flow[0].0,
                        "git@github.com:gitbutlerapp/gitbutler.git".to_string(),
                    );
                    assert_eq!(flow[0].1.len(), 1);
                    assert!(matches!(
                        flow[0].1[0],
                        Credential::Ssh(SshCredential::GitButlerKey(_))
                    ));
                }

                #[test]
                fn ssh() {
                    let test_case = TestCase {
                        remote_url: "git@github.com:gitbutlerapp/gitbutler.git",
                        ..Default::default()
                    };
                    let flow = test_case.run();

                    assert_eq!(flow.len(), 1);

                    assert_eq!(
                        flow[0].0,
                        "git@github.com:gitbutlerapp/gitbutler.git".to_string(),
                    );
                    assert_eq!(flow[0].1.len(), 1);
                    assert!(matches!(
                        flow[0].1[0],
                        Credential::Ssh(SshCredential::GitButlerKey(_))
                    ));
                }
            }

            mod with_preferred_key {
                use super::*;

                #[test]
                fn https() {
                    let test_case = TestCase {
                        remote_url: "https://github.com/gitbutlerapp/gitbutler.git",
                        github_access_token: Some("token"),
                        preferred_key: projects::AuthKey::Local {
                            private_key_path: path::PathBuf::from("/tmp/id_rsa"),
                            passphrase: None,
                        },
                        ..Default::default()
                    };
                    let flow = test_case.run();
                    assert_eq!(flow.len(), 1);
                    assert_eq!(
                        flow[0].0,
                        "git@github.com:gitbutlerapp/gitbutler.git".to_string(),
                    );
                    assert_eq!(
                        flow[0].1,
                        vec![Credential::Ssh(SshCredential::Keyfile {
                            key_path: path::PathBuf::from("/tmp/id_rsa"),
                            passphrase: None,
                        })]
                    );
                }

                #[test]
                fn ssh() {
                    let test_case = TestCase {
                        remote_url: "git@github.com:gitbutlerapp/gitbutler.git",
                        github_access_token: Some("token"),
                        preferred_key: projects::AuthKey::Local {
                            private_key_path: path::PathBuf::from("/tmp/id_rsa"),
                            passphrase: None,
                        },
                        ..Default::default()
                    };
                    let flow = test_case.run();
                    assert_eq!(flow.len(), 1);
                    assert_eq!(
                        flow[0].0,
                        "git@github.com:gitbutlerapp/gitbutler.git".to_string(),
                    );
                    assert_eq!(
                        flow[0].1,
                        vec![Credential::Ssh(SshCredential::Keyfile {
                            key_path: path::PathBuf::from("/tmp/id_rsa"),
                            passphrase: None,
                        })]
                    );
                }
            }
        }
    }
}

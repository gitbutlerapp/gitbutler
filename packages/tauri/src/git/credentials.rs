use std::{env, path};

use resolve_path::PathResolveExt;

use crate::{keys, projects, users};

pub struct Factory {
    preferred_key: projects::AuthKey,
    key: keys::PrivateKey,
    github_token: Option<String>,
}

impl Factory {
    pub fn new(
        project: &projects::Project,
        key: keys::PrivateKey,
        user: Option<&users::User>,
    ) -> Factory {
        Factory {
            preferred_key: project.preferred_key.clone(),
            key,
            github_token: user.and_then(|user| user.github_access_token.clone()),
        }
    }

    pub fn has_github_token(&self) -> bool {
        self.github_token.is_some()
    }

    pub fn for_remote(
        &self,
        remote: &super::Remote,
        config: super::Config,
    ) -> Vec<CredentialsCallback> {
        let is_github = remote
            .url()
            .map(|url| url.map_or(false, |url| url.is_github()))
            .unwrap_or(false);

        let is_https = remote
            .url()
            .map(|url| url.map_or(false, |url| url.scheme == super::Scheme::Https))
            .unwrap_or(false);

        if is_github && is_https {
            if let Some(github_token) = self.github_token.as_ref() {
                return vec![from_token(github_token)];
            }
        }

        if is_https {
            if let Some(url) = remote.url_as_str().ok().flatten() {
                if let Some(credentials) = invoke_credential_helper(url, config) {
                    return vec![from_plaintext(credentials.0, credentials.1)];
                }
            }
        }

        match &self.preferred_key {
            projects::AuthKey::Local {
                private_key_path,
                passphrase,
            } => vec![from_keypath(
                private_key_path.clone(),
                passphrase.as_deref(),
            )],
            projects::AuthKey::Generated => {
                let mut credentials = vec![];
                if let Ok(home_path) = env::var("HOME") {
                    let home_path = std::path::Path::new(&home_path);

                    let id_rsa_path = home_path.join(".ssh").join("id_rsa");
                    if id_rsa_path.exists() {
                        credentials.push(from_keypath(id_rsa_path.clone(), None));
                    }

                    let id_ed25519_path = home_path.join(".ssh").join("id_ed25519");
                    if id_ed25519_path.exists() {
                        credentials.push(from_keypath(id_ed25519_path.clone(), None));
                    }

                    let id_ecdsa_path = home_path.join(".ssh").join("id_ecdsa");
                    if id_ecdsa_path.exists() {
                        credentials.push(from_keypath(id_ecdsa_path.clone(), None));
                    }
                }
                credentials.push(from_key(&self.key));
                credentials
            }
        }
    }

    pub fn credential_helper_supplied(url: &str, config: super::Config) -> bool {
        invoke_credential_helper(url, config).is_some()
    }
}

fn invoke_credential_helper(url: &str, config: super::Config) -> Option<(String, String)> {
    let mut helper = git2::CredentialHelper::new(url);
    let config: git2::Config = config.into();
    helper.config(&config);
    helper.execute()
}

pub type CredentialsCallback<'a> = Box<
    dyn FnMut(&str, Option<&str>, git2::CredentialType) -> Result<git2::Cred, git2::Error> + 'a,
>;

fn from_keypath(key_path: path::PathBuf, passphrase: Option<&str>) -> CredentialsCallback {
    Box::new(move |url, _username_from_url, _allowed_types| {
        let key_path = key_path.resolve();
        tracing::debug!("authenticating with {} using {}", url, key_path.display());
        git2::Cred::ssh_key("git", None, &key_path, passphrase)
    })
}

fn from_key(key: &keys::PrivateKey) -> CredentialsCallback {
    Box::new(|url, _username_from_url, _allowed_types| {
        tracing::debug!("authenticating with {} using gitbutler's key", url);
        git2::Cred::ssh_key_from_memory("git", None, &key.to_string(), None)
    })
}

fn from_token(token: &str) -> CredentialsCallback {
    Box::new(move |url, _username_from_url, _allowed_types| {
        tracing::debug!("authenticating with {} using github token", url);
        git2::Cred::userpass_plaintext("git", token)
    })
}

fn from_plaintext(usr: String, pwd: String) -> CredentialsCallback<'static> {
    Box::new(move |url, _username_from_url, _allowed_types| {
        tracing::info!(
            "authenticating with {} using credentials (via creds helper)",
            url
        );
        git2::Cred::userpass_plaintext(&usr, &pwd)
    })
}

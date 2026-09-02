use std::sync::Mutex;

use anyhow::Result;
use but_secret::{Sensitive, secret};
use serde::{Deserialize, Serialize};

use crate::client::GitHubClient;

/// Persist GitHub account access tokens securely.
pub fn persist_gh_access_token(
    account_id: &GithubAccountIdentifier,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let oauth_account = GitHubAccount::new(account_id, access_token.clone());
    persist_github_account(&oauth_account, storage)
}

/// Forget a GitHub account, deleting its access token.
///
/// The stored account is removed even when its access token is missing from the keychain,
/// so an account whose secret was erased out from under us can still be forgotten.
pub fn delete_gh_access_token(
    account_id: &GithubAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let Some(secret_key) = forget_stored_github_account(account_id, storage)? else {
        return Ok(());
    };

    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::delete(&secret_key, secret::Namespace::BuildKind)
}

/// Retrieve a GitHub account access token for a given username.
pub fn get_gh_access_token(
    account_id: &GithubAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<Sensitive<String>>> {
    let account = find_github_account(account_id, storage)?;
    Ok(account.map(|acct| acct.access_token()))
}

pub fn list_known_github_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<GithubAccountIdentifier>> {
    Ok(storage
        .github_accounts()?
        .iter()
        .map(|account| account.into())
        .collect::<Vec<_>>())
}

pub fn clear_all_github_accounts(storage: &but_forge_storage::Controller) -> Result<()> {
    delete_all_github_accounts(storage)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", tag = "type", content = "info")]
pub enum GithubAccountIdentifier {
    OAuthUsername { username: String },
    PatUsername { username: String },
    Enterprise { username: String, host: String },
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(GithubAccountIdentifier);

impl GithubAccountIdentifier {
    pub fn oauth(username: &str) -> Self {
        GithubAccountIdentifier::OAuthUsername {
            username: username.to_string(),
        }
    }
    pub fn pat(username: &str) -> Self {
        GithubAccountIdentifier::PatUsername {
            username: username.to_string(),
        }
    }
    pub fn enterprise(username: &str, host: &str) -> Self {
        GithubAccountIdentifier::Enterprise {
            username: username.to_string(),
            host: host.to_string(),
        }
    }

    pub fn username(&self) -> &str {
        match self {
            GithubAccountIdentifier::OAuthUsername { username } => username,
            GithubAccountIdentifier::PatUsername { username } => username,
            GithubAccountIdentifier::Enterprise { username, .. } => username,
        }
    }

    /// The key used to store and look up the cached profile for this account.
    pub fn cache_key(&self) -> String {
        match self {
            GithubAccountIdentifier::OAuthUsername { username } => {
                format!("github_oauth_{username}")
            }
            GithubAccountIdentifier::PatUsername { username } => {
                format!("github_pat_{username}")
            }
            GithubAccountIdentifier::Enterprise { host, .. } => {
                format!("github_enterprise_{host}")
            }
        }
    }

    pub fn client(&self, access_token: &Sensitive<String>) -> Result<GitHubClient> {
        match self {
            GithubAccountIdentifier::OAuthUsername { .. }
            | GithubAccountIdentifier::PatUsername { .. } => GitHubClient::new(access_token),
            GithubAccountIdentifier::Enterprise { host, .. } => {
                GitHubClient::new_with_host_override(access_token, host)
            }
        }
    }

    /// Retrieve the custom forge host, if this is an Enterprise account.
    pub fn custom_host(&self) -> Option<String> {
        match self {
            GithubAccountIdentifier::Enterprise { host, .. } => Some(host.to_string()),
            GithubAccountIdentifier::OAuthUsername { .. } => None,
            GithubAccountIdentifier::PatUsername { .. } => None,
        }
    }
}

impl std::fmt::Display for GithubAccountIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GithubAccountIdentifier::OAuthUsername { username } => write!(f, "OAuth: {username}"),
            GithubAccountIdentifier::PatUsername { username } => write!(f, "PAT: {username}"),
            GithubAccountIdentifier::Enterprise { username, host } => {
                write!(f, "Enterprise {username}@{host}")
            }
        }
    }
}

pub enum GitHubAccount {
    OAuth {
        username: String,
        access_token: Sensitive<String>,
    },
    Pat {
        username: String,
        access_token: Sensitive<String>,
    },
    Enterprise {
        username: String,
        host: String,
        access_token: Sensitive<String>,
    },
}

impl From<&GitHubAccount> for but_forge_storage::settings::GitHubAccount {
    fn from(account: &GitHubAccount) -> Self {
        let access_token_key = account.secret_key();
        match account {
            GitHubAccount::OAuth { username, .. } => {
                but_forge_storage::settings::GitHubAccount::OAuth {
                    username: username.to_owned(),
                    access_token_key,
                }
            }
            GitHubAccount::Pat { username, .. } => {
                but_forge_storage::settings::GitHubAccount::Pat {
                    username: username.to_owned(),
                    access_token_key,
                }
            }
            GitHubAccount::Enterprise { host, username, .. } => {
                but_forge_storage::settings::GitHubAccount::Enterprise {
                    username: username.to_owned(),
                    host: host.to_owned(),
                    access_token_key,
                }
            }
        }
    }
}

impl From<&but_forge_storage::settings::GitHubAccount> for GithubAccountIdentifier {
    fn from(account: &but_forge_storage::settings::GitHubAccount) -> Self {
        match account {
            but_forge_storage::settings::GitHubAccount::OAuth { username, .. } => {
                GithubAccountIdentifier::OAuthUsername {
                    username: username.to_owned(),
                }
            }
            but_forge_storage::settings::GitHubAccount::Pat { username, .. } => {
                GithubAccountIdentifier::PatUsername {
                    username: username.to_owned(),
                }
            }
            but_forge_storage::settings::GitHubAccount::Enterprise { host, username, .. } => {
                GithubAccountIdentifier::Enterprise {
                    username: username.to_owned(),
                    host: host.to_owned(),
                }
            }
        }
    }
}

impl GitHubAccount {
    pub fn new(account_id: &GithubAccountIdentifier, access_token: Sensitive<String>) -> Self {
        match account_id {
            GithubAccountIdentifier::OAuthUsername { username } => GitHubAccount::OAuth {
                username: username.to_owned(),
                access_token,
            },
            GithubAccountIdentifier::PatUsername { username } => GitHubAccount::Pat {
                username: username.to_owned(),
                access_token,
            },
            GithubAccountIdentifier::Enterprise { username, host } => GitHubAccount::Enterprise {
                username: username.to_owned(),
                host: host.to_owned(),
                access_token,
            },
        }
    }

    fn secret_key(&self) -> String {
        match self {
            GitHubAccount::OAuth { username, .. } => {
                GithubAccountIdentifier::oauth(username).cache_key()
            }
            GitHubAccount::Pat { username, .. } => {
                GithubAccountIdentifier::pat(username).cache_key()
            }
            GitHubAccount::Enterprise { host, username, .. } => {
                GithubAccountIdentifier::enterprise(username, host).cache_key()
            }
        }
    }

    fn secret_value(&self) -> Result<Sensitive<String>> {
        Ok(self.access_token())
    }

    fn access_token(&self) -> Sensitive<String> {
        match self {
            GitHubAccount::OAuth { access_token, .. } => access_token.clone(),
            GitHubAccount::Pat { access_token, .. } => access_token.clone(),
            GitHubAccount::Enterprise { access_token, .. } => access_token.clone(),
        }
    }
}

fn retrieve_github_secret(account_secret_key: &str) -> Result<Option<Sensitive<String>>> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::retrieve(account_secret_key, secret::Namespace::BuildKind)
}

fn persist_github_account(
    account: &GitHubAccount,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let secret_key = account.secret_key();
    storage.add_github_account(&account.into())?;

    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::persist(
        &secret_key,
        &account.secret_value()?,
        secret::Namespace::BuildKind,
    )
}

fn delete_all_github_accounts(storage: &but_forge_storage::Controller) -> Result<()> {
    let keys_to_delete = storage.clear_all_github_accounts()?;
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    for key in keys_to_delete {
        secret::delete(&key, secret::Namespace::BuildKind)?;
    }
    Ok(())
}

/// Remove the stored account matching `account_id`, returning the key of the secret to erase.
///
/// Matching is by account identity alone, so a stored account is removable even when its
/// access token can no longer be retrieved.
fn forget_stored_github_account(
    account_id: &GithubAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<String>> {
    let Some(account) = find_stored_github_account(account_id, storage)? else {
        return Ok(None);
    };
    let secret_key = account.access_token_key().to_owned();
    storage.remove_github_account(&account)?;
    Ok(Some(secret_key))
}

fn find_stored_github_account(
    account_id: &GithubAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<but_forge_storage::settings::GitHubAccount>> {
    Ok(storage
        .github_accounts()?
        .into_iter()
        .find(|account| GithubAccountIdentifier::from(account) == *account_id))
}

fn find_github_account(
    account_id: &GithubAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<GitHubAccount>> {
    let Some(account) = find_stored_github_account(account_id, storage)? else {
        return Ok(None);
    };
    let Some(access_token) = retrieve_github_secret(account.access_token_key())
        .ok()
        .flatten()
    else {
        return Ok(None);
    };
    Ok(Some(GitHubAccount::new(account_id, access_token)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_storage() -> (but_forge_storage::Controller, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (
            but_forge_storage::Controller::from_path(dir.path().to_owned()),
            dir,
        )
    }

    /// An account whose access token is gone from the keychain must still be forgettable,
    /// otherwise it is stuck in the account list forever.
    #[test]
    fn orphaned_account_is_forgotten() {
        let (storage, _dir) = test_storage();
        let account = but_forge_storage::settings::GitHubAccount::Pat {
            username: "octocat".into(),
            access_token_key: "github_pat_octocat".into(),
        };
        storage.add_github_account(&account).unwrap();

        let account_id = GithubAccountIdentifier::pat("octocat");
        // No secret was ever persisted, so the account has no retrievable access token.
        assert!(
            find_github_account(&account_id, &storage)
                .unwrap()
                .is_none()
        );

        let secret_key = forget_stored_github_account(&account_id, &storage).unwrap();

        assert_eq!(secret_key.as_deref(), Some("github_pat_octocat"));
        assert!(storage.github_accounts().unwrap().is_empty());
    }

    #[test]
    fn forgetting_an_unknown_account_is_a_no_op() {
        let (storage, _dir) = test_storage();
        let account = but_forge_storage::settings::GitHubAccount::Pat {
            username: "octocat".into(),
            access_token_key: "github_pat_octocat".into(),
        };
        storage.add_github_account(&account).unwrap();

        // Same username, different account kind.
        let secret_key =
            forget_stored_github_account(&GithubAccountIdentifier::oauth("octocat"), &storage)
                .unwrap();

        assert_eq!(secret_key, None);
        assert_eq!(storage.github_accounts().unwrap().len(), 1);
    }
}

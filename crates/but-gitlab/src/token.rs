use std::sync::Mutex;

use anyhow::Result;
use but_secret::{Sensitive, secret};
use serde::{Deserialize, Serialize};

use crate::client::GitLabClient;

/// Persist GitLab account access tokens securely.
pub fn persist_gl_access_token(
    account_id: &GitlabAccountIdentifier,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let oauth_account = GitLabAccount::new(account_id, access_token.clone());
    persist_gitlab_account(&oauth_account, storage)
}

/// Forget a GitLab account, deleting its access token.
///
/// The stored account is removed even when its access token is missing from the keychain,
/// so an account whose secret was erased out from under us can still be forgotten.
pub fn delete_gl_access_token(
    account_id: &GitlabAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let Some(secret_key) = forget_stored_gitlab_account(account_id, storage)? else {
        return Ok(());
    };

    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::delete(&secret_key, secret::Namespace::BuildKind)
}

/// Retrieve a GitLab account access token for a given username.
pub fn get_gl_access_token(
    account_id: &GitlabAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<Sensitive<String>>> {
    let account = find_gitlab_account(account_id, storage)?;
    Ok(account.map(|acct| acct.access_token()))
}

pub fn list_known_gitlab_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<GitlabAccountIdentifier>> {
    Ok(storage
        .gitlab_accounts()?
        .iter()
        .map(|account| account.into())
        .collect::<Vec<_>>())
}

pub fn clear_all_gitlab_accounts(storage: &but_forge_storage::Controller) -> Result<()> {
    delete_all_gitlab_accounts(storage)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", tag = "type", content = "info")]
pub enum GitlabAccountIdentifier {
    PatUsername { username: String },
    SelfHosted { username: String, host: String },
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(GitlabAccountIdentifier);

impl GitlabAccountIdentifier {
    pub fn pat(username: &str) -> Self {
        GitlabAccountIdentifier::PatUsername {
            username: username.to_string(),
        }
    }
    pub fn selfhosted(username: &str, host: &str) -> Self {
        GitlabAccountIdentifier::SelfHosted {
            username: username.to_string(),
            host: host.to_string(),
        }
    }

    pub fn username(&self) -> &str {
        match self {
            GitlabAccountIdentifier::PatUsername { username } => username,
            GitlabAccountIdentifier::SelfHosted { username, .. } => username,
        }
    }

    /// The key used to store and look up the cached profile for this account.
    pub fn cache_key(&self) -> String {
        match self {
            GitlabAccountIdentifier::PatUsername { username } => {
                format!("gitlab_pat_{username}")
            }
            GitlabAccountIdentifier::SelfHosted { host, .. } => {
                format!("gitlab_selfhosted_{host}")
            }
        }
    }

    pub fn client(&self, access_token: &Sensitive<String>) -> Result<GitLabClient> {
        match self {
            GitlabAccountIdentifier::PatUsername { .. } => GitLabClient::new(access_token),
            GitlabAccountIdentifier::SelfHosted { host, .. } => {
                GitLabClient::new_with_host_override(access_token, host)
            }
        }
    }

    /// Retrieve the custom forge host, if this is a Self-Hosted account.
    pub fn custom_host(&self) -> Option<String> {
        match self {
            GitlabAccountIdentifier::SelfHosted { host, .. } => Some(host.to_string()),
            GitlabAccountIdentifier::PatUsername { .. } => None,
        }
    }
}

impl std::fmt::Display for GitlabAccountIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitlabAccountIdentifier::PatUsername { username } => write!(f, "PAT: {username}"),
            GitlabAccountIdentifier::SelfHosted { username, host } => {
                write!(f, "Self-hosted {username}@{host}")
            }
        }
    }
}

pub enum GitLabAccount {
    Pat {
        username: String,
        access_token: Sensitive<String>,
    },
    SelfHosted {
        username: String,
        host: String,
        access_token: Sensitive<String>,
    },
}

impl From<&GitLabAccount> for but_forge_storage::settings::GitLabAccount {
    fn from(account: &GitLabAccount) -> Self {
        let access_token_key = account.secret_key();
        match account {
            GitLabAccount::Pat { username, .. } => {
                but_forge_storage::settings::GitLabAccount::Pat {
                    username: username.to_owned(),
                    access_token_key,
                }
            }
            GitLabAccount::SelfHosted { host, username, .. } => {
                but_forge_storage::settings::GitLabAccount::SelfHosted {
                    username: username.to_owned(),
                    host: host.to_owned(),
                    access_token_key,
                }
            }
        }
    }
}

impl From<&but_forge_storage::settings::GitLabAccount> for GitlabAccountIdentifier {
    fn from(account: &but_forge_storage::settings::GitLabAccount) -> Self {
        match account {
            but_forge_storage::settings::GitLabAccount::Pat { username, .. } => {
                GitlabAccountIdentifier::PatUsername {
                    username: username.to_owned(),
                }
            }
            but_forge_storage::settings::GitLabAccount::SelfHosted { host, username, .. } => {
                GitlabAccountIdentifier::SelfHosted {
                    username: username.to_owned(),
                    host: host.to_owned(),
                }
            }
        }
    }
}

impl GitLabAccount {
    pub fn new(account_id: &GitlabAccountIdentifier, access_token: Sensitive<String>) -> Self {
        match account_id {
            GitlabAccountIdentifier::PatUsername { username } => GitLabAccount::Pat {
                username: username.to_owned(),
                access_token,
            },
            GitlabAccountIdentifier::SelfHosted { username, host } => GitLabAccount::SelfHosted {
                username: username.to_owned(),
                host: host.to_owned(),
                access_token,
            },
        }
    }

    fn secret_key(&self) -> String {
        match self {
            GitLabAccount::Pat { username, .. } => {
                GitlabAccountIdentifier::pat(username).cache_key()
            }
            GitLabAccount::SelfHosted { host, username, .. } => {
                GitlabAccountIdentifier::selfhosted(username, host).cache_key()
            }
        }
    }

    fn secret_value(&self) -> Result<Sensitive<String>> {
        Ok(self.access_token())
    }

    fn access_token(&self) -> Sensitive<String> {
        match self {
            GitLabAccount::Pat { access_token, .. } => access_token.clone(),
            GitLabAccount::SelfHosted { access_token, .. } => access_token.clone(),
        }
    }
}

fn retrieve_gitlab_secret(account_secret_key: &str) -> Result<Option<Sensitive<String>>> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::retrieve(account_secret_key, secret::Namespace::BuildKind)
}

fn persist_gitlab_account(
    account: &GitLabAccount,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let secret_key = account.secret_key();
    storage.add_gitlab_account(&account.into())?;

    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::persist(
        &secret_key,
        &account.secret_value()?,
        secret::Namespace::BuildKind,
    )
}

fn delete_all_gitlab_accounts(storage: &but_forge_storage::Controller) -> Result<()> {
    let keys_to_delete = storage.clear_all_gitlab_accounts()?;
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
fn forget_stored_gitlab_account(
    account_id: &GitlabAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<String>> {
    let Some(account) = find_stored_gitlab_account(account_id, storage)? else {
        return Ok(None);
    };
    let secret_key = account.access_token_key().to_owned();
    storage.remove_gitlab_account(&account)?;
    Ok(Some(secret_key))
}

fn find_stored_gitlab_account(
    account_id: &GitlabAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<but_forge_storage::settings::GitLabAccount>> {
    Ok(storage
        .gitlab_accounts()?
        .into_iter()
        .find(|account| GitlabAccountIdentifier::from(account) == *account_id))
}

fn find_gitlab_account(
    account_id: &GitlabAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<GitLabAccount>> {
    let Some(account) = find_stored_gitlab_account(account_id, storage)? else {
        return Ok(None);
    };
    let Some(access_token) = retrieve_gitlab_secret(account.access_token_key())
        .ok()
        .flatten()
    else {
        return Ok(None);
    };
    Ok(Some(GitLabAccount::new(account_id, access_token)))
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
        storage
            .add_gitlab_account(&but_forge_storage::settings::GitLabAccount::Pat {
                username: "octocat".into(),
                access_token_key: "gitlab_pat_octocat".into(),
            })
            .unwrap();

        let account_id = GitlabAccountIdentifier::pat("octocat");
        // No secret was ever persisted, so the account has no retrievable access token.
        assert!(
            find_gitlab_account(&account_id, &storage)
                .unwrap()
                .is_none()
        );

        let secret_key = forget_stored_gitlab_account(&account_id, &storage).unwrap();

        assert_eq!(secret_key.as_deref(), Some("gitlab_pat_octocat"));
        assert!(storage.gitlab_accounts().unwrap().is_empty());
    }

    #[test]
    fn forgetting_an_unknown_account_is_a_no_op() {
        let (storage, _dir) = test_storage();
        storage
            .add_gitlab_account(&but_forge_storage::settings::GitLabAccount::Pat {
                username: "octocat".into(),
                access_token_key: "gitlab_pat_octocat".into(),
            })
            .unwrap();

        let secret_key =
            forget_stored_gitlab_account(&GitlabAccountIdentifier::pat("someone-else"), &storage)
                .unwrap();

        assert_eq!(secret_key, None);
        assert_eq!(storage.gitlab_accounts().unwrap().len(), 1);
    }
}

use std::sync::Mutex;

use anyhow::Result;
use but_secret::{Sensitive, secret};
use serde::{Deserialize, Serialize};

/// Identifier for a Gitee account. Gitee personal access tokens are the only
/// auth method (no OAuth device flow), so accounts are keyed by username.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", tag = "type", content = "info")]
pub enum GiteeAccountIdentifier {
    PatUsername { username: String },
    SelfHosted { username: String, host: String },
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(GiteeAccountIdentifier);

impl GiteeAccountIdentifier {
    pub fn pat(username: &str) -> Self {
        GiteeAccountIdentifier::PatUsername {
            username: username.to_string(),
        }
    }
    pub fn selfhosted(username: &str, host: &str) -> Self {
        GiteeAccountIdentifier::SelfHosted {
            username: username.to_string(),
            host: host.to_string(),
        }
    }

    pub fn username(&self) -> &str {
        match self {
            GiteeAccountIdentifier::PatUsername { username } => username,
            GiteeAccountIdentifier::SelfHosted { username, .. } => username,
        }
    }

    /// Retrieve the custom forge host, if this is a Self-Hosted account.
    pub fn custom_host(&self) -> Option<String> {
        match self {
            GiteeAccountIdentifier::SelfHosted { host, .. } => Some(host.to_string()),
            GiteeAccountIdentifier::PatUsername { .. } => None,
        }
    }

    /// The key used to store and look up the cached profile for this account.
    pub fn cache_key(&self) -> String {
        match self {
            GiteeAccountIdentifier::PatUsername { username } => format!("gitee_pat_{username}"),
            GiteeAccountIdentifier::SelfHosted { host, .. } => format!("gitee_selfhosted_{host}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub access_token: Sensitive<String>,
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

/// Persist Gitee account access tokens securely.
pub fn persist_gitee_access_token(
    account_id: &GiteeAccountIdentifier,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let secret_key = account_id.cache_key();
    let account = match account_id {
        GiteeAccountIdentifier::PatUsername { username } => {
            but_forge_storage::settings::GiteeAccount::Pat {
                username: username.to_owned(),
                access_token_key: secret_key.clone(),
            }
        }
        GiteeAccountIdentifier::SelfHosted { username, host } => {
            but_forge_storage::settings::GiteeAccount::SelfHosted {
                username: username.to_owned(),
                host: host.to_owned(),
                access_token_key: secret_key.clone(),
            }
        }
    };
    storage.add_gitee_account(&account)?;
    secret::persist(&secret_key, &access_token, secret::Namespace::BuildKind)
}

/// Delete a Gitee account access token for a given account.
pub fn delete_gitee_access_token(
    account_id: &GiteeAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    // The token itself may be unreadable here; the account entry has to go regardless.
    let Some(account) = storage
        .gitee_accounts()?
        .into_iter()
        .find(|account| GiteeAccountIdentifier::from(account) == *account_id)
    else {
        return Ok(());
    };
    let secret_key = account.access_token_key().to_owned();
    storage.remove_gitee_account(&account)?;

    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::delete(&secret_key, secret::Namespace::BuildKind)
}

/// Retrieve a Gitee account access token for a given account.
pub fn get_gitee_access_token(
    account_id: &GiteeAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<Sensitive<String>>> {
    let secret_key = account_id.cache_key();
    let known = storage
        .gitee_accounts()?
        .iter()
        .any(|account| GiteeAccountIdentifier::from(account) == *account_id);
    if !known {
        return Ok(None);
    }
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::retrieve(&secret_key, secret::Namespace::BuildKind)
}

pub fn list_known_gitee_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<GiteeAccountIdentifier>> {
    Ok(storage
        .gitee_accounts()?
        .iter()
        .map(|account| account.into())
        .collect::<Vec<_>>())
}

pub fn clear_all_gitee_accounts(storage: &but_forge_storage::Controller) -> Result<()> {
    let accounts = storage.gitee_accounts()?;
    for account in accounts {
        let _ = storage.remove_gitee_account(&account);
        let _ = secret::delete(account.access_token_key(), secret::Namespace::BuildKind);
    }
    Ok(())
}

impl From<&but_forge_storage::settings::GiteeAccount> for GiteeAccountIdentifier {
    fn from(account: &but_forge_storage::settings::GiteeAccount) -> Self {
        match account {
            but_forge_storage::settings::GiteeAccount::Pat { username, .. } => {
                GiteeAccountIdentifier::pat(username)
            }
            but_forge_storage::settings::GiteeAccount::SelfHosted { username, host, .. } => {
                GiteeAccountIdentifier::selfhosted(username, host)
            }
        }
    }
}

impl std::fmt::Display for GiteeAccountIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GiteeAccountIdentifier::PatUsername { username } => write!(f, "PAT: {username}"),
            GiteeAccountIdentifier::SelfHosted { username, host } => {
                write!(f, "Self-hosted {username}@{host}")
            }
        }
    }
}

use std::sync::Mutex;

use anyhow::Result;
use but_forge_storage::settings::GitbutlerCodeAccount;
use but_secret::{Sensitive, secret};
use serde::{Deserialize, Serialize};

/// A GitButler Code account `but` signed in to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", tag = "type", content = "info")]
pub enum GitbutlerCodeAccountIdentifier {
    /// Signed in through GitButler Code's device login.
    DeviceLogin { username: String },
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(GitbutlerCodeAccountIdentifier);

impl GitbutlerCodeAccountIdentifier {
    pub fn device_login(username: &str) -> Self {
        Self::DeviceLogin {
            username: username.to_owned(),
        }
    }

    pub fn username(&self) -> &str {
        match self {
            Self::DeviceLogin { username } => username,
        }
    }

    /// The key the account's token is kept under.
    pub fn secret_key(&self) -> String {
        match self {
            Self::DeviceLogin { username } => format!("gitbutler_code_{username}"),
        }
    }

    /// GitButler Code has one host, so there is never a custom one.
    pub fn custom_host(&self) -> Option<String> {
        None
    }
}

impl std::fmt::Display for GitbutlerCodeAccountIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.username())
    }
}

impl From<&GitbutlerCodeAccount> for GitbutlerCodeAccountIdentifier {
    fn from(account: &GitbutlerCodeAccount) -> Self {
        Self::device_login(account.username())
    }
}

static SECRETS: Mutex<()> = Mutex::new(());

pub(crate) fn persist(
    account: &GitbutlerCodeAccountIdentifier,
    token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let key = account.secret_key();
    storage.add_gitbutler_code_account(&GitbutlerCodeAccount::DeviceLogin {
        username: account.username().to_owned(),
        access_token_key: key.clone(),
    })?;
    let _one_at_a_time = SECRETS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    secret::persist(&key, token, secret::Namespace::BuildKind)
}

pub(crate) fn access_token(
    account: &GitbutlerCodeAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<Sensitive<String>>> {
    let Some(stored) = find(account, storage)? else {
        return Ok(None);
    };
    let _one_at_a_time = SECRETS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    secret::retrieve(stored.access_token_key(), secret::Namespace::BuildKind)
}

pub(crate) fn delete(
    account: &GitbutlerCodeAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    // The token may be unreadable (another build kind, or a keychain refusing the read); the
    // account entry goes regardless.
    let Some(stored) = find(account, storage)? else {
        return Ok(());
    };
    storage.remove_gitbutler_code_account(&stored)?;
    let _one_at_a_time = SECRETS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    secret::delete(stored.access_token_key(), secret::Namespace::BuildKind)
}

pub(crate) fn list_known_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<GitbutlerCodeAccountIdentifier>> {
    Ok(storage
        .gitbutler_code_accounts()?
        .iter()
        .map(Into::into)
        .collect())
}

fn find(
    account: &GitbutlerCodeAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<GitbutlerCodeAccount>> {
    Ok(storage
        .gitbutler_code_accounts()?
        .into_iter()
        .find(|stored| GitbutlerCodeAccountIdentifier::from(stored) == *account))
}

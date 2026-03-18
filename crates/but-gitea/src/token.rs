//! Account identifiers and secure token persistence for Gitea integrations.

use std::sync::Mutex;

use anyhow::Result;
use but_secret::{Sensitive, secret};
use serde::{Deserialize, Serialize};

use crate::client::GiteaClient;

/// Persist a Gitea access token and its corresponding account metadata.
pub fn persist_gitea_access_token(
    account_id: &GiteaAccountIdentifier,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let account = GiteaAccount::new(account_id, access_token.clone());
    persist_gitea_account(&account, storage)
}

/// Delete a stored Gitea access token for the given account.
pub fn delete_gitea_access_token(
    account_id: &GiteaAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let account = find_gitea_account(account_id, storage)?;
    if let Some(account) = account {
        delete_gitea_account(&account, storage)
    } else {
        Ok(())
    }
}

/// Retrieve a stored Gitea access token for the given account.
pub fn get_gitea_access_token(
    account_id: &GiteaAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<Sensitive<String>>> {
    let account = find_gitea_account(account_id, storage)?;
    Ok(account.map(|account| account.access_token()))
}

/// List all Gitea accounts currently known to local forge storage.
pub fn list_known_gitea_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<GiteaAccountIdentifier>> {
    Ok(storage
        .gitea_accounts()?
        .iter()
        .map(Into::into)
        .collect::<Vec<_>>())
}

/// Delete all stored Gitea accounts.
pub fn clear_all_gitea_accounts(storage: &but_forge_storage::Controller) -> Result<()> {
    delete_all_gitea_accounts(storage)?;
    Ok(())
}

/// Stable identifier for a persisted Gitea account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "./gitea/token.ts"))]
pub struct GiteaAccountIdentifier {
    pub username: String,
    pub host: String,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(GiteaAccountIdentifier);

impl GiteaAccountIdentifier {
    /// Build an identifier from the authenticated username and host.
    pub fn new(username: &str, host: &str) -> Self {
        Self {
            username: username.to_string(),
            host: host.to_string(),
        }
    }

    /// Recreate a client for this stored account.
    pub fn client(&self, access_token: &Sensitive<String>) -> Result<GiteaClient> {
        crate::client::client_for(self, access_token)
    }
}

impl std::fmt::Display for GiteaAccountIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.username, self.host)
    }
}

/// Persisted Gitea account with its associated secret token.
pub struct GiteaAccount {
    username: String,
    host: String,
    access_token: Sensitive<String>,
}

impl From<&GiteaAccount> for but_forge_storage::settings::GiteaAccount {
    fn from(account: &GiteaAccount) -> Self {
        but_forge_storage::settings::GiteaAccount {
            username: account.username.clone(),
            host: account.host.clone(),
            access_token_key: account.secret_key(),
        }
    }
}

impl From<&but_forge_storage::settings::GiteaAccount> for GiteaAccountIdentifier {
    fn from(account: &but_forge_storage::settings::GiteaAccount) -> Self {
        Self {
            username: account.username.clone(),
            host: account.host.clone(),
        }
    }
}

impl GiteaAccount {
    pub fn new(account_id: &GiteaAccountIdentifier, access_token: Sensitive<String>) -> Self {
        Self {
            username: account_id.username.clone(),
            host: account_id.host.clone(),
            access_token,
        }
    }

    fn secret_key(&self) -> String {
        format!("gitea_{}_{}", self.host, self.username)
    }

    fn access_token(&self) -> Sensitive<String> {
        self.access_token.clone()
    }
}

fn retrieve_gitea_secret(secret_key: &str) -> Result<Option<Sensitive<String>>> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::retrieve(secret_key, secret::Namespace::BuildKind)
}

fn persist_gitea_account(
    account: &GiteaAccount,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let secret_key = account.secret_key();
    storage.add_gitea_account(&account.into())?;

    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::persist(
        &secret_key,
        &account.access_token(),
        secret::Namespace::BuildKind,
    )
}

fn delete_gitea_account(
    account: &GiteaAccount,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let secret_key = account.secret_key();
    storage.remove_gitea_account(&account.into())?;

    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::delete(&secret_key, secret::Namespace::BuildKind)
}

fn delete_all_gitea_accounts(storage: &but_forge_storage::Controller) -> Result<()> {
    let keys_to_delete = storage.clear_all_gitea_accounts()?;
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    for key in keys_to_delete {
        secret::delete(&key, secret::Namespace::BuildKind)?;
    }
    Ok(())
}

fn find_gitea_account(
    account_id: &GiteaAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<GiteaAccount>> {
    let accounts = storage.gitea_accounts()?;
    let result = accounts.iter().find_map(|account| {
        if account.username == account_id.username
            && account.host == account_id.host
            && let Some(access_token) = retrieve_gitea_secret(&account.access_token_key)
                .ok()
                .flatten()
        {
            return Some(GiteaAccount {
                username: account.username.clone(),
                host: account.host.clone(),
                access_token,
            });
        }
        None
    });

    Ok(result)
}

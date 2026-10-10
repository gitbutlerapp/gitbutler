use std::sync::Mutex;

use anyhow::Result;
use but_secret::{Sensitive, secret};
use serde::{Deserialize, Serialize};

use crate::client::ForgejoClient;

/// A Forgejo account: one user on one instance.
///
/// Forgejo has no canonical hosted instance (Codeberg is just one of many), so
/// unlike the GitLab identifier there is no host-less variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ForgejoAccountIdentifier {
    pub username: String,
    /// The instance base URL as normalized by [`normalize_host`].
    pub host: String,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgejoAccountIdentifier);

impl ForgejoAccountIdentifier {
    pub fn new(username: &str, host: &str) -> Self {
        ForgejoAccountIdentifier {
            username: username.to_owned(),
            host: normalize_host(host),
        }
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    /// The key used to store the token in the keychain and the cached profile.
    ///
    /// Includes the username so two users on the same instance don't overwrite
    /// each other.
    pub fn cache_key(&self) -> String {
        format!("forgejo_{}_{}", self.host, self.username)
    }

    pub fn client(&self, access_token: &Sensitive<String>) -> Result<ForgejoClient> {
        ForgejoClient::new(access_token, &self.host)
    }

    /// The instance host, used to match repository remotes to this account.
    pub fn custom_host(&self) -> Option<String> {
        Some(self.host.clone())
    }
}

impl std::fmt::Display for ForgejoAccountIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.username, self.host)
    }
}

/// Normalize a user-entered instance address to a base URL without a trailing
/// slash or API suffix, defaulting to https: `git.example.com/` becomes
/// `https://git.example.com`.
pub fn normalize_host(host: &str) -> String {
    let host = host.trim().trim_end_matches('/');
    let host = host.strip_suffix("/api/v1").unwrap_or(host);
    if host.contains("://") {
        host.to_owned()
    } else {
        format!("https://{host}")
    }
}

/// Persist a Forgejo account and its access token.
pub(crate) fn persist_access_token(
    account_id: &ForgejoAccountIdentifier,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let key = account_id.cache_key();
    storage.add_forgejo_account(&stored_account(account_id))?;
    let _one_at_a_time_to_prevent_races = SECRET_QUEUE.lock().unwrap();
    secret::persist(&key, access_token, secret::Namespace::BuildKind)
}

/// Delete a Forgejo account and its access token. Unknown accounts are a no-op.
pub(crate) fn delete_access_token(
    account_id: &ForgejoAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    if !list_known_accounts(storage)?.contains(account_id) {
        return Ok(());
    }
    storage.remove_forgejo_account(&stored_account(account_id))?;
    let _one_at_a_time_to_prevent_races = SECRET_QUEUE.lock().unwrap();
    secret::delete(&account_id.cache_key(), secret::Namespace::BuildKind)
}

/// Retrieve the access token of a known Forgejo account.
pub(crate) fn get_access_token(
    account_id: &ForgejoAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<Sensitive<String>>> {
    let Some(account) = storage
        .forgejo_accounts()?
        .into_iter()
        .find(|account| &ForgejoAccountIdentifier::from(account) == account_id)
    else {
        return Ok(None);
    };
    let _one_at_a_time_to_prevent_races = SECRET_QUEUE.lock().unwrap();
    secret::retrieve(&account.access_token_key, secret::Namespace::BuildKind)
}

pub(crate) fn list_known_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<ForgejoAccountIdentifier>> {
    Ok(storage
        .forgejo_accounts()?
        .iter()
        .map(ForgejoAccountIdentifier::from)
        .collect())
}

pub(crate) fn clear_all_accounts(storage: &but_forge_storage::Controller) -> Result<()> {
    let keys = storage.clear_all_forgejo_accounts()?;
    let _one_at_a_time_to_prevent_races = SECRET_QUEUE.lock().unwrap();
    for key in keys {
        secret::delete(&key, secret::Namespace::BuildKind)?;
    }
    Ok(())
}

/// Keychain access is serialized: concurrent keyring calls race on some platforms.
static SECRET_QUEUE: Mutex<()> = Mutex::new(());

fn stored_account(
    account_id: &ForgejoAccountIdentifier,
) -> but_forge_storage::settings::ForgejoAccount {
    but_forge_storage::settings::ForgejoAccount {
        host: account_id.host.clone(),
        username: account_id.username.clone(),
        access_token_key: account_id.cache_key(),
    }
}

impl From<&but_forge_storage::settings::ForgejoAccount> for ForgejoAccountIdentifier {
    fn from(account: &but_forge_storage::settings::ForgejoAccount) -> Self {
        ForgejoAccountIdentifier {
            username: account.username.clone(),
            host: account.host.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_host_accepts_bare_hosts_and_api_urls() {
        for input in [
            "git.example.com",
            "https://git.example.com/",
            " https://git.example.com/api/v1 ",
            "https://git.example.com/api/v1/",
        ] {
            assert_eq!(
                normalize_host(input),
                "https://git.example.com",
                "{input:?} names the same instance"
            );
        }
        assert_eq!(
            normalize_host("http://localhost:3000"),
            "http://localhost:3000",
            "an explicit scheme and port are kept for local instances"
        );
    }

    #[test]
    fn cache_key_separates_users_on_one_host() {
        let alice = ForgejoAccountIdentifier::new("alice", "git.example.com");
        let bob = ForgejoAccountIdentifier::new("bob", "git.example.com");
        assert_ne!(
            alice.cache_key(),
            bob.cache_key(),
            "two users on one instance must not share a keychain entry"
        );
    }
}

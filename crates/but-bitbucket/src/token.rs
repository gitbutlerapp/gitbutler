use std::sync::Mutex;

use anyhow::Result;
use but_secret::{Sensitive, secret};
use serde::{Deserialize, Serialize};

use crate::client::BitbucketClient;

/// Persist Bitbucket account access tokens securely.
pub fn persist_bb_access_token(
    account_id: &BitbucketAccountIdentifier,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let account = BitbucketAccount::new(account_id, access_token.clone());
    persist_bitbucket_account(&account, storage)
}

/// Forget a Bitbucket account, deleting its access token.
///
/// The stored account is removed even when its access token is missing from the keychain,
/// so an account whose secret was erased out from under us can still be forgotten.
pub fn delete_bb_access_token(
    account_id: &BitbucketAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let Some(secret_key) = forget_stored_bitbucket_account(account_id, storage)? else {
        return Ok(());
    };

    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::delete(&secret_key, secret::Namespace::BuildKind)
}

/// Retrieve a Bitbucket account access token for a given account.
pub fn get_bb_access_token(
    account_id: &BitbucketAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<Sensitive<String>>> {
    let account = find_bitbucket_account(account_id, storage)?;
    Ok(account.map(|acct| acct.access_token()))
}

pub fn list_known_bitbucket_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<BitbucketAccountIdentifier>> {
    Ok(storage
        .bitbucket_accounts()?
        .iter()
        .map(|account| account.into())
        .collect::<Vec<_>>())
}

pub fn clear_all_bitbucket_accounts(storage: &but_forge_storage::Controller) -> Result<()> {
    delete_all_bitbucket_accounts(storage)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", tag = "type", content = "info")]
pub enum BitbucketAccountIdentifier {
    /// An Atlassian API token (with scopes). `email` is the Atlassian account
    /// email - it is both the unique account identity and the HTTP Basic
    /// username used when authenticating.
    ApiToken { email: String },
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(BitbucketAccountIdentifier);

impl BitbucketAccountIdentifier {
    pub fn apitoken(email: &str) -> Self {
        BitbucketAccountIdentifier::ApiToken {
            email: email.to_string(),
        }
    }

    pub fn email(&self) -> &str {
        match self {
            BitbucketAccountIdentifier::ApiToken { email } => email,
        }
    }

    /// The key used to store and look up the cached profile for this account.
    pub fn cache_key(&self) -> String {
        match self {
            BitbucketAccountIdentifier::ApiToken { email } => {
                format!("bitbucket_apitoken_{email}")
            }
        }
    }

    pub fn client(&self, access_token: &Sensitive<String>) -> Result<BitbucketClient> {
        match self {
            BitbucketAccountIdentifier::ApiToken { email } => {
                BitbucketClient::new(email, access_token)
            }
        }
    }

    /// Retrieve the custom forge host. Bitbucket Cloud is fixed-host, so this is
    /// always `None`; kept for symmetry with the other forge integrations.
    pub fn custom_host(&self) -> Option<String> {
        None
    }
}

impl std::fmt::Display for BitbucketAccountIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BitbucketAccountIdentifier::ApiToken { email } => {
                write!(f, "API token: {email}")
            }
        }
    }
}

pub enum BitbucketAccount {
    ApiToken {
        email: String,
        access_token: Sensitive<String>,
    },
}

impl From<&BitbucketAccount> for but_forge_storage::settings::BitbucketAccount {
    fn from(account: &BitbucketAccount) -> Self {
        let access_token_key = account.secret_key();
        match account {
            BitbucketAccount::ApiToken { email, .. } => {
                but_forge_storage::settings::BitbucketAccount::ApiToken {
                    email: email.to_owned(),
                    access_token_key,
                }
            }
        }
    }
}

impl From<&but_forge_storage::settings::BitbucketAccount> for BitbucketAccountIdentifier {
    fn from(account: &but_forge_storage::settings::BitbucketAccount) -> Self {
        match account {
            but_forge_storage::settings::BitbucketAccount::ApiToken { email, .. } => {
                BitbucketAccountIdentifier::ApiToken {
                    email: email.to_owned(),
                }
            }
        }
    }
}

impl BitbucketAccount {
    pub fn new(account_id: &BitbucketAccountIdentifier, access_token: Sensitive<String>) -> Self {
        match account_id {
            BitbucketAccountIdentifier::ApiToken { email } => BitbucketAccount::ApiToken {
                email: email.to_owned(),
                access_token,
            },
        }
    }

    fn secret_key(&self) -> String {
        match self {
            BitbucketAccount::ApiToken { email, .. } => {
                BitbucketAccountIdentifier::apitoken(email).cache_key()
            }
        }
    }

    fn secret_value(&self) -> Result<Sensitive<String>> {
        Ok(self.access_token())
    }

    fn access_token(&self) -> Sensitive<String> {
        match self {
            BitbucketAccount::ApiToken { access_token, .. } => access_token.clone(),
        }
    }
}

fn retrieve_bitbucket_secret(account_secret_key: &str) -> Result<Option<Sensitive<String>>> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::retrieve(account_secret_key, secret::Namespace::BuildKind)
}

fn persist_bitbucket_account(
    account: &BitbucketAccount,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let secret_key = account.secret_key();
    storage.add_bitbucket_account(&account.into())?;

    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::persist(
        &secret_key,
        &account.secret_value()?,
        secret::Namespace::BuildKind,
    )
}

fn delete_all_bitbucket_accounts(storage: &but_forge_storage::Controller) -> Result<()> {
    let keys_to_delete = storage.clear_all_bitbucket_accounts()?;
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
fn forget_stored_bitbucket_account(
    account_id: &BitbucketAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<String>> {
    let Some(account) = find_stored_bitbucket_account(account_id, storage)? else {
        return Ok(None);
    };
    let secret_key = account.access_token_key().to_owned();
    storage.remove_bitbucket_account(&account)?;
    Ok(Some(secret_key))
}

fn find_stored_bitbucket_account(
    account_id: &BitbucketAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<but_forge_storage::settings::BitbucketAccount>> {
    Ok(storage
        .bitbucket_accounts()?
        .into_iter()
        .find(|account| BitbucketAccountIdentifier::from(account) == *account_id))
}

fn find_bitbucket_account(
    account_id: &BitbucketAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<BitbucketAccount>> {
    let Some(account) = find_stored_bitbucket_account(account_id, storage)? else {
        return Ok(None);
    };
    let Some(access_token) = retrieve_bitbucket_secret(account.access_token_key())
        .ok()
        .flatten()
    else {
        return Ok(None);
    };
    Ok(Some(BitbucketAccount::new(account_id, access_token)))
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
            .add_bitbucket_account(&but_forge_storage::settings::BitbucketAccount::ApiToken {
                email: "octocat@example.com".into(),
                access_token_key: "bitbucket_apitoken_octocat@example.com".into(),
            })
            .unwrap();

        let account_id = BitbucketAccountIdentifier::apitoken("octocat@example.com");
        // No secret was ever persisted, so the account has no retrievable access token.
        assert!(
            find_bitbucket_account(&account_id, &storage)
                .unwrap()
                .is_none()
        );

        let secret_key = forget_stored_bitbucket_account(&account_id, &storage).unwrap();

        assert_eq!(
            secret_key.as_deref(),
            Some("bitbucket_apitoken_octocat@example.com")
        );
        assert!(storage.bitbucket_accounts().unwrap().is_empty());
    }

    #[test]
    fn forgetting_an_unknown_account_is_a_no_op() {
        let (storage, _dir) = test_storage();
        storage
            .add_bitbucket_account(&but_forge_storage::settings::BitbucketAccount::ApiToken {
                email: "octocat@example.com".into(),
                access_token_key: "bitbucket_apitoken_octocat@example.com".into(),
            })
            .unwrap();

        let secret_key = forget_stored_bitbucket_account(
            &BitbucketAccountIdentifier::apitoken("someone-else@example.com"),
            &storage,
        )
        .unwrap();

        assert_eq!(secret_key, None);
        assert_eq!(storage.bitbucket_accounts().unwrap().len(), 1);
    }
}

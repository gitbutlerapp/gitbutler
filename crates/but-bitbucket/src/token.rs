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

/// Delete a Bitbucket account access token for a given account.
pub fn delete_bb_access_token(
    account_id: &BitbucketAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let account = find_bitbucket_account(account_id, storage)?;
    if let Some(account) = account {
        delete_bitbucket_account(&account, storage)
    } else {
        Ok(())
    }
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

fn delete_bitbucket_account(
    account: &BitbucketAccount,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let secret_key = account.secret_key();
    storage.remove_bitbucket_account(&account.into())?;

    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::delete(&secret_key, secret::Namespace::BuildKind)
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

fn find_bitbucket_account(
    account_id: &BitbucketAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<BitbucketAccount>> {
    let accounts = storage.bitbucket_accounts()?;
    let result = match account_id {
        BitbucketAccountIdentifier::ApiToken { email } => accounts.iter().find_map(|account| {
            let but_forge_storage::settings::BitbucketAccount::ApiToken {
                email: acct_email,
                access_token_key,
            } = account;
            if acct_email == email
                && let Some(access_token) =
                    retrieve_bitbucket_secret(access_token_key).ok().flatten()
            {
                return Some(BitbucketAccount::ApiToken {
                    email: acct_email.clone(),
                    access_token,
                });
            }
            None
        }),
    };
    Ok(result)
}

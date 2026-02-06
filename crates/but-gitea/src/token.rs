use std::sync::Mutex;

use anyhow::Result;
use but_secret::{Sensitive, secret};
use serde::{Deserialize, Serialize};

use crate::client::GiteaClient;

/// Persist Gitea account access tokens securely.
pub fn persist_gitea_access_token(
    account_id: &GiteaAccountIdentifier,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let account = GiteaAccount::new(account_id, access_token.clone());
    persist_gitea_account(&account, storage)
}

/// Delete a Gitea account access token for a given username.
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

/// Retrieve a Gitea account access token for a given username.
pub fn get_gitea_access_token(
    account_id: &GiteaAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<Sensitive<String>>> {
    let account = find_gitea_account(account_id, storage)?;
    Ok(account.map(|acct| acct.access_token()))
}

pub fn list_known_gitea_accounts(storage: &but_forge_storage::Controller) -> Result<Vec<GiteaAccountIdentifier>> {
    Ok(storage
        .gitea_accounts()?
        .iter()
        .map(|account| account.into())
        .collect::<Vec<_>>())
}

pub fn clear_all_gitea_accounts(storage: &but_forge_storage::Controller) -> Result<()> {
    delete_all_gitea_accounts(storage)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "info")]
pub enum GiteaAccountIdentifier {
    PatUsername { username: String },
    SelfHosted { username: String, host: String },
}

impl GiteaAccountIdentifier {
    pub fn pat(username: &str) -> Self {
        GiteaAccountIdentifier::PatUsername {
            username: username.to_string(),
        }
    }
    pub fn selfhosted(username: &str, host: &str) -> Self {
        GiteaAccountIdentifier::SelfHosted {
            username: username.to_string(),
            host: host.to_string(),
        }
    }

    pub fn username(&self) -> &str {
        match self {
            GiteaAccountIdentifier::PatUsername { username } => username,
            GiteaAccountIdentifier::SelfHosted { username, .. } => username,
        }
    }

    pub fn client(&self, access_token: &Sensitive<String>) -> Result<GiteaClient> {
        match self {
            GiteaAccountIdentifier::PatUsername { .. } => {
                anyhow::bail!("Gitea always requires a host URL. Use SelfHosted variant instead.")
            }
            GiteaAccountIdentifier::SelfHosted { host, .. } => GiteaClient::new(access_token, host),
        }
    }
}

impl std::fmt::Display for GiteaAccountIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GiteaAccountIdentifier::PatUsername { username } => write!(f, "PAT: {}", username),
            GiteaAccountIdentifier::SelfHosted { username, host } => {
                write!(f, "Self-hosted {}@{}", username, host)
            }
        }
    }
}

pub enum GiteaAccount {
    #[allow(dead_code)]
    Pat {
        username: String,
        access_token: Sensitive<String>,
    },
    #[allow(dead_code)]
    SelfHosted {
        username: String,
        host: String,
        access_token: Sensitive<String>,
    },
}

impl From<&GiteaAccount> for but_forge_storage::settings::GiteaAccount {
    fn from(account: &GiteaAccount) -> Self {
        let access_token_key = account.secret_key();
        match account {
            GiteaAccount::Pat { username, .. } => but_forge_storage::settings::GiteaAccount::Pat {
                username: username.to_owned(),
                access_token_key,
            },
            GiteaAccount::SelfHosted { host, username, .. } => {
                but_forge_storage::settings::GiteaAccount::SelfHosted {
                    username: username.to_owned(),
                    host: host.to_owned(),
                    access_token_key,
                }
            }
        }
    }
}

impl From<&but_forge_storage::settings::GiteaAccount> for GiteaAccountIdentifier {
    fn from(account: &but_forge_storage::settings::GiteaAccount) -> Self {
        match account {
            but_forge_storage::settings::GiteaAccount::Pat { username, .. } => GiteaAccountIdentifier::PatUsername {
                username: username.to_owned(),
            },
            but_forge_storage::settings::GiteaAccount::SelfHosted { host, username, .. } => {
                GiteaAccountIdentifier::SelfHosted {
                    username: username.to_owned(),
                    host: host.to_owned(),
                }
            }
        }
    }
}

impl GiteaAccount {
    pub fn new(account_id: &GiteaAccountIdentifier, access_token: Sensitive<String>) -> Self {
        match account_id {
            GiteaAccountIdentifier::PatUsername { username } => GiteaAccount::Pat {
                username: username.to_owned(),
                access_token,
            },
            GiteaAccountIdentifier::SelfHosted { username, host } => GiteaAccount::SelfHosted {
                username: username.to_owned(),
                host: host.to_owned(),
                access_token,
            },
        }
    }

    fn secret_key(&self) -> String {
        match self {
            GiteaAccount::Pat { username, .. } => format!("gitea_pat_{}", username),
            GiteaAccount::SelfHosted { host, .. } => format!("gitea_selfhosted_{}", host),
        }
    }

    fn secret_value(&self) -> Result<Sensitive<String>> {
        Ok(self.access_token())
    }

    fn access_token(&self) -> Sensitive<String> {
        match self {
            GiteaAccount::Pat { access_token, .. } => access_token.clone(),
            GiteaAccount::SelfHosted { access_token, .. } => access_token.clone(),
        }
    }
}

fn retrieve_gitea_secret(account_secret_key: &str) -> Result<Option<Sensitive<String>>> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::retrieve(account_secret_key, secret::Namespace::BuildKind)
}

fn persist_gitea_account(account: &GiteaAccount, storage: &but_forge_storage::Controller) -> Result<()> {
    let secret_key = account.secret_key();
    storage.add_gitea_account(&account.into())?;

    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::persist(&secret_key, &account.secret_value()?, secret::Namespace::BuildKind)
}

fn delete_gitea_account(account: &GiteaAccount, storage: &but_forge_storage::Controller) -> Result<()> {
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
    let result = match account_id {
        GiteaAccountIdentifier::PatUsername { username } => accounts.iter().find_map(|account| {
            if let but_forge_storage::settings::GiteaAccount::Pat {
                username: acct_username,
                access_token_key,
            } = account
                && acct_username == username
                && let Some(access_token) = retrieve_gitea_secret(access_token_key).ok().flatten()
            {
                return Some(GiteaAccount::Pat {
                    username: acct_username.clone(),
                    access_token,
                });
            }
            None
        }),
        GiteaAccountIdentifier::SelfHosted { username, host } => accounts.iter().find_map(|account| {
            if let but_forge_storage::settings::GiteaAccount::SelfHosted {
                username: acct_username,
                host: acct_host,
                access_token_key,
            } = account
                && acct_host == host
                && acct_username == username
                && let Some(access_token) = retrieve_gitea_secret(access_token_key).ok().flatten()
            {
                return Some(GiteaAccount::SelfHosted {
                    username: acct_username.clone(),
                    host: acct_host.clone(),
                    access_token,
                });
            }
            None
        }),
    };
    Ok(result)
}

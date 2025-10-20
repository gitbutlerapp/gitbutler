use std::sync::Mutex;

use anyhow::{Context, Result};
use but_secret::{Sensitive, secret};
use serde::{Deserialize, Serialize};

/// Persist GitHub OAuth access tokens securely.
pub fn persist_gh_access_token(login: &str, access_token: &Sensitive<String>) -> Result<()> {
    let mut map = retrieve_github_account_map()?;
    if let Some(entry) = map.iter_mut().find(|entry| match entry {
        GitHubAccount::OAuth { username, .. } => username == login,
        _ => false,
    }) {
        entry.set_access_token(access_token.clone());
        persist_github_account(entry)
    } else {
        let account = GitHubAccount::OAuth {
            username: login.to_string(),
            access_token: access_token.clone(),
        };
        persist_github_account(&account)
    }
}

/// Delete a GitHub OAuth access token for a given username.
pub fn delete_gh_access_token(login: &str) -> Result<()> {
    let map = retrieve_github_account_map()?;
    let account = map.into_iter().find(|entry| match entry {
        GitHubAccount::OAuth { username, .. } => username == login,
        _ => false,
    });
    if let Some(account) = account {
        delete_github_account(&account)
    } else {
        Ok(())
    }
}

/// Retrieve a GitHub OAuth access token for a given username.
pub fn get_gh_access_token(login: &str) -> Result<Option<Sensitive<String>>> {
    let map = retrieve_github_account_map()?;
    Ok(map
        .into_iter()
        .find(|entry| entry.username() == login)
        .map(|entry| entry.access_token()))
}

pub fn list_known_github_usernames() -> Result<Vec<String>> {
    let map = retrieve_github_account_map()?;
    Ok(map.into_iter().map(|entry| entry.username()).collect())
}

pub fn clear_all_github_tokens() -> Result<()> {
    let map = retrieve_github_account_map()?;
    for account in map {
        delete_github_account(&account)?;
    }
    Ok(())
}

const GITHUB_KNOWN_ACCOUTNS_LIST_KEY: &str = "github_known_accounts";

pub enum GitHubAccount {
    OAuth {
        username: String,
        access_token: Sensitive<String>,
    },
    #[allow(dead_code)]
    Pat {
        username: String,
        access_token: Sensitive<String>,
    },
}

impl From<SerializableGitHubAccount> for GitHubAccount {
    fn from(ser: SerializableGitHubAccount) -> Self {
        match ser {
            SerializableGitHubAccount::OAuth {
                username,
                access_token,
            } => GitHubAccount::OAuth {
                username,
                access_token: Sensitive(access_token),
            },
            SerializableGitHubAccount::Pat {
                username,
                access_token,
            } => GitHubAccount::Pat {
                username,
                access_token: Sensitive(access_token),
            },
        }
    }
}

impl GitHubAccount {
    fn secret_key(&self) -> String {
        match self {
            GitHubAccount::OAuth { username, .. } => format!("github_oauth_{}", username),
            GitHubAccount::Pat { username, .. } => format!("github_pat_{}", username),
        }
    }

    fn secret_value(&self) -> Result<Sensitive<String>> {
        let ser = SerializableGitHubAccount::from(self);
        let ser_string = serde_json::to_string(&ser)
            .context("Failed to serialize GitHub account for secret storage")?;
        Ok(Sensitive(ser_string))
    }

    fn username(&self) -> String {
        match self {
            GitHubAccount::OAuth { username, .. } => username.to_string(),
            GitHubAccount::Pat { username, .. } => username.to_string(),
        }
    }

    fn access_token(&self) -> Sensitive<String> {
        match self {
            GitHubAccount::OAuth { access_token, .. } => access_token.clone(),
            GitHubAccount::Pat { access_token, .. } => access_token.clone(),
        }
    }

    fn set_access_token(&mut self, access_token: Sensitive<String>) {
        match self {
            GitHubAccount::OAuth {
                access_token: at, ..
            } => *at = access_token,
            GitHubAccount::Pat {
                access_token: at, ..
            } => *at = access_token,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SerializableGitHubAccount {
    OAuth {
        username: String,
        access_token: String,
    },
    Pat {
        username: String,
        access_token: String,
    },
}

impl From<&GitHubAccount> for SerializableGitHubAccount {
    fn from(account: &GitHubAccount) -> Self {
        match account {
            GitHubAccount::OAuth {
                username,
                access_token,
            } => SerializableGitHubAccount::OAuth {
                username: username.clone(),
                access_token: access_token.clone().0,
            },
            GitHubAccount::Pat {
                username,
                access_token,
            } => SerializableGitHubAccount::Pat {
                username: username.clone(),
                access_token: access_token.clone().0,
            },
        }
    }
}

/// List all known GitHub account keys.
fn retrieve_known_github_accounts_keys() -> Result<Vec<String>> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    if let Some(serialized_list) =
        secret::retrieve(GITHUB_KNOWN_ACCOUTNS_LIST_KEY, secret::Namespace::BuildKind)?
    {
        let list = serde_json::from_str::<Vec<String>>(&serialized_list.0)
            .context("Failed to deserialize known GitHub accounts list")?;
        Ok(list)
    } else {
        Ok(vec![])
    }
}

/// Add a GitHub account key to the known list, if not already present.
fn add_account_key_to_known_list(account_key: &str) -> Result<()> {
    let mut keys = retrieve_known_github_accounts_keys()?;
    if !keys.contains(&account_key.to_string()) {
        keys.push(account_key.to_string());
        static FAIR_QUEUE: Mutex<()> = Mutex::new(());
        let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
        let serialized_list = serde_json::to_string(&keys)
            .context("Failed to serialize known GitHub accounts list")?;
        secret::persist(
            GITHUB_KNOWN_ACCOUTNS_LIST_KEY,
            &Sensitive(serialized_list),
            secret::Namespace::BuildKind,
        )
    } else {
        Ok(())
    }
}

/// Delete a GitHub account key from the known list.
fn delete_account_key_from_known_list(account_key: &str) -> Result<()> {
    let mut keys = retrieve_known_github_accounts_keys()?;
    if let Some(pos) = keys.iter().position(|k| k == account_key) {
        keys.remove(pos);
        static FAIR_QUEUE: Mutex<()> = Mutex::new(());
        let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
        let serialized_list = serde_json::to_string(&keys)
            .context("Failed to serialize known GitHub accounts list")?;
        secret::persist(
            GITHUB_KNOWN_ACCOUTNS_LIST_KEY,
            &Sensitive(serialized_list),
            secret::Namespace::BuildKind,
        )
    } else {
        Ok(())
    }
}

fn retrieve_github_account(account_secret_key: &str) -> Result<Option<GitHubAccount>> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    if let Some(serialized_account) =
        secret::retrieve(account_secret_key, secret::Namespace::BuildKind)?
    {
        let ser =
            serde_json::from_str::<SerializableGitHubAccount>(&serialized_account.0.to_string())
                .context("Failed to deserialize GitHub account")?;
        Ok(Some(ser.into()))
    } else {
        Ok(None)
    }
}

fn persist_github_account(account: &GitHubAccount) -> Result<()> {
    let secret_key = account.secret_key();
    add_account_key_to_known_list(&secret_key)?;

    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::persist(
        &secret_key,
        &account.secret_value()?,
        secret::Namespace::BuildKind,
    )
}

fn delete_github_account(account: &GitHubAccount) -> Result<()> {
    let secret_key = account.secret_key();
    delete_account_key_from_known_list(&secret_key)?;
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::delete(&secret_key, secret::Namespace::BuildKind)
}

fn retrieve_github_account_map() -> Result<Vec<GitHubAccount>> {
    let keys = retrieve_known_github_accounts_keys()?;
    let mut accounts = vec![];
    for key in keys {
        if let Some(account) = retrieve_github_account(&key)? {
            accounts.push(account);
        }
    }
    Ok(accounts)
}

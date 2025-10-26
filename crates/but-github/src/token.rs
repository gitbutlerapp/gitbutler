use std::sync::Mutex;

use anyhow::Result;
use but_secret::{Sensitive, secret};
use serde::{Deserialize, Serialize};

/// Persist GitHub account access tokens securely.
pub fn persist_gh_access_token(
    account_id: &GithubAccountIdentifier,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::controller::Controller,
) -> Result<()> {
    let oauth_account = GitHubAccount::new(account_id, access_token.clone());
    persist_github_account(&oauth_account, storage)
}

/// Delete a GitHub account access token for a given username.
pub fn delete_gh_access_token(
    account_id: &GithubAccountIdentifier,
    storage: &but_forge_storage::controller::Controller,
) -> Result<()> {
    let account = find_github_account(account_id, storage)?;
    if let Some(account) = account {
        delete_github_account(&account, storage)
    } else {
        Ok(())
    }
}

/// Retrieve a GitHub account access token for a given username.
pub fn get_gh_access_token(
    account_id: &GithubAccountIdentifier,
    storage: &but_forge_storage::controller::Controller,
) -> Result<Option<Sensitive<String>>> {
    let account = find_github_account(account_id, storage)?;
    Ok(account.map(|acct| acct.access_token()))
}

pub fn list_known_github_usernames(
    storage: &but_forge_storage::controller::Controller,
) -> Result<Vec<String>> {
    Ok(storage
        .github_accounts()?
        .iter()
        .map(|account| account.username().to_string())
        .collect::<Vec<String>>())
}

pub fn clear_all_github_accounts(
    storage: &but_forge_storage::controller::Controller,
) -> Result<()> {
    delete_all_github_accounts(storage)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "username")]
pub enum GithubAccountIdentifier {
    OAuthUsername(String),
    PatUsername(String),
}

impl GithubAccountIdentifier {
    pub fn oauth(username: &str) -> Self {
        GithubAccountIdentifier::OAuthUsername(username.to_string())
    }
    pub fn pat(username: &str) -> Self {
        GithubAccountIdentifier::PatUsername(username.to_string())
    }
}

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
        }
    }
}

impl GitHubAccount {
    pub fn new(account_id: &GithubAccountIdentifier, access_token: Sensitive<String>) -> Self {
        match account_id {
            GithubAccountIdentifier::OAuthUsername(username) => GitHubAccount::OAuth {
                username: username.to_string(),
                access_token,
            },
            GithubAccountIdentifier::PatUsername(username) => GitHubAccount::Pat {
                username: username.to_string(),
                access_token,
            },
        }
    }

    fn secret_key(&self) -> String {
        match self {
            GitHubAccount::OAuth { username, .. } => format!("github_oauth_{}", username),
            GitHubAccount::Pat { username, .. } => format!("github_pat_{}", username),
        }
    }

    fn secret_value(&self) -> Result<Sensitive<String>> {
        Ok(self.access_token())
    }

    fn access_token(&self) -> Sensitive<String> {
        match self {
            GitHubAccount::OAuth { access_token, .. } => access_token.clone(),
            GitHubAccount::Pat { access_token, .. } => access_token.clone(),
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
    storage: &but_forge_storage::controller::Controller,
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

fn delete_github_account(
    account: &GitHubAccount,
    storage: &but_forge_storage::controller::Controller,
) -> Result<()> {
    let secret_key = account.secret_key();
    storage.remove_github_account(&account.into())?;

    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::delete(&secret_key, secret::Namespace::BuildKind)
}

fn delete_all_github_accounts(storage: &but_forge_storage::controller::Controller) -> Result<()> {
    let keys_to_delete = storage.clear_all_github_accounts()?;
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    for key in keys_to_delete {
        secret::delete(&key, secret::Namespace::BuildKind)?;
    }
    Ok(())
}

fn find_github_account(
    account_id: &GithubAccountIdentifier,
    storage: &but_forge_storage::controller::Controller,
) -> Result<Option<GitHubAccount>> {
    let accounts = storage.github_accounts()?;
    let result = match account_id {
        GithubAccountIdentifier::OAuthUsername(username) => accounts.iter().find_map(|account| {
            if let but_forge_storage::settings::GitHubAccount::OAuth {
                username: acct_username,
                access_token_key,
            } = account
                && acct_username == username
                && let Some(access_token) = retrieve_github_secret(access_token_key).ok().flatten()
            {
                return Some(GitHubAccount::OAuth {
                    username: acct_username.clone(),
                    access_token,
                });
            }
            None
        }),
        GithubAccountIdentifier::PatUsername(username) => accounts.iter().find_map(|account| {
            if let but_forge_storage::settings::GitHubAccount::Pat {
                username: acct_username,
                access_token_key,
            } = account
                && acct_username == username
                && let Some(access_token) = retrieve_github_secret(access_token_key).ok().flatten()
            {
                return Some(GitHubAccount::Pat {
                    username: acct_username.clone(),
                    access_token,
                });
            }
            None
        }),
    };
    Ok(result)
}

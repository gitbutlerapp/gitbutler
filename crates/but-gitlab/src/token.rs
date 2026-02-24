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

/// Delete a GitLab account access token for a given username.
pub fn delete_gl_access_token(
    account_id: &GitlabAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let account = find_gitlab_account(account_id, storage)?;
    if let Some(account) = account {
        delete_gitlab_account(&account, storage)
    } else {
        Ok(())
    }
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
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", tag = "type", content = "info")]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "./gitlab/token.ts"))]
pub enum GitlabAccountIdentifier {
    PatUsername { username: String },
    SelfHosted { username: String, host: String },
}

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
            GitLabAccount::Pat { username, .. } => format!("gitlab_pat_{username}"),
            GitLabAccount::SelfHosted { host, .. } => format!("gitlab_selfhosted_{host}"),
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

fn delete_gitlab_account(
    account: &GitLabAccount,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let secret_key = account.secret_key();
    storage.remove_gitlab_account(&account.into())?;

    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::delete(&secret_key, secret::Namespace::BuildKind)
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

fn find_gitlab_account(
    account_id: &GitlabAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<GitLabAccount>> {
    let accounts = storage.gitlab_accounts()?;
    let result = match account_id {
        GitlabAccountIdentifier::PatUsername { username } => accounts.iter().find_map(|account| {
            if let but_forge_storage::settings::GitLabAccount::Pat {
                username: acct_username,
                access_token_key,
            } = account
                && acct_username == username
                && let Some(access_token) = retrieve_gitlab_secret(access_token_key).ok().flatten()
            {
                return Some(GitLabAccount::Pat {
                    username: acct_username.clone(),
                    access_token,
                });
            }
            None
        }),
        GitlabAccountIdentifier::SelfHosted { username, host } => {
            accounts.iter().find_map(|account| {
                if let but_forge_storage::settings::GitLabAccount::SelfHosted {
                    username: acct_username,
                    host: acct_host,
                    access_token_key,
                } = account
                    && acct_host == host
                    && acct_username == username
                    && let Some(access_token) =
                        retrieve_gitlab_secret(access_token_key).ok().flatten()
                {
                    return Some(GitLabAccount::SelfHosted {
                        username: acct_username.clone(),
                        host: acct_host.clone(),
                        access_token,
                    });
                }
                None
            })
        }
    };
    Ok(result)
}

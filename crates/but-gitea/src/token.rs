use anyhow::Result;
use but_secret::{Sensitive, secret};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

/// Persist Gitea account access tokens securely.
pub fn persist_gitea_access_token(
    account_id: &GiteaAccountIdentifier,
    access_token: &Sensitive<String>,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    let account = GiteaAccount::new(account_id, access_token.clone());
    persist_gitea_account(&account, storage)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GiteaAccountIdentifier {
    pub username: String,
    pub host: String,
}

impl GiteaAccountIdentifier {
    pub fn new(username: &str, host: &str) -> Self {
        Self {
            username: username.to_string(),
            host: host.to_string(),
        }
    }
}

pub struct GiteaAccount {
    pub username: String,
    pub host: String,
    pub access_token: Sensitive<String>,
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
        // Sanitize host to be safe for filenames/keys if needed.
        // Simple replacement of chars that might be problematic.
        let safe_host = self.host.replace(['.', ':', '/'], "_");
        format!("gitea_{}_{}", safe_host, self.username)
    }
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
        GiteaAccountIdentifier {
            username: account.username.clone(),
            host: account.host.clone(),
        }
    }
}

fn persist_gitea_account(
    account: &GiteaAccount,
    storage: &but_forge_storage::Controller,
) -> Result<()> {
    storage.add_gitea_account(&account.into())?;

    let secret_key = account.secret_key();
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    secret::persist(
        &secret_key,
        &account.access_token,
        secret::Namespace::BuildKind,
    )
}

/// Retrieve a Gitea account access token.
pub fn get_gitea_access_token(
    account_id: &GiteaAccountIdentifier,
    storage: &but_forge_storage::Controller,
) -> Result<Option<Sensitive<String>>> {
    let accounts = storage.gitea_accounts()?;
    let stored_account = accounts
        .iter()
        .find(|a| a.username == account_id.username && a.host == account_id.host);

    if let Some(stored_account) = stored_account {
        static FAIR_QUEUE: Mutex<()> = Mutex::new(());
        let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
        but_secret::secret::retrieve(
            &stored_account.access_token_key,
            secret::Namespace::BuildKind,
        )
    } else {
        Ok(None)
    }
}

/// List known Gitea accounts.
pub fn list_known_gitea_accounts(
    storage: &but_forge_storage::Controller,
) -> Result<Vec<GiteaAccountIdentifier>> {
    let accounts = storage.gitea_accounts()?;
    Ok(accounts.iter().map(Into::into).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_key_generation() {
        let account = GiteaAccount {
            username: "testuser".to_string(),
            host: "https://gitea.com".to_string(),
            access_token: Sensitive("token".to_string()),
        };
        // Expecting sanitized host
        assert_eq!(account.secret_key(), "gitea_https___gitea_com_testuser");
    }
}

use std::{path::PathBuf, sync::Mutex};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::settings::{ForgeSettings, GitHubAccount};

const FORGE_SETTINGS_FILE: &str = "forge_settings.json";

#[derive(Debug, Clone)]
pub(crate) struct Storage {
    inner: gitbutler_storage::Storage,
}

impl Storage {
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Storage {
            inner: gitbutler_storage::Storage::new(path),
        }
    }

    pub fn read(&self) -> Result<ForgeSettings> {
        let settings = match self.inner.read(FORGE_SETTINGS_FILE)? {
            Some(settings) => {
                let settings: ForgeSettings = serde_json::from_str(&settings)?;
                settings
            }
            None => Default::default(),
        };

        self.migrate_accounts(settings)
    }

    pub fn save(&self, settings: &ForgeSettings) -> Result<()> {
        let data = serde_json::to_string_pretty(settings)?;
        self.inner.write(FORGE_SETTINGS_FILE, &data)?;
        Ok(())
    }

    /// Migrate known GitHub accounts from secret storage to the new settings structure.
    /// This can be removed after the stable release.
    fn migrate_accounts(&self, settings: ForgeSettings) -> Result<ForgeSettings> {
        let old_handle = "github_known_accounts";
        let namespace = but_secret::secret::Namespace::BuildKind;

        if settings.github.known_accounts.is_empty()
            && let Some(known_accounts) = but_secret::secret::retrieve(old_handle, namespace)?
        {
            // Migrate old known accounts from secret storage
            let known_account_keys: Vec<String> = serde_json::from_str(&known_accounts)?;
            let mut new_settings = settings.clone();
            for account_key in known_account_keys {
                if let Some(migrated_account) = migrate_account(&account_key)? {
                    new_settings.github.known_accounts.push(migrated_account);
                }
            }
            self.save(&new_settings)?;
            return Ok(new_settings);
        }

        Ok(settings)
    }
}

// ===============================
// Migration of GitHub accounts.
//
// All of the code below this line is only needed to migrate old GitHub accounts,
// and can be removed after the stable release.
// ===============================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DeprecatedSerializableGitHubAccount {
    OAuth {
        username: String,
        access_token: String,
    },
    Pat {
        username: String,
        access_token: String,
    },
}

fn read_deprecated_github_secret(
    account_secret_handle: &str,
) -> Result<Option<DeprecatedSerializableGitHubAccount>> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    if let Some(secret_value) = but_secret::secret::retrieve(
        account_secret_handle,
        but_secret::secret::Namespace::BuildKind,
    )? {
        let account: DeprecatedSerializableGitHubAccount = serde_json::from_str(&secret_value)?;
        Ok(Some(account))
    } else {
        Ok(None)
    }
}

fn store_access_token_in_secret(account_secret_handle: &str, access_token: &str) -> Result<()> {
    static FAIR_QUEUE: Mutex<()> = Mutex::new(());
    let _one_at_a_time_to_prevent_races = FAIR_QUEUE.lock().unwrap();
    but_secret::secret::persist(
        account_secret_handle,
        &but_secret::Sensitive(access_token.to_string()),
        but_secret::secret::Namespace::BuildKind,
    )
}

fn migrate_account(account_secret_handle: &str) -> Result<Option<GitHubAccount>> {
    // Read the deprecated account structure.
    // If it fails to parse it we assume there is nothing to migrate.
    if let Some(deprecated_account) = read_deprecated_github_secret(account_secret_handle)
        .ok()
        .flatten()
    {
        let new_account = match deprecated_account {
            DeprecatedSerializableGitHubAccount::OAuth {
                username,
                access_token,
            } => {
                // Store the plain access token in the same secret
                store_access_token_in_secret(account_secret_handle, &access_token)?;
                GitHubAccount::OAuth {
                    username: username.clone(),
                    access_token_key: account_secret_handle.to_string(),
                }
            }
            DeprecatedSerializableGitHubAccount::Pat {
                username,
                access_token,
            } => {
                // Store the plain access token in the same secret
                store_access_token_in_secret(account_secret_handle, &access_token)?;
                GitHubAccount::Pat {
                    username: username.clone(),
                    access_token_key: account_secret_handle.to_string(),
                }
            }
        };

        return Ok(Some(new_account));
    }
    Ok(None)
}

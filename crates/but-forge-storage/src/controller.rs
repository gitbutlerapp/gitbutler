use std::path::PathBuf;

use crate::storage;

#[derive(Clone, Debug)]
pub struct Controller {
    settings_storage: storage::Storage,
}

impl Controller {
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            settings_storage: storage::Storage::from_path(&path),
        }
    }

    /// Get all known GitHub accounts.
    pub fn github_accounts(&self) -> anyhow::Result<Vec<crate::settings::GitHubAccount>> {
        let settings = self.read_settings()?;
        Ok(settings.github.known_accounts)
    }

    /// Add a GitHub account if it does not already exist.
    pub fn add_github_account(
        &self,
        account: &crate::settings::GitHubAccount,
    ) -> anyhow::Result<()> {
        let mut settings = self.read_settings()?;

        if settings.github.known_accounts.iter().any(|a| a == account) {
            return Ok(());
        }

        settings.github.known_accounts.push(account.to_owned());
        self.save_settings(&settings)
    }

    /// Clear all GitHub accounts.
    /// Returns the list of access token keys that should be deleted.
    pub fn clear_all_github_accounts(&self) -> anyhow::Result<Vec<String>> {
        let mut settings = self.read_settings()?;
        let access_tokens_to_delete = settings
            .github
            .known_accounts
            .iter()
            .map(|account| account.access_token_key().to_string())
            .collect::<Vec<String>>();
        settings.github.known_accounts.clear();
        self.save_settings(&settings)?;

        Ok(access_tokens_to_delete)
    }

    /// Remove a GitHub account.
    pub fn remove_github_account(
        &self,
        account: &crate::settings::GitHubAccount,
    ) -> anyhow::Result<()> {
        let mut settings = self.read_settings()?;

        settings.github.known_accounts.retain(|a| a != account);

        self.save_settings(&settings)
    }

    fn read_settings(&self) -> anyhow::Result<crate::settings::ForgeSettings> {
        self.settings_storage.read()
    }

    fn save_settings(&self, settings: &crate::settings::ForgeSettings) -> anyhow::Result<()> {
        self.settings_storage.save(settings)
    }
}

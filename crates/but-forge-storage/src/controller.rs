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
        for key in &access_tokens_to_delete {
            settings.cached_profiles.remove(key);
        }
        settings.github.known_accounts.clear();
        self.save_settings(&settings)?;

        Ok(access_tokens_to_delete)
    }

    /// Get the cached profile for an account by its `access_token_key`.
    pub fn cached_profile(
        &self,
        key: &str,
    ) -> anyhow::Result<Option<crate::settings::CachedProfile>> {
        let settings = self.read_settings()?;
        Ok(settings.cached_profiles.get(key).cloned())
    }

    /// Set or clear the cached profile for an account by its `access_token_key`.
    pub fn set_cached_profile(
        &self,
        key: &str,
        profile: Option<crate::settings::CachedProfile>,
    ) -> anyhow::Result<()> {
        let mut settings = self.read_settings()?;
        match profile {
            Some(p) => settings.cached_profiles.insert(key.to_owned(), p),
            None => settings.cached_profiles.remove(key),
        };
        self.save_settings(&settings)
    }

    /// Remove a GitHub account and its cached profile.
    pub fn remove_github_account(
        &self,
        account: &crate::settings::GitHubAccount,
    ) -> anyhow::Result<()> {
        let mut settings = self.read_settings()?;
        settings.cached_profiles.remove(account.access_token_key());
        settings.github.known_accounts.retain(|a| a != account);
        self.save_settings(&settings)
    }

    /// Get all known GitLab accounts.
    pub fn gitlab_accounts(&self) -> anyhow::Result<Vec<crate::settings::GitLabAccount>> {
        let settings = self.read_settings()?;
        Ok(settings.gitlab.known_accounts)
    }

    /// Add a GitLab account if it does not already exist.
    pub fn add_gitlab_account(
        &self,
        account: &crate::settings::GitLabAccount,
    ) -> anyhow::Result<()> {
        let mut settings = self.read_settings()?;

        if settings.gitlab.known_accounts.iter().any(|a| a == account) {
            return Ok(());
        }

        settings.gitlab.known_accounts.push(account.to_owned());
        self.save_settings(&settings)
    }

    /// Clear all GitLab accounts.
    /// Returns the list of access token keys that should be deleted.
    pub fn clear_all_gitlab_accounts(&self) -> anyhow::Result<Vec<String>> {
        let mut settings = self.read_settings()?;
        let access_tokens_to_delete = settings
            .gitlab
            .known_accounts
            .iter()
            .map(|account| account.access_token_key().to_string())
            .collect::<Vec<String>>();
        for key in &access_tokens_to_delete {
            settings.cached_profiles.remove(key);
        }
        settings.gitlab.known_accounts.clear();
        self.save_settings(&settings)?;

        Ok(access_tokens_to_delete)
    }

    /// Remove a GitLab account and its cached profile.
    pub fn remove_gitlab_account(
        &self,
        account: &crate::settings::GitLabAccount,
    ) -> anyhow::Result<()> {
        let mut settings = self.read_settings()?;
        settings.cached_profiles.remove(account.access_token_key());
        settings.gitlab.known_accounts.retain(|a| a != account);
        self.save_settings(&settings)
    }

    /// Get all known Bitbucket accounts.
    pub fn bitbucket_accounts(&self) -> anyhow::Result<Vec<crate::settings::BitbucketAccount>> {
        let settings = self.read_settings()?;
        Ok(settings.bitbucket.known_accounts)
    }

    /// Add a Bitbucket account if it does not already exist.
    pub fn add_bitbucket_account(
        &self,
        account: &crate::settings::BitbucketAccount,
    ) -> anyhow::Result<()> {
        let mut settings = self.read_settings()?;

        if settings
            .bitbucket
            .known_accounts
            .iter()
            .any(|a| a == account)
        {
            return Ok(());
        }

        settings.bitbucket.known_accounts.push(account.to_owned());
        self.save_settings(&settings)
    }

    /// Clear all Bitbucket accounts.
    /// Returns the list of access token keys that should be deleted.
    pub fn clear_all_bitbucket_accounts(&self) -> anyhow::Result<Vec<String>> {
        let mut settings = self.read_settings()?;
        let access_tokens_to_delete = settings
            .bitbucket
            .known_accounts
            .iter()
            .map(|account| account.access_token_key().to_string())
            .collect::<Vec<String>>();
        for key in &access_tokens_to_delete {
            settings.cached_profiles.remove(key);
        }
        settings.bitbucket.known_accounts.clear();
        self.save_settings(&settings)?;

        Ok(access_tokens_to_delete)
    }

    /// Remove a Bitbucket account and its cached profile.
    pub fn remove_bitbucket_account(
        &self,
        account: &crate::settings::BitbucketAccount,
    ) -> anyhow::Result<()> {
        let mut settings = self.read_settings()?;
        settings.cached_profiles.remove(account.access_token_key());
        settings.bitbucket.known_accounts.retain(|a| a != account);
        self.save_settings(&settings)
    }

    // forgejo-fork
    /// Get all known Forgejo accounts.
    pub fn forgejo_accounts(&self) -> anyhow::Result<Vec<crate::settings::ForgejoAccount>> {
        Ok(self.read_settings()?.forgejo.known_accounts)
    }

    /// Add a Forgejo account if it does not already exist.
    pub fn add_forgejo_account(
        &self,
        account: &crate::settings::ForgejoAccount,
    ) -> anyhow::Result<()> {
        let mut settings = self.read_settings()?;
        if settings.forgejo.known_accounts.contains(account) {
            return Ok(());
        }
        settings.forgejo.known_accounts.push(account.to_owned());
        self.save_settings(&settings)
    }

    /// Clear all Forgejo accounts.
    /// Returns the list of access token keys that should be deleted.
    pub fn clear_all_forgejo_accounts(&self) -> anyhow::Result<Vec<String>> {
        let mut settings = self.read_settings()?;
        let keys = settings
            .forgejo
            .known_accounts
            .drain(..)
            .map(|account| account.access_token_key)
            .collect::<Vec<_>>();
        for key in &keys {
            settings.cached_profiles.remove(key);
        }
        self.save_settings(&settings)?;
        Ok(keys)
    }

    /// Remove a Forgejo account and its cached profile.
    pub fn remove_forgejo_account(
        &self,
        account: &crate::settings::ForgejoAccount,
    ) -> anyhow::Result<()> {
        let mut settings = self.read_settings()?;
        settings.cached_profiles.remove(&account.access_token_key);
        settings.forgejo.known_accounts.retain(|a| a != account);
        self.save_settings(&settings)
    }

    fn read_settings(&self) -> anyhow::Result<crate::settings::ForgeSettings> {
        self.settings_storage.read()
    }

    fn save_settings(&self, settings: &crate::settings::ForgeSettings) -> anyhow::Result<()> {
        self.settings_storage.save(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{
        BitbucketAccount, CachedProfile, ForgejoAccount, GitHubAccount, GitLabAccount,
    };

    fn test_controller() -> (Controller, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let controller = Controller::from_path(dir.path());
        (controller, dir)
    }

    #[test]
    fn remove_github_account_clears_cached_profile() {
        let (controller, _dir) = test_controller();
        let account = GitHubAccount::Pat {
            username: "testuser".into(),
            access_token_key: "github_pat_testuser".into(),
        };
        controller.add_github_account(&account).unwrap();
        controller
            .set_cached_profile(
                "github_pat_testuser",
                Some(CachedProfile {
                    name: Some("Test".into()),
                    ..Default::default()
                }),
            )
            .unwrap();

        assert!(
            controller
                .cached_profile("github_pat_testuser")
                .unwrap()
                .is_some()
        );
        controller.remove_github_account(&account).unwrap();
        assert!(
            controller
                .cached_profile("github_pat_testuser")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn clear_all_github_accounts_clears_cached_profiles() {
        let (controller, _dir) = test_controller();
        let account = GitHubAccount::OAuth {
            username: "user1".into(),
            access_token_key: "github_oauth_user1".into(),
        };
        controller.add_github_account(&account).unwrap();
        controller
            .set_cached_profile(
                "github_oauth_user1",
                Some(CachedProfile {
                    name: Some("User 1".into()),
                    ..Default::default()
                }),
            )
            .unwrap();

        let keys = controller.clear_all_github_accounts().unwrap();
        assert_eq!(keys, vec!["github_oauth_user1"]);
        assert!(
            controller
                .cached_profile("github_oauth_user1")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn remove_gitlab_account_clears_cached_profile() {
        let (controller, _dir) = test_controller();
        let account = GitLabAccount::Pat {
            username: "gluser".into(),
            access_token_key: "gitlab_pat_gluser".into(),
        };
        controller.add_gitlab_account(&account).unwrap();
        controller
            .set_cached_profile(
                "gitlab_pat_gluser",
                Some(CachedProfile {
                    email: Some("gl@test.com".into()),
                    ..Default::default()
                }),
            )
            .unwrap();

        assert!(
            controller
                .cached_profile("gitlab_pat_gluser")
                .unwrap()
                .is_some()
        );
        controller.remove_gitlab_account(&account).unwrap();
        assert!(
            controller
                .cached_profile("gitlab_pat_gluser")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn forgejo_accounts_on_one_host_are_kept_apart() {
        let (controller, _dir) = test_controller();
        let alice = ForgejoAccount {
            host: "https://git.example.com".into(),
            username: "alice".into(),
            access_token_key: "forgejo_https://git.example.com_alice".into(),
        };
        let bob = ForgejoAccount {
            username: "bob".into(),
            access_token_key: "forgejo_https://git.example.com_bob".into(),
            ..alice.clone()
        };
        controller.add_forgejo_account(&alice).unwrap();
        controller.add_forgejo_account(&bob).unwrap();
        controller.add_forgejo_account(&bob).unwrap();
        assert_eq!(
            controller.forgejo_accounts().unwrap(),
            vec![alice.clone(), bob.clone()],
            "two users on the same host are distinct accounts, and re-adding is a no-op"
        );

        controller.remove_forgejo_account(&alice).unwrap();
        assert_eq!(controller.forgejo_accounts().unwrap(), vec![bob.clone()]);
        assert_eq!(
            controller.clear_all_forgejo_accounts().unwrap(),
            vec![bob.access_token_key],
            "clearing hands back the keychain keys to delete"
        );
        assert!(controller.forgejo_accounts().unwrap().is_empty());
    }

    #[test]
    fn remove_bitbucket_account_clears_cached_profile() {
        let (controller, _dir) = test_controller();
        let account = BitbucketAccount::ApiToken {
            email: "bb@test.com".into(),
            access_token_key: "bitbucket_apitoken_bb@test.com".into(),
        };
        controller.add_bitbucket_account(&account).unwrap();
        controller
            .set_cached_profile(
                "bitbucket_apitoken_bb@test.com",
                Some(CachedProfile {
                    email: Some("bb@test.com".into()),
                    ..Default::default()
                }),
            )
            .unwrap();

        assert!(
            controller
                .cached_profile("bitbucket_apitoken_bb@test.com")
                .unwrap()
                .is_some()
        );
        controller.remove_bitbucket_account(&account).unwrap();
        assert!(
            controller
                .cached_profile("bitbucket_apitoken_bb@test.com")
                .unwrap()
                .is_none()
        );
    }
}

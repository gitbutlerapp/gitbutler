use std::any::Any;

use but_forge_storage::settings::GitHubAccount;
use but_github::{GithubAccountIdentifier, check_credentials};
use keyring::credential::{CredentialApi, CredentialBuilderApi};

struct LockedKeychain;

impl CredentialApi for LockedKeychain {
    fn set_secret(&self, _secret: &[u8]) -> keyring::Result<()> {
        Err(locked())
    }

    fn get_secret(&self) -> keyring::Result<Vec<u8>> {
        Err(locked())
    }

    fn delete_credential(&self) -> keyring::Result<()> {
        Err(locked())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl CredentialBuilderApi for LockedKeychain {
    fn build(
        &self,
        _target: Option<&str>,
        _service: &str,
        _user: &str,
    ) -> keyring::Result<Box<keyring::Credential>> {
        Ok(Box::new(LockedKeychain))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn locked() -> keyring::Error {
    keyring::Error::NoStorageAccess("keychain is locked".into())
}

#[tokio::test(flavor = "current_thread")]
async fn unreadable_token_is_reported_instead_of_missing_credentials() {
    keyring::set_default_credential_builder(Box::new(LockedKeychain));
    let dir = tempfile::tempdir().unwrap();
    let storage = but_forge_storage::Controller::from_path(dir.path());
    storage
        .add_github_account(&GitHubAccount::OAuth {
            username: "alice".into(),
            access_token_key: "github_oauth_alice".into(),
        })
        .unwrap();

    let err = check_credentials(&GithubAccountIdentifier::oauth("alice"), &storage)
        .await
        .expect_err("a keychain that refuses the read must not look like a missing token");

    assert!(
        format!("{err:#}").contains("keychain is locked"),
        "the keychain's reason reaches the caller, got: {err:#}"
    );
}

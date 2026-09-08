use but_forge_storage::settings::GitHubAccount;
use but_github::{GithubAccountIdentifier, forget_gh_access_token, list_known_github_accounts};

/// Forgetting must not depend on the token: a different build kind may have stored it,
/// or the keychain may refuse the read, and the account still has to go.
#[test]
fn forget_removes_an_account_whose_token_is_missing() {
    keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
    let dir = tempfile::tempdir().unwrap();
    let storage = but_forge_storage::Controller::from_path(dir.path());
    storage
        .add_github_account(&GitHubAccount::OAuth {
            username: "alice".into(),
            access_token_key: "github_oauth_alice".into(),
        })
        .unwrap();
    let account = GithubAccountIdentifier::oauth("alice");
    assert_eq!(
        list_known_github_accounts(&storage).unwrap(),
        std::slice::from_ref(&account)
    );

    forget_gh_access_token(&account, &storage).unwrap();

    assert_eq!(list_known_github_accounts(&storage).unwrap(), []);
}

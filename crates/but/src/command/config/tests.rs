use super::*;

#[test]
fn forgotten_accounts_print_as_config_forge_lists_them() {
    let account = AccountToForget::GitHub(but_github::GithubAccountIdentifier::OAuthUsername {
        username: "alice".to_owned(),
    });
    let json = serde_json::to_value(ForgetOutcome::Forgot(Vec::from([account])).on_json()).unwrap();

    assert_eq!(
        json,
        serde_json::json!({
            "forgotten": [{
                "provider": "GitHub",
                "username": "alice",
                "account_type": "OAuth",
            }]
        })
    );
}

use but_error::{AnyhowContextExt as _, Code::ForgeNotAuthenticated};
use but_forge_storage::{
    Controller,
    settings::{BitbucketAccount, GitHubAccount, GitLabAccount},
};

use but_forge::{ForgeName, ForgeRepoInfo, ForgeUser};

pub(crate) fn accounts() -> (tempfile::TempDir, Controller, Vec<ForgeUser>) {
    let tmp = tempfile::tempdir().unwrap();
    let storage = Controller::from_path(tmp.path());
    let github = GitHubAccount::OAuth {
        username: "alice".into(),
        access_token_key: "unused".into(),
    };
    let gitlab = GitLabAccount::Pat {
        username: "alice".into(),
        access_token_key: "unused".into(),
    };
    let bitbucket = BitbucketAccount::ApiToken {
        email: "alice@example.com".into(),
        access_token_key: "unused".into(),
    };
    storage.add_github_account(&github).unwrap();
    storage.add_gitlab_account(&gitlab).unwrap();
    storage.add_bitbucket_account(&bitbucket).unwrap();
    let users = vec![
        ForgeUser::GitHub((&github).into()),
        ForgeUser::GitLab((&gitlab).into()),
        ForgeUser::Bitbucket((&bitbucket).into()),
    ];
    (tmp, storage, users)
}

pub(crate) fn repo(forge: ForgeName, host: &str) -> ForgeRepoInfo {
    ForgeRepoInfo {
        forge,
        host: host.into(),
        port: None,
        owner: "owner".into(),
        repo: "repo".into(),
        protocol: "https".into(),
    }
}

pub(crate) fn assert_auth_error(error: anyhow::Error) {
    assert!(
        error
            .custom_context()
            .is_some_and(|context| context.code == ForgeNotAuthenticated),
        "typed auth error"
    );
    let message = error.to_string();
    assert!(
        message.starts_with("Not authenticated with") && message.contains("matches"),
        "host/account rejection precedes token lookup: {message}"
    );
}

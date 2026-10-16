mod association;
mod ci;
mod db;
mod forge;
mod forge_info;
mod merge_message;
mod repo;
mod review;
mod test_support;

use but_forge::{ForgeName, ForgeRepoInfo, ForgeUser, current_forge_login, derive_forge_repo_info};

#[test]
fn derives_supported_forge_repository_urls() {
    let cases = [
        (
            "git@github.com:owner/example.github.io.git",
            ForgeName::GitHub,
            "owner",
            "example.github.io",
            "ssh",
        ),
        (
            "https://github.com/owner/example.github.io.git",
            ForgeName::GitHub,
            "owner",
            "example.github.io",
            "https",
        ),
        (
            "https://github.com/owner/repository.git.git",
            ForgeName::GitHub,
            "owner",
            "repository.git",
            "https",
        ),
        (
            "https://github.com/owner/repo/",
            ForgeName::GitHub,
            "owner",
            "repo",
            "https",
        ),
        (
            "https://gitlab.com/group/subgroup/repo.git",
            ForgeName::GitLab,
            "group/subgroup",
            "repo",
            "https",
        ),
        (
            "git@bitbucket.org:owner/repo.git",
            ForgeName::Bitbucket,
            "owner",
            "repo",
            "ssh",
        ),
        (
            "https://dev.azure.com/org/project/_git/repo",
            ForgeName::Azure,
            "org/project",
            "repo",
            "https",
        ),
        (
            "git@ssh.dev.azure.com:v3/org/project/repo.git",
            ForgeName::Azure,
            "org/project",
            "repo",
            "ssh",
        ),
    ];

    for (url, forge, owner, repo, protocol) in cases {
        let actual = derive_forge_repo_info(url, None).unwrap();
        assert_eq!(actual.forge, forge, "forge should be derived from {url}");
        assert_eq!(actual.owner, owner, "owner should be derived from {url}");
        assert_eq!(actual.repo, repo, "repository should be derived from {url}");
        assert_eq!(
            actual.protocol, protocol,
            "protocol should be derived from {url}"
        );
    }
}

#[test]
fn azure_encoded_path_characters_are_not_url_delimiters() {
    for (url, expected_repo) in [
        (
            "https://dev.azure.com/org/project/_git/repo%23one",
            "repo#one",
        ),
        (
            "https://dev.azure.com/org/project/_git/repo%3Fone",
            "repo?one",
        ),
        (
            "https://dev.azure.com/org/project/_git/repo%23one?query=value#fragment",
            "repo#one",
        ),
        (
            "https://dev.azure.com/org/project/_git/repo%2523one",
            "repo%23one",
        ),
        ("git@ssh.dev.azure.com:v3/org/project/repo#one", "repo#one"),
        ("git@ssh.dev.azure.com:v3/org/project/repo?one", "repo?one"),
        (
            "ssh://git@ssh.dev.azure.com/v3/org/project/repo%23one",
            "repo#one",
        ),
    ] {
        let actual = derive_forge_repo_info(url, None).unwrap();
        assert_eq!(actual.owner, "org/project", "owner should survive in {url}");
        assert_eq!(
            actual.repo, expected_repo,
            "encoded path characters are not query or fragment delimiters in {url}"
        );
    }
}

#[test]
fn azure_https_organization_can_be_named_v3() {
    let actual =
        derive_forge_repo_info("https://dev.azure.com/v3/project/_git/repo", None).unwrap();
    assert_eq!(actual.forge, ForgeName::Azure, "host identifies Azure");
    assert_eq!(
        actual.owner, "v3/project",
        "v3 is an HTTPS organization, not an SSH path marker"
    );
    assert_eq!(actual.repo, "repo", "repository name should survive");
}

#[test]
fn rejects_non_forge_and_malformed_repository_urls() {
    for url in [
        "../local-repo",
        "file:///tmp/repo",
        "https://github.com/owner",
    ] {
        for override_forge in [None, Some(ForgeName::GitLab)] {
            assert!(
                derive_forge_repo_info(url, override_forge).is_none(),
                "an override must not make {url} identify a hosted forge repository"
            );
        }
    }
}

fn github_repo_info() -> ForgeRepoInfo {
    ForgeRepoInfo {
        forge: ForgeName::GitHub,
        host: "github.com".into(),
        port: None,
        owner: "gitbutlerapp".into(),
        repo: "gitbutler".into(),
        protocol: "https".into(),
    }
}

fn gitlab_repo_info() -> ForgeRepoInfo {
    ForgeRepoInfo {
        forge: ForgeName::GitLab,
        host: "gitlab.com".into(),
        port: None,
        owner: "gitbutlerapp".into(),
        repo: "gitbutler".into(),
        protocol: "https".into(),
    }
}

fn github_storage(usernames: &[&str]) -> (but_forge_storage::Controller, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let storage = but_forge_storage::Controller::from_path(tmp.path());
    for username in usernames {
        storage
            .add_github_account(&but_forge_storage::settings::GitHubAccount::OAuth {
                username: (*username).into(),
                access_token_key: format!("github_oauth_{username}"),
            })
            .unwrap();
    }
    (storage, tmp)
}

fn gitlab_storage(usernames: &[&str]) -> (but_forge_storage::Controller, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let storage = but_forge_storage::Controller::from_path(tmp.path());
    for username in usernames {
        storage
            .add_gitlab_account(&but_forge_storage::settings::GitLabAccount::Pat {
                username: (*username).into(),
                access_token_key: format!("gitlab_pat_{username}"),
            })
            .unwrap();
    }
    (storage, tmp)
}
#[test]
fn current_login_uses_the_first_account_without_a_preference() {
    let (github, _gh_tmp) = github_storage(&["alice"]);
    let (gitlab, _gl_tmp) = gitlab_storage(&["alice"]);

    assert_eq!(
        current_forge_login(&None, &github_repo_info(), &github).unwrap(),
        Some("alice".into()),
        "the GitHub client defaults to the first stored account"
    );
    assert_eq!(
        current_forge_login(&None, &gitlab_repo_info(), &gitlab).unwrap(),
        Some("alice".into()),
        "the GitLab client defaults to the first stored account"
    );
}

#[test]
fn current_login_uses_the_configured_preferred_account() {
    let (github, _gh_tmp) = github_storage(&["alice", "bob"]);
    let (gitlab, _gl_tmp) = gitlab_storage(&["alice", "bob"]);

    assert_eq!(
        current_forge_login(
            &Some(ForgeUser::GitHub(
                but_github::GithubAccountIdentifier::oauth("bob"),
            )),
            &github_repo_info(),
            &github,
        )
        .unwrap(),
        Some("bob".into()),
        "a stored preferred account is the one the GitHub client authenticates as"
    );
    assert_eq!(
        current_forge_login(
            &Some(ForgeUser::GitLab(but_gitlab::GitlabAccountIdentifier::pat(
                "bob"
            ),)),
            &gitlab_repo_info(),
            &gitlab,
        )
        .unwrap(),
        Some("bob".into()),
        "a stored preferred account is the one the GitLab client authenticates as"
    );
}

#[test]
fn current_login_rejects_a_missing_preferred_account() {
    let (github, _gh_tmp) = github_storage(&["alice"]);
    let (gitlab, _gl_tmp) = gitlab_storage(&["alice"]);

    assert_eq!(
        current_forge_login(
            &Some(ForgeUser::GitHub(
                but_github::GithubAccountIdentifier::oauth("forgotten"),
            )),
            &github_repo_info(),
            &github,
        )
        .unwrap(),
        None,
        "the GitHub client refuses a preferred account gone from storage, so no login is reported"
    );
    assert_eq!(
        current_forge_login(
            &Some(ForgeUser::GitLab(but_gitlab::GitlabAccountIdentifier::pat(
                "forgotten"
            ),)),
            &gitlab_repo_info(),
            &gitlab,
        )
        .unwrap(),
        None,
        "the GitLab client refuses a preferred account gone from storage, so no login is reported"
    );
}
#[test]
fn derive_repo_info_uses_override_for_self_hosted_url() {
    // End-to-end for #14319: resolve the override before parsing the
    // forge-specific repository path. This synthetic host has no matching
    // account, so only the override can identify the forge.
    let info = derive_forge_repo_info(
        "https://git.selfhosted.example/group/repo.git",
        Some(ForgeName::GitLab),
    )
    .expect("override should resolve the forge for an otherwise-unknown host");
    assert_eq!(info.forge, ForgeName::GitLab);
    assert_eq!(info.owner, "group");
    assert_eq!(info.repo, "repo");
    assert_eq!(info.protocol, "https");
}

#[test]
fn override_uses_forge_specific_parser_for_encoded_and_nested_paths() {
    for (url, forge, owner, repo, protocol) in [
        (
            "https://git.selfhosted.example/group/subgroup/repo%23one.git.git?query=value#fragment",
            ForgeName::GitLab,
            "group/subgroup",
            "repo#one.git",
            "https",
        ),
        (
            "ssh://git@git.selfhosted.example:2222/group/subgroup/repo%23one.git",
            ForgeName::GitLab,
            "group/subgroup",
            "repo#one",
            "ssh",
        ),
        (
            "https://git.selfhosted.example/v3/My%20Project/_git/repo%3Fone",
            ForgeName::Azure,
            "v3/My Project",
            "repo?one",
            "https",
        ),
        (
            "git@git.selfhosted.example:v3/org/project/repo#one.git",
            ForgeName::Azure,
            "org/project",
            "repo#one",
            "ssh",
        ),
    ] {
        let info = derive_forge_repo_info(url, Some(forge.clone()))
            .expect("the override must select the forge-specific repository parser");
        assert_eq!(info.forge, forge, "the override must identify {url}");
        assert_eq!(info.owner, owner, "the namespace must survive in {url}");
        assert_eq!(info.repo, repo, "path decoding must stay correct in {url}");
        assert_eq!(
            info.protocol, protocol,
            "the transport must survive in {url}"
        );
    }
}

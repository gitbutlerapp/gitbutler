use but_forge::{ForgeName, ForgeUser, compare_branch_url, forge_info};

// Recognizable hosts resolve without account storage; override tests use
// synthetic hosts with no matching stored accounts.

#[test]
fn override_preserves_self_hosted_https_origin_and_encoded_namespace() {
    let remote = "https://git.selfhosted.example:8080/group/subgroup/My%20Repo%23one.git?query=value#fragment";
    let info = forge_info(remote, &[], Some(ForgeName::GitLab))
        .expect("the override must identify the self-hosted GitLab repository");
    assert_eq!(
        info.name,
        ForgeName::GitLab,
        "the saved override is the fallback"
    );
    assert_eq!(
        info.base_url, "https://git.selfhosted.example:8080/group/subgroup/My%20Repo%23one",
        "the remote's origin and encoded nested path must survive the override"
    );
    assert_eq!(
        compare_branch_url(remote, "main", "feat", None, &[], Some(ForgeName::GitLab)).unwrap(),
        "https://git.selfhosted.example:8080/group/subgroup/My%20Repo%23one/-/compare/main...feat",
        "compare URLs must honor the same override, origin, and path as forge_info"
    );
}

#[test]
fn override_uses_configured_account_origin_for_ssh_remote() {
    let remote = "ssh://git@git.selfhosted.example:2222/group/subgroup/My%20Repo%23one.git";
    let accounts = [ForgeUser::GitLab(
        but_gitlab::GitlabAccountIdentifier::selfhosted(
            "bob",
            "http://git.selfhosted.example:8080/api/v4",
        ),
    )];
    let info = forge_info(remote, &accounts, Some(ForgeName::GitLab)).unwrap();
    assert_eq!(
        info.base_url, "http://git.selfhosted.example:8080/group/subgroup/My%20Repo%23one",
        "the account's web scheme and port must replace the SSH transport origin"
    );
    assert_eq!(
        compare_branch_url(
            remote,
            "main",
            "feat",
            None,
            &accounts,
            Some(ForgeName::GitLab)
        )
        .unwrap(),
        "http://git.selfhosted.example:8080/group/subgroup/My%20Repo%23one/-/compare/main...feat",
        "compare URLs must preserve the account origin and encoded path with an override"
    );
}

#[test]
fn detected_azure_host_wins_over_override_with_encoded_paths() {
    for remote in [
        "https://dev.azure.com/org/My%20Project/_git/repo%23one",
        "ssh://git@ssh.dev.azure.com:2222/v3/org/My%20Project/repo%23one",
    ] {
        let info = forge_info(remote, &[], Some(ForgeName::GitLab)).unwrap();
        assert_eq!(
            info.name,
            ForgeName::Azure,
            "the Azure host must win over a stale override"
        );
        assert_eq!(
            info.base_url, "https://dev.azure.com/org/My%20Project/_git/repo%23one",
            "Azure path parsing and browser-origin conversion must survive a stale override"
        );
        assert_eq!(
            compare_branch_url(remote, "main", "feat", None, &[], Some(ForgeName::GitLab)).unwrap(),
            "https://dev.azure.com/org/My%20Project/_git/repo%23one/branchCompare?baseVersion=GBmain&targetVersion=GBfeat",
            "compare URLs must use Azure's path shape rather than the override's forge"
        );
    }
}

#[test]
fn azure_https_base_url_keeps_org_project_and_repo() {
    let info = forge_info(
        "https://dev.azure.com/myorg/myproject/_git/myrepo",
        &[],
        None,
    )
    .unwrap();
    assert_eq!(info.name, ForgeName::Azure);
    // Regression: the org and repo segments must both survive — the
    // GenericProvider used to drop the repo name entirely.
    assert_eq!(
        info.base_url,
        "https://dev.azure.com/myorg/myproject/_git/myrepo"
    );
}

#[test]
fn azure_browser_url_preserves_path_escaping() {
    for remote in [
        "https://dev.azure.com/org/My%20Project/_git/repo",
        "https://dev.azure.com/org/project/_git/repo%25one",
        "https://dev.azure.com/org/project/_git/repo%23one",
    ] {
        let info = forge_info(remote, &[], None).unwrap();
        assert_eq!(
            info.base_url, remote,
            "browser URLs must preserve encoded path characters in {remote}"
        );
    }
}

#[test]
fn azure_https_v3_organization_browser_url() {
    let remote = "https://dev.azure.com/v3/project/_git/repo";
    let info = forge_info(remote, &[], None).unwrap();
    assert_eq!(
        info.base_url, remote,
        "an organization named v3 must remain part of the browser URL"
    );
}

#[test]
fn azure_ssh_base_url_uses_browsable_https_host() {
    let info = forge_info("git@ssh.dev.azure.com:v3/myorg/myproject/myrepo", &[], None).unwrap();
    assert_eq!(info.name, ForgeName::Azure);
    // ssh.dev.azure.com → dev.azure.com, and org/project/repo intact.
    assert_eq!(
        info.base_url,
        "https://dev.azure.com/myorg/myproject/_git/myrepo"
    );
}

#[test]
fn azure_compare_url() {
    let url = compare_branch_url(
        "https://dev.azure.com/myorg/myproject/_git/myrepo",
        "main",
        "feature",
        None,
        &[],
        None,
    )
    .unwrap();
    assert_eq!(
        url,
        "https://dev.azure.com/myorg/myproject/_git/myrepo/branchCompare?baseVersion=GBmain&targetVersion=GBfeature"
    );
}

#[test]
fn github_repository_name_can_contain_dot_git() {
    let info = forge_info("git@github.com:owner/example.github.io.git", &[], None).unwrap();
    assert_eq!(info.base_url, "https://github.com/owner/example.github.io");
}

#[test]
fn github_ssh_base_url_and_fork_compare() {
    let info = forge_info("git@github.com:owner/repo.git", &[], None).unwrap();
    assert_eq!(info.name, ForgeName::GitHub);
    assert_eq!(info.base_url, "https://github.com/owner/repo");

    let url = compare_branch_url(
        "git@github.com:owner/repo.git",
        "main",
        "feat",
        Some("fork"),
        &[],
        None,
    )
    .unwrap();
    assert_eq!(
        url,
        "https://github.com/owner/repo/compare/main...fork:feat"
    );
}

#[test]
fn gitlab_nested_group_base_url() {
    let info = forge_info("https://gitlab.com/group/subgroup/repo.git", &[], None).unwrap();
    assert_eq!(info.base_url, "https://gitlab.com/group/subgroup/repo");
}

#[test]
fn gitlab_compare_url() {
    let url = compare_branch_url(
        "https://gitlab.com/group/repo.git",
        "main",
        "feat",
        None,
        &[],
        None,
    )
    .unwrap();
    assert_eq!(url, "https://gitlab.com/group/repo/-/compare/main...feat");
}

#[test]
fn self_hosted_gitlab_custom_port_is_preserved() {
    // Regression for #14626: a self-hosted GitLab on a non-standard
    // port must keep the `:8080` in every generated web URL.
    let remote = "https://gitlab.example.com:8080/group/repo.git";
    let info = forge_info(remote, &[], None).unwrap();
    assert_eq!(info.name, ForgeName::GitLab);
    assert_eq!(info.base_url, "https://gitlab.example.com:8080/group/repo");
    assert_eq!(
        composed_commit_url(remote, "abc123"),
        "https://gitlab.example.com:8080/group/repo/-/commit/abc123"
    );
    assert_eq!(
        composed_pr_url(remote, 42),
        "https://gitlab.example.com:8080/group/repo/-/merge_requests/42"
    );
    assert_eq!(
        compare_branch_url(remote, "main", "feat", None, &[], None).unwrap(),
        "https://gitlab.example.com:8080/group/repo/-/compare/main...feat"
    );
}

#[test]
fn ssh_remote_transport_port_is_dropped_from_web_url() {
    let info = forge_info(
        "ssh://git@gitlab.example.com:2222/group/repo.git",
        &[],
        None,
    )
    .unwrap();
    assert_eq!(info.name, ForgeName::GitLab);
    assert_eq!(info.base_url, "https://gitlab.example.com/group/repo");
}

#[test]
fn ssh_remote_uses_configured_account_web_origin() {
    // Regression for #14626: an SSH remote carries no https port, so
    // the web origin must come from the configured self-hosted account.
    let remote = "git@gitlab.example.com:group/repo.git";
    let accounts = [ForgeUser::GitLab(
        but_gitlab::GitlabAccountIdentifier::selfhosted("bob", "https://gitlab.example.com:8080"),
    )];
    let info = forge_info(remote, &accounts, None).unwrap();
    assert_eq!(info.base_url, "https://gitlab.example.com:8080/group/repo");
    assert_eq!(
        compare_branch_url(remote, "main", "feat", None, &accounts, None).unwrap(),
        "https://gitlab.example.com:8080/group/repo/-/compare/main...feat"
    );
}

#[test]
fn account_with_api_endpoint_custom_host_yields_web_origin() {
    let accounts = [ForgeUser::GitLab(
        but_gitlab::GitlabAccountIdentifier::selfhosted(
            "bob",
            "http://gitlab.example.com:8080/api/v4",
        ),
    )];
    let info = forge_info("git@gitlab.example.com:group/repo.git", &accounts, None).unwrap();
    assert_eq!(info.base_url, "http://gitlab.example.com:8080/group/repo");
}

#[test]
fn account_for_other_host_does_not_change_web_origin() {
    let accounts = [ForgeUser::GitLab(
        but_gitlab::GitlabAccountIdentifier::selfhosted("bob", "https://gitlab.other.com:8080"),
    )];
    let info = forge_info("git@gitlab.example.com:group/repo.git", &accounts, None).unwrap();
    assert_eq!(info.base_url, "https://gitlab.example.com/group/repo");
}

#[test]
fn https_remote_origin_wins_over_account_without_port() {
    // #14678 behavior must survive: an https remote's explicit port is
    // authoritative even when the account host was stored without one.
    let accounts = [ForgeUser::GitLab(
        but_gitlab::GitlabAccountIdentifier::selfhosted("bob", "gitlab.example.com"),
    )];
    let info = forge_info(
        "https://gitlab.example.com:8080/group/repo.git",
        &accounts,
        None,
    )
    .unwrap();
    assert_eq!(info.base_url, "https://gitlab.example.com:8080/group/repo");
}

#[test]
fn stored_self_hosted_account_round_trips_into_web_origin() {
    // The path but-api takes: account persisted to storage, listed back,
    // and fed into forge_info — the reporter's exact setup in #14626.
    let tmp = tempfile::tempdir().unwrap();
    let storage = but_forge_storage::Controller::from_path(tmp.path());
    storage
        .add_gitlab_account(&but_forge_storage::settings::GitLabAccount::SelfHosted {
            username: "bob".into(),
            host: "https://gitlab.example.com:8080".into(),
            access_token_key: "unused".into(),
        })
        .unwrap();
    let accounts: Vec<ForgeUser> = but_gitlab::list_known_gitlab_accounts(&storage)
        .unwrap()
        .into_iter()
        .map(ForgeUser::GitLab)
        .collect();
    let info = forge_info("git@gitlab.example.com:group/repo.git", &accounts, None).unwrap();
    assert_eq!(info.base_url, "https://gitlab.example.com:8080/group/repo");
}

#[test]
fn bitbucket_compare_url_encodes_dest() {
    let url = compare_branch_url(
        "https://bitbucket.org/owner/repo.git",
        "release/1.0",
        "feat",
        None,
        &[],
        None,
    )
    .unwrap();
    assert_eq!(
        url,
        "https://bitbucket.org/owner/repo/branch/feat?dest=release%2F1.0"
    );
}

#[test]
fn compare_urls_encode_url_delimiters_in_branch_names() {
    for (remote, fork, expected) in [
        (
            "https://github.com/owner/repo.git",
            None,
            "https://github.com/owner/repo/compare/release/100%25...fix/%2342-crash",
        ),
        (
            "https://github.com/owner/repo.git",
            Some("fork"),
            "https://github.com/owner/repo/compare/release/100%25...fork:fix/%2342-crash",
        ),
        (
            "https://gitlab.com/group/repo.git",
            None,
            "https://gitlab.com/group/repo/-/compare/release/100%25...fix/%2342-crash",
        ),
        (
            "https://bitbucket.org/owner/repo.git",
            None,
            "https://bitbucket.org/owner/repo/branch/fix/%2342-crash?dest=release%2F100%25",
        ),
        (
            "https://dev.azure.com/myorg/myproject/_git/myrepo",
            None,
            "https://dev.azure.com/myorg/myproject/_git/myrepo/branchCompare?baseVersion=GBrelease%2F100%25&targetVersion=GBfix%2F%2342-crash",
        ),
    ] {
        assert_eq!(
            compare_branch_url(remote, "release/100%", "fix/#42-crash", fork, &[], None).unwrap(),
            expected,
            "`#` must not start a fragment and `%` must not start an escape"
        );
    }
}

// Commit and PR/MR hyperlinks are built on the frontend as
// `{baseUrl}{commitUrlPath}{sha}` / `{baseUrl}{prUrlPath}{number}`.
// These assert the full composed URL per forge so a typo in the
// path segments can't silently ship broken links — including
// Bitbucket and Azure, which have no other forge integration to
// surface a break.

/// Compose what the frontend `commitUrl(forge, sha)` helper produces.
fn composed_commit_url(remote: &str, sha: &str) -> String {
    let info = forge_info(remote, &[], None).unwrap();
    format!("{}{}{}", info.base_url, info.commit_url_path, sha)
}

/// Compose what the frontend `prUrl(forge, number)` helper produces.
fn composed_pr_url(remote: &str, number: i64) -> String {
    let info = forge_info(remote, &[], None).unwrap();
    format!("{}{}{}", info.base_url, info.pr_url_path, number)
}

#[test]
fn github_commit_and_pr_urls() {
    assert_eq!(
        composed_commit_url("https://github.com/owner/repo.git", "abc123"),
        "https://github.com/owner/repo/commit/abc123"
    );
    assert_eq!(
        composed_pr_url("https://github.com/owner/repo.git", 42),
        "https://github.com/owner/repo/pull/42"
    );
}

#[test]
fn gitlab_commit_and_mr_urls() {
    let info = forge_info("https://gitlab.com/group/repo.git", &[], None).unwrap();
    assert!(info.capabilities.checks);
    assert_eq!(
        composed_commit_url("https://gitlab.com/group/repo.git", "abc123"),
        "https://gitlab.com/group/repo/-/commit/abc123"
    );
    assert_eq!(
        composed_pr_url("https://gitlab.com/group/repo.git", 42),
        "https://gitlab.com/group/repo/-/merge_requests/42"
    );
}

#[test]
fn bitbucket_commit_and_pr_urls() {
    assert_eq!(
        composed_commit_url("https://bitbucket.org/owner/repo.git", "abc123"),
        "https://bitbucket.org/owner/repo/commits/abc123"
    );
    assert_eq!(
        composed_pr_url("https://bitbucket.org/owner/repo.git", 42),
        "https://bitbucket.org/owner/repo/pull-requests/42"
    );
}

#[test]
fn azure_commit_and_pr_urls() {
    assert_eq!(
        composed_commit_url(
            "https://dev.azure.com/myorg/myproject/_git/myrepo",
            "abc123"
        ),
        "https://dev.azure.com/myorg/myproject/_git/myrepo/commit/abc123"
    );
    assert_eq!(
        composed_pr_url("https://dev.azure.com/myorg/myproject/_git/myrepo", 42),
        "https://dev.azure.com/myorg/myproject/_git/myrepo/pullrequest/42"
    );
}

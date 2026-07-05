mod forge;
pub use crate::forge::{ForgeName, ForgeRepoInfo, ForgeUser, deserialize_preferred_forge_user_opt};

mod association;
mod ci;
mod db;
pub use db::{cached_review_states, list_cached_forge_reviews};
mod forge_info;
mod merge_message;
pub use merge_message::{MergedReviewFromMessage, merged_review_from_message};
mod remote_url;
mod repo;
mod review;
pub use association::{
    ReviewAssociation, preferred_review, review_associations_by_head, review_for_head_ref,
    reviews_by_head,
};
pub use ci::{CiCheck, CiConclusion, CiOutput, CiStatus, ci_checks_for_ref_with_cache};
pub use forge_info::{ForgeCapabilities, ForgeInfo, ForgeUnitInfo, compare_branch_url, forge_info};
pub use repo::{RepoInfo, RepoPermissions, get_repo_info};
pub use review::{
    CacheConfig, CreateForgeReviewParams, ForgeAccountValidity, ForgeReview, ForgeReviewComment,
    ForgeReviewFilter, ForgeReviewLabel, ForgeReviewReaction, ForgeReviewReactionCount,
    ForgeReviewSubmission, ForgeReviewSubmissionState, ForgeReviewTargetUpdate, ForgeReviewThread,
    ForgeReviewThreadComment, ForgeReviewThreadSide, ForgeReviewTimelineEvent,
    ForgeReviewTimelineEventKind, ForgeReviewUpdate, ForgeReviewUser, GitHubStackingMode,
    PublishReviewOutcome, ReviewMergeMethod, ReviewMergeStatus, ReviewStackingDescription,
    ReviewState, ReviewSyncOutcome, ReviewTemplateFunctions, ReviewUpdatePayload,
    add_comment_reaction, add_review_labels, add_review_reaction, add_submission_reaction,
    available_review_templates, cache_review, check_forge_account_is_valid,
    compute_review_target_updates, create_forge_review, create_review_comment,
    create_review_thread_reply, delete_review_comment, get_forge_review, get_review_base_repo_url,
    get_review_merge_status, get_review_template_functions, list_comment_reactions,
    list_forge_reviews_for_branch, list_forge_reviews_with_cache, list_repo_labels,
    list_review_comments, list_review_reactions, list_review_submissions, list_review_threads,
    list_review_timeline_events, list_reviewer_candidates, merge_review,
    prepare_review_target_updates, remove_comment_reaction, remove_review_label,
    remove_review_reaction, remove_submission_reaction, request_review, restore_native_stacks,
    set_review_auto_merge_state, set_review_draftiness, set_review_thread_resolved, sync_reviews,
    update_review, update_review_comment, withdraw_review_request,
};

fn determine_forge_from_host(host: &str) -> Option<ForgeName> {
    if host.contains("github.com") || host.starts_with("github.") {
        Some(ForgeName::GitHub)
    } else if host.contains("gitlab.com") || host.starts_with("gitlab.") {
        Some(ForgeName::GitLab)
    } else if host.contains("bitbucket.org") {
        Some(ForgeName::Bitbucket)
    } else if host.contains("azure.com") {
        Some(ForgeName::Azure)
    } else {
        None
    }
}

/// Derive the forge repository information from a remote URL.
///
/// The forge type is resolved by `resolve_forge_name`: the URL host first,
/// then a match against configured accounts' custom hosts, and finally the
/// project's `override_forge` (from its stored `forge_override`) as a last
/// resort. Pass `None` for `override_forge` when no project preference applies,
/// e.g. when comparing two arbitrary remote URLs for identity.
pub fn derive_forge_repo_info(
    url: &str,
    override_forge: Option<ForgeName>,
) -> Option<ForgeRepoInfo> {
    let remote = remote_url::RemoteUrl::parse(url)?;
    let forge = resolve_forge_name(&remote.host, override_forge, || {
        // Only fetch the accounts if the forge type can't be determined from the
        // repository's host - reading them involves retrieving data from storage.
        get_all_forge_accounts().unwrap_or_default()
    })?;
    let (owner, repo) = remote.repository_parts(&forge)?;

    Some(ForgeRepoInfo {
        forge,
        owner,
        repo,
        protocol: remote.protocol,
    })
}

/// Resolve the forge type for a repository `host`, in priority order:
///
/// 1. keywords in the host itself (`github.com`, `gitlab.*`, ...),
/// 2. a configured account whose custom host matches (fetched lazily via
///    `accounts`, since that reads from storage),
/// 3. the project's saved `forge_override`, honored only as a last resort.
///
/// Step 3 mirrors the pre-0.20.1 frontend, which applied the override only when
/// host detection returned "default" (`if forgeType === "default" &&
/// forgeOverride`). It is what lets a self-hosted forge - e.g. GitLab on a
/// private host - resolve again before an account has been configured (#14319).
/// A recognizable host or a matching account always wins over the override, so
/// a stale preference can never mislabel a repository whose forge is known.
fn resolve_forge_name(
    host: &str,
    override_forge: Option<ForgeName>,
    accounts: impl FnOnce() -> Vec<ForgeUser>,
) -> Option<ForgeName> {
    determine_forge_from_host(host)
        .or_else(|| match_host_to_accounts_custom_host(host, &accounts()))
        .or(override_forge)
}

/// Look for the best matching account by comparing the repository host to the
/// account custom host string.
fn match_host_to_accounts_custom_host(host: &str, accounts: &[ForgeUser]) -> Option<ForgeName> {
    accounts.iter().find_map(|account| {
        let custom_host = account.custom_host()?;
        custom_host_matches_repository_host(host, &custom_host).then(|| account.forge_name())
    })
}

/// Compare a repository host to an account custom-host string.
///
/// Motivation:
/// account custom hosts may be stored as full API endpoints (for example
/// `https://api.repository.com/v1/api`), while repository remotes usually
/// provide only the repository host (`repository.com`).
///
/// Behavior:
/// - both inputs are normalized (scheme, path/query/fragment, user-info, and
///   numeric port are removed; casing is ignored)
/// - exact host matches return `true`
/// - subdomain custom-hosts match their root repository host
///   (`api.repository.com` matches `repository.com`)
/// - partial suffixes do not match (`api.notrepository.com` does not match
///   `repository.com`)
fn custom_host_matches_repository_host(repository_host: &str, account_custom_host: &str) -> bool {
    let normalized_repository_host = normalize_host_for_comparison(repository_host);
    let normalized_account_host = normalize_host_for_comparison(account_custom_host);

    if normalized_repository_host.is_empty() || normalized_account_host.is_empty() {
        return false;
    }

    normalized_account_host == normalized_repository_host
        || normalized_account_host.ends_with(&format!(".{normalized_repository_host}"))
}

fn normalize_host_for_comparison(value: &str) -> String {
    let without_scheme = value.split_once("://").map_or(value, |(_, rest)| rest);
    let without_path = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let without_user_info = without_path
        .rsplit_once('@')
        .map_or(without_path, |(_, host)| host);

    let without_port = match without_user_info.rsplit_once(':') {
        Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) => host,
        _ => without_user_info,
    };

    without_port
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

/// The login this repository's forge calls authenticate as, mirroring how
/// the per-forge clients resolve their account: the preferred account when
/// it is still in storage, otherwise (with no preference set) the first
/// known account of the repository's forge. `None` when no account is
/// configured — or when the preferred account is gone from storage, where
/// the clients refuse to authenticate rather than fall back.
pub fn current_forge_login(
    preferred_forge_user: &Option<ForgeUser>,
    forge_repo_info: &ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
) -> anyhow::Result<Option<String>> {
    match forge_repo_info.forge {
        ForgeName::GitHub => {
            let accounts = but_github::list_known_github_accounts(storage)?;
            let preferred = preferred_forge_user.as_ref().and_then(|user| user.github());
            Ok(match preferred {
                Some(preferred) => accounts.iter().find(|account| *account == preferred),
                None => accounts.first(),
            }
            .map(|account| account.username().to_string()))
        }
        ForgeName::GitLab => {
            let accounts = but_gitlab::list_known_gitlab_accounts(storage)?;
            let preferred = preferred_forge_user.as_ref().and_then(|user| user.gitlab());
            Ok(match preferred {
                Some(preferred) => accounts.iter().find(|account| *account == preferred),
                None => accounts.first(),
            }
            .map(|account| account.username().to_string()))
        }
        _ => Ok(None),
    }
}

/// Get all known forge accounts
pub fn get_all_forge_accounts() -> anyhow::Result<Vec<ForgeUser>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    let gh_accounts = but_github::list_known_github_accounts(&storage)?;
    let gl_accounts = but_gitlab::list_known_gitlab_accounts(&storage)?;

    let mut forge_users = vec![];
    for gh_account in gh_accounts {
        forge_users.push(ForgeUser::GitHub(gh_account));
    }

    for gl_account in gl_accounts {
        forge_users.push(ForgeUser::GitLab(gl_account));
    }

    Ok(forge_users)
}

#[cfg(test)]
mod tests {
    use super::{
        ForgeName, ForgeRepoInfo, ForgeUser, current_forge_login, derive_forge_repo_info,
        match_host_to_accounts_custom_host, normalize_host_for_comparison, resolve_forge_name,
    };

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
            owner: "gitbutlerapp".into(),
            repo: "gitbutler".into(),
            protocol: "https".into(),
        }
    }

    fn gitlab_repo_info() -> ForgeRepoInfo {
        ForgeRepoInfo {
            forge: ForgeName::GitLab,
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
    fn forge_override_resolves_unrecognized_self_hosted_host() {
        // The #14319 regression: a self-hosted host matches no built-in forge
        // and (here) no account, so the project's saved override is the only
        // thing that can identify it. Without the override this returns `None`
        // and the UI reports "No forge could be determined".
        assert_eq!(
            resolve_forge_name("git.selfhosted.example", Some(ForgeName::GitLab), Vec::new),
            Some(ForgeName::GitLab)
        );
    }

    #[test]
    fn detected_host_wins_over_override() {
        // A recognizable host is authoritative - the override must never
        // override a forge we can already identify from the URL. Here the host
        // is GitLab and the (contradictory) override is GitHub; GitLab wins.
        assert_eq!(
            resolve_forge_name("gitlab.com", Some(ForgeName::GitHub), Vec::new),
            Some(ForgeName::GitLab)
        );
    }

    #[test]
    fn detected_host_does_not_load_accounts() {
        assert_eq!(
            resolve_forge_name("github.com", Some(ForgeName::GitLab), || {
                panic!("a recognizable host must resolve without reading account storage")
            }),
            Some(ForgeName::GitHub),
            "host detection must remain authoritative and avoid account storage"
        );
    }

    #[test]
    fn matching_account_wins_over_override() {
        // A configured account whose custom host matches is a stronger signal
        // than a saved preference, so it takes precedence over the override.
        let accounts = vec![ForgeUser::GitLab(
            but_gitlab::GitlabAccountIdentifier::selfhosted(
                "bob",
                "HTTP://api.gl.example.com:8080/api/v4",
            ),
        )];
        assert_eq!(
            resolve_forge_name("gl.example.com", Some(ForgeName::GitHub), || accounts
                .clone()),
            Some(ForgeName::GitLab)
        );
    }

    #[test]
    fn no_override_and_unknown_host_resolves_to_none() {
        assert_eq!(
            resolve_forge_name("git.selfhosted.example", None, Vec::new),
            None
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

    #[test]
    fn from_override_str_is_case_and_whitespace_insensitive() {
        assert_eq!(
            ForgeName::from_override_str("gitlab"),
            Some(ForgeName::GitLab)
        );
        assert_eq!(
            ForgeName::from_override_str("  GitLab "),
            Some(ForgeName::GitLab)
        );
        // "default" (the unset sentinel) and anything unrecognized are no-override.
        assert_eq!(ForgeName::from_override_str("default"), None);
        assert_eq!(ForgeName::from_override_str("subversion"), None);
    }

    #[test]
    fn matches_github_enterprise_custom_host() {
        let accounts = vec![ForgeUser::GitHub(
            but_github::GithubAccountIdentifier::enterprise("alice", "gh.example.com"),
        )];

        assert_eq!(
            match_host_to_accounts_custom_host("gh.example.com", &accounts),
            Some(ForgeName::GitHub)
        );
    }

    #[test]
    fn matches_gitlab_self_hosted_custom_host() {
        let accounts = vec![ForgeUser::GitLab(
            but_gitlab::GitlabAccountIdentifier::selfhosted("bob", "gl.example.com"),
        )];

        assert_eq!(
            match_host_to_accounts_custom_host("gl.example.com", &accounts),
            Some(ForgeName::GitLab)
        );
    }

    #[test]
    fn does_not_match_accounts_without_custom_host() {
        let accounts = vec![
            ForgeUser::GitHub(but_github::GithubAccountIdentifier::oauth("alice")),
            ForgeUser::GitHub(but_github::GithubAccountIdentifier::pat("charlie")),
            ForgeUser::GitLab(but_gitlab::GitlabAccountIdentifier::pat("bob")),
        ];

        assert_eq!(
            match_host_to_accounts_custom_host("gh.example.com", &accounts),
            None
        );
    }

    #[test]
    fn returns_none_when_custom_hosts_do_not_match() {
        let accounts = vec![
            ForgeUser::GitHub(but_github::GithubAccountIdentifier::enterprise(
                "alice",
                "gh.example.com",
            )),
            ForgeUser::GitLab(but_gitlab::GitlabAccountIdentifier::selfhosted(
                "bob",
                "gl.example.com",
            )),
        ];

        assert_eq!(
            match_host_to_accounts_custom_host("no-match.example.com", &accounts),
            None
        );
    }

    #[test]
    fn matches_repository_host_against_custom_host_with_subdomain_and_path() {
        let accounts = vec![ForgeUser::GitLab(
            but_gitlab::GitlabAccountIdentifier::selfhosted(
                "bob",
                "https://api.repository.com/v1/api",
            ),
        )];

        assert_eq!(
            match_host_to_accounts_custom_host("repository.com", &accounts),
            Some(ForgeName::GitLab)
        );
    }

    #[test]
    fn matches_repository_host_against_custom_host_with_scheme_port_and_path() {
        let accounts = vec![ForgeUser::GitHub(
            but_github::GithubAccountIdentifier::enterprise(
                "alice",
                "https://api.repository.com:8443/v1/api",
            ),
        )];

        assert_eq!(
            match_host_to_accounts_custom_host("repository.com", &accounts),
            Some(ForgeName::GitHub)
        );
    }

    #[test]
    fn does_not_match_partial_domain_suffixes() {
        let accounts = vec![ForgeUser::GitLab(
            but_gitlab::GitlabAccountIdentifier::selfhosted("bob", "api.notrepository.com/v1"),
        )];

        assert_eq!(
            match_host_to_accounts_custom_host("repository.com", &accounts),
            None
        );
    }

    #[test]
    fn matches_repository_host_case_insensitively_against_custom_host() {
        let accounts = vec![ForgeUser::GitLab(
            but_gitlab::GitlabAccountIdentifier::selfhosted(
                "bob",
                "HTTPS://API.REPOSITORY.COM/v1/api",
            ),
        )];

        assert_eq!(
            match_host_to_accounts_custom_host("Repository.COM", &accounts),
            Some(ForgeName::GitLab)
        );
    }

    #[test]
    fn normalize_host_for_comparison_strips_url_parts_and_normalizes_case() {
        assert_eq!(
            normalize_host_for_comparison("HTTPS://user@API.Repository.com:8443/v1/api?x=1#frag"),
            "api.repository.com"
        );
    }

    #[test]
    fn normalize_host_for_comparison_trims_whitespace_and_trailing_dot() {
        assert_eq!(
            normalize_host_for_comparison("  repository.com.  "),
            "repository.com"
        );
    }
}

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
    reviews_by_head, without_unrelated_forks,
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
        get_all_forge_accounts()
            .unwrap_or_default()
            .into_iter()
            .filter(|account| {
                account_matches_repository(
                    account,
                    &ForgeRepoInfo {
                        forge: account.forge_name(),
                        host: remote.host.clone(),
                        port: remote.port,
                        protocol: remote.protocol.clone(),
                        owner: String::new(),
                        repo: String::new(),
                    },
                )
            })
            .collect()
    })?;
    let (owner, repo) = remote.repository_parts(&forge)?;

    Some(ForgeRepoInfo {
        forge,
        host: remote.host,
        port: remote.port,
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
/// - the explicit `api.` subdomain matches its root repository host
///   (`api.repository.com` matches `repository.com`)
/// - partial suffixes do not match (`api.notrepository.com` does not match
///   `repository.com`)
fn custom_host_matches_repository_host(repository_host: &str, account_custom_host: &str) -> bool {
    let normalized_repository_host = normalize_host_for_comparison(repository_host);
    let normalized_account_host = normalize_host_for_comparison(account_custom_host);

    if normalized_repository_host.is_empty() || normalized_account_host.is_empty() {
        return false;
    }

    api_host_matches_repository_host(&normalized_repository_host, &normalized_account_host)
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

/// Compare repository hosts without making distinct instances equal.
pub(crate) fn repository_hosts_match(left: &str, right: &str) -> bool {
    left.trim_end_matches('.')
        .eq_ignore_ascii_case(right.trim_end_matches('.'))
}

/// Match host names, allowing only the repository host itself or its explicit
/// `api.` subdomain. The `api.` exception applies only to the API host, not the
/// repository host; arbitrary subdomains never match.
fn api_host_matches_repository_host(repository_host: &str, api_host: &str) -> bool {
    repository_hosts_match(repository_host, api_host)
        || repository_hosts_match(&format!("api.{repository_host}"), api_host)
}

/// Canonicalize a remote's host for forge-instance comparisons and API account matching.
/// Known public SSH aliases map to the public repository host (e.g. `ssh.github.com`
/// becomes `github.com`), so SSH and HTTPS remotes identify the same forge instance.
/// Other hosts are unchanged. This does not produce an API endpoint: the selected
/// account's client configuration supplies that URL.
fn canonical_repository_host<'a>(forge: &ForgeName, host: &'a str) -> &'a str {
    let alias = match forge {
        ForgeName::GitHub => "ssh.github.com",
        ForgeName::GitLab => "altssh.gitlab.com",
        ForgeName::Bitbucket => "altssh.bitbucket.org",
        ForgeName::Azure => "ssh.dev.azure.com",
    };
    if repository_hosts_match(host, alias) {
        forge.public_host().unwrap_or(host)
    } else {
        host
    }
}

/// A forge hint is not authorization to use credentials from another instance.
/// Match custom accounts against their configured HTTP(S) API endpoint, not
/// host-detection keywords or arbitrary subdomains.
fn account_matches_repository(account: &ForgeUser, repo: &ForgeRepoInfo) -> bool {
    if account.forge_name() != repo.forge {
        return false;
    }
    let Some(endpoint) = account.custom_host() else {
        return repo.is_public_host() && repo.identity_port().is_none_or(|port| port == 443);
    };
    let host = canonical_repository_host(&repo.forge, &repo.host);
    let Some((scheme, rest)) = endpoint.trim().split_once("://") else {
        return false;
    };
    // URL schemes are case-insensitive, while gix parses lowercase schemes for Git compatibility.
    let endpoint = format!("{}://{rest}", scheme.to_ascii_lowercase());
    let Ok(endpoint) = gix::url::parse(endpoint.as_bytes()) else {
        return false;
    };
    let port = endpoint.port.unwrap_or(match endpoint.scheme {
        gix::url::Scheme::Http => 80,
        gix::url::Scheme::Https => 443,
        _ => return false,
    });
    repo.identity_port().is_none_or(|known| known == port)
        && endpoint
            .host
            .is_some_and(|api| api_host_matches_repository_host(host, &api))
}

/// Select only stored accounts for the repository's forge and host. A saved
/// preference must itself be eligible; it never falls back to another account.
pub(crate) fn find_forge_account(
    preferred: &Option<ForgeUser>,
    repo: &ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
) -> anyhow::Result<Option<ForgeUser>> {
    let accounts: Vec<ForgeUser> = match repo.forge {
        ForgeName::GitHub => but_github::list_known_github_accounts(storage)?
            .into_iter()
            .map(ForgeUser::GitHub)
            .collect(),
        ForgeName::GitLab => but_gitlab::list_known_gitlab_accounts(storage)?
            .into_iter()
            .map(ForgeUser::GitLab)
            .collect(),
        ForgeName::Bitbucket => but_bitbucket::list_known_bitbucket_accounts(storage)?
            .into_iter()
            .map(ForgeUser::Bitbucket)
            .collect(),
        ForgeName::Azure => return Ok(None),
    };
    Ok(accounts.into_iter().find(|account| {
        preferred.as_ref().is_none_or(|user| user == account)
            && account_matches_repository(account, repo)
    }))
}

/// Resolve the account before any live repository API call. The account's
/// existing client configuration supplies the endpoint; never retarget a token.
/// Azure returns `None` without reading storage, as authenticated API dispatch isn't implemented.
pub(crate) fn resolve_account(
    preferred: &Option<ForgeUser>,
    repo: &ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
) -> anyhow::Result<Option<ForgeUser>> {
    if repo.forge == ForgeName::Azure {
        return Ok(None);
    }
    find_forge_account(preferred, repo, storage)?.map(Some).ok_or_else(|| {
        let message = format!(
            "Not authenticated with {:?} for '{}'. No stored account matches this host and account preference. Connect an account for this host under Settings → Integrations, or run 'but config forge auth'.",
            repo.forge, repo.host
        );
        anyhow::anyhow!(but_error::Context::new(message).with_code(but_error::Code::ForgeNotAuthenticated))
    })
}

/// The login repository API calls authenticate as: the stored preferred
/// account if it matches the forge and host, otherwise (with no preference)
/// the first matching account. `None` when no eligible account is configured.
pub fn current_forge_login(
    preferred_forge_user: &Option<ForgeUser>,
    forge_repo_info: &ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
) -> anyhow::Result<Option<String>> {
    let account = find_forge_account(preferred_forge_user, forge_repo_info, storage)?;
    Ok(account.and_then(|account| match account {
        ForgeUser::GitHub(account) => Some(account.username().to_string()),
        ForgeUser::GitLab(account) => Some(account.username().to_string()),
        ForgeUser::Bitbucket(_) => None,
    }))
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
pub(crate) mod test_support {
    use but_error::{AnyhowContextExt as _, Code::ForgeNotAuthenticated};
    use but_forge_storage::{
        Controller,
        settings::{BitbucketAccount, GitHubAccount, GitLabAccount},
    };

    use super::{ForgeName, ForgeRepoInfo, ForgeUser};

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
}

#[cfg(test)]
mod tests {
    use but_forge_storage::settings::{GitHubAccount, GitLabAccount};
    use but_github::GithubAccountIdentifier as Gh;
    use but_gitlab::GitlabAccountIdentifier as Gl;

    use super::ForgeUser::{GitHub, GitLab};
    use super::test_support::{accounts, assert_auth_error, repo};
    use super::{
        ForgeName, ForgeUser, account_matches_repository as account_matches, current_forge_login,
        derive_forge_repo_info, match_host_to_accounts_custom_host, normalize_host_for_comparison,
        resolve_account, resolve_forge_name,
    };

    #[test]
    fn account_resolution_selects_the_matching_instance_not_the_first_saved_account() {
        let (_tmp, storage, public) = accounts();
        let endpoint = "https://code.example:8443/api/v4";
        for host in ["https://other.example/api/v4", endpoint] {
            let account = GitLabAccount::SelfHosted {
                username: "private".into(),
                host: host.into(),
                access_token_key: "unused".into(),
            };
            storage.add_gitlab_account(&account).unwrap();
        }
        let expected = GitLab(Gl::selfhosted("private", endpoint));
        let mut private_repo = repo(ForgeName::GitLab, "code.example");
        private_repo.protocol = "ssh".into();
        for preferred in [None, Some(expected.clone())] {
            let account = resolve_account(&preferred, &private_repo, &storage).unwrap();
            let login = current_forge_login(&preferred, &private_repo, &storage).unwrap();
            assert_eq!(
                (account, login),
                (Some(expected.clone()), Some("private".into())),
                "retain the matching endpoint and report the same live login"
            );
        }
        for preferred in public.into_iter().chain([
            GitLab(Gl::selfhosted("private", "https://other.example/api/v4")),
            GitLab(Gl::selfhosted("missing", endpoint)),
        ]) {
            assert_auth_error(
                resolve_account(&Some(preferred), &private_repo, &storage).unwrap_err(),
            );
        }
        let public = storage.github_accounts().unwrap().remove(0);
        storage.clear_all_github_accounts().unwrap();
        let private = GitHubAccount::Enterprise {
            username: "private".into(),
            host: "https://github.example/api/v3".into(),
            access_token_key: "unused".into(),
        };
        for account in [&private, &public] {
            storage.add_github_account(account).unwrap();
        }
        for (host, account) in [("github.com", &public), ("github.example", &private)] {
            assert_eq!(
                resolve_account(&None, &repo(ForgeName::GitHub, host), &storage).unwrap(),
                Some(GitHub(account.into())),
                "skip wrong-instance accounts"
            );
        }
        let file = tempfile::NamedTempFile::new().unwrap();
        let storage = but_forge_storage::Controller::from_path(file.path());
        let preferred = Some(GitHub((&public).into()));
        let repo = repo(ForgeName::Azure, "dev.azure.com");
        assert_eq!(
            resolve_account(&preferred, &repo, &storage).unwrap(),
            None,
            "Azure unsupported dispatch bypasses unusable storage"
        );
    }

    #[test]
    fn account_host_matching_requires_a_valid_exact_or_api_endpoint() {
        let (_tmp, storage, _) = accounts();
        for host in [
            "gitlab.com.evil.example",
            "mygitlab.com",
            "gitlab.private.example",
            "code.private.example",
        ] {
            let remote = format!("https://{host}/owner/repo");
            let repository = derive_forge_repo_info(&remote, Some(ForgeName::GitLab)).unwrap();
            assert_eq!(
                current_forge_login(&None, &repository, &storage).unwrap(),
                None,
                "keywords and overrides do not authorize a public account for {host}"
            );
            assert_auth_error(resolve_account(&None, &repository, &storage).unwrap_err());
        }
        #[rustfmt::skip]
        let cases = [
            ("https://notcode.example/api", "https", None, false, "partial host suffixes do not match"),
            ("https://other.code.example/api", "https", None, false, "arbitrary subdomains do not match"),
            ("https://code.example.evil/api", "https", None, false, "a host prefix does not identify the same server"),
            ("code.example", "https", None, false, "API endpoints require an absolute HTTP(S) URL"),
            ("ssh://code.example/api", "https", None, false, "SSH is not an API endpoint scheme"),
            ("https:///api/v4", "https", None, false, "API endpoints require a host"),
            ("https://code.example:bad/api", "https", None, false, "invalid API ports are rejected"),
            ("HTTPS://CODE.EXAMPLE.:443/api", "https", None, true, "scheme and host casing and a trailing dot do not change identity"),
            ("https://api.code.example/api", "https", None, true, "the explicit api. subdomain is allowed"),
            ("https://API.CODE.EXAMPLE./api", "https", None, true, "api. matching ignores host casing and a trailing dot"),
            ("https://nested.api.code.example/api", "https", None, false, "the api. exception does not allow nested subdomains"),
            ("https://code.example:8443/api", "https", Some(8443), true, "matching explicit API ports identify the same server"),
            ("https://code.example:8443/api", "https", Some(9443), false, "different known API ports identify distinct servers"),
            ("https://code.example:8443/api", "https", None, false, "an omitted HTTPS remote port means 443, not any port"),
            ("https://code.example:8443/api", "ssh", Some(2222), true, "a private SSH transport port does not constrain the API port"),
            ("http://code.example/api", "http", None, true, "matching HTTP URLs both default to port 80"),
            ("https://code.example:443/api", "https", None, true, "explicit 443 matches the default HTTPS port"),
        ];
        for (endpoint, protocol, port, expected, reason) in cases {
            let mut repository = repo(ForgeName::GitLab, "code.example");
            repository.protocol = protocol.into();
            repository.port = port;
            let account = GitLab(Gl::selfhosted("alice", endpoint));
            assert_eq!(
                account_matches(&account, &repository),
                expected,
                "{reason}: API endpoint {endpoint} for repository {protocol}://code.example (port {port:?})"
            );
        }
        let accounts = vec![
            GitHub(Gh::enterprise("alice", "https://other.code.example/api/v3")),
            GitLab(Gl::selfhosted("bob", "https://code.example/api/v4")),
        ];
        assert_eq!(
            resolve_forge_name("code.example", Some(ForgeName::GitHub), || accounts),
            Some(ForgeName::GitLab),
            "unrelated subdomains cannot win"
        );
    }

    #[test]
    fn azure_repository_identity_matches_public_ssh_and_https_urls() {
        let https =
            derive_forge_repo_info("https://dev.azure.com/org/project/_git/repo", None).unwrap();
        for url in [
            "git@ssh.dev.azure.com:v3/org/project/repo.git",
            "ssh://git@SSH.DEV.AZURE.COM.:2222/v3/org/project/repo.git",
            "git@dev.azure.com:v3/org/project/repo.git",
        ] {
            let ssh = derive_forge_repo_info(url, None).unwrap();
            assert!(
                https.same_repository(&ssh),
                "Azure aliases identify the same repository: {url}"
            );
            assert!(
                ssh.same_repository(&https),
                "repository compatibility must be symmetric: {url}"
            );
            assert_eq!(
                ssh.identity_port(),
                Some(443),
                "public Azure APIs use HTTPS"
            );
            let mut different = https.clone();
            different.port = Some(8443);
            assert!(
                !ssh.same_repository(&different),
                "SSH transport ports do not change the API port"
            );
            assert!(
                !ssh.same_server(&different),
                "non-default API ports remain distinct"
            );
            different.port = None;
            different.host = "private.azure.example".into();
            assert!(
                !ssh.same_repository(&different),
                "private Azure hosts must remain distinct"
            );
            different.host = https.host.clone();
            different.owner = "other/project".into();
            assert!(
                !ssh.same_repository(&different),
                "different organizations identify different repositories"
            );
        }
    }

    #[test]
    fn repository_identity_distinguishes_hosts_and_known_api_ports() {
        let (_tmp, storage, accounts) = accounts();
        for (account, alias) in accounts.iter().zip([
            "ssh.github.com",
            "altssh.gitlab.com",
            "altssh.bitbucket.org",
        ]) {
            let forge = account.forge_name();
            let public = repo(forge.clone(), forge.public_host().unwrap());
            let mut ssh = repo(account.forge_name(), &alias.to_ascii_uppercase());
            ssh.host.push('.');
            ssh.protocol = "ssh".into();
            ssh.port = Some(2222);
            assert!(
                public.same_repository(&ssh),
                "known aliases ignore SSH transport ports"
            );
            assert!(
                public.is_public_host() && ssh.is_public_host(),
                "public hosts and known aliases share public-instance classification"
            );
            assert_eq!(
                resolve_account(&None, &ssh, &storage).unwrap(),
                Some(account.clone()),
                "alias selects the stored public account"
            );
            let mut other = public.clone();
            other.port = Some(8443);
            assert!(!ssh.same_server(&other), "public SSH identifies port 443");
            assert!(other.is_public_host(), "host classification excludes ports");
            assert!(!account_matches(account, &other), "public API port");
            other.port = None;
            other.host = "private.example".into();
            assert!(
                !public.same_repository(&other),
                "host alone distinguishes identity"
            );
            assert!(
                !other.is_public_host(),
                "private hosts are not public instances"
            );
            assert!(
                !public.same_server(&other),
                "different canonical hosts are distinct"
            );
            other.host = public.host.clone();
            other.forge = ForgeName::Azure;
            assert!(
                !public.same_server(&other),
                "same host does not equate providers"
            );
            assert!(
                !other.is_public_host(),
                "a different provider's public host is not an Azure public host"
            );
            if forge == ForgeName::GitHub {
                let account = GitHub(Gh::enterprise("alice", "https://github.com:8443/api"));
                assert!(!account_matches(&account, &ssh), "public SSH API port");
            }
        }
        let mut first = repo(ForgeName::GitLab, "code.example");
        first.port = Some(8443);
        let mut second = first.clone();
        second.port = Some(9443);
        let mut ssh = first.clone();
        ssh.protocol = "ssh".into();
        assert!(
            !first.same_repository(&second),
            "HTTP ports identify distinct instances"
        );
        for known in [&first, &second] {
            assert!(
                known.same_repository(&ssh) && ssh.same_repository(known),
                "private SSH compatibility is symmetric but does not equate distinct HTTP ports"
            );
        }
        assert!(!first.same_server(&second), "reject known unequal ports");
        assert!(first.same_server(&ssh), "private SSH compatibility");
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

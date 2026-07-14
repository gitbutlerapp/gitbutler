use git_url_parse::{GitUrl, types::provider::GenericProvider};

mod forge;
pub use crate::forge::{ForgeName, ForgeRepoInfo, ForgeUser, deserialize_preferred_forge_user_opt};

mod association;
mod ci;
mod db;
mod forge_info;
mod repo;
mod review;
pub use association::{review_for_head_ref, reviews_by_head};
pub use ci::{CiCheck, CiConclusion, CiOutput, CiStatus, ci_checks_for_ref_with_cache};
pub use forge_info::{ForgeCapabilities, ForgeInfo, ForgeUnitInfo, compare_branch_url, forge_info};
pub use repo::{RepoInfo, RepoPermissions, get_repo_info};
pub use review::{
    CacheConfig, CreateForgeReviewParams, ForgeAccountValidity, ForgeReview, ForgeReviewFilter,
    ForgeReviewTargetUpdate, ForgeReviewUpdate, ReviewMergeMethod, ReviewMergeStatus, ReviewState,
    ReviewTemplateFunctions, ReviewUpdatePayload, available_review_templates, cache_review,
    check_forge_account_is_valid, compute_review_target_updates, create_forge_review,
    get_forge_review, get_review_base_repo_url, get_review_merge_status,
    get_review_template_functions, list_forge_reviews_for_branch, list_forge_reviews_with_cache,
    merge_review, set_review_auto_merge_state, set_review_draftiness, sync_reviews, update_review,
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
    let git_url = GitUrl::parse(url).ok()?;
    let host = git_url.host()?;
    let protocol = git_url.scheme()?;

    let provider_info: GenericProvider = git_url.provider_info().ok()?;
    let forge = resolve_forge_name(host, override_forge, || {
        // Only fetch the accounts if the forge type can't be determined from the
        // repository's host - reading them involves retrieving data from storage.
        get_all_forge_accounts().unwrap_or_default()
    })?;

    Some(ForgeRepoInfo {
        forge,
        owner: provider_info.owner().to_string(),
        repo: provider_info.repo().to_string(),
        protocol: protocol.to_string(),
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
    let user = accounts.iter().find(|account| match account {
        ForgeUser::GitHub(gh_account) => gh_account
            .custom_host()
            .as_deref()
            .is_some_and(|custom_host| custom_host_matches_repository_host(host, custom_host)),
        ForgeUser::GitLab(gl_account) => gl_account
            .custom_host()
            .as_deref()
            .is_some_and(|custom_host| custom_host_matches_repository_host(host, custom_host)),
        // Bitbucket Cloud is fixed-host, so it never matches a custom host.
        ForgeUser::Bitbucket(bb_account) => bb_account
            .custom_host()
            .as_deref()
            .is_some_and(|custom_host| custom_host_matches_repository_host(host, custom_host)),
    });

    match user {
        Some(ForgeUser::GitHub(_)) => Some(ForgeName::GitHub),
        Some(ForgeUser::GitLab(_)) => Some(ForgeName::GitLab),
        Some(ForgeUser::Bitbucket(_)) => Some(ForgeName::Bitbucket),
        None => None,
    }
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
        ForgeName, ForgeUser, derive_forge_repo_info, match_host_to_accounts_custom_host,
        normalize_host_for_comparison, resolve_forge_name,
    };

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
    fn matching_account_wins_over_override() {
        // A configured account whose custom host matches is a stronger signal
        // than a saved preference, so it takes precedence over the override.
        let accounts = vec![ForgeUser::GitLab(
            but_gitlab::GitlabAccountIdentifier::selfhosted("bob", "gl.example.com"),
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
        // End-to-end for #14319, exercising the real parse -> provider_info ->
        // resolve path (not just the host->forge helper): a self-hosted URL that
        // host detection can't classify still yields a `ForgeRepoInfo` via the
        // override, with owner/repo parsed from the URL. Guards the ordering trap
        // that the override is only consulted *after* `provider_info()` succeeds -
        // if `GenericProvider` rejected this URL shape the fix would silently
        // no-op. (The account lookup finds nothing for this synthetic host, so
        // the result is deterministic regardless of locally-configured accounts.)
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

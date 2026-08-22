use git_url_parse::{GitUrl, types::provider::GenericProvider};

mod forge;
pub use crate::forge::{ForgeName, ForgeRepoInfo, ForgeUser, deserialize_preferred_forge_user_opt};

mod association;
mod ci;
mod db;
pub use db::list_cached_forge_reviews;
mod forge_info;
mod merge_message;
pub use merge_message::{MergedReviewFromMessage, merged_review_from_message};
mod repo;
mod review;
pub use association::{pr_numbers_by_head, preferred_review, review_for_head_ref, reviews_by_head};
pub use ci::{CiCheck, CiConclusion, CiOutput, CiStatus, ci_checks_for_ref_with_cache};
pub use forge_info::{ForgeCapabilities, ForgeInfo, ForgeUnitInfo, compare_branch_url, forge_info};
pub use repo::{RepoInfo, RepoPermissions, get_repo_info};
pub use review::{
    CacheConfig, CreateForgeReviewParams, ForgeAccountValidity, ForgeReview, ForgeReviewComment,
    ForgeReviewFilter, ForgeReviewLabel, ForgeReviewReaction, ForgeReviewReactionCount,
    ForgeReviewSubmission, ForgeReviewSubmissionState, ForgeReviewTargetUpdate,
    ForgeReviewTimelineEvent, ForgeReviewTimelineEventKind, ForgeReviewUpdate, ForgeReviewUser,
    GitHubStackingMode, PublishReviewOutcome, ReviewMergeMethod, ReviewMergeStatus,
    ReviewStackingDescription, ReviewState, ReviewSyncOutcome, ReviewTemplateFunctions,
    ReviewUpdatePayload, add_comment_reaction, add_review_labels, add_review_reaction,
    available_review_templates, cache_review, check_forge_account_is_valid,
    compute_review_target_updates, create_forge_review, create_review_comment,
    delete_review_comment, get_forge_review, get_review_base_repo_url, get_review_merge_status,
    get_review_template_functions, list_comment_reactions, list_forge_reviews_for_branch,
    list_forge_reviews_with_cache, list_repo_labels, list_review_comments, list_review_reactions,
    list_review_submissions, list_review_timeline_events, list_reviewer_candidates, merge_review,
    prepare_review_target_updates, remove_comment_reaction, remove_review_label,
    remove_review_reaction, request_review, restore_native_stacks, set_review_auto_merge_state,
    set_review_draftiness, sync_reviews, update_review, update_review_comment,
    withdraw_review_request,
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
/// If the forge type can't be determined by simply looking for keywords in the repositories URL,
/// look through all the known accounts and try to match their custom host strings to the repository's URL host.
/// Looking at the known accounts involves retrieving data from storage, so that is a bit more expensive
/// and that's why it's a fallback mechanism.
pub fn derive_forge_repo_info(url: &str) -> Option<ForgeRepoInfo> {
    // git-url-parse 0.6.0's GenericProvider strips the git-suffix with
    // `take_until(".git")`, which stops at the FIRST ".git" substring rather
    // than the terminal suffix. So `org/org.github.io.git` yields repo="org"
    // and every URL built from it (PR list, base URL, compare) points at the
    // wrong repository and 404s. Strip a single trailing ".git" ourselves so
    // the parser takes its correct `is_not("/")` branch. Safe because a real
    // repo name can't end in ".git".
    let url = url.strip_suffix(".git").unwrap_or(url);
    let git_url = GitUrl::parse(url).ok()?;
    let host = git_url.host()?;
    let protocol = git_url.scheme()?;

    let provider_info: GenericProvider = git_url.provider_info().ok()?;
    // Attempt to figure out the forge by looking at the host string and
    // falling back to matching it to the known accounts custom host URL.
    let forge = determine_forge_from_host(host).or_else(|| {
        // Only fetch the accounts if it can't determine the forge type from the repository's host.
        let accounts = get_all_forge_accounts().unwrap_or_default();
        match_host_to_accounts_custom_host(host, &accounts)
    })?;

    Some(ForgeRepoInfo {
        forge,
        owner: provider_info.owner().to_string(),
        repo: provider_info.repo().to_string(),
        protocol: protocol.to_string(),
    })
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

/// The login this repository's forge calls authenticate as: the preferred
/// account when it is known to storage, otherwise the first known account of
/// the repository's forge — mirroring how the per-forge clients resolve
/// their account. `None` when no matching account is configured.
pub fn current_forge_login(
    preferred_forge_user: &Option<ForgeUser>,
    forge_repo_info: &ForgeRepoInfo,
    storage: &but_forge_storage::Controller,
) -> anyhow::Result<Option<String>> {
    match forge_repo_info.forge {
        ForgeName::GitHub => {
            let accounts = but_github::list_known_github_accounts(storage)?;
            let preferred = preferred_forge_user
                .as_ref()
                .and_then(|user| user.github())
                .filter(|preferred| accounts.contains(preferred));
            Ok(preferred
                .or(accounts.first())
                .map(|account| account.username().to_string()))
        }
        ForgeName::GitLab => {
            let accounts = but_gitlab::list_known_gitlab_accounts(storage)?;
            let preferred = preferred_forge_user
                .as_ref()
                .and_then(|user| user.gitlab())
                .filter(|preferred| accounts.contains(preferred));
            Ok(preferred
                .or(accounts.first())
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
        ForgeName, ForgeUser, derive_forge_repo_info, match_host_to_accounts_custom_host,
        normalize_host_for_comparison,
    };

    // Regression for #15302: a repo name that contains ".git" as a substring
    // (or a trailing ".git" git-suffix) must be parsed intact. The upstream
    // GenericProvider stopped at the first ".git" and turned
    // `org/org.github.io.git` into repo="org", so `but pr new` queried
    // `GET /repos/org/org/pulls` and 404'd. These cases all resolve their
    // forge from the github.com host, so `derive_forge_repo_info` never
    // touches account storage.
    #[test]
    fn repo_name_containing_dotgit_is_parsed_correctly() {
        let cases = [
            ("git@github.com:org/repo", "org", "repo"),
            ("git@github.com:org/repo.git", "org", "repo"),
            ("https://github.com/org/repo", "org", "repo"),
            ("https://github.com/org/repo.git", "org", "repo"),
            ("git@github.com:org/org.github.io", "org", "org.github.io"),
            (
                "git@github.com:org/org.github.io.git",
                "org",
                "org.github.io",
            ),
            (
                "https://github.com/org/org.github.io",
                "org",
                "org.github.io",
            ),
            (
                "https://github.com/org/org.github.io.git",
                "org",
                "org.github.io",
            ),
            (
                "git@github.com:org/repo.github.io.git",
                "org",
                "repo.github.io",
            ),
        ];

        for (url, owner, repo) in cases {
            let info = derive_forge_repo_info(url)
                .unwrap_or_else(|| panic!("failed to parse forge info from {url}"));
            assert_eq!(info.forge, ForgeName::GitHub, "forge for {url}");
            assert_eq!(info.owner, owner, "owner for {url}");
            assert_eq!(info.repo, repo, "repo for {url}");
        }
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

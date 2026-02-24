use git_url_parse::{GitUrl, types::provider::GenericProvider};

mod forge;
pub use crate::forge::{ForgeName, ForgeRepoInfo, ForgeUser, deserialize_preferred_forge_user_opt};

mod ci;
mod db;
mod review;
pub use ci::{CiCheck, CiConclusion, CiOutput, CiStatus, ci_checks_for_ref_with_cache};
pub use review::{
    CacheConfig, CreateForgeReviewParams, ForgeAccountValidity, ForgeReview,
    ForgeReviewDescriptionUpdate, ForgeReviewFilter, ReviewTemplateFunctions,
    available_review_templates, check_forge_account_is_valid, create_forge_review,
    get_forge_review, get_review_template_functions, list_forge_reviews_for_branch,
    list_forge_reviews_with_cache, merge_review, set_review_auto_merge_state,
    set_review_draftiness, update_review_description_tables,
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
pub fn derive_forge_repo_info(url: &str) -> Option<ForgeRepoInfo> {
    let git_url = GitUrl::parse(url).ok()?;
    let host = git_url.host()?;
    let protocol = git_url.scheme()?;

    let provider_info: GenericProvider = git_url.provider_info().ok()?;
    let accounts = get_all_forge_accounts().ok().unwrap_or_default();
    // Attempt to figure out the forge by looking at the host string and
    // falling back to matching it to the known accounts custom host URL.
    let forge = determine_forge_from_host(host)
        .or_else(|| match_host_to_accounts_custom_host(host, &accounts))?;

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
    let user = accounts.iter().find(|account| match account {
        ForgeUser::GitHub(gh_account) => gh_account
            .custom_host()
            .as_deref()
            .is_some_and(|custom_host| custom_host_matches_repository_host(host, custom_host)),
        ForgeUser::GitLab(gl_account) => gl_account
            .custom_host()
            .as_deref()
            .is_some_and(|custom_host| custom_host_matches_repository_host(host, custom_host)),
    });

    match user {
        Some(ForgeUser::GitHub(_)) => Some(ForgeName::GitHub),
        Some(ForgeUser::GitLab(_)) => Some(ForgeName::GitLab),
        None => None,
    }
}

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
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(get_all_forge_accounts_async()))
    } else {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(get_all_forge_accounts_async())
    }
}

async fn get_all_forge_accounts_async() -> anyhow::Result<Vec<ForgeUser>> {
    let storage = but_forge_storage::Controller::from_path(but_path::app_data_dir()?);
    let gh_accounts = but_github::list_known_github_accounts(&storage).await?;
    let gl_accounts = but_gitlab::list_known_gitlab_accounts(&storage).await?;

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
    use super::{ForgeName, ForgeUser, match_host_to_accounts_custom_host};

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
}

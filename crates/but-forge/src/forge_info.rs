use serde::Serialize;

use crate::forge::ForgeRepoInfo;
use crate::{ForgeName, ForgeUser};

/// Per-forge display + URL config delivered to the frontend so it
/// doesn't need to branch on forge name. Computed from the project's
/// own remote URL plus a forge-name lookup for the rest.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ForgeInfo {
    pub name: ForgeName,
    /// Already SSH→HTTPS converted and includes Azure's organization
    /// segment when applicable. Append the *Path values below to build
    /// the various web URLs.
    pub base_url: String,
    /// Format: `{baseUrl}{commitUrlPath}{commitId}`.
    pub commit_url_path: String,
    /// Format: `{baseUrl}{prUrlPath}{number}`.
    pub pr_url_path: String,
    /// Display labels for PR/MR.
    pub unit: ForgeUnitInfo,
    /// PostHog event prefix ("PR Successful", "Gitlab MR Successful").
    pub posthog_label: String,
    /// Which Rust-backed services the forge supports today.
    pub capabilities: ForgeCapabilities,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeInfo);

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ForgeUnitInfo {
    pub name: String,
    pub abbr: String,
    pub symbol: String,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeUnitInfo);

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ForgeCapabilities {
    pub checks: bool,
    pub repo_info: bool,
    pub pr_service: bool,
    pub list_service: bool,
    /// Conversation comments and review submissions can be read and written.
    pub review_comments: bool,
    /// Labels and review requests can be listed and changed.
    pub review_management: bool,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeCapabilities);

/// Build the per-project ForgeInfo from the project's remote URL.
///
/// `accounts` are the known forge accounts; when one is configured for the
/// remote's host, its custom host supplies the web scheme and port that an
/// SSH remote cannot.
pub fn forge_info(remote_url: &str, accounts: &[ForgeUser]) -> Option<ForgeInfo> {
    let repo_info = crate::derive_forge_repo_info(remote_url)?;
    let base_url = build_base_url(remote_url, &repo_info, accounts);
    let (commit_path, pr_path) = url_paths(&repo_info.forge);
    let (unit, posthog) = label_for(&repo_info.forge);
    let capabilities = capabilities_for(&repo_info.forge);
    Some(ForgeInfo {
        name: repo_info.forge,
        base_url,
        commit_url_path: commit_path.into(),
        pr_url_path: pr_path.into(),
        unit,
        posthog_label: posthog.into(),
        capabilities,
    })
}

/// Web compare URL for a branch (used by "Open in browser" actions).
/// `fork` is the owner namespace for forks (GitHub `bob:branch` form).
pub fn compare_branch_url(
    remote_url: &str,
    base: &str,
    branch: &str,
    fork: Option<&str>,
    accounts: &[ForgeUser],
) -> Option<String> {
    let repo_info = crate::derive_forge_repo_info(remote_url)?;
    let base_url = build_base_url(remote_url, &repo_info, accounts);
    let head = match fork {
        Some(f) => format!("{f}:{branch}"),
        None => branch.to_string(),
    };
    Some(match repo_info.forge {
        ForgeName::GitHub => format!("{base_url}/compare/{base}...{head}"),
        ForgeName::GitLab => format!("{base_url}/-/compare/{base}...{head}"),
        ForgeName::Bitbucket => format!(
            "{base_url}/branch/{head}?dest={}",
            urlencoding::encode(base)
        ),
        ForgeName::Azure => {
            format!("{base_url}/branchCompare?baseVersion=GB{base}&targetVersion=GB{head}")
        }
    })
}

fn build_base_url(remote_url: &str, repo_info: &ForgeRepoInfo, accounts: &[ForgeUser]) -> String {
    // Web URLs need https — git+ssh remotes can't open in a browser.
    let rewrote_scheme = repo_info.protocol == "ssh" || repo_info.protocol == "git";
    let scheme = if rewrote_scheme {
        "https"
    } else {
        repo_info.protocol.as_str()
    };
    let parsed = git_url_parse::GitUrl::parse(remote_url).ok();
    let host = parsed
        .as_ref()
        .and_then(|u| u.host().map(|h| h.to_string()))
        .unwrap_or_else(|| match repo_info.forge {
            ForgeName::GitHub => "github.com".into(),
            ForgeName::GitLab => "gitlab.com".into(),
            ForgeName::Bitbucket => "bitbucket.org".into(),
            ForgeName::Azure => "dev.azure.com".into(),
        });
    let host = match parsed.as_ref().and_then(|u| u.port()) {
        Some(port) if !rewrote_scheme => format!("{host}:{port}"),
        _ => host,
    };
    match repo_info.forge {
        ForgeName::Azure => {
            // `derive_forge_repo_info` uses git-url-parse's GenericProvider,
            // which mangles Azure's org/project/repo triple into a single
            // owner/repo pair — dropping the repo name (and for SSH remotes
            // the org too). Re-parse with the Azure-specific provider.
            // Web URLs are {host}/{org}/{project}/_git/{repo}; the browser
            // host is always dev.azure.com (the ssh.* host can't open in a
            // browser).
            let host = host.strip_prefix("ssh.").unwrap_or(&host);
            match git_url_parse::GitUrl::parse(remote_url).ok().and_then(|u| {
                u.provider_info::<git_url_parse::types::provider::AzureDevOpsProvider>()
                    .ok()
            }) {
                Some(az) => format!(
                    "{scheme}://{host}/{}/{}/_git/{}",
                    az.org(),
                    az.project(),
                    az.repo()
                ),
                // Fallback: best-effort with the generic owner/repo.
                None => format!(
                    "{scheme}://{host}/{}/_git/{}",
                    repo_info.owner, repo_info.repo
                ),
            }
        }
        _ => {
            // An http(s) remote's own origin is authoritative; only rewritten
            // ssh/git remotes lack the web scheme and port.
            let origin = rewrote_scheme
                .then(|| account_web_origin(accounts, &repo_info.forge, &host))
                .flatten()
                .unwrap_or_else(|| format!("{scheme}://{host}"));
            let owner = &repo_info.owner;
            let repo = &repo_info.repo;
            format!("{origin}/{owner}/{repo}")
        }
    }
}

/// The web origin of the configured account for `host`, if any. The custom
/// host is the instance URL the user entered, so unlike the remote URL it
/// carries the web scheme and port even when the remote is SSH.
fn account_web_origin(accounts: &[ForgeUser], forge: &ForgeName, host: &str) -> Option<String> {
    let host = crate::normalize_host_for_comparison(host);
    accounts.iter().find_map(|account| {
        if account.forge_name() != *forge {
            return None;
        }
        let custom_host = account.custom_host()?;
        (crate::normalize_host_for_comparison(&custom_host) == host)
            .then(|| custom_host_origin(&custom_host))
    })
}

/// Reduce a stored custom host (hostname, origin, or full API endpoint URL)
/// to its web origin, defaulting to https when no scheme was given.
fn custom_host_origin(custom_host: &str) -> String {
    let trimmed = custom_host.trim();
    let (scheme, rest) = trimmed
        .split_once("://")
        .map_or(("https", trimmed), |(scheme, rest)| (scheme, rest));
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    format!("{}://{authority}", scheme.to_ascii_lowercase())
}

fn url_paths(forge: &ForgeName) -> (&'static str, &'static str) {
    match forge {
        ForgeName::GitHub => ("/commit/", "/pull/"),
        ForgeName::GitLab => ("/-/commit/", "/-/merge_requests/"),
        ForgeName::Bitbucket => ("/commits/", "/pull-requests/"),
        ForgeName::Azure => ("/commit/", "/pullrequest/"),
    }
}

fn label_for(forge: &ForgeName) -> (ForgeUnitInfo, &'static str) {
    match forge {
        ForgeName::GitHub | ForgeName::Bitbucket | ForgeName::Azure => (
            ForgeUnitInfo {
                name: "Pull request".into(),
                abbr: "PR".into(),
                symbol: "#".into(),
            },
            "PR",
        ),
        ForgeName::GitLab => (
            ForgeUnitInfo {
                name: "Merge request".into(),
                abbr: "MR".into(),
                symbol: "!".into(),
            },
            "Gitlab MR",
        ),
    }
}

fn capabilities_for(forge: &ForgeName) -> ForgeCapabilities {
    match forge {
        ForgeName::GitHub => ForgeCapabilities {
            checks: true,
            repo_info: true,
            pr_service: true,
            list_service: true,
            review_comments: true,
            review_management: true,
        },
        ForgeName::GitLab => ForgeCapabilities {
            checks: true,
            repo_info: true,
            pr_service: true,
            list_service: true,
            review_comments: false,
            review_management: false,
        },
        ForgeName::Bitbucket => ForgeCapabilities {
            checks: true,
            repo_info: true,
            pr_service: true,
            list_service: true,
            review_comments: false,
            review_management: false,
        },
        ForgeName::Azure => ForgeCapabilities {
            checks: false,
            repo_info: false,
            pr_service: false,
            list_service: false,
            review_comments: false,
            review_management: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These remotes all resolve their forge from the host string, so
    // `derive_forge_repo_info` never falls back to account storage.

    #[test]
    fn azure_https_base_url_keeps_org_project_and_repo() {
        let info = forge_info("https://dev.azure.com/myorg/myproject/_git/myrepo", &[]).unwrap();
        assert_eq!(info.name, ForgeName::Azure);
        // Regression: the org and repo segments must both survive — the
        // GenericProvider used to drop the repo name entirely.
        assert_eq!(
            info.base_url,
            "https://dev.azure.com/myorg/myproject/_git/myrepo"
        );
    }

    #[test]
    fn azure_ssh_base_url_uses_browsable_https_host() {
        let info = forge_info("git@ssh.dev.azure.com:v3/myorg/myproject/myrepo", &[]).unwrap();
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
        )
        .unwrap();
        assert_eq!(
            url,
            "https://dev.azure.com/myorg/myproject/_git/myrepo/branchCompare?baseVersion=GBmain&targetVersion=GBfeature"
        );
    }

    #[test]
    fn github_ssh_base_url_and_fork_compare() {
        let info = forge_info("git@github.com:owner/repo.git", &[]).unwrap();
        assert_eq!(info.name, ForgeName::GitHub);
        assert_eq!(info.base_url, "https://github.com/owner/repo");

        let url = compare_branch_url(
            "git@github.com:owner/repo.git",
            "main",
            "feat",
            Some("fork"),
            &[],
        )
        .unwrap();
        assert_eq!(
            url,
            "https://github.com/owner/repo/compare/main...fork:feat"
        );
    }

    #[test]
    fn gitlab_compare_url() {
        let url = compare_branch_url(
            "https://gitlab.com/group/repo.git",
            "main",
            "feat",
            None,
            &[],
        )
        .unwrap();
        assert_eq!(url, "https://gitlab.com/group/repo/-/compare/main...feat");
    }

    #[test]
    fn self_hosted_gitlab_custom_port_is_preserved() {
        // Regression for #14626: a self-hosted GitLab on a non-standard
        // port must keep the `:8080` in every generated web URL.
        let remote = "https://gitlab.example.com:8080/group/repo.git";
        let info = forge_info(remote, &[]).unwrap();
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
            compare_branch_url(remote, "main", "feat", None, &[]).unwrap(),
            "https://gitlab.example.com:8080/group/repo/-/compare/main...feat"
        );
    }

    #[test]
    fn ssh_remote_transport_port_is_dropped_from_web_url() {
        let info = forge_info("ssh://git@gitlab.example.com:2222/group/repo.git", &[]).unwrap();
        assert_eq!(info.name, ForgeName::GitLab);
        assert_eq!(info.base_url, "https://gitlab.example.com/group/repo");
    }

    #[test]
    fn ssh_remote_uses_configured_account_web_origin() {
        // Regression for #14626: an SSH remote carries no https port, so
        // the web origin must come from the configured self-hosted account.
        let remote = "git@gitlab.example.com:group/repo.git";
        let accounts = [ForgeUser::GitLab(
            but_gitlab::GitlabAccountIdentifier::selfhosted(
                "bob",
                "https://gitlab.example.com:8080",
            ),
        )];
        let info = forge_info(remote, &accounts).unwrap();
        assert_eq!(info.base_url, "https://gitlab.example.com:8080/group/repo");
        assert_eq!(
            compare_branch_url(remote, "main", "feat", None, &accounts).unwrap(),
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
        let info = forge_info("git@gitlab.example.com:group/repo.git", &accounts).unwrap();
        assert_eq!(info.base_url, "http://gitlab.example.com:8080/group/repo");
    }

    #[test]
    fn account_for_other_host_does_not_change_web_origin() {
        let accounts = [ForgeUser::GitLab(
            but_gitlab::GitlabAccountIdentifier::selfhosted("bob", "https://gitlab.other.com:8080"),
        )];
        let info = forge_info("git@gitlab.example.com:group/repo.git", &accounts).unwrap();
        assert_eq!(info.base_url, "https://gitlab.example.com/group/repo");
    }

    #[test]
    fn https_remote_origin_wins_over_account_without_port() {
        // #14678 behavior must survive: an https remote's explicit port is
        // authoritative even when the account host was stored without one.
        let accounts = [ForgeUser::GitLab(
            but_gitlab::GitlabAccountIdentifier::selfhosted("bob", "gitlab.example.com"),
        )];
        let info = forge_info("https://gitlab.example.com:8080/group/repo.git", &accounts).unwrap();
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
        let info = forge_info("git@gitlab.example.com:group/repo.git", &accounts).unwrap();
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
        )
        .unwrap();
        assert_eq!(
            url,
            "https://bitbucket.org/owner/repo/branch/feat?dest=release%2F1.0"
        );
    }

    // Commit and PR/MR hyperlinks are built on the frontend as
    // `{baseUrl}{commitUrlPath}{sha}` / `{baseUrl}{prUrlPath}{number}`.
    // These assert the full composed URL per forge so a typo in the
    // path segments can't silently ship broken links — including
    // Bitbucket and Azure, which have no other forge integration to
    // surface a break.

    /// Compose what the frontend `commitUrl(forge, sha)` helper produces.
    fn composed_commit_url(remote: &str, sha: &str) -> String {
        let info = forge_info(remote, &[]).unwrap();
        format!("{}{}{}", info.base_url, info.commit_url_path, sha)
    }

    /// Compose what the frontend `prUrl(forge, number)` helper produces.
    fn composed_pr_url(remote: &str, number: i64) -> String {
        let info = forge_info(remote, &[]).unwrap();
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
        let info = forge_info("https://gitlab.com/group/repo.git", &[]).unwrap();
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
}

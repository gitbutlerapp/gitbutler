use serde::Serialize;

use crate::ForgeName;
use crate::forge::ForgeRepoInfo;

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
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ForgeCapabilities);

/// Build the per-project ForgeInfo from the project's remote URL.
pub fn forge_info(remote_url: &str) -> Option<ForgeInfo> {
    let repo_info = crate::derive_forge_repo_info(remote_url)?;
    let base_url = build_base_url(remote_url, &repo_info);
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
) -> Option<String> {
    let repo_info = crate::derive_forge_repo_info(remote_url)?;
    let base_url = build_base_url(remote_url, &repo_info);
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

fn build_base_url(remote_url: &str, repo_info: &ForgeRepoInfo) -> String {
    // Web URLs need https — git+ssh remotes can't open in a browser.
    let scheme = if repo_info.protocol == "ssh" || repo_info.protocol == "git" {
        "https"
    } else {
        repo_info.protocol.as_str()
    };
    let host = git_url_parse::GitUrl::parse(remote_url)
        .ok()
        .and_then(|u| u.host().map(|h| h.to_string()))
        .unwrap_or_else(|| match repo_info.forge {
            ForgeName::GitHub => "github.com".into(),
            ForgeName::GitLab => "gitlab.com".into(),
            ForgeName::Bitbucket => "bitbucket.org".into(),
            ForgeName::Azure => "dev.azure.com".into(),
        });
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
            let owner = &repo_info.owner;
            let repo = &repo_info.repo;
            format!("{scheme}://{host}/{owner}/{repo}")
        }
    }
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
        },
        ForgeName::GitLab => ForgeCapabilities {
            checks: true,
            repo_info: true,
            pr_service: true,
            list_service: true,
        },
        ForgeName::Bitbucket => ForgeCapabilities {
            checks: true,
            repo_info: true,
            pr_service: true,
            list_service: true,
        },
        ForgeName::Azure => ForgeCapabilities {
            checks: false,
            repo_info: false,
            pr_service: false,
            list_service: false,
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
        let info = forge_info("https://dev.azure.com/myorg/myproject/_git/myrepo").unwrap();
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
        let info = forge_info("git@ssh.dev.azure.com:v3/myorg/myproject/myrepo").unwrap();
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
        )
        .unwrap();
        assert_eq!(
            url,
            "https://dev.azure.com/myorg/myproject/_git/myrepo/branchCompare?baseVersion=GBmain&targetVersion=GBfeature"
        );
    }

    #[test]
    fn github_ssh_base_url_and_fork_compare() {
        let info = forge_info("git@github.com:owner/repo.git").unwrap();
        assert_eq!(info.name, ForgeName::GitHub);
        assert_eq!(info.base_url, "https://github.com/owner/repo");

        let url = compare_branch_url(
            "git@github.com:owner/repo.git",
            "main",
            "feat",
            Some("fork"),
        )
        .unwrap();
        assert_eq!(
            url,
            "https://github.com/owner/repo/compare/main...fork:feat"
        );
    }

    #[test]
    fn gitlab_compare_url() {
        let url =
            compare_branch_url("https://gitlab.com/group/repo.git", "main", "feat", None).unwrap();
        assert_eq!(url, "https://gitlab.com/group/repo/-/compare/main...feat");
    }

    #[test]
    fn bitbucket_compare_url_encodes_dest() {
        let url = compare_branch_url(
            "https://bitbucket.org/owner/repo.git",
            "release/1.0",
            "feat",
            None,
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
        let info = forge_info(remote).unwrap();
        format!("{}{}{}", info.base_url, info.commit_url_path, sha)
    }

    /// Compose what the frontend `prUrl(forge, number)` helper produces.
    fn composed_pr_url(remote: &str, number: i64) -> String {
        let info = forge_info(remote).unwrap();
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
        let info = forge_info("https://gitlab.com/group/repo.git").unwrap();
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

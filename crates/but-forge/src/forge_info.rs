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
/// `override_forge` is the project's saved forge preference, applied as a last
/// resort when the URL and configured accounts don't identify a forge (#14319).
pub fn forge_info(
    remote_url: &str,
    accounts: &[ForgeUser],
    override_forge: Option<ForgeName>,
) -> Option<ForgeInfo> {
    let repo_info = crate::derive_forge_repo_info(remote_url, override_forge)?;
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
/// `accounts` and `override_forge` have the same meaning as in [`forge_info`].
pub fn compare_branch_url(
    remote_url: &str,
    base: &str,
    branch: &str,
    fork: Option<&str>,
    accounts: &[ForgeUser],
    override_forge: Option<ForgeName>,
) -> Option<String> {
    let repo_info = crate::derive_forge_repo_info(remote_url, override_forge)?;
    let base_url = build_base_url(remote_url, &repo_info, accounts);
    let head = match fork {
        Some(f) => format!("{f}:{branch}"),
        None => branch.to_string(),
    };
    // Branch names may contain URL delimiters such as `#` or `%`.
    let path_head = match fork {
        Some(f) => format!("{}:{}", urlencoding::encode(f), encode_path(branch)),
        None => encode_path(branch),
    };
    Some(match repo_info.forge {
        ForgeName::GitHub => format!("{base_url}/compare/{}...{path_head}", encode_path(base)),
        ForgeName::GitLab => format!("{base_url}/-/compare/{}...{path_head}", encode_path(base)),
        ForgeName::Bitbucket => format!(
            "{base_url}/branch/{path_head}?dest={}",
            urlencoding::encode(base)
        ),
        ForgeName::Azure => format!(
            "{base_url}/branchCompare?baseVersion=GB{}&targetVersion=GB{}",
            urlencoding::encode(base),
            urlencoding::encode(&head)
        ),
    })
}

/// Percent-encode each `/`-separated segment, keeping the separators.
fn encode_path(path: &str) -> String {
    path.split('/')
        .map(urlencoding::encode)
        .collect::<Vec<_>>()
        .join("/")
}

fn build_base_url(remote_url: &str, repo_info: &ForgeRepoInfo, accounts: &[ForgeUser]) -> String {
    // Web URLs need https — git+ssh remotes can't open in a browser.
    let rewrote_scheme = repo_info.protocol == "ssh" || repo_info.protocol == "git";
    let scheme = if rewrote_scheme {
        "https"
    } else {
        repo_info.protocol.as_str()
    };
    let parsed = crate::remote_url::RemoteUrl::parse(remote_url);
    let host = parsed
        .as_ref()
        .map(|url| url.host.clone())
        .unwrap_or_else(|| match repo_info.forge {
            ForgeName::GitHub => "github.com".into(),
            ForgeName::GitLab => "gitlab.com".into(),
            ForgeName::Bitbucket => "bitbucket.org".into(),
            ForgeName::Azure => "dev.azure.com".into(),
        });
    let host = match parsed.as_ref().and_then(|url| url.port) {
        Some(port) if !rewrote_scheme => format!("{host}:{port}"),
        _ => host,
    };
    // Owners may contain namespace separators (GitLab groups or Azure org/project).
    let owner = encode_path(&repo_info.owner);
    let repo = urlencoding::encode(&repo_info.repo);
    match repo_info.forge {
        ForgeName::Azure => {
            // Web URLs are {host}/{org}/{project}/_git/{repo}; the browser
            // host is always dev.azure.com (the ssh.* host can't open in a
            // browser). Azure repository owners include both org and project.
            let host = host.strip_prefix("ssh.").unwrap_or(&host);
            format!("{scheme}://{host}/{owner}/_git/{repo}")
        }
        _ => {
            // An http(s) remote's own origin is authoritative; only rewritten
            // ssh/git remotes lack the web scheme and port.
            let origin = rewrote_scheme
                .then(|| account_web_origin(accounts, &repo_info.forge, &host))
                .flatten()
                .unwrap_or_else(|| format!("{scheme}://{host}"));
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

    #[test]
    fn self_hosted_base_url_built_from_override_repo_info() {
        // On the #14319 override path, `derive_forge_repo_info` fabricates
        // `ForgeRepoInfo` exactly as the account path would - owner/repo parsed
        // from the URL - so `build_base_url` must produce the self-hosted web
        // URL from it, taking the host from the remote rather than a hardcoded
        // per-forge default.
        let repo_info = ForgeRepoInfo {
            forge: ForgeName::GitLab,
            host: "git.selfhosted.example".into(),
            port: None,
            owner: "group".into(),
            repo: "repo".into(),
            protocol: "https".into(),
        };
        assert_eq!(
            build_base_url(
                "https://git.selfhosted.example/group/repo.git",
                &repo_info,
                &[]
            ),
            "https://git.selfhosted.example/group/repo"
        );
    }
}

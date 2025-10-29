use git_url_parse::{GitUrl, types::provider::GenericProvider};

use crate::forge::{ForgeName, ForgeRepoInfo};
pub use crate::forge::{ForgeUser, deserialize_preferred_forge_user_opt};

pub mod forge;
pub mod review;

fn determine_forge_from_host(host: &str) -> Option<ForgeName> {
    if host.contains("github.com") {
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

    Some(ForgeRepoInfo {
        forge: determine_forge_from_host(host)?,
        owner: provider_info.owner().to_string(),
        repo: provider_info.repo().to_string(),
        protocol: protocol.to_string(),
    })
}

/// Determine the forge type from a given URL.
pub fn determine_forge_from_url(url: &str) -> Option<ForgeName> {
    let repo_info = derive_forge_repo_info(url)?;
    Some(repo_info.forge)
}

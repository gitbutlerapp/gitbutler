use crate::forge::ForgeName;

pub mod forge;
pub mod review;

/// Determine the forge type from a given URL.
pub fn determine_forge_from_url(url: &str) -> Option<ForgeName> {
    if url.contains("github.com") {
        Some(ForgeName::GitHub)
    } else if url.contains("gitlab.com") || url.starts_with("gitlab.") {
        Some(ForgeName::GitLab)
    } else if url.contains("bitbucket.org") {
        Some(ForgeName::Bitbucket)
    } else if url.contains("azure.com") {
        Some(ForgeName::Azure)
    } else {
        None
    }
}

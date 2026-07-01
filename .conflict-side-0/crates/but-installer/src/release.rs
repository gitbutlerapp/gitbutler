//! Release fetching and validation

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use crate::{config::InstallerConfig, http::create_client};

/// Release information from the GitButler API
#[derive(Debug, Deserialize)]
pub struct Release {
    pub version: String,
    pub platforms: HashMap<String, PlatformInfo>,
}

#[derive(Debug, Deserialize)]
pub struct PlatformInfo {
    pub url: Option<String>,

    #[cfg(target_os = "macos")]
    pub signature: String,
}

pub(crate) fn fetch_release(config: &InstallerConfig) -> Result<Release> {
    let url = config.releases_url();
    let mut easy = create_client()?;

    easy.url(&url)
        .with_context(|| format!("Failed to set URL: {url}"))?;

    let mut response_data = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|data| {
                response_data.extend_from_slice(data);
                Ok(data.len())
            })
            .context("Failed to set write function")?;
        transfer
            .perform()
            .with_context(|| format!("Failed to fetch release information from {url}"))?;
    }

    let response_code = easy
        .response_code()
        .context("Failed to get response code")?;

    if response_code != 200 {
        match &config.version_request {
            crate::config::VersionRequest::Specific(version) => {
                bail!(
                    "Failed to fetch release information for version {version}. Version may not exist. HTTP {response_code}"
                );
            }
            crate::config::VersionRequest::Nightly => {
                bail!("Failed to fetch nightly release information. HTTP {response_code}");
            }
            crate::config::VersionRequest::Release => {
                bail!("Failed to fetch release information from {url}. HTTP {response_code}");
            }
        }
    }

    // Validate the effective URL after following redirects
    // This protects against malicious redirects to untrusted domains or insecure protocols
    let effective_url = easy
        .effective_url()
        .context("Failed to get effective URL")?
        .ok_or_else(|| anyhow!("Effective URL is missing"))?;

    validate_api_url(effective_url).with_context(|| {
        format!("Release API was redirected to an untrusted URL: {effective_url}")
    })?;

    let release: Release =
        serde_json::from_slice(&response_data).context("Failed to parse release information")?;

    // Verify we got the version we requested (skip check for nightly)
    if let crate::config::VersionRequest::Specific(ref requested) = config.version_request
        && release.version != requested.as_str()
    {
        bail!(
            "API returned version {} but requested version {}",
            release.version,
            requested
        );
    }

    Ok(release)
}

/// Common URL validation logic for GitButler domains.
///
/// Validates HTTPS protocol, parses URL, and checks the host against a predicate.
fn validate_gitbutler_url(
    url: &str,
    url_type: &str,
    is_host_valid: impl Fn(&str) -> bool,
) -> Result<()> {
    // Only allow HTTPS URLs
    if !url.starts_with("https://") {
        bail!("{url_type} must use HTTPS: {url}");
    }

    // Extract host from URL
    let url_parsed =
        url::Url::parse(url).with_context(|| format!("Invalid {} URL", url_type.to_lowercase()))?;
    let host = url_parsed
        .host_str()
        .ok_or_else(|| anyhow!("No host in {} URL", url_type.to_lowercase()))?;

    // Validate host using the provided predicate
    if !is_host_valid(host) {
        bail!("{url_type} is not from a trusted GitButler domain: {url}");
    }

    Ok(())
}

/// Validates that an API URL is from the trusted API domain.
///
/// API endpoints should only be served from app.gitbutler.com to prevent
/// redirecting API requests to other subdomains.
pub(crate) fn validate_api_url(url: &str) -> Result<()> {
    validate_gitbutler_url(url, "API URL", |host| host == "app.gitbutler.com")
}

/// Validates that a download URL is from a trusted GitButler domain.
///
/// This is more permissive than API validation, allowing downloads from:
/// - `gitbutler.com` (root domain)
/// - Any `*.gitbutler.com` subdomain (e.g., `releases.gitbutler.com`, `cdn.gitbutler.com`)
///
/// This broader policy allows for operational flexibility (CDN distribution, mirrors, etc.)
/// while maintaining security since all `*.gitbutler.com` subdomains are under GitButler's control.
/// An attacker would need to compromise GitButler's DNS or infrastructure to exploit this.
pub(crate) fn validate_download_url(url: &str) -> Result<()> {
    validate_gitbutler_url(url, "Download URL", |host| {
        host == "gitbutler.com" || host.ends_with(".gitbutler.com")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_api_url() {
        // Valid API URLs - only app.gitbutler.com
        assert!(validate_api_url("https://app.gitbutler.com/releases").is_ok());
        assert!(validate_api_url("https://app.gitbutler.com/releases/nightly").is_ok());
        assert!(validate_api_url("https://app.gitbutler.com/releases/version/0.18.7").is_ok());

        // Invalid - wrong protocol
        assert!(validate_api_url("http://app.gitbutler.com/releases").is_err());

        // Invalid - wrong subdomain (even if trusted for downloads)
        assert!(validate_api_url("https://releases.gitbutler.com/releases").is_err());
        assert!(validate_api_url("https://gitbutler.com/releases").is_err());
        assert!(validate_api_url("https://api.gitbutler.com/releases").is_err());

        // Invalid - untrusted domains
        assert!(validate_api_url("https://evil.com/releases").is_err());
        assert!(validate_api_url("https://app.gitbutler.com.evil.com/releases").is_err());
    }

    #[test]
    fn test_validate_download_url() {
        // Valid - root domain
        assert!(validate_download_url("https://gitbutler.com/downloads/file.tar.gz").is_ok());

        // Valid - any *.gitbutler.com subdomain
        assert!(validate_download_url("https://releases.gitbutler.com/file.tar.gz").is_ok());
        assert!(validate_download_url("https://app.gitbutler.com/downloads/file.tar.gz").is_ok());
        assert!(validate_download_url("https://cdn.gitbutler.com/file.tar.gz").is_ok());
        assert!(validate_download_url("https://mirror.gitbutler.com/file.tar.gz").is_ok());

        // Invalid - wrong protocol
        assert!(validate_download_url("http://gitbutler.com/file.tar.gz").is_err());
        assert!(validate_download_url("http://releases.gitbutler.com/file.tar.gz").is_err());

        // Invalid - subdomain spoofing attempts
        assert!(validate_download_url("https://gitbutler.com.evil.com/file.tar.gz").is_err());
        assert!(validate_download_url("https://evilgitbutler.com/file.tar.gz").is_err());

        // Invalid - untrusted domains
        assert!(validate_download_url("https://evil.com/file.tar.gz").is_err());
    }

    #[test]
    fn test_release_parsing_allows_null_platform_url() {
        let json = r#"{
            "version": "0.5.1",
            "platforms": {
                "darwin-aarch64": {
                    "url": "https://releases.gitbutler.com/file.tar.gz",
                    "signature": "sig-darwin"
                },
                "windows-x86_64": {
                    "url": null,
                    "signature": "sig-windows"
                }
            }
        }"#;

        let release: Release = serde_json::from_str(json).unwrap();

        let darwin = release.platforms.get("darwin-aarch64").unwrap();
        let windows = release.platforms.get("windows-x86_64").unwrap();

        assert_eq!(
            darwin.url.as_deref(),
            Some("https://releases.gitbutler.com/file.tar.gz")
        );
        assert!(windows.url.is_none());
    }
}

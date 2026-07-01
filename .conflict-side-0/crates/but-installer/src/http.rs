//! HTTP client configuration

use std::time::Duration;

use anyhow::{Context, Result};
use curl::easy::Easy;

const REQUEST_TIMEOUT_SECS: u64 = 300;
const CONNECT_TIMEOUT_SECS: u64 = 10;
const MAX_REDIRECTS: u32 = 5;
const USER_AGENT: &str = concat!(
    "GitButler-Installer/",
    env!("CARGO_PKG_VERSION"),
    " (Rust installer)"
);

/// Create a configured curl Easy handle with appropriate timeouts and user agent
pub(crate) fn create_client() -> Result<Easy> {
    let mut easy = Easy::new();
    easy.useragent(USER_AGENT)
        .context("Failed to set user agent")?;
    easy.timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .context("Failed to set timeout")?;
    easy.connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .context("Failed to set connect timeout")?;
    easy.follow_location(true)
        .context("Failed to enable redirect following")?;
    easy.max_redirections(MAX_REDIRECTS)
        .context("Failed to set max redirects")?;
    Ok(easy)
}

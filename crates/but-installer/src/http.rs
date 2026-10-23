//! HTTP client configuration

use std::time::Duration;

use reqwest::{
    blocking::{Client, Response},
    redirect::Policy,
};

const REQUEST_TIMEOUT_SECS: u64 = 300;
const CONNECT_TIMEOUT_SECS: u64 = 10;
const MAX_REDIRECTS: usize = 5;
const USER_AGENT: &str = concat!(
    "GitButler-Installer/",
    env!("CARGO_PKG_VERSION"),
    " (Rust installer)"
);

/// Send a GET request, following redirects, and return the response with its body still unread.
pub(crate) fn get(url: &str) -> reqwest::Result<Response> {
    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .redirect(Policy::limited(MAX_REDIRECTS))
        .build()?
        .get(url)
        .send()
}

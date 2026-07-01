//! Client for the GitButler web API authentication endpoints.
//!
//! These functions make server-side HTTP calls to `app.gitbutler.com` (or staging)
//! so that browser-based frontends don't need to make cross-origin requests.
//! They are also usable from the CLI (`but auth`) without any web framework dependency.
//!
//! The public API is synchronous — async HTTP calls are executed on a dedicated
//! thread with a short-lived Tokio runtime, following the same pattern as `but-forge`.

use std::time::Duration;

use anyhow::{Context, Result};
use but_path::AppChannel;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("failed to build HTTP client")
}

/// Error returned when the upstream API rejects a request with an HTTP error status.
#[derive(Debug, thiserror::Error)]
#[error("API request failed ({status}): {body}")]
pub struct ApiHttpError {
    pub status: StatusCode,
    pub body: String,
}

/// Returns the GitButler API base URL.
///
/// Resolution order:
/// 1. `GITBUTLER_API_URL` env var at runtime (backend-specific escape hatch)
/// 2. `PUBLIC_API_BASE_URL` env var at runtime (shared with the desktop frontend)
/// 3. Compile-time [`AppChannel`]:
///    - `Release` / `Nightly` → `https://app.gitbutler.com`
///    - `Dev` → `https://app.staging.gitbutler.com`
pub fn default_api_url() -> String {
    if let Some(url) = api_url_override_from_env(|key| std::env::var(key).ok()) {
        return url;
    }
    match AppChannel::new() {
        AppChannel::Release | AppChannel::Nightly => "https://app.gitbutler.com",
        AppChannel::Dev => "https://app.staging.gitbutler.com",
    }
    .to_string()
}

fn normalize_api_url(url: String) -> String {
    url.trim_end_matches('/').to_string()
}

fn api_url_override_from_env(mut get_var: impl FnMut(&str) -> Option<String>) -> Option<String> {
    get_var("GITBUTLER_API_URL")
        .or_else(|| get_var("PUBLIC_API_BASE_URL"))
        .map(normalize_api_url)
}

/// Response from `POST /api/login/token.json`.
#[derive(Debug, Deserialize, Serialize)]
pub struct LoginToken {
    /// Polling token returned by the API for completing login.
    ///
    /// This value is sensitive. Although it is obtained via a server-side HTTP
    /// call, this struct may be returned from backend commands to browser-based
    /// frontends, so callers must avoid exposing or logging it unnecessarily.
    pub token: String,
    /// Token shown to the user after authentication on gitbutler.com.
    pub browser_token: String,
    /// Expiration timestamp.
    pub expires: String,
    /// The full URL to redirect the user's browser to for login.
    pub url: String,
}

/// Request a new login token from the GitButler API.
///
/// The returned [`LoginToken::url`] should be opened in the user's browser.
/// After the user authenticates, they receive a token that can be validated
/// with [`validate_token_owner`].
pub fn fetch_login_token() -> Result<LoginToken> {
    let api_url = default_api_url();
    run_async(async move {
        let url = format!("{api_url}/api/login/token.json");
        let client = http_client();
        let resp = client
            .post(&url)
            .send()
            .await
            .context("Failed to reach GitButler API for login token")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiHttpError { status, body }.into());
        }
        resp.json()
            .await
            .context("Failed to parse login token response")
    })
}

/// Validate an access token and return the user info from the GitButler API.
///
/// Calls `GET /api/login/whoami` with the given token. On success the user
/// object is returned as a [`serde_json::Value`] so callers can deserialize
/// into whatever type they need.
pub fn fetch_user_by_token(token: &str) -> Result<serde_json::Value> {
    let api_url = default_api_url();
    let token = token.to_string();
    run_async(async move {
        let url = format!("{api_url}/api/login/whoami");
        let client = http_client();
        let resp = client
            .get(&url)
            .header("X-Auth-Token", &token)
            .send()
            .await
            .context("Failed to reach GitButler API for token validation")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiHttpError { status, body }.into());
        }
        resp.json().await.context("Failed to parse whoami response")
    })
}

/// Read the stored user's access token from disk.
fn stored_access_token() -> Result<String> {
    let user = crate::get_user()?.context("No logged-in user")?;
    Ok(user.access_token()?.0)
}

/// Fetch the authenticated user's profile from the GitButler API.
///
/// Calls `GET /api/user.json` using the stored user's access token.
pub fn fetch_user_profile() -> Result<serde_json::Value> {
    let api_url = default_api_url();
    let token = stored_access_token()?;
    run_async(async move {
        let url = format!("{api_url}/api/user.json");
        let client = http_client();
        let resp = client
            .get(&url)
            .header("X-Auth-Token", &token)
            .send()
            .await
            .context("Failed to reach GitButler API for user profile")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiHttpError { status, body }.into());
        }
        resp.json()
            .await
            .context("Failed to parse user profile response")
    })
}

/// Parameters for updating the user profile.
#[derive(Debug, Deserialize)]
pub struct UpdateUserParams {
    pub name: Option<String>,
    pub website: Option<String>,
    pub twitter: Option<String>,
    pub bluesky: Option<String>,
    pub timezone: Option<String>,
    pub location: Option<String>,
    pub email_share: Option<bool>,
    /// Base64-encoded avatar image bytes.
    pub avatar_base64: Option<String>,
    /// Original filename of the avatar (e.g. "photo.png").
    pub avatar_filename: Option<String>,
}

/// Update the authenticated user's profile on the GitButler API.
///
/// Calls `PUT /api/user.json` with multipart form data.
pub fn update_user_profile(params: UpdateUserParams) -> Result<serde_json::Value> {
    let api_url = default_api_url();
    let token = stored_access_token()?;
    run_async(async move {
        let url = format!("{api_url}/api/user.json");
        let client = http_client();

        let mut form = reqwest::multipart::Form::new();
        if let Some(name) = params.name {
            form = form.text("name", name);
        }
        if let Some(website) = params.website {
            form = form.text("website", website);
        }
        if let Some(twitter) = params.twitter {
            form = form.text("twitter", twitter);
        }
        if let Some(bluesky) = params.bluesky {
            form = form.text("bluesky", bluesky);
        }
        if let Some(timezone) = params.timezone {
            form = form.text("timezone", timezone);
        }
        if let Some(location) = params.location {
            form = form.text("location", location);
        }
        if let Some(email_share) = params.email_share {
            form = form.text("email_share", email_share.to_string());
        }
        if let Some(avatar_b64) = params.avatar_base64 {
            use base64::Engine as _;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&avatar_b64)
                .context("Invalid base64 in avatar data")?;
            let filename = params
                .avatar_filename
                .unwrap_or_else(|| "avatar".to_string());
            let part = reqwest::multipart::Part::bytes(bytes).file_name(filename);
            form = form.part("avatar", part);
        }

        let resp = client
            .put(&url)
            .header("X-Auth-Token", &token)
            .multipart(form)
            .send()
            .await
            .context("Failed to reach GitButler API for profile update")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(ApiHttpError { status, body }.into());
        }
        resp.json()
            .await
            .context("Failed to parse profile update response")
    })
}

/// Check whether a token belongs to a specific user.
///
/// This is a convenience wrapper around [`fetch_user_by_token`] used by
/// the remote-access auth middleware to verify that the authenticated user
/// matches the local machine owner.
///
/// Returns `Ok(false)` for authentication failures (401/403) so that
/// callers can distinguish "not the owner" from actual errors. Other HTTP
/// errors (e.g. 429 rate-limit, 5xx) and network failures produce `Err`.
pub fn validate_token_owner(token: &str, expected_user_id: u64) -> Result<bool> {
    let user = match fetch_user_by_token(token) {
        Ok(user) => user,
        Err(e) => match e.downcast_ref::<ApiHttpError>() {
            Some(http_err)
                if http_err.status == StatusCode::UNAUTHORIZED
                    || http_err.status == StatusCode::FORBIDDEN =>
            {
                return Ok(false);
            }
            _ => return Err(e),
        },
    };
    let id = user
        .get("id")
        .and_then(|v| v.as_u64())
        .context("whoami response missing 'id' field")?;
    Ok(id == expected_user_id)
}

/// Execute an async future on a dedicated thread with its own Tokio runtime.
///
/// This keeps the crate's public API synchronous while still using async HTTP
/// internally, following the same pattern as `but-forge`.
fn run_async<F, T>(future: F) -> Result<T>
where
    F: std::future::Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .expect("failed to create tokio runtime")
            .block_on(future)
    })
    .join()
    .map_err(|e| anyhow::anyhow!("thread panicked: {e:?}"))?
}

#[cfg(test)]
mod tests {
    use super::api_url_override_from_env;

    #[test]
    fn prefers_backend_specific_override() {
        let url = api_url_override_from_env(|key| match key {
            "GITBUTLER_API_URL" => Some("https://backend.example.com".to_string()),
            "PUBLIC_API_BASE_URL" => Some("https://frontend.example.com".to_string()),
            _ => None,
        });

        assert_eq!(url.as_deref(), Some("https://backend.example.com"));
    }

    #[test]
    fn falls_back_to_shared_frontend_override() {
        let url = api_url_override_from_env(|key| match key {
            "PUBLIC_API_BASE_URL" => Some("https://frontend.example.com".to_string()),
            _ => None,
        });

        assert_eq!(url.as_deref(), Some("https://frontend.example.com"));
    }
}

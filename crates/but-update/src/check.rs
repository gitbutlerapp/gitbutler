use std::env;
use std::time::Duration;

use but_secret::secret;
use but_settings::AppSettings;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};

const UPDATES_CHECK_URL: &str = "https://app.gitbutler.com/updates";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Identifies which GitButler application variant to check for updates.
///
/// Different application variants may have different release schedules and update channels.
#[derive(Debug, Serialize)]
pub enum AppName {
    /// The Tauri-based desktop GUI application.
    ///
    /// This is the main GitButler desktop application with a graphical user interface.
    Tauri,
    /// The command-line interface tool (`but`).
    ///
    /// This is the CLI tool for interacting with GitButler from the terminal.
    Cli,
}

impl std::fmt::Display for AppName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppName::Tauri => write!(f, "tauri"),
            AppName::Cli => write!(f, "but-cli"),
        }
    }
}

/// Checks if a newer version of the specified GitButler application is available.
///
/// Performs a synchronous HTTP request to the GitButler update server with a 30-second timeout.
/// Returns information about the latest available version, including download URLs and release notes.
///
/// The `CHANNEL` environment variable (set at compile time) determines which release channel to query.
/// Defaults to `"nightly"` if not set. The `VERSION` environment variable specifies the current version.
/// Defaults to `"0.0.0"` if not set.
///
/// # Errors
///
/// Returns an error if the network request fails or times out, the server returns an invalid response,
/// or the update check thread panics.
pub fn check_status(
    app_name: AppName,
    app_settings: &AppSettings,
) -> anyhow::Result<CheckUpdateStatus> {
    check_status_with_url(app_name, app_settings, None)
}

/// Testing variant of [`check_status`] that allows overriding the update server URL.
///
/// Primarily useful for integration tests with mock servers.
pub fn check_status_with_url(
    app_name: AppName,
    app_settings: &AppSettings,
    url_override: Option<&str>,
) -> anyhow::Result<CheckUpdateStatus> {
    let channel = option_env!("CHANNEL").unwrap_or("nightly");
    let os = env::consts::OS;
    let arch = env::consts::ARCH;
    let version = option_env!("VERSION").unwrap_or("0.0.0");
    let creds = secret::retrieve("gitbutler_access_token", secret::Namespace::BuildKind)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(USER_AGENT, HeaderValue::from_static("GitButler"));
    if let Some(key) = creds
        && let Ok(header_value) = key.0.parse()
    {
        headers.insert("X-Auth-Token", header_value);
    }

    let request_body = CheckUpdatesRequest {
        channel: channel.to_string(),
        os: os.to_string(),
        arch: arch.to_string(),
        version: version.to_string(),
        app_name: app_name.to_string(),
        posthog_id: if app_settings.telemetry.app_metrics_enabled {
            app_settings.telemetry.app_distinct_id.clone()
        } else {
            None
        },
        install: install(),
    };

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(REQUEST_TIMEOUT)
        .build()?;

    let url = url_override.unwrap_or(UPDATES_CHECK_URL).to_string();

    let result = std::thread::spawn(move || -> anyhow::Result<CheckUpdateStatus> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create runtime: {}", e))?;

        runtime.block_on(async {
            let response = client
                .post(url)
                .json(&request_body)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("Request failed: {}", e))?
                .error_for_status()
                .map_err(|e| anyhow::anyhow!("Server returned error: {}", e))?;

            let update_info = response
                .json::<CheckUpdateStatus>()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to parse response: {}", e))?;

            Ok(update_info)
        })
    })
    .join()
    .map_err(|_| anyhow::anyhow!("Update check thread panicked"))?;

    // Save to cache (best-effort, failures are silently ignored)
    if let Ok(status) = &result {
        crate::cache::save(status);
    }

    result
}

/// A request to check for the presence of a newer version of the application for a specified
///  - release channel
/// - operating system
/// - architecture
///
/// The server will compare the provided version against the latest published version
#[derive(Serialize)]
struct CheckUpdatesRequest {
    /// The release channel (e.g., "nightly", "release").
    channel: String,
    /// Operating system to check for.
    os: String,
    /// Architecture to check for.
    arch: String,
    /// The current version of the application.
    version: String,
    /// The name of the application (e.g., "tauri", "but-cli").
    app_name: String,
    /// The PostHog distinct ID for telemetry (if enabled).
    #[serde(skip_serializing_if = "Option::is_none")]
    posthog_id: Option<String>,
    /// Optional installation info.
    #[serde(skip_serializing_if = "Option::is_none")]
    install: Option<String>,
}

/// Information about the latest available version and whether an update is needed.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CheckUpdateStatus {
    /// `true` if the current version matches the latest available version, `false` otherwise.
    ///
    /// When this is `false`, you should prompt the user to update or automatically download
    /// the update based on your application's update policy.
    pub up_to_date: bool,

    /// The version string of the latest available release (e.g., "0.18.3").
    ///
    /// This field is always present and can be compared with the current application version
    /// to determine if an update is available.
    pub latest_version: String,

    /// Markdown-formatted release notes describing changes in the latest version.
    ///
    /// This field is `None` if the server doesn't provide release notes.
    /// When present, this should be displayed to the user to inform them about
    /// what's new in the update.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_notes: Option<String>,

    /// Direct download URL for the update package.
    ///
    /// This field is `None` if no update is needed (`up_to_date == true`) or if the server
    /// doesn't provide a direct download link. The URL points to a platform-specific installer
    /// or update package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Cryptographic signature for verifying the authenticity of the downloaded update.
    ///
    /// This field is `None` if no signature is available. When present, this should be used
    /// to verify the integrity and authenticity of the downloaded update package before
    /// installation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

fn install() -> Option<String> {
    Some(format!(
        "{:x}",
        <sha2::Sha256 as sha2::Digest>::digest(
            format!("{}{}", machine_uid::get().ok()?, "gitbutler").as_bytes()
        )
    ))
}

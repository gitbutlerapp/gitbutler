#![allow(deprecated)]
use serde::{Deserialize, Serialize};

mod legacy;
pub use legacy::LegacySettings;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Whether the user has passed the onboarding flow.
    pub onboarding_complete: bool,
    /// Telemetry settings
    pub telemetry: app_settings::TelemetrySettings,
    /// Client ID for the GitHub OAuth application.
    pub github_oauth_app: app_settings::GitHubOAuthAppSettings,
}

pub mod app_settings;
mod json;
mod persistence;
mod watch;
pub use watch::AppSettingsWithDiskSync;

pub mod api;

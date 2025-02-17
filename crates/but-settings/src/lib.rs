#![allow(deprecated)]
use serde::{Deserialize, Serialize};

mod legacy;
pub use legacy::LegacySettings;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// The amount of context lines to show in unified diffs, above and below the hunk.
    pub context_lines: u32,
    /// Whether the user has passed the onboarding flow.
    pub onboarding_complete: bool,
    /// Telemetry settings
    pub telemetry: app_settings::TelemetrySettings,
    /// Client ID for the GitHub OAuth application.
    pub github_oauth_app: app_settings::GitHubOAuthAppSettings,
    /// Application feature flags.
    pub feature_flags: app_settings::FeatureFlags,
}

impl Default for AppSettings {
    fn default() -> Self {
        // this is safe because we know the default settings are a static assets file that is always valid
        let settings: serde_json::Value = serde_json_lenient::from_str(persistence::DEFAULTS)
            .expect("BUG: default settings are always a valid JSON");
        serde_json::from_value(settings)
            .expect("BUG: default settings structure always matches the type")
    }
}

pub mod app_settings;
mod json;
mod persistence;
mod watch;
pub use watch::AppSettingsWithDiskSync;

pub mod api;

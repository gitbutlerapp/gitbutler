#![allow(deprecated)]
use serde::{Deserialize, Serialize};

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
    /// Allows for additional "connect-src" hosts to be included. Requires app restart.
    pub extra_csp: app_settings::ExtraCsp,
    /// Settings related to fetching.
    pub fetch: app_settings::Fetch,
    /// Settings related to Claude Code.
    pub claude: app_settings::Claude,
    /// Settings related to code reviews and pull requests.
    pub reviews: app_settings::Reviews,
    /// UI settings.
    pub ui: app_settings::UiSettings,
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

/// Preset customizations for applications to use in [AppSettingsWithDiskSync::new_with_customization()], but tested and maintained here.
pub mod customization {
    use crate::json;
    use serde_json::json;

    pub fn merge_two(
        new: serde_json::Value,
        previous: Option<serde_json::Value>,
    ) -> serde_json::Value {
        match previous {
            None => new,
            Some(mut previous) => {
                json::merge_non_null_json_value(new, &mut previous);
                previous
            }
        }
    }

    /// Tell the UI that the 'but' binary is packaged.
    pub fn packaged_but_binary() -> serde_json::Value {
        json!({
            "ui": {
                "cliIsManagedByPackageManager": true
            }
        })
    }

    /// Tell the UI that no auto-update checks should be performed.
    pub fn disable_auto_update_checks() -> serde_json::Value {
        json!({
            "ui": {
                "checkForUpdatesIntervalInSeconds": 0
            }
        })
    }
}

pub mod app_settings;
mod json;
mod persistence;
mod watch;
pub use watch::AppSettingsWithDiskSync;

pub mod api;

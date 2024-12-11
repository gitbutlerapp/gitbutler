use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// Telemetry settings
    pub telemetry: TelemetrySettings,
    /// Client ID for the GitHub OAuth application
    pub github_oauth_app: GitHubOAuthAppSettings,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySettings {
    /// Whether the anonymous metrics are enabled.
    pub app_metrics_enabled: bool,
    /// Whether anonymous error reporting is enabled.
    pub app_error_reporting_enabled: bool,
    /// Whether non-anonymous metrics are enabled.
    pub app_non_anon_metrics_enabled: bool,
    /// Whether the user has confirmed the analytics (has passed the onboarding flow).
    pub app_analytics_confirmed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitHubOAuthAppSettings {
    /// Client ID for the GitHub OAuth application. Set this to use custom (non-GitButler) OAuth application.
    pub oauth_client_id: String,
}

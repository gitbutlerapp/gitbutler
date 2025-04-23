use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySettings {
    /// Whether the anonymous metrics are enabled.
    pub app_metrics_enabled: bool,
    /// Whether anonymous error reporting is enabled.
    pub app_error_reporting_enabled: bool,
    /// Whether non-anonymous metrics are enabled.
    pub app_non_anon_metrics_enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitHubOAuthAppSettings {
    /// Client ID for the GitHub OAuth application. Set this to use custom (non-GitButler) OAuth application.
    pub oauth_client_id: String,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FeatureFlags {
    /// Enables the v3 design, as well as the purgatory mode (no uncommitted diff ownership assignments).
    pub v3: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExtraCsp {
    /// Additional hosts that the application can connect to.
    pub hosts: Vec<String>,
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TelemetrySettings {
    /// Whether the anonymous metrics are enabled.
    pub app_metrics_enabled: bool,
    /// Whether anonymous error reporting is enabled.
    pub app_error_reporting_enabled: bool,
    /// Whether non-anonymous metrics are enabled.
    pub app_non_anon_metrics_enabled: bool,
    /// Distinct ID, if reporting is enabled.
    pub app_distinct_id: Option<String>,
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
    #[serde(
        default = "FeatureFlags::always_true",
        deserialize_with = "FeatureFlags::deserialize_v3_true"
    )]
    pub v3: bool,
    /// Enable the usage of V3 workspace APIs.
    #[serde(default = "default_true")]
    pub ws3: bool,
    /// Enable the usage of GitButler Acitions.
    pub actions: bool,
    /// Enable the usage of the butbot chat.
    pub butbot: bool,
    /// Enable processing of workspace rules.
    pub rules: bool,
}

fn default_true() -> bool {
    true
}

impl FeatureFlags {
    fn always_true() -> bool {
        true
    }

    fn deserialize_v3_true<'de, D>(_deserializer: D) -> Result<bool, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(true)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExtraCsp {
    /// Additional hosts that the application can connect to.
    pub hosts: Vec<String>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Fetch {
    /// The frequency at which the app will automatically fetch. A negative value (e.g. -1) disables auto fetching.
    pub auto_fetch_interval_minutes: isize,
}

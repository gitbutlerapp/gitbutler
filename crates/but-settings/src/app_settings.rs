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
    /// Enable undo/redo support.
    ///
    /// ### Progression for implementation
    ///
    /// * use snapshot system in undo/redo queue
    ///     - consider not referring to these objects by reference to `git gc` will catch them,
    ///       or even purge them on shutdown. Alternatively, keep them in-memory with in-memory objects.
    /// * add user-control to snapshot system to purge now, or purge after time X. That way data isn't stored forever.
    /// * Finally, consider implementing undo/redo with invasive primitives that are undoable/redoable themselves for
    ///   the most efficient solution, inherently in memory, i.e.
    ///     - CRUD reference
    ///     - CRUD metadata
    ///     - CRUD workspace
    ///     - CRUD files
    pub undo: bool,
    /// Enable the usage of GitButler Acitions.
    pub actions: bool,
    /// Enable the usage of the butbot chat.
    pub butbot: bool,
    /// Enable processing of workspace rules.
    pub rules: bool,
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

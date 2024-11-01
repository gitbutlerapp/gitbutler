/// Application settings
/// Constructed via the `tauri_plugin_store::Store` from `settings.json`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AppSettings {
    pub app_metrics_enabled: Option<bool>,
    pub app_error_reporting_enabled: Option<bool>,
    pub app_non_anon_metrics_enabled: Option<bool>,
    pub app_analytics_confirmed: Option<bool>,
}

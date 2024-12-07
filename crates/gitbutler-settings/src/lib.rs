use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

const SETTINGS_FILE: &str = "settings.json";

/// Application settings
/// Constructed via the `tauri_plugin_store::Store` from `settings.json`
#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq, Hash)]
pub struct AppSettings {
    pub app_metrics_enabled: Option<bool>,
    pub app_error_reporting_enabled: Option<bool>,
    pub app_non_anon_metrics_enabled: Option<bool>,
    pub app_analytics_confirmed: Option<bool>,
    /// Client ID for the GitHub OAuth application
    pub github_oauth_client_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq, Hash)]
pub struct AppSettingsUpdateRequest {
    pub app_metrics_enabled: Option<bool>,
    pub app_error_reporting_enabled: Option<bool>,
    pub app_non_anon_metrics_enabled: Option<bool>,
    pub app_analytics_confirmed: Option<bool>,
}

impl AppSettings {
    pub fn try_from_path(path: impl Into<PathBuf>) -> Result<Self> {
        let file_contents = fs::read_to_string(path.into().join(SETTINGS_FILE))?;
        let app_settings: AppSettings = serde_json::from_str(&file_contents)?;
        Ok(app_settings)
    }

    pub fn update(
        &mut self,
        path: impl Into<PathBuf>,
        request: AppSettingsUpdateRequest,
    ) -> Result<()> {
        if let Some(app_metrics_enabled) = request.app_metrics_enabled {
            self.app_metrics_enabled = Some(app_metrics_enabled);
        }
        if let Some(app_error_reporting_enabled) = request.app_error_reporting_enabled {
            self.app_error_reporting_enabled = Some(app_error_reporting_enabled);
        }
        if let Some(app_non_anon_metrics_enabled) = request.app_non_anon_metrics_enabled {
            self.app_non_anon_metrics_enabled = Some(app_non_anon_metrics_enabled);
        }
        if let Some(app_analytics_confirmed) = request.app_analytics_confirmed {
            self.app_analytics_confirmed = Some(app_analytics_confirmed);
        }

        let file_contents = serde_json::to_string_pretty(self)?;
        fs::write(path.into().join(SETTINGS_FILE), file_contents)?;
        Ok(())
    }
}

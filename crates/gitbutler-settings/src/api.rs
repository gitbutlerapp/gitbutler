use crate::{AppSettings, SettingsHandle};
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub fn get_app_settings(handle: &SettingsHandle) -> Result<AppSettings> {
    let settings = handle.read()?;
    Ok(settings.clone())
}

pub fn update_onboarding_complete(handle: &SettingsHandle, update: bool) -> Result<()> {
    let mut settings = handle.write()?;
    settings.onboarding_complete = update;
    settings.save(handle.config_path())
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::TelemetrySettings`].
pub struct TelemetryUpdate {
    pub app_metrics_enabled: Option<bool>,
    pub app_error_reporting_enabled: Option<bool>,
    pub app_non_anon_metrics_enabled: Option<bool>,
}

pub fn update_telemetry(handle: &SettingsHandle, update: TelemetryUpdate) -> Result<()> {
    let mut settings = handle.write()?;
    if let Some(app_metrics_enabled) = update.app_metrics_enabled {
        settings.telemetry.app_metrics_enabled = app_metrics_enabled;
    }
    if let Some(app_error_reporting_enabled) = update.app_error_reporting_enabled {
        settings.telemetry.app_error_reporting_enabled = app_error_reporting_enabled;
    }
    if let Some(app_non_anon_metrics_enabled) = update.app_non_anon_metrics_enabled {
        settings.telemetry.app_non_anon_metrics_enabled = app_non_anon_metrics_enabled;
    }
    settings.save(handle.config_path())
}

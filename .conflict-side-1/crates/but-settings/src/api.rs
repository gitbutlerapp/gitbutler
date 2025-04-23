use crate::AppSettingsWithDiskSync;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::TelemetrySettings`].
pub struct TelemetryUpdate {
    pub app_metrics_enabled: Option<bool>,
    pub app_error_reporting_enabled: Option<bool>,
    pub app_non_anon_metrics_enabled: Option<bool>,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::FeatureFlags`].
pub struct FeatureFlagsUpdate {
    pub v3: Option<bool>,
}

/// Mutation, immediately followed by writing everything to disk.
impl AppSettingsWithDiskSync {
    pub fn update_onboarding_complete(&self, update: bool) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        settings.onboarding_complete = update;
        settings.save()
    }

    pub fn update_telemetry(&self, update: TelemetryUpdate) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(app_metrics_enabled) = update.app_metrics_enabled {
            settings.telemetry.app_metrics_enabled = app_metrics_enabled;
        }
        if let Some(app_error_reporting_enabled) = update.app_error_reporting_enabled {
            settings.telemetry.app_error_reporting_enabled = app_error_reporting_enabled;
        }
        if let Some(app_non_anon_metrics_enabled) = update.app_non_anon_metrics_enabled {
            settings.telemetry.app_non_anon_metrics_enabled = app_non_anon_metrics_enabled;
        }
        settings.save()
    }

    pub fn update_feature_flags(&self, update: FeatureFlagsUpdate) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(v3) = update.v3 {
            settings.feature_flags.v3 = v3;
        }
        settings.save()
    }
}

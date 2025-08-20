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
    pub ws3: Option<bool>,
    pub actions: Option<bool>,
    pub butbot: Option<bool>,
    pub rules: Option<bool>,
    pub single_branch: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
/// Update request for [`crate::app_settings::Claude`].
pub struct ClaudeUpdate {
    pub executable: Option<String>,
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

    pub fn update_telemetry_distinct_id(&self, app_distinct_id: Option<String>) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        settings.telemetry.app_distinct_id = app_distinct_id;
        settings.save()
    }

    pub fn update_feature_flags(
        &self,
        FeatureFlagsUpdate {
            ws3,
            actions,
            butbot,
            rules,
            single_branch,
        }: FeatureFlagsUpdate,
    ) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(ws3) = ws3 {
            settings.feature_flags.ws3 = ws3;
        }
        if let Some(actions) = actions {
            settings.feature_flags.actions = actions;
        }
        if let Some(butbot) = butbot {
            settings.feature_flags.butbot = butbot;
        }
        if let Some(rules) = rules {
            settings.feature_flags.rules = rules;
        }
        if let Some(single_branch) = single_branch {
            settings.feature_flags.single_branch = single_branch;
        }
        settings.save()
    }

    pub fn update_claude(&self, update: ClaudeUpdate) -> Result<()> {
        let mut settings = self.get_mut_enforce_save()?;
        if let Some(executable) = update.executable {
            settings.claude.executable = executable;
        }
        settings.save()
    }
}
